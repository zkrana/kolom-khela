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
    Pen { id: "econo", name: "ইকোনো বলপেন", desc: "সবার পরিচিত সেই কলম। সবাই এটা দিয়েই শুরু করে।", price: 0, hl: 70.0, r: 8.0, m: 2.0 },
    Pen { id: "matador", name: "ম্যাটাডোর", desc: "একটু ভারী, একটু লম্বা।", price: 30, hl: 72.0, r: 8.0, m: 2.4 },
    Pen { id: "gel", name: "জেল গ্রিপ কলম", desc: "মোটা গ্রিপ, জোরালো মার।", price: 70, hl: 70.0, r: 9.0, m: 2.9 },
    Pen { id: "marker", name: "মোটা মার্কার", desc: "লম্বা আর মোটা। ঠেলে সরানো কঠিন।", price: 140, hl: 76.0, r: 11.0, m: 3.6 },
    Pen { id: "steel", name: "স্টিলের ফাউন্টেন পেন", desc: "ক্লাসের সবচেয়ে ভারী কলম।", price: 250, hl: 80.0, r: 9.0, m: 4.4 },
];

pub const WIN_REWARD: i64 = 10;
pub const DAILY_REWARDED_WINS: i64 = 15;

pub fn find(id: &str) -> Option<&'static Pen> {
    PENS.iter().find(|p| p.id == id)
}

pub fn find_or_default(id: &str) -> &'static Pen {
    find(id).unwrap_or(&PENS[0])
}
