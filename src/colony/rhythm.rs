//! Character in the day's shape: which work a settler takes to (`taste_of`), and when they
//! rise and lie down (`wake_minute`, `bed_minute`). DF's dwarves differ in what they like to do
//! by their preferences and personality; here each kind of work's pull is a little luck (by the
//! name) leaned by who they are: lovers of the wild forage and fish (the patient fish better),
//! the strong fell and quarry (and lovers of the wild do not), the orderly haul, craftsmen and the
//! dutiful build. The hard-working rise at five and lie down early; lovers of leisure lie abed
//! till half past seven; the immoderate and the thrill-seeking sit up late.

use super::*;
use crate::persona::{Attr, Facet, Persona, Val};

/// The pull of each kind of work (forage, fish, fell/quarry, haul, build), 0.7..1.4.
pub fn taste_of(p: &Persona, name: &str) -> [f32; 5] {
    let h = crate::persona::seed_of(name, 0x7A57E);
    let luck = |k: u64| 0.85 + 0.3 * ((h >> (k * 9)) % 1000) as f32 / 1000.0;
    let v = |x: Val| p.value(x) as f32 / 50.0;
    let f = |x: Facet| (p.facet(x) as f32 - 50.0) / 50.0;
    let a = |x: Attr| p.attr(x) / 1000.0 - 1.0;
    let lean = [
        1.0 + 0.25 * v(Val::Nature),
        1.0 + 0.15 * v(Val::Nature) + 0.2 * a(Attr::Patience),
        1.0 + 0.25 * a(Attr::Strength) - 0.2 * v(Val::Nature).max(0.0),
        1.0 + 0.2 * f(Facet::Orderliness),
        1.0 + 0.2 * v(Val::Craftsmanship) + 0.1 * f(Facet::Dutifulness),
    ];
    let mut t = [0.0; 5];
    for k in 0..5 { t[k] = (luck(k as u64) * lean[k]).clamp(0.7, 1.4); }
    t
}

impl Colony {
    /// The minute of the day settler `i` rises (5:00 .. 7:30).
    pub fn wake_minute(&self, i: usize) -> u64 {
        let p = &self.settlers[i].persona;
        let lazy = (p.value(Val::Leisure) as f32 - p.value(Val::HardWork) as f32) / 50.0;
        (360.0 + 60.0 * lazy).clamp(300.0, 450.0) as u64
    }

    /// The minute of the day settler `i` lies down (20:00 .. 22:30).
    pub fn bed_minute(&self, i: usize) -> u64 {
        let p = &self.settlers[i].persona;
        let owl = ((p.facet(Facet::Immoderation) as f32 + p.facet(Facet::ExcitementSeeking) as f32) / 2.0 - 50.0) / 50.0 - 0.5 * p.value(Val::HardWork) as f32 / 50.0;
        (1260.0 + 60.0 * owl).clamp(1200.0, 1350.0) as u64
    }

    /// Whether settler `i`'s own night is now (abed, by their rhythm).
    pub fn abed(&self, i: usize) -> bool {
        let m = self.clock.tick % TICKS_PER_DAY;
        m < self.wake_minute(i) || m >= self.bed_minute(i)
    }
}
