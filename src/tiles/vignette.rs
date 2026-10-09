//! A small inked picture for each great moment's card: a roundel at the card's left, drawn from
//! what the moment is about with the same sprites as the map: the beast or the raiders of a raid,
//! the house a work made (with its sign), a grave for a death, a ghost, a trader and mule, the
//! refugees, a dream's moon, the tool of a role, a pick at the rock, water, a gem, a sound from
//! below as an eye in the dark, the lord's crown, winter's snow.

use super::ink::{mix, Finish, Pen, Rgb, INK, BLOOD};

const PAPER_IN: Rgb = [240.0, 230.0, 206.0];

/// Draw the roundel for moment `m` centred at (cx, cy), radius `r` pixels.
pub fn draw(put: &mut dyn FnMut(i64, i64, Rgb, f32), colony: Option<&crate::colony::Colony>, m: &crate::colony::Moment, cx: f32, cy: f32, r: f32) {
    {
        let mut pen = Pen::new(put, cx, cy, 2.0 * r);
        pen.ellipse_f(0.0, 0.0, 1.0, 1.0, INK, Finish::Plain);
        pen.ellipse_f(0.0, 0.0, 0.93, 0.93, PAPER_IN, Finish::Plain);
        pen.shape(INK, Finish::Paint, [-0.9, -0.9, 0.9, 0.9], &|u, v| { let d = (u * u + v * v).sqrt(); (d - 0.86).abs() < 0.012 });
    }
    let t = format!("{} {}", m.title, m.text).to_lowercase();
    let has = |k: &str| t.contains(k);
    // Scenes are drawn in a box half the roundel across, on a ground line.
    let ground = |put: &mut dyn FnMut(i64, i64, Rgb, f32)| {
        let mut pen = Pen::new(put, cx, cy, 2.0 * r);
        pen.shape([214.0, 196.0, 156.0], Finish::Paint, [-0.9, 0.35, 0.9, 0.9], &|u, v| v > 0.42 && u * u + v * v < 0.8);
        pen.line_a((-0.75, 0.42), (0.75, 0.42), INK, 1.0, 0.6);
    };
    let title = m.title.to_lowercase();
    // The beast the moment names: the colony's threat or a cavern's beast, if it is the one in
    // the text (the threat moves on after a raid); else a beast by its words.
    let named = |n: &str| !n.is_empty() && (m.text.contains(n) || m.title.contains(n));
    let monster = colony.and_then(|c| c.arc.as_ref()).filter(|a| named(&a.threat.name)).and_then(|a| a.threat.monster.as_ref())
        .or_else(|| colony.and_then(|c| c.foes_seen.iter().find(|(n, _)| named(n)).map(|(_, m)| m)))
        .or_else(|| colony.and_then(|c| c.map.caverns.iter().filter_map(|cv| cv.beast.as_ref()).find(|(n, _)| named(n)).map(|(_, m)| m)));
    let beast_raid = monster.is_some() || colony.and_then(|c| c.arc.as_ref()).map_or(false, |a| named(&a.threat.name) && matches!(a.threat.kind, crate::colony::arc::ThreatKind::Beast | crate::colony::arc::ThreatKind::Deep));
    // Who came, as the text names them first: a band's name says so ("raiders of", "a war band",
    // "a band of", "deserters"); else it was a beast.
    let actor = m.text.split([',', '.']).next().unwrap_or("").to_lowercase();
    let band = ["raiders", "a war band", "war band", "a band of", "deserters", "outlaws", "soldiers"].iter().any(|k| actor.contains(k));
    let beast_raid = beast_raid || (!band && !actor.is_empty() && (title.contains("raid") || title.contains("coming")));
    let threat_look = monster.map(super::beasts::of_monster).or_else(|| beast_raid.then(|| {
        let mut l = super::beasts::of_name(&actor);
        if l.base == super::beasts::Base::Quad && l.quad.horn == super::beasts::Horn::None && !actor.contains("wolf") {
            // A beast the camp knows only by name: a great dark horned thing.
            l = super::beasts::of_name("bear");
            l.coat = [70.0, 58.0, 56.0];
            l.quad.horn = super::beasts::Horn::Bovine;
            l.parts |= super::beasts::part::SPINES;
        }
        l.glow = Some([230.0, 70.0, 40.0]);
        l
    }));
    if title.contains("envoy") || title.contains("tribute") {
        // An envoy: a figure in their people's colours with a staff of office and a scroll.
        ground(put);
        let threat = colony.and_then(|c| c.arc.as_ref()).map(|a| &a.threat);
        let mut f = super::folk::raider(threat, &m.text, 0, None);
        f.arm = super::folk::Arm::Staff; f.helm = super::folk::Helm::Cap;
        super::folk::draw(put, &f, cx - r * 0.15, cy + r * 0.12, r / 22.0 * 1.4, false, false, 1.0);
        let mut pen = Pen::new(put, cx, cy, 2.0 * r);
        pen.rect(0.25, -0.05, 0.6, 0.25, [236.0, 226.0, 200.0]);
        pen.ellipse_f(0.42, 0.1, 0.05, 0.05, [150.0, 40.0, 30.0], Finish::Plain);
        return;
    }
    if title.contains("raid") || title.contains("they are coming") || has("climbs out of the mine") || has("coming up the stair") {
        ground(put);
        if let Some(look) = threat_look {
            let size = if look.flies { r * 1.5 } else { r * 1.9 };
            super::beasts::draw(put, &look, cx, cy + r * if look.flies { 1.05 } else { 0.45 }, size, false, super::beasts::Pose::Strike, 1.0);
        } else {
            let threat = colony.and_then(|c| c.arc.as_ref()).map(|a| &a.threat).filter(|t| named(&t.name));
            let name = threat.map(|t| t.name.clone()).unwrap_or_else(|| "raiders".into());
            for (k, dx) in [(0u32, -0.35f32), (1, 0.3)] {
                let f = super::folk::raider(threat, &name, k, None);
                super::folk::draw(put, &f, cx + dx * r, cy + r * 0.12, r / 22.0 * 1.3, k == 1, true, 1.0);
            }
        }
        return;
    }
    if title.ends_with(" finished") || title.contains("the hut stands") {
        ground(put);
        house(put, cx, cy, r, &title);
        return;
    }
    if has("ghost") {
        let f = super::folk::Folk { skin: [214.0, 226.0, 220.0], hair: [190.0, 200.0, 196.0], dress: [196.0, 214.0, 208.0], helm: super::folk::Helm::None, arm: super::folk::Arm::None, shield: None, tusks: false, pointed: false, beard: false, glow: Some([170.0, 240.0, 220.0]), pale: true };
        let mut pen = Pen::new(put, cx, cy, 2.0 * r);
        pen.ellipse_f(0.0, 0.0, 0.85, 0.85, [60.0, 72.0, 96.0], Finish::Paint);
        drop(pen);
        super::folk::draw(put, &f, cx, cy + r * 0.2, r / 22.0 * 1.6, false, false, 0.75);
        return;
    }
    if has("dies") || has("is dead") || has("death") || has("killed") || has("grave") || has("buried") || title.contains("lost") {
        ground(put);
        let mut pen = Pen::new(put, cx, cy, 2.0 * r);
        pen.ellipse(0.0, 0.42, 0.5, 0.14, [150.0, 124.0, 90.0]);
        pen.bone(&[(0.0, 0.42), (0.0, -0.45)], [122.0, 86.0, 54.0], (r * 0.09).max(2.0));
        pen.bone(&[(-0.25, -0.18), (0.25, -0.18)], [122.0, 86.0, 54.0], (r * 0.09).max(2.0));
        return;
    }
    if has("born") || has("a child") {
        ground(put);
        let mut pen = Pen::new(put, cx, cy, 2.0 * r);
        pen.shape([164.0, 120.0, 78.0], Finish::Inked, [-0.5, -0.05, 0.5, 0.45], &|u, v| v > 0.0 && v < 0.4 && u.abs() < 0.48 - (v - 0.4).abs() * 0.2);
        pen.ellipse(-0.18, 0.02, 0.15, 0.13, [232.0, 196.0, 164.0]);
        pen.rect(-0.05, -0.05, 0.4, 0.15, [236.0, 228.0, 210.0]);
        return;
    }
    if has("caravan") || has("traders") || has("trader") && !has("rumour") {
        ground(put);
        let mule = super::beasts::of_name("mule");
        super::beasts::draw(put, &mule, cx - r * 0.3, cy + r * 0.45, r * 1.0, false, super::beasts::Pose::Walk(true), 1.0);
        super::folk::draw(put, &super::folk::trader("traders", 0), cx + r * 0.4, cy + r * 0.12, r / 22.0 * 1.2, true, false, 1.0);
        return;
    }
    let season = ["winter", "summer", "spring", "autumn", "the thaw"].iter().any(|k| title.contains(k));
    if (has("refugee") || has("migrant") || has("comes") || has("visitor") || has("seeker") || has("hunter")) && !title.contains("lord") && !season {
        ground(put);
        let n = if has("refugee") || has("migrant") { 2 } else { 1 };
        for k in 0..n {
            let mut f = super::folk::trader(&m.title, k as u32 + 3);
            f.helm = if has("refugee") { super::folk::Helm::Hood } else { super::folk::Helm::Cap };
            f.arm = super::folk::Arm::Staff;
            let dx = if n == 1 { 0.0 } else { -0.3 + 0.6 * k as f32 };
            super::folk::draw(put, &f, cx + dx * r, cy + r * 0.12, r / 22.0 * 1.3, k == 1, false, 1.0);
        }
        return;
    }
    let mut pen = Pen::new(put, cx, cy, 2.0 * r);
    let gold = [214.0, 170.0, 60.0];
    if has("rumour") || has("word comes") || has("news") {
        pen.rect(-0.45, -0.45, 0.45, 0.4, [236.0, 226.0, 200.0]);
        pen.ellipse(-0.45, -0.02, 0.1, 0.45, [214.0, 200.0, 170.0]);
        pen.ellipse(0.45, -0.02, 0.1, 0.45, [214.0, 200.0, 170.0]);
        for k in 0..4 { let v = -0.25 + k as f32 * 0.15; pen.line((-0.3, v), (0.3, v), INK, 1.0); }
    } else if has("dream") {
        pen.ellipse_f(0.0, 0.0, 0.85, 0.85, [52.0, 62.0, 92.0], Finish::Paint);
        pen.ellipse(0.1, -0.1, 0.36, 0.36, [240.0, 226.0, 170.0]);
        pen.ellipse_f(0.28, -0.22, 0.32, 0.32, [52.0, 62.0, 92.0], Finish::Paint);
        for (u, v) in [(-0.5f32, -0.3f32), (-0.3, 0.4), (0.45, 0.35), (-0.55, 0.15)] { pen.dot(u, v, [250.0, 240.0, 200.0]); }
    } else if has("woodcutter") || has("fells") {
        pen.bone(&[(-0.4, 0.5), (0.25, -0.25)], [122.0, 86.0, 54.0], (r * 0.08).max(2.0));
        pen.poly(&[(0.12, -0.42), (0.5, -0.2), (0.38, 0.05), (0.18, -0.12)], [176.0, 178.0, 184.0]);
    } else if has("builder") || has("hammer") {
        pen.bone(&[(-0.4, 0.5), (0.15, -0.1)], [122.0, 86.0, 54.0], (r * 0.08).max(2.0));
        pen.rect(-0.05, -0.38, 0.4, -0.1, [150.0, 150.0, 158.0]);
    } else if has("forager") || has("berries") {
        super::glyphs::draw(&mut |x, y, c, a| pen.pixel(x, y, c, a), super::glyphs::Glyph::Berries, cx, cy, r * 1.1, None);
    } else if has("fisher") || has("fish") {
        super::glyphs::draw(&mut |x, y, c, a| pen.pixel(x, y, c, a), super::glyphs::Glyph::Fish, cx, cy, r * 1.2, None);
    } else if has("carrier") {
        super::glyphs::draw(&mut |x, y, c, a| pen.pixel(x, y, c, a), super::glyphs::Glyph::Provisions, cx, cy, r * 1.1, None);
    } else if has("speaks for") || has("mandate") || has("speaker") {
        pen.path(&[(-0.5, -0.4), (0.0, 0.1), (0.5, -0.4)], gold, (r * 0.07).max(2.0));
        pen.ellipse(0.0, 0.25, 0.22, 0.22, gold);
        pen.ellipse_f(0.0, 0.25, 0.1, 0.1, [190.0, 150.0, 50.0], Finish::Plain);
    } else if has("lord") || has("crown") || has("rising") {
        pen.poly(&[(-0.5, 0.3), (-0.5, -0.25), (-0.25, 0.05), (0.0, -0.4), (0.25, 0.05), (0.5, -0.25), (0.5, 0.3)], gold);
        pen.dot(0.0, 0.12, [140.0, 40.0, 50.0]);
    } else if has("temple") || has("pray") || has("god") || has("priest") {
        pen.ellipse(0.0, 0.0, 0.3, 0.3, gold);
        for k in 0..12 { let a = k as f32 * 0.5236; pen.line((a.cos() * 0.38, a.sin() * 0.38), (a.cos() * 0.62, a.sin() * 0.62), gold, (r * 0.05).max(1.5)); }
    } else if has("breaks") || has("tantrum") || has("despair") || has("fit") {
        drop(pen);
        super::status_ink::draw_bubble(put, if has("despair") { super::status_ink::Emblem::Despair } else { super::status_ink::Emblem::Tantrum }, cx, cy + r * 0.55, r / 14.0);
        return;
    } else if has("mood") || has("possess") || has("fey") || has("artifact") {
        for k in 0..8 { let a = k as f32 * 0.785; let rr = if k % 2 == 0 { 0.6 } else { 0.28 }; pen.line((0.0, 0.0), (a.cos() * rr, a.sin() * rr), gold, (r * 0.06).max(1.5)); }
        pen.ellipse(0.0, 0.0, 0.14, 0.14, [250.0, 230.0, 160.0]);
    } else if has("water in the rock") || has("aquifer") {
        for (u, v) in [(-0.25f32, -0.1f32), (0.15, -0.3), (0.2, 0.2)] {
            pen.shape([90.0, 130.0, 180.0], Finish::Inked, [u - 0.2, v - 0.35, u + 0.2, v + 0.2], &move |a, b| { let (x, y) = (a - u, b - v); (x * x + y * y < 0.03) || (y < 0.0 && y > -0.32 && x.abs() < 0.17 * (1.0 + y / 0.32)) });
        }
    } else if has("amber") || has("gem") || has("rock crystal") || has("garnet") || has("ruby") || has("emerald") || has("sapphire") {
        let c = super::glyphs::gem_colour(&t);
        super::glyphs::draw(&mut |x, y, col, a| pen.pixel(x, y, col, a), super::glyphs::Glyph::Gem, cx, cy, r * 1.2, Some(c));
    } else if has("ore") || has("seam") || has("adamant") || has("metal") {
        super::glyphs::draw(&mut |x, y, col, a| pen.pixel(x, y, col, a), super::glyphs::Glyph::Ore, cx, cy, r * 1.2, Some(super::glyphs::metal_colour(&t)));
    } else if has("sound from below") || has("something vast") || has("forgotten") {
        pen.ellipse_f(0.0, 0.0, 0.85, 0.85, [36.0, 30.0, 32.0], Finish::Paint);
        pen.ellipse(0.0, 0.0, 0.42, 0.2, [214.0, 170.0, 60.0]);
        pen.ellipse_f(0.0, 0.0, 0.06, 0.18, INK, Finish::Paint);
        pen.glow(0.0, 0.0, 0.6, [230.0, 120.0, 40.0], 0.4);
    } else if has("cave") || has("cavern") || has("break") || has("dug") || has("delve") || has("mine") {
        pen.shape([60.0, 50.0, 46.0], Finish::Inked, [-0.5, -0.5, 0.5, 0.5], &|u, v| v > -0.1 && u.abs() < 0.42 || (v <= -0.1 && u * u + (v + 0.1).powi(2) < 0.42 * 0.42));
        pen.bone(&[(-0.6, 0.6), (0.1, -0.1)], [122.0, 86.0, 54.0], (r * 0.07).max(2.0));
        pen.path(&[(-0.15, -0.3), (0.15, -0.05), (0.38, 0.2)], [176.0, 178.0, 184.0], (r * 0.08).max(2.0));
    } else if has("winter") || has("snow") || has("freezes") || has("frost") {
        for k in 0..6 { let a = k as f32 * 1.047; pen.line((0.0, 0.0), (a.cos() * 0.55, a.sin() * 0.55), [120.0, 150.0, 190.0], (r * 0.05).max(1.5)); pen.line((a.cos() * 0.35, a.sin() * 0.35), (a.cos() * 0.35 + (a + 0.8).cos() * 0.15, a.sin() * 0.35 + (a + 0.8).sin() * 0.15), [120.0, 150.0, 190.0], 1.5); }
    } else if has("wound") || has("tends") || has("heal") {
        pen.rect(-0.12, -0.45, 0.12, 0.45, BLOOD);
        pen.rect(-0.45, -0.12, 0.45, 0.12, BLOOD);
    } else if has("siege") || has("fires") {
        drop(pen);
        ground(put);
        let mut pen = Pen::new(put, cx, cy, 2.0 * r);
        pen.poly(&[(-0.6, 0.42), (-0.2, -0.3), (0.2, 0.42)], [200.0, 188.0, 160.0]);
        pen.poly(&[(0.1, 0.42), (0.4, -0.05), (0.7, 0.42)], mix([150.0, 52.0, 44.0], [220.0, 210.0, 190.0], 0.4));
    } else {
        // The camp's fire.
        pen.ellipse(0.0, 0.25, 0.42, 0.15, [178.0, 172.0, 160.0]);
        pen.poly(&[(-0.25, 0.25), (-0.1, -0.15), (0.0, -0.5), (0.1, -0.15), (0.25, 0.25)], [226.0, 120.0, 46.0]);
        pen.poly_f(&[(-0.1, 0.22), (0.0, -0.2), (0.1, 0.22)], [250.0, 214.0, 120.0], Finish::Paint);
    }
}

/// A house front with the work's sign over its door.
fn house(put: &mut dyn FnMut(i64, i64, Rgb, f32), cx: f32, cy: f32, r: f32, title: &str) {
    use crate::colony::projects::ProjectKind as K;
    let kind = [("storehouse", K::Storehouse), ("tavern", K::Tavern), ("library", K::Library), ("workshop", K::Workshop), ("kitchen", K::Kitchen), ("temple", K::Temple),
        ("guildhall", K::GuildHall), ("hall for the lord", K::LordsHall), ("smokehouse", K::Smokehouse)].iter().find(|(k, _)| title.contains(k)).map(|x| x.1);
    let mut pen = Pen::new(put, cx, cy, 2.0 * r);
    let stoneish = title.contains("well") || title.contains("windbreak") || title.contains("lining");
    if title.contains("palisade") || title.contains("mending") {
        for k in 0..6 { let u = -0.55 + k as f32 * 0.22; pen.poly(&[(u - 0.1, 0.42), (u - 0.1, -0.2), (u, -0.38), (u + 0.1, -0.2), (u + 0.1, 0.42)], mix([164.0, 120.0, 78.0], [122.0, 86.0, 54.0], (k % 3) as f32 / 3.0)); }
        return;
    }
    if title.contains("woodpile") {
        for row in 0..3 { for k in 0..(4 - row) { pen.ellipse(-0.36 + k as f32 * 0.24 + row as f32 * 0.12, 0.3 - row as f32 * 0.2, 0.11, 0.1, [214.0, 182.0, 128.0]); } }
        return;
    }
    if title.contains("field") || title.contains("farm") {
        for k in 0..5 { let v = 0.05 + k as f32 * 0.08; pen.line((-0.6, v), (0.6, v), [150.0, 120.0, 60.0], 2.0); }
        for k in 0..6 { let u = -0.5 + k as f32 * 0.2; pen.line((u, 0.0), (u, -0.25), [120.0, 140.0, 60.0], 1.5); pen.ellipse(u, -0.28, 0.04, 0.07, [206.0, 170.0, 80.0]); }
        return;
    }
    if title.contains("well") {
        pen.rect(-0.35, 0.0, 0.35, 0.42, [178.0, 172.0, 160.0]);
        for u in [-0.3f32, 0.3] { pen.bone(&[(u, 0.0), (u, -0.45)], [122.0, 86.0, 54.0], (r * 0.05).max(1.5)); }
        pen.poly(&[(-0.45, -0.4), (0.0, -0.62), (0.45, -0.4)], [196.0, 168.0, 108.0]);
        return;
    }
    let walls = if stoneish { [178.0, 172.0, 160.0] } else { [176.0, 140.0, 96.0] };
    pen.rect(-0.45, -0.1, 0.45, 0.42, walls);
    for k in 1..4 { let v = -0.1 + k as f32 * 0.13; pen.line_a((-0.45, v), (0.45, v), INK, 1.0, 0.35); }
    pen.poly(&[(-0.58, -0.08), (0.0, -0.58), (0.58, -0.08)], [196.0, 168.0, 108.0]);
    pen.rect(-0.1, 0.12, 0.1, 0.42, [100.0, 70.0, 46.0]);
    if let Some(e) = kind.and_then(super::camp_ink::emblem_pub) {
        pen.rect(0.18, 0.0, 0.42, 0.2, [214.0, 196.0, 156.0]);
        let mut through = |x: i64, y: i64, c: Rgb, a: f32| pen.pixel(x, y, c, a);
        let mut small = Pen::new(&mut through, cx + 0.3 * r, cy + 0.1 * r, r * 0.75);
        e(&mut small, 0.0, 0.0);
    }
    if title.contains("temple") { pen.poly(&[(-0.1, -0.5), (0.0, -0.8), (0.1, -0.5)], [214.0, 176.0, 70.0]); }
}
