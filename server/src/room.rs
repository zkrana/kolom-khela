// One live room = one task. Everything about a match (seats, turns, physics, results) lives here, so the
// server is the only authority: clients can only ask to start, rematch, shoot, or (the host) set the level and
// add or remove computer players.
use crate::bot::{self, Shot};
use crate::db::{self, PlayerResult};
use crate::game::{self, Body};
use crate::AppState;
use rand::Rng;
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};
use tokio::time::MissedTickBehavior;

pub type Tx = UnboundedSender<String>;
const MAX_SEATS: usize = 4;
const MAX_SIM_TICKS: u32 = 60 * 14;
/// Turns in a row that run out without a flick before the player counts as gone and is out of the game.
const MAX_MISSED: u32 = 3;
/// A seat whose player has disconnected only gets this long, so the game never waits on an empty chair.
const AWAY_SECS: u64 = 3;
/// The computer players' names. Each new one takes the first name not already at the desk.
const BOT_NAMES: [&str; 3] = ["রোবো রাজু", "রোবো মিতু", "রোবো টুনি"];

/// How hard the match is. The host picks it in the lobby; everything that makes a game short and frustrating
/// (one fall and you are out, a flick that crosses the whole desk, a short clock) eases off on the lower levels.
#[derive(Clone, Copy, PartialEq, Debug)]
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
    /// Nobody falls for this many turns: the game ends and survivors are ranked by lives, then nearness to the middle.
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

/// Turns go round the desk the way people sit, not in the order they joined. Seats are bottom, top, left, right
/// (see game::home_spot), so with four players the order is bottom, left, top, right: clockwise on screen.
fn ring(n: usize) -> Vec<usize> {
    match n {
        3 => vec![0, 2, 1],
        4 => vec![0, 2, 1, 3],
        _ => (0..n).collect(),
    }
}

pub enum Cmd {
    Join { user_id: i64, name: String, pen: String, avatar: Option<i64>, guest: bool, tx: Tx },
    /// `tx` says which connection closed: a player with two tabs, or one who reconnected before the old socket
    /// noticed it was dead, must not be thrown out by the stale one.
    Leave { user_id: i64, tx: Tx },
    SetDifficulty { user_id: i64, level: String },
    AddBot { user_id: i64 },
    RemoveBot { user_id: i64, bot_id: i64 },
    Start { user_id: i64 },
    Rematch { user_id: i64 },
    Shoot { user_id: i64, dx: f32, dy: f32, power: f32, lx: f32, ly: f32 },
    Close,
}

/// A copy of what is happening in a room, refreshed on every state change, for the live counter and the admin
/// pages. The room task owns the real thing; this is read without asking it.
#[derive(Clone, Default, serde::Serialize)]
pub struct RoomInfo {
    pub code: String,
    pub host: String,
    pub players: usize,
    pub phase: &'static str,
    #[serde(rename = "createdAt")]
    pub created_at: i64,
    pub difficulty: &'static str,
    /// People with an open connection: seated, and watching. Guests are counted in both and on their own.
    pub people: usize,
    pub watchers: usize,
    pub guests: usize,
    pub bots: usize,
    /// Most people connected at once so far; written to rooms_log when the room closes.
    pub peak: usize,
    /// Flicks in the match in progress, and matches finished in this room.
    pub turns: u32,
    pub games: u32,
    #[serde(rename = "startedAt")]
    pub started_at: i64,
    pub seats: Vec<SeatInfo>,
}

#[derive(Clone, serde::Serialize)]
pub struct SeatInfo {
    pub name: String,
    pub pen: String,
    pub bot: bool,
    pub guest: bool,
    pub online: bool,
    pub alive: bool,
    pub lives: u32,
    pub knocks: u32,
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
    /// Negative for a computer player: they have no account.
    user_id: i64,
    name: String,
    pen: String,
    avatar: Option<i64>,
    tx: Option<Tx>,
    bot: bool,
    guest: bool,
    reward: i64,
    alive: bool,
    /// Falls left before this player is out. It starts at the difficulty's lives and never goes back up in a game.
    lives: u32,
    /// Other players' koloms this one knocked off the desk, and its own falls: shown at the end.
    knocks: u32,
    falls: u32,
    /// Turns in a row that ran out without a flick. Reset by any flick.
    missed: u32,
    elim: u32, // 0 = still in; otherwise the order the player went out (1 = first = the looser)
}

impl Seat {
    /// At the desk: a person with an open connection, or a computer player (which never leaves).
    fn here(&self) -> bool {
        self.bot || self.tx.is_some()
    }
}

/// Someone connected who has no seat: the room was full or a match was already on. They see everything, and
/// take a free chair when the room goes back to the lobby.
struct Watcher {
    user_id: i64,
    name: String,
    pen: String,
    avatar: Option<i64>,
    guest: bool,
    tx: Tx,
}

struct Room {
    state: Arc<AppState>,
    code: String,
    host_id: i64,
    info: Arc<Mutex<RoomInfo>>,
    seats: Vec<Seat>,
    watchers: Vec<Watcher>,
    phase: Phase,
    difficulty: Difficulty,
    bodies: Vec<Body>,
    turn: usize,
    deadline: Instant,
    /// When the computer on turn has finished "thinking" and picks its flick.
    bot_think: Option<Instant>,
    /// The flick it picked, shown to everyone as an aim first and played at this moment.
    bot_shot: Option<(Instant, Shot)>,
    next_bot_id: i64,
    sim: bool,
    sim_ticks: u32,
    hit: f32,
    fell: Vec<u32>,
    elim_counter: u32,
    turns: u32,
    results: Value,
    idle_secs: u32,
    ticks: u32,
    /// For the records: when this match began, turns that ran out, how it ended, and the room's own history.
    started_at: i64,
    skips: u32,
    games_here: u32,
    peak_people: usize,
    used_bots: bool,
    log_id: Option<i64>,
    close_reason: &'static str,
}

pub fn spawn_room(state: Arc<AppState>, code: String, host_id: i64, host_name: String) -> RoomHandle {
    let (tx, rx) = unbounded_channel();
    let info = Arc::new(Mutex::new(RoomInfo { code: code.clone(), host: host_name, phase: "lobby", difficulty: "medium", created_at: db::now(), ..Default::default() }));
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
        bot_think: None,
        bot_shot: None,
        next_bot_id: 0,
        sim: false,
        sim_ticks: 0,
        hit: 0.0,
        fell: Vec::new(),
        elim_counter: 0,
        turns: 0,
        results: Value::Null,
        idle_secs: 0,
        ticks: 0,
        started_at: 0,
        skips: 0,
        games_here: 0,
        peak_people: 0,
        used_bots: false,
        log_id: None,
        close_reason: "gone",
    };
    tokio::spawn(run(room, rx));
    RoomHandle { tx, info }
}

async fn run(mut room: Room, mut rx: UnboundedReceiver<Cmd>) {
    let host_name = room.info.lock().map(|i| i.host.clone()).unwrap_or_default();
    room.log_id = db::room_opened(&room.state.pool, &room.code, room.host_id, &host_name).await;
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
    if let Some(id) = room.log_id {
        db::room_closed(&room.state.pool, id, room.close_reason, room.games_here as i64, room.peak_people as i64, room.used_bots).await;
    }
}

impl Room {
    /// People with an open connection. Computer players do not count: a room with only computers left closes.
    fn connected(&self) -> usize {
        self.seats.iter().filter(|s| s.tx.is_some()).count() + self.watchers.len()
    }

    fn sync_info(&mut self) {
        let people = self.connected();
        self.peak_people = self.peak_people.max(people);
        self.used_bots |= self.seats.iter().any(|s| s.bot);
        if let Ok(mut i) = self.info.lock() {
            i.players = self.seats.len();
            i.phase = match self.phase {
                Phase::Lobby => "lobby",
                Phase::Playing => "playing",
                Phase::Over => "over",
            };
            i.difficulty = self.difficulty.key();
            i.people = people;
            i.watchers = self.watchers.len();
            i.peak = self.peak_people;
            i.guests = self.seats.iter().filter(|s| s.guest && s.tx.is_some()).count() + self.watchers.iter().filter(|w| w.guest).count();
            i.bots = self.seats.iter().filter(|s| s.bot).count();
            i.turns = self.turns;
            i.games = self.games_here;
            i.started_at = if self.phase == Phase::Lobby { 0 } else { self.started_at };
            if let Some(h) = self.seats.iter().find(|s| s.user_id == self.host_id) {
                i.host = h.name.clone();
            }
            i.seats = self
                .seats
                .iter()
                .map(|s| SeatInfo { name: s.name.clone(), pen: s.pen.clone(), bot: s.bot, guest: s.guest, online: s.here(), alive: s.alive, lives: s.lives, knocks: s.knocks })
                .collect();
        }
    }

    fn broadcast(&self, msg: &str) {
        for s in &self.seats {
            if let Some(tx) = &s.tx {
                let _ = tx.send(msg.to_string());
            }
        }
        for w in &self.watchers {
            let _ = w.tx.send(msg.to_string());
        }
    }

    fn log(&self, text: String) {
        self.broadcast(&json!({ "t": "log", "msg": text }).to_string());
    }

    /// What the winner is paid. Beating only computers pays half: practice is still worth something, but a few
    /// minutes against the easy computer should not out-earn a real match with friends.
    fn prize(&self) -> i64 {
        let people = self.seats.iter().filter(|s| !s.bot).count();
        if people <= 1 {
            self.difficulty.reward() / 2
        } else {
            self.difficulty.reward()
        }
    }

    fn state_json(&self) -> String {
        let players: Vec<Value> = self
            .seats
            .iter()
            .enumerate()
            .map(|(i, s)| {
                json!({
                    "seat": i, "id": s.user_id, "name": s.name, "pen": s.pen, "avatar": s.avatar,
                    "alive": s.alive, "online": s.here(), "bot": s.bot,
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
            "turnSecs": self.difficulty.turn_secs(),
            "reward": self.prize(),
            "players": players,
            "turn": self.turn,
            "order": ring(self.seats.len()),
            "watchers": self.watchers.len(),
            "simulating": self.sim,
            "left_ms": left,
            "bodies": bodies,
            "results": self.results,
            "board": [game::W, game::H],
        })
        .to_string()
    }

    /// In the lobby the desk shows everyone's kolom at its starting place, so a player sees the pen they picked (and
    /// where they will sit) before anything begins. Nothing can be shot in the lobby, so these are display only.
    fn refresh_lobby_board(&mut self) {
        if self.phase != Phase::Lobby {
            return;
        }
        let specs: Vec<&crate::pens::Pen> = self.seats.iter().map(|s| crate::pens::find_or_default(&s.pen)).collect();
        self.bodies = game::new_board(&specs);
    }

    fn push_state(&mut self) {
        self.sync_info();
        self.broadcast(&self.state_json());
    }

    fn sit(&mut self, user_id: i64, name: String, pen: String, avatar: Option<i64>, guest: bool, tx: Tx) {
        let lives = self.difficulty.lives();
        self.seats.push(Seat { user_id, name, pen, avatar, tx: Some(tx), bot: false, guest, reward: 0, alive: true, lives, knocks: 0, falls: 0, missed: 0, elim: 0 });
    }

    /// The host has gone: the first person still at the desk takes over, so start, rematch and the level never get
    /// stuck behind an empty chair. Computers are never host.
    fn pass_host(&mut self, leaving: i64) {
        if self.host_id != leaving {
            return;
        }
        if let Some(s) = self.seats.iter().find(|s| !s.bot && s.tx.is_some() && s.user_id != leaving) {
            self.host_id = s.user_id;
            let name = s.name.clone();
            self.log(format!("হোস্ট চলে গেছেন, এখন হোস্ট {name}।"));
        }
    }

    async fn handle(&mut self, cmd: Cmd) -> bool {
        match cmd {
            Cmd::Close => {
                self.close_reason = "admin";
                self.broadcast(&json!({ "t": "closed" }).to_string());
                return true;
            }
            Cmd::Join { user_id, name, pen, avatar, guest, tx } => {
                self.idle_secs = 0;
                self.watchers.retain(|w| w.user_id != user_id);
                if let Some(seat) = self.seats.iter_mut().find(|s| s.user_id == user_id) {
                    seat.tx = Some(tx);
                    seat.avatar = avatar;
                    if self.phase == Phase::Lobby {
                        seat.pen = pen;
                    }
                    self.refresh_lobby_board();
                } else if self.phase == Phase::Lobby && self.seats.len() < MAX_SEATS {
                    self.sit(user_id, name, pen, avatar, guest, tx);
                    self.refresh_lobby_board();
                } else if let Some(i) = (self.phase == Phase::Lobby).then(|| self.seats.iter().rposition(|s| s.bot)).flatten() {
                    // A full lobby with a computer in it: a person who followed the invite link takes the computer's chair.
                    let gone = self.seats.remove(i).name;
                    self.sit(user_id, name.clone(), pen, avatar, guest, tx);
                    self.refresh_lobby_board();
                    self.log(format!("{name} এসে {gone}-এর জায়গায় বসলেন।"));
                } else {
                    self.watchers.push(Watcher { user_id, name, pen, avatar, guest, tx });
                }
                self.push_state();
            }
            Cmd::Leave { user_id, tx } => {
                self.watchers.retain(|w| !(w.user_id == user_id && w.tx.same_channel(&tx)));
                let Some(i) = self.seats.iter().position(|s| s.user_id == user_id && s.tx.as_ref().is_some_and(|t| t.same_channel(&tx))) else {
                    self.push_state();
                    return false;
                };
                if self.phase == Phase::Lobby {
                    self.seats.remove(i);
                    self.refresh_lobby_board();
                } else {
                    self.seats[i].tx = None;
                    // Gone in the middle of their own turn: the clock drops to the short one now, not next turn.
                    if self.phase == Phase::Playing && !self.sim && self.turn == i {
                        self.deadline = self.deadline.min(Instant::now() + Duration::from_secs(AWAY_SECS));
                    }
                }
                self.pass_host(user_id);
                self.push_state();
            }
            Cmd::SetDifficulty { user_id, level } => {
                // Only the host, and only in the lobby, so nobody can change the rules mid-match.
                if user_id != self.host_id || self.phase != Phase::Lobby {
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
            Cmd::AddBot { user_id } => {
                if user_id != self.host_id || self.phase != Phase::Lobby || self.seats.len() >= MAX_SEATS {
                    return false;
                }
                let Some(name) = BOT_NAMES.iter().find(|n| !self.seats.iter().any(|s| s.bot && s.name == **n)) else { return false };
                self.next_bot_id -= 1;
                // The computer sits down with the ordinary pen: it should win on play, not on a heavier kolom.
                let lives = self.difficulty.lives();
                self.seats.push(Seat {
                    user_id: self.next_bot_id,
                    name: name.to_string(),
                    pen: "econo".into(),
                    avatar: None,
                    tx: None,
                    bot: true,
                    guest: false,
                    reward: 0,
                    alive: true,
                    lives,
                    knocks: 0,
                    falls: 0,
                    missed: 0,
                    elim: 0,
                });
                self.refresh_lobby_board();
                self.push_state();
            }
            Cmd::RemoveBot { user_id, bot_id } => {
                if user_id != self.host_id || self.phase != Phase::Lobby {
                    return false;
                }
                let before = self.seats.len();
                self.seats.retain(|s| !(s.bot && s.user_id == bot_id));
                if self.seats.len() != before {
                    self.refresh_lobby_board();
                    self.push_state();
                }
            }
            Cmd::Start { user_id } => {
                if user_id != self.host_id || self.phase != Phase::Lobby {
                    return false;
                }
                self.seats.retain(|s| s.here());
                if self.seats.len() < 2 {
                    self.broadcast(&json!({ "t": "error", "msg": "শুরু করতে কমপক্ষে ২ জন খেলোয়াড় লাগবে। বন্ধুকে ডাকুন বা কম্পিউটার যোগ করুন।" }).to_string());
                    return false;
                }
                self.begin();
            }
            Cmd::Rematch { user_id } => {
                if user_id != self.host_id || self.phase != Phase::Over {
                    return false;
                }
                self.seats.retain(|s| s.here());
                // People who came in while the match was on were watching; now there is a fresh lobby, so they take
                // the free chairs, the same as if they had arrived a minute later.
                for w in std::mem::take(&mut self.watchers) {
                    if self.seats.len() < MAX_SEATS {
                        self.sit(w.user_id, w.name, w.pen, w.avatar, w.guest, w.tx);
                    } else {
                        self.watchers.push(w);
                    }
                }
                let lives = self.difficulty.lives();
                for s in self.seats.iter_mut() {
                    s.reward = 0;
                    s.alive = true;
                    s.lives = lives;
                    s.knocks = 0;
                    s.falls = 0;
                    s.missed = 0;
                    s.elim = 0;
                }
                self.results = Value::Null;
                self.phase = Phase::Lobby;
                self.refresh_lobby_board();
                self.push_state();
            }
            Cmd::Shoot { user_id, dx, dy, power, lx, ly } => {
                let Some(seat) = self.seats.get(self.turn) else { return false };
                if seat.user_id != user_id || seat.bot {
                    return false;
                }
                self.shoot(dx, dy, power, lx, ly);
            }
        }
        false
    }

    /// The one place a flick is applied, for people and computers alike.
    fn shoot(&mut self, dx: f32, dy: f32, power: f32, lx: f32, ly: f32) {
        if self.phase != Phase::Playing || self.sim || !self.seats.get(self.turn).is_some_and(|s| s.alive) {
            return;
        }
        let len = (dx * dx + dy * dy).sqrt();
        if !len.is_finite() || len < 0.0001 || !power.is_finite() || !lx.is_finite() || !ly.is_finite() {
            return;
        }
        let p = power.clamp(0.05, 1.0);
        let top = game::MAX_SPEED * self.difficulty.speed();
        let id = self.turn as u32;
        if let Some(pen) = self.bodies.iter_mut().find(|b| b.id == id && b.alive) {
            game::flick(pen, dx, dy, p * top, lx, ly);
            self.seats[self.turn].missed = 0;
            self.bot_think = None;
            self.bot_shot = None;
            self.sim = true;
            self.sim_ticks = 0;
            self.fell.clear();
            self.push_state();
        }
    }

    fn begin(&mut self) {
        let lives = self.difficulty.lives();
        for s in self.seats.iter_mut() {
            s.reward = 0;
            s.alive = true;
            s.lives = lives;
            s.knocks = 0;
            s.falls = 0;
            s.missed = 0;
            s.elim = 0;
        }
        let specs: Vec<&crate::pens::Pen> = self.seats.iter().map(|s| crate::pens::find_or_default(&s.pen)).collect();
        self.bodies = game::new_board(&specs);
        self.turns = 0;
        self.skips = 0;
        self.started_at = db::now();
        self.phase = Phase::Playing;
        self.elim_counter = 0;
        self.sim = false;
        self.results = Value::Null;
        self.start_turn(0);
        self.push_state();
        self.log(format!("খেলা শুরু! প্রত্যেকের {lives}টি করে প্রাণ। বেঞ্চে চেপে পেছনে টেনে ছেড়ে দিন, অন্যের কলম ফেলে দিন।"));
    }

    fn start_turn(&mut self, i: usize) {
        self.turn = i;
        let now = Instant::now();
        let s = &self.seats[i];
        let secs = if s.here() { self.difficulty.turn_secs() } else { AWAY_SECS };
        self.deadline = now + Duration::from_secs(secs);
        // A computer takes a moment, like a person lining up a shot, rather than firing the instant its turn comes.
        self.bot_think = s.bot.then(|| now + Duration::from_millis(rand::thread_rng().gen_range(700..1400)));
        self.bot_shot = None;
    }

    async fn tick(&mut self) -> bool {
        self.ticks = self.ticks.wrapping_add(1);
        if self.ticks % 60 == 0 {
            if self.connected() == 0 {
                self.idle_secs += 1;
                if self.idle_secs >= 60 {
                    self.close_reason = "idle";
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
                    b.w = 0.0;
                }
                self.sim = false;
                self.finish_turn().await;
            }
        } else if self.seats[self.turn].bot {
            self.bot_turn();
        } else if Instant::now() >= self.deadline {
            self.time_up().await;
        }
        false
    }

    /// The computer on turn: think, show its aim to everyone, then flick.
    fn bot_turn(&mut self) {
        let now = Instant::now();
        if let Some((at, s)) = self.bot_shot {
            if now >= at {
                self.shoot(s.dx, s.dy, s.power, s.lx, s.ly);
            }
            return;
        }
        if !self.bot_think.is_some_and(|t| now >= t) {
            return;
        }
        self.bot_think = None;
        let lives: Vec<u32> = self.seats.iter().map(|s| s.lives).collect();
        let top = game::MAX_SPEED * self.difficulty.speed();
        match bot::choose(&self.bodies, self.turn as u32, &lives, top, self.difficulty) {
            Some(s) => {
                // The same message shape a finger makes, so every page draws the computer's pull-back and predicted
                // path just as it draws a person's, and the flick that follows is never a surprise.
                self.broadcast(&json!({ "t": "aim", "seat": self.turn, "dx": s.dx, "dy": s.dy, "power": s.power, "lx": s.lx, "ly": s.ly }).to_string());
                self.bot_shot = Some((now + Duration::from_millis(900), s));
            }
            None => {
                self.next_turn();
                self.push_state();
            }
        }
    }

    /// The clock ran out on a person. Once is a skipped turn; MAX_MISSED in a row and they have walked away, so they
    /// are out and the rest are not kept waiting for the whole game.
    async fn time_up(&mut self) {
        let i = self.turn;
        self.seats[i].missed += 1;
        self.skips += 1;
        let name = self.seats[i].name.clone();
        if self.seats[i].missed >= MAX_MISSED {
            self.knock_out(i);
            self.log(format!("{name} পরপর {MAX_MISSED} বার খেলেননি, তাই আউট।"));
            if self.seats.iter().filter(|s| s.alive).count() <= 1 {
                self.finish_game("walkaway").await;
                return;
            }
        } else {
            self.log(format!("{name}-এর সময় শেষ, পালা বাদ"));
        }
        self.next_turn();
        self.push_state();
    }

    /// Out of the game: no lives, the kolom off the desk, and the next place in the elimination order.
    fn knock_out(&mut self, i: usize) {
        self.elim_counter += 1;
        let s = &mut self.seats[i];
        s.alive = false;
        s.lives = 0;
        s.elim = self.elim_counter;
        if let Some(b) = self.bodies.iter_mut().find(|b| b.id == i as u32) {
            b.alive = false;
        }
    }

    fn next_turn(&mut self) {
        let n = self.seats.len();
        let order = ring(n);
        let at = order.iter().position(|&s| s == self.turn).unwrap_or(0);
        for step in 1..=n {
            let i = order[(at + step) % n];
            if self.seats[i].alive {
                self.start_turn(i);
                return;
            }
        }
    }

    async fn finish_turn(&mut self) {
        let mut fell = std::mem::take(&mut self.fell);
        let shooter = self.turn;
        let players = self.seats.len();
        let mut back: Vec<u32> = Vec::new();
        // When the shooter's own kolom goes over in the same flick as others, it counts as falling first: knocking
        // yourself off never earns a better place than the pens you took with you.
        fell.sort_by_key(|&id| id as usize != shooter);

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
                self.knock_out(seat);
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
            if alive > 1 {
                self.log("অনেকক্ষণ কেউ পড়েনি! যার প্রাণ বেশি, তারপর যার কলম বেঞ্চের মাঝখানের কাছে, সে জিতল।".into());
            }
            self.finish_game(if alive > 1 { "turns" } else { "knockout" }).await;
            return;
        }
        self.next_turn();
        self.push_state();
    }

    /// `why`: "knockout" (one left standing), "turns" (the turn limit), or "walkaway" (the rest stopped playing).
    async fn finish_game(&mut self, why: &'static str) {
        // Survivors first: more lives left ranks higher, then nearer the middle of the desk. Then the fallen: the
        // last to go out ranks higher, and the first to go out is the looser.
        let centre = |i: usize| -> f32 {
            self.bodies.iter().find(|b| b.id == i as u32 && b.alive).map(|b| (b.x - game::W / 2.0).hypot(b.y - game::H / 2.0)).unwrap_or(f32::MAX)
        };
        let mut order: Vec<usize> = (0..self.seats.len()).collect();
        order.sort_by(|&a, &b| {
            let (sa, sb) = (&self.seats[a], &self.seats[b]);
            sb.alive
                .cmp(&sa.alive)
                .then(sb.elim.cmp(&sa.elim))
                .then(sb.lives.cmp(&sa.lives))
                .then(centre(a).partial_cmp(&centre(b)).unwrap_or(std::cmp::Ordering::Equal))
        });
        let first_fallen = self.seats.iter().filter(|s| s.elim > 0).min_by_key(|s| s.elim).map(|s| s.user_id);
        let looser_id = first_fallen.or_else(|| order.last().map(|&i| self.seats[i].user_id));
        let winner_idx = order[0];
        // A computer has no account to pay.
        let reward = if self.seats[winner_idx].bot { 0 } else { db::reward_win(&self.state.pool, self.seats[winner_idx].user_id, self.prize()).await };
        self.seats[winner_idx].reward = reward;
        let results: Vec<PlayerResult> = order
            .iter()
            .enumerate()
            .map(|(rank, &i)| {
                let s = &self.seats[i];
                PlayerResult {
                    user_id: s.user_id,
                    name: s.name.clone(),
                    coins: s.reward,
                    rank: rank as i64 + 1,
                    looser: Some(s.user_id) == looser_id,
                    seat: i as i64,
                    pen: s.pen.clone(),
                    knocks: s.knocks as i64,
                    falls: s.falls as i64,
                    lives: s.lives as i64,
                    bot: s.bot,
                    guest: s.guest,
                }
            })
            .collect();
        self.results = json!(order
            .iter()
            .enumerate()
            .map(|(rank, &i)| {
                let s = &self.seats[i];
                json!({
                    "id": s.user_id, "name": s.name, "avatar": s.avatar, "pen": s.pen, "bot": s.bot,
                    "reward": s.reward, "rank": rank as i64 + 1,
                    "looser": Some(s.user_id) == looser_id,
                    "knocks": s.knocks, "falls": s.falls, "lives": s.lives,
                })
            })
            .collect::<Vec<_>>());
        let record = db::GameRecord {
            code: &self.code,
            room_id: self.log_id,
            difficulty: self.difficulty.key(),
            started_at: self.started_at,
            turns: self.turns as i64,
            skips: self.skips as i64,
            host_id: self.host_id,
            end_reason: why,
        };
        db::save_game(&self.state.pool, &record, &results).await;
        self.games_here += 1;
        self.phase = Phase::Over;
        self.sim = false;
        self.bot_think = None;
        self.bot_shot = None;
        self.push_state();
    }
}

#[cfg(test)]
mod tests {
    use super::ring;

    #[test]
    fn turns_go_round_the_desk() {
        assert_eq!(ring(2), vec![0, 1]);
        assert_eq!(ring(3), vec![0, 2, 1]); // bottom, left, top
        assert_eq!(ring(4), vec![0, 2, 1, 3]); // bottom, left, top, right
    }
}
