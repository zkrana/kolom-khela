// The pen catalogue. Heavier pens hit harder and are harder to knock about; longer ones reach further.
pub struct Pen {
    pub id: &'static str,
    pub name: &'static str,
    pub desc: &'static str,
    pub price: i64,
    pub hl: f32, // half the straight length
    pub r: f32,  // half the thickness
    pub m: f32,  // weight
}

pub const PENS: &[Pen] = &[
    Pen { id: "econo", name: "Econo ballpoint", desc: "The classic. Everyone starts with one.", price: 0, hl: 70.0, r: 8.0, m: 2.0 },
    Pen { id: "matador", name: "Matador", desc: "A touch heavier, a touch longer.", price: 30, hl: 72.0, r: 8.0, m: 2.4 },
    Pen { id: "gel", name: "Gel grip pen", desc: "Chunky grip, solid hits.", price: 70, hl: 70.0, r: 9.0, m: 2.9 },
    Pen { id: "marker", name: "Fat marker", desc: "Long and thick. Hard to push around.", price: 140, hl: 76.0, r: 11.0, m: 3.6 },
    Pen { id: "steel", name: "Steel fountain pen", desc: "The heaviest pen in the class.", price: 250, hl: 80.0, r: 9.0, m: 4.4 },
];

pub const WIN_REWARD: i64 = 10;
pub const DAILY_REWARDED_WINS: i64 = 15;

pub fn find(id: &str) -> Option<&'static Pen> {
    PENS.iter().find(|p| p.id == id)
}

pub fn find_or_default(id: &str) -> &'static Pen {
    find(id).unwrap_or(&PENS[0])
}
