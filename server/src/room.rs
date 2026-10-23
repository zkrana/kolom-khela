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
const TURN_SECS: u64 = 25;
const MAX_SIM_TICKS: u32 = 60 * 14;
const MAX_TURNS: u32 = 60; // nobody falls for this long: the pen nearest the middle wins

pub enum Cmd {
    Join { user_id: i64, name: String, pen: String, avatar: Option<i64>, tx: Tx },
    Leave { user_id: i64 },
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
    elim: u32, // 0 = still in; otherwise the order the kolom fell (1 = first = the looser)
}

struct Room {
    state: Arc<AppState>,
    code: String,
    host_id: i64,
    info: Arc<Mutex<RoomInfo>>,
    seats: Vec<Seat>,
    watchers: Vec<(i64, Tx)>,
    phase: Phase,
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
            .map(|(i, s)| json!({ "seat": i, "id": s.user_id, "name": s.name, "pen": s.pen, "avatar": s.avatar, "alive": s.alive, "online": s.tx.is_some() }))
            .collect();
        let bodies: Vec<&Body> = self.bodies.iter().filter(|b| b.alive).collect();
        let left = if self.phase == Phase::Playing && !self.sim { self.deadline.saturating_duration_since(Instant::now()).as_millis() as u64 } else { 0 };
        json!({
            "t": "state",
            "code": self.code,
            "phase": match self.phase { Phase::Lobby => "lobby", Phase::Playing => "playing", Phase::Over => "over" },
            "host": self.host_id,
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
                    self.seats.push(Seat { user_id, name, pen, avatar, tx: Some(tx), reward: 0, alive: true, elim: 0 });
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
                for s in self.seats.iter_mut() {
                    s.reward = 0;
                    s.alive = true;
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
                if let Some(pen) = self.bodies.iter_mut().find(|b| b.id == self.turn as u32 && b.alive) {
                    game::flick(pen, dx, dy, p * game::MAX_SPEED, lx, ly);
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
        for s in self.seats.iter_mut() {
            s.reward = 0;
            s.alive = true;
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
        self.deadline = Instant::now() + Duration::from_secs(TURN_SECS);
        self.push_state();
        self.log("খেলা শুরু! বেঞ্চের যেকোনো জায়গায় চেপে পেছনে টেনে ছেড়ে দিন। অন্যের কলম ফেলে দিন।".into());
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
                let secs = if self.seats[i].tx.is_some() { TURN_SECS } else { 3 };
                self.deadline = Instant::now() + Duration::from_secs(secs);
                return;
            }
        }
    }

    async fn finish_turn(&mut self) {
        let fell = std::mem::take(&mut self.fell);
        for id in fell {
            let s = &mut self.seats[id as usize];
            s.alive = false;
            self.elim_counter += 1;
            s.elim = self.elim_counter;
            let msg = format!("{}-এর কলম বেঞ্চ থেকে পড়ে গেছে!", s.name);
            self.log(msg);
        }
        self.turns += 1;
        let alive = self.seats.iter().filter(|s| s.alive).count();
        if alive <= 1 || self.turns >= MAX_TURNS {
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
        let reward = db::reward_win(&self.state.pool, self.seats[winner_idx].user_id).await;
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
        self.results = json!(results
            .iter()
            .map(|r| json!({ "id": r.user_id, "name": r.name, "reward": r.coins, "rank": r.rank, "looser": r.looser }))
            .collect::<Vec<_>>());
        db::save_game(&self.state.pool, &self.code, &results).await;
        self.phase = Phase::Over;
        self.sim = false;
        self.push_state();
    }
}
