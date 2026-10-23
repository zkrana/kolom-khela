// The computer player. It plays by exactly the rules a person does: all it ever produces is a flick (a direction,
// a power, and where on the kolom the finger lands), and the room applies that flick the same way it applies a
// person's. To pick one it tries a few dozen flicks on a copy of the desk with the real physics, scores where
// everything ends up, and keeps the best. Then its hand shakes a little. How much it shakes, and how choosy it is,
// depend on the level: that is what makes the easy computer beatable and the hard one a fight.
use crate::game::{self, Body, H, W};
use crate::room::Difficulty;
use rand::Rng;

#[derive(Clone, Copy, Debug)]
pub struct Shot {
    pub dx: f32,
    pub dy: f32,
    pub power: f32,
    pub lx: f32,
    pub ly: f32,
}

/// The flick the computer plays on its turn, shake included. `lives` is indexed by seat (= body id).
/// None only when its own kolom is not on the desk, which the room never asks about.
pub fn choose(bodies: &[Body], me: u32, lives: &[u32], top_speed: f32, level: Difficulty) -> Option<Shot> {
    let ranked = ranked(bodies, me, lives, top_speed);
    if ranked.is_empty() {
        return None;
    }
    let mut rng = rand::thread_rng();
    // How many of the best shots it picks between, and how far its aim and strength wander.
    let (choosy, angle, power) = match level {
        Difficulty::Easy => (6, 0.13, 0.20),
        Difficulty::Medium => (3, 0.06, 0.10),
        Difficulty::Hard => (1, 0.02, 0.04),
    };
    let mut s = ranked[rng.gen_range(0..ranked.len().min(choosy))].1;
    let a: f32 = rng.gen_range(-angle..=angle);
    let (c, sn) = (a.cos(), a.sin());
    (s.dx, s.dy) = (s.dx * c - s.dy * sn, s.dx * sn + s.dy * c);
    s.power = (s.power * (1.0 + rng.gen_range(-power..=power))).clamp(0.05, 1.0);
    Some(s)
}

/// Every flick it considered, best first, with the score each one earned on a copy of the desk. No randomness.
pub fn ranked(bodies: &[Body], me: u32, lives: &[u32], top_speed: f32) -> Vec<(f32, Shot)> {
    let Some(mine) = bodies.iter().find(|b| b.id == me && b.alive) else { return Vec::new() };
    let mut shots = Vec::new();
    // At every other kolom: its middle and towards both ends, at a few strengths, straight or with spin.
    for o in bodies.iter().filter(|b| b.alive && b.id != me) {
        let (c, s) = (o.ang.cos(), o.ang.sin());
        for along in [0.0f32, 0.6, -0.6] {
            let (dx, dy) = (o.x + c * o.hl * along - mine.x, o.y + s * o.hl * along - mine.y);
            for power in [0.35f32, 0.5, 0.65, 0.8, 1.0] {
                for lx in [0.0, mine.hl * 0.6] {
                    shots.push(Shot { dx, dy, power, lx, ly: 0.0 });
                }
            }
        }
    }
    // A quiet move: a short slide towards the middle, for when every attack looks worse than doing very little.
    let (cx, cy) = (W / 2.0 - mine.x, H / 2.0 - mine.y);
    if cx.hypot(cy) > 40.0 {
        for power in [0.15f32, 0.25, 0.4] {
            shots.push(Shot { dx: cx, dy: cy, power, lx: 0.0, ly: 0.0 });
        }
    }
    let mut out: Vec<(f32, Shot)> = shots.into_iter().map(|s| (outcome(bodies, me, lives, top_speed, s), s)).collect();
    out.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
    out
}

/// Plays the flick on a copy of the desk until everything stops, and scores the result.
fn outcome(bodies: &[Body], me: u32, lives: &[u32], top_speed: f32, s: Shot) -> f32 {
    let mut sim = bodies.to_vec();
    let Some(pen) = sim.iter_mut().find(|b| b.id == me && b.alive) else { return f32::MIN };
    game::flick(pen, s.dx, s.dy, s.power * top_speed, s.lx, s.ly);
    let mut fell = Vec::new();
    for _ in 0..900 {
        fell.extend(game::step_ex(&mut sim).0);
        if game::all_rest(&sim) {
            break;
        }
    }
    let mut score = 0.0;
    for id in &fell {
        let last_life = lives.get(*id as usize).copied().unwrap_or(1) <= 1;
        score += match (*id == me, last_life) {
            (true, true) => -5000.0, // never throw the game away
            (true, false) => -900.0,
            (false, true) => 700.0, // knocking someone out beats knocking someone back
            (false, false) => 400.0,
        };
    }
    // Where things come to rest: its own kolom wants to be away from the edges, everyone else's near them.
    let edge = |b: &Body| b.x.min(W - b.x).min(b.y).min(H - b.y);
    for b in sim.iter().filter(|b| b.alive) {
        let near = (180.0 - edge(b)).max(0.0);
        if b.id == me {
            score -= near * 2.0 + (b.x - W / 2.0).hypot(b.y - H / 2.0) * 0.05;
        } else {
            score += near * 0.8;
        }
    }
    score
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pens::PENS;

    fn board(n: usize) -> Vec<Body> {
        let specs: Vec<&crate::pens::Pen> = (0..n).map(|_| &PENS[0]).collect();
        game::new_board(&specs)
    }

    fn play(bodies: &mut [Body], who: u32, s: Shot, top: f32) -> Vec<u32> {
        let pen = bodies.iter_mut().find(|b| b.id == who).unwrap();
        game::flick(pen, s.dx, s.dy, s.power * top, s.lx, s.ly);
        let mut fell = Vec::new();
        for _ in 0..900 {
            fell.extend(game::step_ex(bodies).0);
            if game::all_rest(bodies) {
                break;
            }
        }
        fell
    }

    #[test]
    fn it_takes_an_easy_knock_when_one_is_there() {
        let mut b = board(2);
        // The other kolom lies just behind the computer's, close to the edge: one push sends it over.
        b[0].y = 580.0;
        b[1].x = 400.0;
        b[1].y = 700.0;
        let top = game::MAX_SPEED * Difficulty::Hard.speed();
        let best = ranked(&b, 0, &[1, 1], top)[0].1;
        let fell = play(&mut b, 0, best, top);
        assert!(fell.contains(&1) && !fell.contains(&0), "fell: {fell:?}");
    }

    #[test]
    fn its_best_shot_never_drops_its_own_last_life() {
        let top = game::MAX_SPEED * Difficulty::Hard.speed();
        for n in 2..=4 {
            let mut b = board(n);
            let lives = vec![1; n];
            let best = ranked(&b, 0, &lives, top)[0].1;
            let fell = play(&mut b, 0, best, top);
            assert!(!fell.contains(&0), "{n} players: the computer knocked itself off with {best:?}");
        }
    }

    /// Four computers play a whole hard game by the room's rules (lives, respawn, turn order). It must end, with
    /// one winner, well inside the turn limit, and nothing on the desk may ever turn into NaN.
    #[test]
    fn a_four_computer_game_finishes() {
        for level in [Difficulty::Easy, Difficulty::Hard] {
            let n = 4;
            let mut b = board(n);
            let mut lives = vec![level.lives(); n];
            let top = game::MAX_SPEED * level.speed();
            let order = [0usize, 2, 1, 3];
            let mut turns = 0;
            let mut at = 0;
            while lives.iter().filter(|&&l| l > 0).count() > 1 && turns < 400 {
                let who = order[at % n];
                at += 1;
                if lives[who] == 0 {
                    continue;
                }
                turns += 1;
                let s = choose(&b, who as u32, &lives, top, level).expect("its kolom is on the desk");
                for id in play(&mut b, who as u32, s, top) {
                    lives[id as usize] -= 1;
                    if lives[id as usize] > 0 {
                        game::respawn(&mut b, id, n);
                    }
                }
                assert!(b.iter().all(|x| x.x.is_finite() && x.y.is_finite() && x.ang.is_finite()));
            }
            assert!(turns < 400, "{} game did not finish", level.key());
            assert_eq!(lives.iter().filter(|&&l| l > 0).count(), 1);
        }
    }
}
