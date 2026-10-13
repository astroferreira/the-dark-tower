//! Names for stars, planets and moons: two or three syllables in an old, star-gazers' tongue.

use rand::Rng;

const ONSETS: [&str; 24] = [
    "", "", "b", "c", "d", "h", "k", "l", "m", "n", "r", "s", "t", "v", "th", "ph", "z", "al", "s", "v", "c", "m", "or", "n",
];
const VOWELS: [&str; 12] = ["a", "e", "i", "o", "u", "ae", "ei", "y", "a", "e", "o", "i"];
const CODAS: [&str; 12] = ["", "", "", "n", "r", "s", "th", "l", "x", "m", "rn", "st"];
const ENDINGS: [&str; 10] = ["os", "is", "a", "on", "ar", "eth", "ia", "un", "el", "ys"];

/// A name of two or three syllables, capitalised.
pub fn name(rng: &mut impl Rng) -> String {
    loop {
        let syllables = if rng.gen_bool(0.6) { 1 } else { 2 };
        let mut s = String::new();
        for _ in 0..syllables {
            s.push_str(ONSETS[rng.gen_range(0..ONSETS.len())]);
            s.push_str(VOWELS[rng.gen_range(0..VOWELS.len())]);
            s.push_str(CODAS[rng.gen_range(0..CODAS.len())]);
        }
        s.push_str(ENDINGS[rng.gen_range(0..ENDINGS.len())]);
        let ok = (4..=10).contains(&s.len())
            && !s.contains("aa") && !s.contains("ii") && !s.contains("uu") && !s.contains("yy")
            && crate::history::naming::generator::pronounceable(&s, 10);
        if ok {
            let mut c = s.chars();
            return c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or(s);
        }
    }
}

/// Names for every body of a system, none repeated.
pub struct Namer { used: Vec<String> }

impl Namer {
    pub fn new() -> Self { Namer { used: Vec::new() } }

    pub fn next(&mut self, rng: &mut impl Rng) -> String {
        loop {
            let n = name(rng);
            if !self.used.contains(&n) { self.used.push(n.clone()); return n; }
        }
    }
}
