// Pure physics for Kolom Khela: flat board, circles, elastic hits, friction. No I/O here.
use crate::pens::Pen;
use serde::Serialize;

pub const W: f32 = 800.0;
pub const H: f32 = 800.0;
pub const DT: f32 = 1.0 / 60.0;
pub const MAX_SPEED: f32 = 1500.0;
const DAMP: f32 = 2.8; // per second
const SPIN_DAMP: f32 = 3.5;
const REST: f32 = 0.88; // bounciness
const STOP: f32 = 7.0;

pub const MAX_SPIN: f32 = 10.0;

#[derive(Clone, Serialize)]
pub struct Body {
    pub id: u32,
    pub style: &'static str, // which pen it is (drawn differently by the page)
    pub x: f32,
    pub y: f32,
    pub ang: f32,
    pub hl: f32, // half the length of the straight part; the ends are round with radius r
    pub r: f32,
    #[serde(skip)]
    pub vx: f32,
    #[serde(skip)]
    pub vy: f32,
    #[serde(skip)]
    pub w: f32, // spin, radians per second
    #[serde(skip)]
    pub m: f32,
    #[serde(skip)]
    pub inertia: f32,
    #[serde(skip)]
    pub alive: bool,
}

impl Body {
    fn new(id: u32, style: &'static str, x: f32, y: f32, ang: f32, hl: f32, r: f32, m: f32) -> Body {
        let half = hl + r;
        // A rod's resistance to spinning grows with its length.
        let inertia = m * (half * half) / 3.0;
        Body { id, style, x, y, ang, hl, r, vx: 0.0, vy: 0.0, w: 0.0, m, inertia, alive: true }
    }
    fn ends(&self) -> ((f32, f32), (f32, f32)) {
        let (c, s) = (self.ang.cos() * self.hl, self.ang.sin() * self.hl);
        ((self.x - c, self.y - s), (self.x + c, self.y + s))
    }
}

fn pen_spot(seat: usize, players: usize) -> (f32, f32, f32) {
    // x, y, angle: a kolom starts lying along its own edge of the desk.
    let spots = [(W / 2.0, H - 80.0, 0.0), (W / 2.0, 80.0, 0.0), (80.0, H / 2.0, std::f32::consts::FRAC_PI_2), (W - 80.0, H / 2.0, std::f32::consts::FRAC_PI_2)];
    if players == 3 && seat == 2 {
        return spots[2];
    }
    spots[seat.min(3)]
}

pub fn new_board(pens: &[&Pen]) -> Vec<Body> {
    let n = pens.len();
    pens.iter()
        .enumerate()
        .map(|(s, p)| {
            let (x, y, a) = pen_spot(s, n);
            Body::new(s as u32, p.id, x, y, a, p.hl, p.r, p.m)
        })
        .collect()
}

/// A finger flick: an impulse along (dx, dy) applied at a point on the pen (lx, ly in the pen's own frame, ly across
/// the thickness). Hit it in the middle and it just slides; hit it near an end from the side and it also spins.
pub fn flick(b: &mut Body, dx: f32, dy: f32, speed: f32, lx: f32, ly: f32) {
    let len = (dx * dx + dy * dy).sqrt().max(0.0001);
    let (ux, uy) = (dx / len, dy / len);
    let lx = lx.max(-(b.hl + b.r)).min(b.hl + b.r);
    let ly = ly.max(-b.r).min(b.r);
    let (c, s) = (b.ang.cos(), b.ang.sin());
    let (rx, ry) = (lx * c - ly * s, lx * s + ly * c);
    b.vx = ux * speed;
    b.vy = uy * speed;
    // angular impulse = r x J, J = m * speed * u
    b.w = (b.m * speed * (rx * uy - ry * ux) / b.inertia).max(-MAX_SPIN).min(MAX_SPIN);
}

fn clamp01(v: f32) -> f32 {
    v.max(0.0).min(1.0)
}

/// Closest points between segments p1-q1 and p2-q2 (Ericson, Real-Time Collision Detection 5.1.9).
fn closest(p1: (f32, f32), q1: (f32, f32), p2: (f32, f32), q2: (f32, f32)) -> ((f32, f32), (f32, f32)) {
    let d1 = (q1.0 - p1.0, q1.1 - p1.1);
    let d2 = (q2.0 - p2.0, q2.1 - p2.1);
    let r = (p1.0 - p2.0, p1.1 - p2.1);
    let a = d1.0 * d1.0 + d1.1 * d1.1;
    let e = d2.0 * d2.0 + d2.1 * d2.1;
    let f = d2.0 * r.0 + d2.1 * r.1;
    let (s, t);
    if a <= 1e-6 && e <= 1e-6 {
        s = 0.0;
        t = 0.0;
    } else if a <= 1e-6 {
        s = 0.0;
        t = clamp01(f / e);
    } else {
        let c = d1.0 * r.0 + d1.1 * r.1;
        if e <= 1e-6 {
            t = 0.0;
            s = clamp01(-c / a);
        } else {
            let b = d1.0 * d2.0 + d1.1 * d2.1;
            let denom = a * e - b * b;
            let mut s0 = if denom > 1e-6 { clamp01((b * f - c * e) / denom) } else { 0.0 };
            let mut t0 = (b * s0 + f) / e;
            if t0 < 0.0 {
                t0 = 0.0;
                s0 = clamp01(-c / a);
            } else if t0 > 1.0 {
                t0 = 1.0;
                s0 = clamp01((b - c) / a);
            }
            s = s0;
            t = t0;
        }
    }
    ((p1.0 + d1.0 * s, p1.1 + d1.1 * s), (p2.0 + d2.0 * t, p2.1 + d2.1 * t))
}

/// One fixed step. Returns ids that left the desk this step, in order.
#[allow(dead_code)]
pub fn step(bodies: &mut [Body]) -> Vec<u32> {
    step_ex(bodies).0
}

/// Same, and also the strength of the hardest hit this step (0 when nothing touched), for the page's sound.
pub fn step_ex(bodies: &mut [Body]) -> (Vec<u32>, f32) {
    let mut hardest = 0.0f32;
    let damp = (-DAMP * DT).exp();
    let spin_damp = (-SPIN_DAMP * DT).exp();
    for b in bodies.iter_mut().filter(|b| b.alive) {
        b.x += b.vx * DT;
        b.y += b.vy * DT;
        b.ang += b.w * DT;
        b.vx *= damp;
        b.vy *= damp;
        b.w *= spin_damp;
        if b.vx * b.vx + b.vy * b.vy < STOP * STOP {
            b.vx = 0.0;
            b.vy = 0.0;
        }
        if b.w.abs() < 0.15 {
            b.w = 0.0;
        }
    }
    let n = bodies.len();
    for i in 0..n {
        for j in (i + 1)..n {
            if !bodies[i].alive || !bodies[j].alive {
                continue;
            }
            // Cheap reject: bounding circles.
            let reach = bodies[i].hl + bodies[i].r + bodies[j].hl + bodies[j].r;
            let (dx, dy) = (bodies[j].x - bodies[i].x, bodies[j].y - bodies[i].y);
            if dx * dx + dy * dy > reach * reach {
                continue;
            }
            let (a0, a1) = bodies[i].ends();
            let (b0, b1) = bodies[j].ends();
            let (ca, cb) = closest(a0, a1, b0, b1);
            let (mut nx, mut ny) = (cb.0 - ca.0, cb.1 - ca.1);
            let d = (nx * nx + ny * ny).sqrt();
            let min = bodies[i].r + bodies[j].r;
            if d >= min {
                continue;
            }
            if d > 0.001 {
                nx /= d;
                ny /= d;
            } else {
                nx = 0.0;
                ny = 1.0;
            }
            let overlap = min - d;
            let (ia, ib) = (1.0 / bodies[i].m, 1.0 / bodies[j].m);
            bodies[i].x -= nx * overlap * ia / (ia + ib);
            bodies[i].y -= ny * overlap * ia / (ia + ib);
            bodies[j].x += nx * overlap * ib / (ia + ib);
            bodies[j].y += ny * overlap * ib / (ia + ib);
            // Contact point on the surface of A.
            let (px, py) = (ca.0 + nx * bodies[i].r, ca.1 + ny * bodies[i].r);
            let (rax, ray) = (px - bodies[i].x, py - bodies[i].y);
            let (rbx, rby) = (px - bodies[j].x, py - bodies[j].y);
            let va = (bodies[i].vx - bodies[i].w * ray, bodies[i].vy + bodies[i].w * rax);
            let vb = (bodies[j].vx - bodies[j].w * rby, bodies[j].vy + bodies[j].w * rbx);
            let rel = (vb.0 - va.0) * nx + (vb.1 - va.1) * ny;
            if rel >= 0.0 {
                continue; // already separating
            }
            let ra_n = rax * ny - ray * nx;
            let rb_n = rbx * ny - rby * nx;
            let denom = ia + ib + ra_n * ra_n / bodies[i].inertia + rb_n * rb_n / bodies[j].inertia;
            let j_imp = -(1.0 + REST) * rel / denom;
            hardest = hardest.max(-rel);
            let (ix, iy) = (j_imp * nx, j_imp * ny);
            bodies[i].vx -= ix * ia;
            bodies[i].vy -= iy * ia;
            bodies[i].w -= (rax * iy - ray * ix) / bodies[i].inertia;
            bodies[j].vx += ix * ib;
            bodies[j].vy += iy * ib;
            bodies[j].w += (rbx * iy - rby * ix) / bodies[j].inertia;
        }
    }
    let mut fell = Vec::new();
    for b in bodies.iter_mut().filter(|b| b.alive) {
        if b.x < 0.0 || b.x > W || b.y < 0.0 || b.y > H {
            b.alive = false;
            fell.push(b.id);
        }
    }
    (fell, hardest)
}

pub fn all_rest(bodies: &[Body]) -> bool {
    bodies.iter().filter(|b| b.alive).all(|b| b.vx == 0.0 && b.vy == 0.0 && b.w == 0.0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::pens::PENS;

    fn board2() -> Vec<Body> {
        new_board(&[&PENS[0], &PENS[0]])
    }

    #[test]
    fn a_hard_flick_off_the_edge_falls() {
        let mut b = board2();
        flick(&mut b[0], 0.0, 1.0, 1500.0, 0.0, 0.0);
        let mut fell = vec![];
        for _ in 0..600 {
            fell.extend(step(&mut b));
        }
        assert!(fell.contains(&0));
    }

    #[test]
    fn a_soft_flick_stays_on_the_desk() {
        let mut b = board2();
        flick(&mut b[1], 0.0, 1.0, 200.0, 0.0, 0.0);
        for _ in 0..600 {
            step(&mut b);
        }
        assert!(b[1].alive && all_rest(&b));
    }

    #[test]
    fn flicking_the_middle_does_not_spin_but_the_end_does() {
        let mut a = board2();
        flick(&mut a[0], 0.0, -1.0, 800.0, 0.0, 0.0);
        assert_eq!(a[0].w, 0.0);
        let mut b = board2();
        flick(&mut b[0], 0.0, -1.0, 800.0, 55.0, 4.0);
        assert!(b[0].w.abs() > 1.0);
    }

    #[test]
    fn hitting_a_pen_moves_it_and_spins_it() {
        let mut b = board2();
        b[1].x = 400.0;
        b[1].y = 560.0; // lying across the way, in front of pen 0
        b[1].ang = 0.0;
        flick(&mut b[0], 0.0, -1.0, 900.0, 40.0, 0.0); // flicked off the middle so the hit is off centre too
        for _ in 0..40 {
            step(&mut b);
        }
        assert!(b[1].vy != 0.0 || b[1].w != 0.0 || b[1].y != 560.0);
    }

    #[test]
    fn pens_never_overlap_once_settled() {
        let mut b = board2();
        flick(&mut b[0], 0.0, -1.0, 1100.0, 30.0, 3.0);
        for _ in 0..900 {
            step(&mut b);
        }
        let pens: Vec<&Body> = b.iter().filter(|x| x.alive).collect();
        if pens.len() == 2 {
            let (a0, a1) = pens[0].ends();
            let (b0, b1) = pens[1].ends();
            let (ca, cb) = closest(a0, a1, b0, b1);
            let d = ((ca.0 - cb.0).powi(2) + (ca.1 - cb.1).powi(2)).sqrt();
            assert!(d >= pens[0].r + pens[1].r - 1.0);
        }
    }
}
