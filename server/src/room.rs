// One live room = one task. Everything about a match (seats, turns, physics, results) lives here, so the
// server is the only authority: clients can only ask to start, rematch or shoot.
use crate::db::{self, PlayerResult};
use crate::game::{self, Body};
use crate::AppState;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::time::MissedTickBehavior;

pub type Tx = UnboundedSender<String>;
const MAX_SEATS: usize = 4;
const MAX_SIM_TICKS: u32 = 60 * 14;

/// How hard the match is. The host picks it in the lobby; everything that makes a game short and frustrating
/// (one fall and you are out, a flick that crosses the whole desk, a short clock) eases off on the lower levels.
#[derive(Clone, Copy, PartialEq)]
pub enum Difficulty {
    Easy,
    Medium,
    Hard,
}

impl Difficulty {
    pub fn parse(s: &str) -> Difficulty {
        match s {
            "easy" => Difficulty::Easy,
            "hard" => Difficulty::Hard,
            _ => Difficulty::Medium,
        }
    }
    pub fn key(self) -> &'static str {
        match self {
            Difficulty::Easy => "easy",
            Difficulty::Medium => "medium",
            Difficulty::Hard => "hard",
        }
    }
    /// How many times a kolom may fall before its owner is out.
    pub fn lives(self) -> u32 {
        match self {
            Difficulty::Easy => 3,
            Difficulty::Medium => 2,
            Difficulty::Hard => 1,
        }
    }
    /// Share of the engine's top speed a full-power flick gets. Slower is easier to aim and, with the home margin
    /// in game::home_spot, leaves more room before a shot at your own edge costs a life.
    pub fn speed(self) -> f32 {
        match self {
            Difficulty::Easy => 0.60,
            Difficulty::Medium => 0.72,
            Difficulty::Hard => 0.85,
        }
    }
    pub fn turn_secs(self) -> u64 {
        match self {
            Difficulty::Easy => 35,
            Difficulty::Medium => 25,
            Difficulty::Hard => 18,
        }
    }
    /// Nobody falls for this many turns: the kolom nearest the middle wins.
    pub fn max_turns(self) -> u32 {
        match self {
            Difficulty::Easy => 80,
            Difficulty::Medium => 60,
            Difficulty::Hard => 40,
        }
    }
    /// Coins for the winner. A harder game is worth more.
    pub fn reward(self) -> i64 {
        match self {
            Difficulty::Easy => 5,
            Difficulty::Medium => 10,
            Difficulty::Hard => 20,
        }
    }
}

pub enum Cmd {
    Join { user_id: i64, name: String, pen: String, avatar: Option<i64>, tx: Tx },
    Leave { user_id: i64 },
    SetDifficulty { user_id: i64, level: String },
    Start { user_id: i64 },
    Rematch { user_id: i64 },
    Shoot { user_id: i64, dx: f32, dy: f32, power: f32, lx: f32, ly: f32 },
    Close,
}

#[derive(Clone)]
pub struct RoomInfo {
    pub code: String,
    pub host: String,
    pub players: usize,
    pub phase: &'static str,
    pub created_at: i64,
}

#[derive(Clone)]
pub struct RoomHandle {
    pub tx: UnboundedSender<Cmd>,
    pub info: Arc<Mutex<RoomInfo>>,
}

#[derive(PartialEq, Clone, Copy)]
enum Phase {
    Lobby,
    Playing,
    Over,
}

struct Seat {
    user_id: i64,
    name: String,
    pen: String,
    avatar: Option<i64>,
    tx: Option<Tx>,
    reward: i64,
    alive: bool,
    /// Falls left before this player is out. It starts at the difficulty's lives and never goes back up in a game.
    lives: u32,
    /// Other players' koloms this one knocked off the desk, and its own falls: shown at the end.
    knocks: u32,
    falls: u32,
    elim: u32, // 0 = still in; otherwise the order the player ran out of lives (1 = first = the looser)
}

struct Room {
    state: Arc<AppState>,
    code: String,
    host_id: i64,
    info: Arc<Mutex<RoomInfo>>,
    seats: Vec<Seat>,
    watchers: Vec<(i64, Tx)>,
    phase: Phase,
    difficulty: Difficulty,
    bodies: Vec<Body>,
    turn: usize,
    deadline: Instant,
    sim: bool,
    sim_ticks: u32,
    hit: f32,
    fell: Vec<u32>,
    elim_counter: u32,
    turns: u32,
    results: Value,
    idle_secs: u32,
    ticks: u32,
}

pub fn spawn_room(state: Arc<AppState>, code: String, host_id: i64, host_name: String) -> RoomHandle {
    let (tx, rx) = unbounded_channel();
    let info = Arc::new(Mutex::new(RoomInfo { code: code.clone(), host: host_name, players: 0, phase: "lobby", created_at: db::now() }));
    let room = Room {
        state,
        code,
        host_id,
        info: info.clone(),
        seats: Vec::new(),
        watchers: Vec::new(),
        phase: Phase::Lobby,
        difficulty: Difficulty::Medium,
        bodies: Vec::new(),
        turn: 0,
        deadline: Instant::now(),
        sim: false,
        sim_ticks: 0,
        hit: 0.0,
        fell: Vec::new(),
        elim_counter: 0,
        turns: 0,
        results: Value::Null,
        idle_secs: 0,
        ticks: 0,
    };
    tokio::spawn(run(room, rx));
    RoomHandle { tx, info }
}

async fn run(mut room: Room, mut rx: UnboundedReceiver<Cmd>) {
    let mut tick = tokio::time::interval(Duration::from_millis(16));
    tick.set_missed_tick_behavior(MissedTickBehavior::Delay);
    loop {
        tokio::select! {
            cmd = rx.recv() => match cmd {
                Some(c) => { if room.handle(c).await { break; } }
                None => break,
            },
            _ = tick.tick() => { if room.tick().await { break; } }
        }
    }
    room.state.rooms.lock().await.remove(&room.code);
}

impl Room {
    fn connected(&self) -> usize {
        self.seats.iter().filter(|s| s.tx.is_some()).count() + self.watchers.len()
    }

    fn sync_info(&self) {
        if let Ok(mut i) = self.info.lock() {
            i.players = self.seats.len();
            i.phase = match self.phase {
                Phase::Lobby => "lobby",
                Phase::Playing => "playing",
                Phase::Over => "over",
            };
        }
    }

    fn broadcast(&self, msg: &str) {
        for s in &self.seats {
            if let Some(tx) = &s.tx {
                let _ = tx.send(msg.to_string());
            }
        }
        for (_, tx) in &self.watchers {
            let _ = tx.send(msg.to_string());
        }
    }

    fn log(&self, text: String) {
        self.broadcast(&json!({ "t": "log", "msg": text }).to_string());
    }

    fn state_json(&self) -> String {
        let players: Vec<Value> = self
            .seats
            .iter()
            .enumerate()
            .map(|(i, s)| {
                json!({
                    "seat": i, "id": s.user_id, "name": s.name, "pen": s.pen, "avatar": s.avatar,
                    "alive": s.alive, "online": s.tx.is_some(),
                    "lives": s.lives, "knocks": s.knocks, "falls": s.falls,
                })
            })
            .collect();
        let bodies: Vec<&Body> = self.bodies.iter().filter(|b| b.alive).collect();
        let left = if self.phase == Phase::Playing && !self.sim { self.deadline.saturating_duration_since(Instant::now()).as_millis() as u64 } else { 0 };
        json!({
            "t": "state",
            "code": self.code,
            "phase": match self.phase { Phase::Lobby => "lobby", Phase::Playing => "playing", Phase::Over => "over" },
            "host": self.host_id,
            "difficulty": self.difficulty.key(),
            // What a full-power flick does, so the page can draw an honest preview of where the kolom will stop.
            "shot": { "speed": game::MAX_SPEED * self.difficulty.speed(), "damp": game::DAMP, "spinDamp": game::SPIN_DAMP, "maxSpin": game::MAX_SPIN },
            "maxLives": self.difficulty.lives(),
            "reward": self.difficulty.reward(),
            "players": players,
            "turn": self.turn,
            "simulating": self.sim,
            "left_ms": left,
            "bodies": bodies,
            "results": self.results,
            "board": [game::W, game::H],
        })
        .to_string()
    }

    fn push_state(&self) {
        self.sync_info();
        self.broadcast(&self.state_json());
    }

    async fn handle(&mut self, cmd: Cmd) -> bool {
        match cmd {
            Cmd::Close => {
                self.broadcast(&json!({ "t": "closed" }).to_string());
                return true;
            }
            Cmd::Join { user_id, name, pen, avatar, tx } => {
                self.idle_secs = 0;
                if let Some(seat) = self.seats.iter_mut().find(|s| s.user_id == user_id) {
                    seat.tx = Some(tx);
                    seat.avatar = avatar;
                    if self.phase == Phase::Lobby {
                        seat.pen = pen;
                    }
                } else if self.phase == Phase::Lobby && self.seats.len() < MAX_SEATS {
                    let lives = self.difficulty.lives();
                    self.seats.push(Seat { user_id, name, pen, avatar, tx: Some(tx), reward: 0, alive: true, lives, knocks: 0, falls: 0, elim: 0 });
                } else {
                    self.watchers.retain(|(id, _)| *id != user_id);
                    self.watchers.push((user_id, tx));
                }
                self.push_state();
            }
            Cmd::Leave { user_id } => {
                self.watchers.retain(|(id, _)| *id != user_id);
                if self.phase == Phase::Lobby {
                    self.seats.retain(|s| s.user_id != user_id);
                    if self.host_id == user_id {
                        if let Some(s) = self.seats.first() {
                            self.host_id = s.user_id;
                        }
                    }
                } else if let Some(s) = self.seats.iter_mut().find(|s| s.user_id == user_id) {
                    s.tx = None;
                }
                self.push_state();
            }
            Cmd::SetDifficulty { user_id, level } => {
                // Only the host, and only between games, so nobody can change the rules mid-match.
                if user_id != self.host_id || self.phase == Phase::Playing {
                    return false;
                }
                let next = Difficulty::parse(&level);
                if next == self.difficulty {
                    return false;
                }
                self.difficulty = next;
                for s in self.seats.iter_mut() {
                    s.lives = next.lives();
                }
                self.push_state();
            }
            Cmd::Start { user_id } => {
                if user_id != self.host_id || self.phase != Phase::Lobby {
                    return false;
                }
                self.seats.retain(|s| s.tx.is_some());
                if self.seats.len() < 2 {
                    self.broadcast(&json!({ "t": "error", "msg": "শুরু করতে কমপক্ষে ২ জন খেলোয়াড় লাগবে" }).to_string());
                    return false;
                }
                self.begin();
            }
            Cmd::Rematch { user_id } => {
                if user_id != self.host_id || self.phase != Phase::Over {
                    return false;
                }
                self.seats.retain(|s| s.tx.is_some());
                let lives = self.difficulty.lives();
                for s in self.seats.iter_mut() {
                    s.reward = 0;
                    s.alive = true;
                    s.lives = lives;
                    s.knocks = 0;
                    s.falls = 0;
                    s.elim = 0;
                }
                self.results = Value::Null;
                self.bodies.clear();
                self.phase = Phase::Lobby;
                self.push_state();
            }
            Cmd::Shoot { user_id, dx, dy, power, lx, ly } => {
                if self.phase != Phase::Playing || self.sim {
                    return false;
                }
                let Some(seat) = self.seats.get(self.turn) else { return false };
                if seat.user_id != user_id || !seat.alive {
                    return false;
                }
                let len = (dx * dx + dy * dy).sqrt();
                if !len.is_finite() || len < 0.0001 || !power.is_finite() || !lx.is_finite() || !ly.is_finite() {
                    return false;
                }
                let p = power.clamp(0.05, 1.0);
                let top = game::MAX_SPEED * self.difficulty.speed();
                if let Some(pen) = self.bodies.iter_mut().find(|b| b.id == self.turn as u32 && b.alive) {
                    game::flick(pen, dx, dy, p * top, lx, ly);
                    self.sim = true;
                    self.sim_ticks = 0;
                    self.fell.clear();
                    self.push_state();
                }
            }
        }
        false
    }

    fn begin(&mut self) {
        let lives = self.difficulty.lives();
        for s in self.seats.iter_mut() {
            s.reward = 0;
            s.alive = true;
            s.lives = lives;
            s.knocks = 0;
            s.falls = 0;
            s.elim = 0;
        }
        let specs: Vec<&crate::pens::Pen> = self.seats.iter().map(|s| crate::pens::find_or_default(&s.pen)).collect();
        self.bodies = game::new_board(&specs);
        self.turns = 0;
        self.phase = Phase::Playing;
        self.turn = 0;
        self.elim_counter = 0;
        self.sim = false;
        self.results = Value::Null;
        self.deadline = Instant::now() + Duration::from_secs(self.difficulty.turn_secs());
        self.push_state();
        let lives = self.difficulty.lives();
        self.log(format!("খেলা শুরু! প্রত্যেকের {lives}টি করে প্রাণ। বেঞ্চে চেপে পেছনে টেনে ছেড়ে দিন, অন্যের কলম ফেলে দিন।"));
    }

    async fn tick(&mut self) -> bool {
        self.ticks = self.ticks.wrapping_add(1);
        if self.ticks % 60 == 0 {
            if self.connected() == 0 {
                self.idle_secs += 1;
                if self.idle_secs >= 60 {
                    return true;
                }
            } else {
                self.idle_secs = 0;
            }
        }
        if self.phase != Phase::Playing {
            return false;
        }
        if self.sim {
            self.sim_ticks += 1;
            let (fell, hit) = game::step_ex(&mut self.bodies);
            self.fell.extend(fell);
            self.hit = self.hit.max(hit);
            if self.sim_ticks % 2 == 0 {
                let b: Vec<Value> = self
                    .bodies
                    .iter()
                    .filter(|b| b.alive)
                    .map(|b| json!([b.id, (b.x * 10.0).round() / 10.0, (b.y * 10.0).round() / 10.0, (b.ang * 100.0).round() / 100.0]))
                    .collect();
                let h = self.hit.round(); // hardest hit since the last frame (closing speed), for the sound
                self.hit = 0.0;
                self.broadcast(&json!({ "t": "frame", "b": b, "h": h }).to_string());
            }
            if game::all_rest(&self.bodies) || self.sim_ticks > MAX_SIM_TICKS {
                for b in self.bodies.iter_mut() {
                    b.vx = 0.0;
                    b.vy = 0.0;
                }
                self.sim = false;
                self.finish_turn().await;
            }
        } else if Instant::now() >= self.deadline {
            let name = self.seats[self.turn].name.clone();
            self.log(format!("{name}-এর সময় শেষ, পালা বাদ"));
            self.next_turn();
            self.push_state();
        }
        false
    }

    fn next_turn(&mut self) {
        let n = self.seats.len();
        for step in 1..=n {
            let i = (self.turn + step) % n;
            if self.seats[i].alive {
                self.turn = i;
                // A player who left gets a short turn so the game never waits on an empty chair.
                let secs = if self.seats[i].tx.is_some() { self.difficulty.turn_secs() } else { 3 };
                self.deadline = Instant::now() + Duration::from_secs(secs);
                return;
            }
        }
    }

    async fn finish_turn(&mut self) {
        let fell = std::mem::take(&mut self.fell);
        let shooter = self.turn;
        let players = self.seats.len();
        let mut back: Vec<u32> = Vec::new();

        for id in fell {
            let seat = id as usize;
            let own_goal = seat == shooter;
            let (name, lives, out) = {
                let s = &mut self.seats[seat];
                s.falls += 1;
                s.lives = s.lives.saturating_sub(1);
                (s.name.clone(), s.lives, s.lives == 0)
            };
            // The shooter gets the credit, unless they knocked their own kolom off.
            if !own_goal {
                self.seats[shooter].knocks += 1;
            }
            if out {
                self.seats[seat].alive = false;
                self.elim_counter += 1;
                self.seats[seat].elim = self.elim_counter;
                self.log(format!("{name}-এর কলম পড়ে গেছে। প্রাণ শেষ, {name} আউট!"));
            } else {
                back.push(id);
                let word = if own_goal { "নিজের কলমই ফেলে দিলেন" } else { "কলম পড়ে গেছে" };
                self.log(format!("{name}-এর {word}। আর {lives}টি প্রাণ বাকি।"));
            }
        }

        // Everyone who still has a life comes back on their own edge before the next turn.
        for id in &back {
            game::respawn(&mut self.bodies, *id, players);
        }
        if !back.is_empty() {
            self.broadcast(&json!({ "t": "respawn", "ids": back }).to_string());
        }

        self.turns += 1;
        let alive = self.seats.iter().filter(|s| s.alive).count();
        if alive <= 1 || self.turns >= self.difficulty.max_turns() {
            self.finish_game().await;
            return;
        }
        self.next_turn();
        self.push_state();
    }

    async fn finish_game(&mut self) {
        // Survivors first (nearest the middle of the desk ranks higher), then the fallen: the last to fall ranks higher,
        // and the first to fall is the looser.
        let centre = |i: usize| -> f32 {
            self.bodies.iter().find(|b| b.id == i as u32 && b.alive).map(|b| (b.x - game::W / 2.0).hypot(b.y - game::H / 2.0)).unwrap_or(f32::MAX)
        };
        let mut order: Vec<usize> = (0..self.seats.len()).collect();
        order.sort_by(|&a, &b| {
            let (sa, sb) = (&self.seats[a], &self.seats[b]);
            sb.alive
                .cmp(&sa.alive)
                .then(sb.elim.cmp(&sa.elim))
                .then(centre(a).partial_cmp(&centre(b)).unwrap_or(std::cmp::Ordering::Equal))
        });
        let first_fallen = self.seats.iter().filter(|s| s.elim > 0).min_by_key(|s| s.elim).map(|s| s.user_id);
        let looser_id = first_fallen.or_else(|| order.last().map(|&i| self.seats[i].user_id));
        let winner_idx = order[0];
        let reward = db::reward_win(&self.state.pool, self.seats[winner_idx].user_id, self.difficulty.reward()).await;
        self.seats[winner_idx].reward = reward;
        let results: Vec<PlayerResult> = order
            .iter()
            .enumerate()
            .map(|(rank, &i)| PlayerResult {
                user_id: self.seats[i].user_id,
                name: self.seats[i].name.clone(),
                coins: self.seats[i].reward,
                rank: rank as i64 + 1,
                looser: Some(self.seats[i].user_id) == looser_id,
            })
            .collect();
        self.results = json!(order
            .iter()
            .enumerate()
            .map(|(rank, &i)| {
                let s = &self.seats[i];
                json!({
                    "id": s.user_id, "name": s.name, "avatar": s.avatar, "pen": s.pen,
                    "reward": s.reward, "rank": rank as i64 + 1,
                    "looser": Some(s.user_id) == looser_id,
                    "knocks": s.knocks, "falls": s.falls, "lives": s.lives,
                })
            })
            .collect::<Vec<_>>());
        db::save_game(&self.state.pool, &self.code, &results).await;
        self.phase = Phase::Over;
        self.sim = false;
        self.push_state();
    }
}
