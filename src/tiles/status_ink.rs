//! What a settler is going through, drawn over their figure: a small parchment bubble by the head
//! with one ink sign for the most pressing thing (a stress break, a strange mood, a wound, what a
//! need sends them to do, drink, a meal, a black mood), and marks on the figure itself (a bandage
//! on the wounded, a circlet for the lord, a hood for the priest, a chain for the speaker, a
//! wide hat for a visitor from the world). DF shows these as the unit's thoughts; here they are
//! seen on the map, so the camp's inner life is not only in the log.

use super::ink::{mix, Finish, Pen, Rgb, INK, BLOOD};
use crate::colony::needs::Need;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Emblem {
    Tantrum, Despair, Lost,
    Fey, Secretive, Possessed, Macabre, Fell,
    Hurt, Pray, Talk, Rest, Watch, Admire, Walk, Thrill, Help, Learn, Think, Merry, Tale, Martial, Whittle, Busy, Drink, Meal, Gloom, Love, Grief, Acquire, Dreaming,
}

/// The sign over settler `i`, if any (most pressing first).
pub fn emblem_of(colony: &crate::colony::Colony, i: usize) -> Option<Emblem> {
    use crate::colony::mind::Break;
    use crate::colony::mood::MoodKind;
    let s = &colony.settlers[i];
    let tick = colony.clock.tick;
    if let Some((b, until)) = s.mind.broken { if until > tick { return Some(match b { Break::Tantrum => Emblem::Tantrum, Break::Despair => Emblem::Despair, Break::Wandering => Emblem::Lost }); } }
    if let Some(m) = colony.mood.as_ref().filter(|m| m.who == i && !m.done) {
        return Some(match m.kind { MoodKind::Fey => Emblem::Fey, MoodKind::Secretive => Emblem::Secretive, MoodKind::Possessed => Emblem::Possessed, MoodKind::Macabre => Emblem::Macabre, MoodKind::Fell => Emblem::Fell });
    }
    if s.wounds.iter().any(|w| w.healed_at > tick && w.severity >= 2) { return Some(Emblem::Hurt); }
    if let (crate::colony::Job::Wander(_), Some(a)) = (s.job, s.need_act.as_ref()) {
        return Some(match a.need {
            Need::Pray => Emblem::Pray, Need::Socialize | Need::Friends | Need::Family => Emblem::Talk, Need::TakeItEasy => Emblem::Rest,
            Need::SeeAnimal => Emblem::Watch, Need::AdmireArt => Emblem::Admire, Need::Wander => Emblem::Walk, Need::Excitement => Emblem::Thrill,
            Need::HelpSomebody => Emblem::Help, Need::Learn => Emblem::Learn, Need::ThinkAbstractly => Emblem::Think, Need::MakeMerry => Emblem::Merry,
            Need::Tradition => Emblem::Tale, Need::Martial => Emblem::Martial, Need::Craft | Need::BeCreative => Emblem::Whittle,
            Need::StayOccupied => Emblem::Busy, Need::Drink => Emblem::Drink, Need::GoodMeal => Emblem::Meal,
            Need::Romance => Emblem::Love, Need::Remember => Emblem::Grief, Need::Acquire => Emblem::Acquire,
        });
    }
    // Asleep under the patron's dream (`Patron::dreams`): a moon in the bubble.
    if s.job == crate::colony::Job::Sleep && colony.patron.dreams.iter().any(|d| d.0 == i && colony.clock.day() <= d.2 + 1) { return Some(Emblem::Dreaming); }
    if s.job == crate::colony::Job::Eat { return Some(Emblem::Meal); }
    if s.last_drink > 0 && tick.saturating_sub(s.last_drink) < 90 { return Some(Emblem::Drink); }
    if s.mind.stress >= 0.8 { return Some(Emblem::Gloom); }
    None
}

/// The bubble: parchment with an ink rim and a tail pointing down to the head at (x, y).
pub fn draw_bubble(put: &mut dyn FnMut(i64, i64, Rgb, f32), e: Emblem, x: f32, y: f32, scale: f32) {
    let size = (24.0 * scale).max(14.0);
    let (cx, cy) = (x, y - size * 0.55);
    let mut pen = Pen::new(put, cx, cy, size);
    let paper = match e { Emblem::Tantrum | Emblem::Fell => [242.0, 214.0, 200.0], Emblem::Possessed | Emblem::Secretive => [226.0, 214.0, 232.0], _ => [244.0, 236.0, 216.0] };
    pen.poly_f(&[(-0.14, 0.5), (0.0, 0.88), (0.12, 0.5)], paper, Finish::Plain);
    pen.ellipse_f(0.0, 0.0, 0.72, 0.6, paper, Finish::Plain);
    icon(&mut pen, e);
}

fn icon(pen: &mut Pen, e: Emblem) {
    let w = (pen.half * 0.09).max(1.0);
    let gold = [214.0, 170.0, 60.0];
    match e {
        Emblem::Tantrum => { pen.path(&[(-0.4, 0.15), (-0.2, -0.25), (0.0, 0.2), (0.15, -0.3), (0.38, 0.12)], BLOOD, w * 1.4); pen.path(&[(-0.3, -0.35), (-0.15, -0.2)], BLOOD, w); }
        Emblem::Despair => { pen.ellipse_f(0.0, -0.12, 0.4, 0.2, [90.0, 92.0, 104.0], Finish::Plain); for u in [-0.22f32, 0.0, 0.22] { pen.line((u, 0.12), (u - 0.06, 0.36), [80.0, 110.0, 150.0], w); } }
        Emblem::Lost => { pen.path(&[(-0.2, -0.2), (-0.05, -0.36), (0.15, -0.3), (0.18, -0.12), (0.0, 0.02), (0.0, 0.15)], INK, w * 1.3); pen.dot(0.0, 0.32, INK); }
        Emblem::Fey => { for k in 0..8 { let a = k as f32 * 0.785; let r = if k % 2 == 0 { 0.42 } else { 0.18 }; pen.line((0.0, 0.0), (a.cos() * r, a.sin() * r), gold, w * 1.2); } pen.ellipse_f(0.0, 0.0, 0.1, 0.1, [250.0, 230.0, 160.0], Finish::Plain); }
        Emblem::Secretive => { pen.path(&[(-0.38, 0.0), (-0.15, 0.15), (0.15, 0.15), (0.38, 0.0)], INK, w * 1.3); for u in [-0.2f32, 0.0, 0.2] { pen.line((u, 0.15), (u, 0.28), INK, w); } }
        Emblem::Possessed => { let pts: Vec<(f32, f32)> = (0..18).map(|k| { let a = k as f32 * 0.6; let r = 0.04 + k as f32 * 0.022; (a.cos() * r, a.sin() * r) }).collect(); pen.path(&pts, [110.0, 60.0, 140.0], w * 1.2); }
        Emblem::Macabre | Emblem::Fell => {
            let c = if e == Emblem::Fell { [200.0, 70.0, 56.0] } else { [232.0, 224.0, 204.0] };
            pen.ellipse(0.0, -0.06, 0.3, 0.27, c); pen.rect(-0.14, 0.12, 0.14, 0.32, c);
            for u in [-0.12f32, 0.12] { pen.ellipse_f(u, -0.06, 0.07, 0.08, INK, Finish::Paint); }
        }
        Emblem::Hurt => { pen.rect_f(-0.1, -0.36, 0.1, 0.36, BLOOD, Finish::Plain); pen.rect_f(-0.36, -0.1, 0.36, 0.1, BLOOD, Finish::Plain); }
        Emblem::Pray => { pen.poly(&[(-0.05, 0.32), (-0.2, 0.0), (-0.02, -0.38), (0.0, 0.3)], [226.0, 196.0, 164.0]); pen.poly(&[(0.05, 0.32), (0.2, 0.0), (0.02, -0.38), (0.0, 0.3)], [214.0, 184.0, 152.0]); pen.path(&[(-0.34, -0.3), (-0.25, -0.38)], gold, w); pen.path(&[(0.34, -0.3), (0.25, -0.38)], gold, w); }
        Emblem::Talk => { for u in [-0.25f32, 0.0, 0.25] { pen.ellipse_f(u, 0.0, 0.07, 0.07, INK, Finish::Plain); } }
        Emblem::Rest => { pen.path(&[(-0.3, -0.2), (0.0, -0.2), (-0.3, 0.1), (0.0, 0.1)], INK, w); pen.path(&[(0.08, 0.05), (0.3, 0.05), (0.08, 0.28), (0.3, 0.28)], INK, w); }
        Emblem::Watch | Emblem::Admire => { pen.ellipse_f(0.0, 0.0, 0.38, 0.2, [250.0, 248.0, 240.0], Finish::Plain); pen.ellipse_f(0.0, 0.0, 0.13, 0.14, if e == Emblem::Admire { gold } else { [90.0, 110.0, 80.0] }, Finish::Plain); pen.dot(0.0, 0.0, INK); }
        Emblem::Walk => { for (u, v) in [(-0.2f32, 0.2f32), (0.15, -0.15)] { pen.ellipse_f(u, v, 0.1, 0.15, INK, Finish::Paint); pen.ellipse_f(u, v - 0.2, 0.06, 0.05, INK, Finish::Paint); } }
        Emblem::Thrill => { pen.line((0.0, -0.36), (0.0, 0.12), BLOOD, w * 1.6); pen.dot(0.0, 0.3, BLOOD); }
        Emblem::Help => { pen.ellipse(-0.14, 0.0, 0.2, 0.2, [214.0, 90.0, 80.0]); pen.ellipse(0.14, 0.0, 0.2, 0.2, [214.0, 90.0, 80.0]); pen.poly_f(&[(-0.32, 0.08), (0.0, 0.36), (0.32, 0.08)], [214.0, 90.0, 80.0], Finish::Paint); }
        Emblem::Learn | Emblem::Tale => { pen.rect(-0.34, -0.22, 0.0, 0.26, if e == Emblem::Learn { [130.0, 58.0, 46.0] } else { [236.0, 226.0, 200.0] }); pen.rect(0.0, -0.22, 0.34, 0.26, if e == Emblem::Learn { [130.0, 58.0, 46.0] } else { [236.0, 226.0, 200.0] }); for k in 0..2 { let v = -0.08 + k as f32 * 0.14; pen.line((-0.26, v), (-0.08, v), INK, 1.0); pen.line((0.08, v), (0.26, v), INK, 1.0); } }
        Emblem::Think => { for (u, v, r) in [(-0.2f32, 0.2f32, 0.08f32), (0.0, 0.0, 0.11), (0.22, -0.18, 0.14)] { pen.ellipse(u, v, r, r, [250.0, 248.0, 240.0]); } }
        Emblem::Merry => { pen.ellipse(-0.12, 0.22, 0.12, 0.09, INK); pen.line((-0.01, 0.22), (0.02, -0.32), INK, w); pen.path(&[(0.02, -0.32), (0.25, -0.2), (0.22, -0.05)], INK, w); }
        Emblem::Martial => { pen.bone(&[(-0.3, 0.3), (0.3, -0.3)], [200.0, 202.0, 210.0], w); pen.bone(&[(0.3, 0.3), (-0.3, -0.3)], [200.0, 202.0, 210.0], w); }
        Emblem::Whittle => { pen.poly(&[(-0.3, 0.25), (0.18, -0.25), (0.28, -0.16), (-0.18, 0.32)], [200.0, 202.0, 210.0]); pen.rect(-0.38, 0.22, -0.18, 0.36, [122.0, 86.0, 54.0]); }
        Emblem::Busy => { pen.bone(&[(-0.25, 0.3), (0.1, -0.05)], [122.0, 86.0, 54.0], w); pen.rect(0.0, -0.3, 0.32, -0.05, [150.0, 150.0, 158.0]); }
        Emblem::Drink => { pen.rect(-0.2, -0.22, 0.14, 0.32, [176.0, 124.0, 70.0]); pen.rect_f(-0.2, -0.3, 0.14, -0.16, [244.0, 236.0, 210.0], Finish::Plain); pen.path(&[(0.14, -0.1), (0.3, -0.05), (0.3, 0.15), (0.14, 0.2)], INK, w); }
        Emblem::Meal => { pen.ellipse(0.0, 0.08, 0.36, 0.2, [196.0, 160.0, 110.0]); pen.ellipse_f(0.0, 0.0, 0.26, 0.09, [150.0, 100.0, 60.0], Finish::Paint); pen.line((0.15, -0.35), (0.3, 0.0), INK, w); }
        Emblem::Love => {
            let red = [190.0, 60.0, 70.0];
            pen.ellipse(-0.14, -0.08, 0.18, 0.18, red); pen.ellipse(0.14, -0.08, 0.18, 0.18, red);
            pen.poly_f(&[(-0.3, 0.0), (0.0, 0.34), (0.3, 0.0), (0.0, -0.05)], red, Finish::Paint);
        }
        Emblem::Grief => {
            pen.rect(-0.1, -0.05, 0.1, 0.38, [236.0, 228.0, 206.0]);
            pen.glow(0.0, -0.22, 0.25, [250.0, 200.0, 110.0], 0.8);
            pen.poly_f(&[(-0.05, -0.08), (0.0, -0.34), (0.05, -0.08)], [240.0, 170.0, 70.0], Finish::Plain);
        }
        Emblem::Acquire => {
            pen.ellipse(0.0, 0.08, 0.3, 0.26, [176.0, 136.0, 90.0]);
            pen.rect(-0.08, -0.28, 0.08, -0.14, [176.0, 136.0, 90.0]);
            pen.line((-0.12, -0.16), (0.12, -0.16), INK, 1.0);
            pen.ellipse(0.24, 0.24, 0.1, 0.07, [214.0, 176.0, 70.0]);
        }
        Emblem::Dreaming => {
            pen.ellipse_f(0.0, 0.0, 0.6, 0.48, [52.0, 62.0, 92.0], Finish::Paint);
            pen.ellipse(0.05, -0.02, 0.28, 0.28, [240.0, 226.0, 170.0]);
            pen.ellipse_f(0.2, -0.1, 0.24, 0.24, [52.0, 62.0, 92.0], Finish::Paint);
            pen.dot(-0.35, -0.2, [250.0, 240.0, 200.0]); pen.dot(-0.25, 0.25, [250.0, 240.0, 200.0]);
        }
        Emblem::Gloom => { pen.ellipse_f(0.0, 0.0, 0.42, 0.24, [70.0, 70.0, 80.0], Finish::Plain); pen.ellipse_f(-0.18, -0.08, 0.2, 0.16, [96.0, 96.0, 106.0], Finish::Paint); }
    }
}

/// Marks on the figure itself, in the figure's own units (a 22 px box at scale 1 round the bust,
/// as `folk.rs` draws): a bandage, an office's headgear, a visitor's hat. (hx, hy) is the head's
/// centre on screen, `hr` its radius in pixels.
pub fn figure_marks(put: &mut dyn FnMut(i64, i64, Rgb, f32), colony: &crate::colony::Colony, i: usize, hx: f32, hy: f32, hr: f32) {
    let s = &colony.settlers[i];
    let tick = colony.clock.tick;
    let mut pen = Pen::new(put, hx, hy, hr * 2.0);
    // Unit = the head's radius.
    if s.wounds.iter().any(|w| w.healed_at > tick) {
        pen.shape([240.0, 236.0, 226.0], Finish::Plain, [-1.05, -0.55, 1.05, 0.0], &|u, v| (u * u + v * v) <= 1.1 && v > -0.55 && v < -0.12);
        pen.dot(0.45, -0.32, BLOOD);
    }
    let office = s.office.as_deref().unwrap_or("");
    if office.starts_with("Lord") {
        for k in 0..3 { let u = -0.5 + k as f32 * 0.5; pen.poly(&[(u - 0.2, -0.75), (u, -1.3), (u + 0.2, -0.75)], [214.0, 176.0, 70.0]); }
        pen.rect(-0.8, -0.95, 0.8, -0.65, [214.0, 176.0, 70.0]);
    } else if office.starts_with("Keeps the temple") {
        pen.ellipse_f(0.0, -1.35, 0.7, 0.18, [226.0, 196.0, 90.0], Finish::Plain);
        pen.ellipse_f(0.0, -1.35, 0.45, 0.08, mix([244.0, 236.0, 216.0], [226.0, 196.0, 90.0], 0.3), Finish::Paint);
    } else if office.starts_with("Speaks") {
        pen.path(&[(-0.8, 1.0), (0.0, 1.45), (0.8, 1.0)], [214.0, 176.0, 70.0], (pen.half * 0.12).max(1.0));
        pen.ellipse(0.0, 1.5, 0.22, 0.22, [214.0, 176.0, 70.0]);
    } else if office.starts_with("Tends") {
        pen.rect_f(0.95, 0.9, 1.45, 1.4, [240.0, 236.0, 226.0], Finish::Plain);
        pen.rect_f(1.15, 0.95, 1.25, 1.35, BLOOD, Finish::Paint);
        pen.rect_f(1.0, 1.1, 1.4, 1.2, BLOOD, Finish::Paint);
    } else if office.starts_with("Cooks") {
        pen.ellipse(0.0, -1.25, 0.62, 0.36, [244.0, 240.0, 230.0]);
        pen.rect(-0.55, -1.05, 0.55, -0.75, [244.0, 240.0, 230.0]);
    }
    if let (Some(calling), true) = (s.visitor.as_deref(), s.guest_until > tick) { guest_marks(&mut pen, calling); }
    // A vampire, once the sharp-eyed have noticed (`night.rs`): a pallor and red eyes at night.
    if colony.vampire_noticed && colony.vampire.map_or(false, |v| v.0 == i) && colony.clock.is_night() {
        pen.glow(-0.4, -0.05, 0.35, [230.0, 40.0, 30.0], 0.8);
        pen.glow(0.4, -0.05, 0.35, [230.0, 40.0, 30.0], 0.8);
        pen.dot(-0.4, -0.05, [250.0, 70.0, 50.0]);
        pen.dot(0.4, -0.05, [250.0, 70.0, 50.0]);
    }
}

/// A guest's look by their calling, on a pen whose unit is the head's radius (centred on it).
pub fn guest_marks(pen: &mut Pen, calling: &str) {
        // A guest from the world, by their calling (`visitors.rs`): the monster hunter's
        // feathered cap and bow, the teller's or loremaster's wide hat and lute or book, the
        // seeker's hood and lantern, the sellsword's helm and sword.
        let lw = (pen.half * 0.12).max(1.0);
        if calling.starts_with("a monster hunter") {
            pen.ellipse(0.0, -0.85, 1.05, 0.4, [90.0, 110.0, 70.0]);
            pen.bone(&[(0.6, -1.0), (1.3, -1.9)], [196.0, 60.0, 50.0], lw * 1.5);
            pen.path(&[(-1.9, -1.2), (-1.5, 0.0), (-1.9, 1.6)], [140.0, 100.0, 60.0], lw * 1.6);
            pen.line((-1.9, -1.2), (-1.9, 1.6), [226.0, 218.0, 200.0], 1.0);
        } else if calling.starts_with("a loremaster") || calling.starts_with("a teller") {
            pen.ellipse(0.0, -0.75, 1.5, 0.32, [96.0, 80.0, 70.0]);
            pen.ellipse(0.0, -1.0, 0.75, 0.45, [110.0, 92.0, 80.0]);
            if calling.starts_with("a loremaster") { pen.rect(-2.2, 0.8, -1.3, 1.9, [130.0, 58.0, 46.0]); pen.line((-1.75, 0.8), (-1.75, 1.9), INK, 1.0); }
            else { pen.ellipse(-1.8, 1.4, 0.45, 0.55, [176.0, 128.0, 80.0]); pen.bone(&[(-1.8, 0.9), (-1.6, -0.6)], [140.0, 100.0, 60.0], lw * 1.2); }
        } else if calling.starts_with("a seeker") {
            pen.shape([110.0, 96.0, 120.0], Finish::Inked, [-1.3, -1.6, 1.3, 0.9], &|u, v| { let o = (u / 1.15).powi(2) + ((v + 0.1) / 1.25).powi(2) <= 1.0; let f = (u / 0.85).powi(2) + ((v - 0.05) / 0.9).powi(2) <= 1.0 && v > -0.5; o && !f });
            pen.line((-1.7, -0.2), (-1.7, 0.6), INK, 1.0);
            pen.rect(-2.05, 0.6, -1.35, 1.3, [196.0, 170.0, 110.0]);
            pen.glow(-1.7, 0.95, 0.8, [250.0, 200.0, 110.0], 0.6);
        } else {
            pen.shape([150.0, 150.0, 156.0], Finish::Inked, [-1.1, -1.25, 1.1, -0.1], &|u, v| v < -0.15 && (u / 1.08).powi(2) + (v / 1.2).powi(2) <= 1.0);
            pen.rect_f(-0.12, -0.3, 0.12, 0.35, [150.0, 150.0, 156.0], Finish::Plain);
            pen.bone(&[(1.6, 1.8), (2.3, -0.8)], [200.0, 202.0, 210.0], lw * 1.6);
            pen.line((1.4, 1.1), (2.0, 1.3), INK, lw);
        }
    }
