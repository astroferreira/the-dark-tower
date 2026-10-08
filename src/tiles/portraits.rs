//! Ink portraits that carry history: a small head for every settler, built from parts (race,
//! age, hair, beard, dress) and marked by what happened to them: a scar from a named battle or
//! the night of the raid, grey hair after a loss, a patch over a lost eye. Drawn per pixel from
//! shape masks with ink outlines, like the arms, so it reads at 48 px.

use crate::history::entities::races::RaceType;
use crate::history::EventId;

#[derive(Clone, Debug)]
pub struct Portrait {
    pub skin: [f32; 3],
    pub hair: [f32; 3],
    pub dress: [f32; 3],
    /// 0 cropped, 1 long, 2 topknot, 3 bald, 4 braids.
    pub hair_style: u8,
    /// 0 none, 1 short, 2 long, 3 forked.
    pub beard: u8,
    pub ears_pointed: bool,
    pub tusks: bool,
    pub old: bool,
    pub grey: bool,
    /// A scar across the cheek, and the event that left it.
    pub scar: Option<(String, Option<EventId>)>,
    pub eye_patch: bool,
    /// Head shape: 0 oval, 1 square, 2 narrow.
    pub head: u8,
    /// 0 none, 1 hood, 2 cap, 3 circlet, 4 helm.
    pub headgear: u8,
    pub eyes: [f32; 3],
}

const INK: [f32; 3] = [56.0, 42.0, 32.0];

fn hash(s: &str, salt: u64) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325 ^ salt;
    for b in s.bytes() { h ^= b as u64; h = h.wrapping_mul(0x100_0000_01b3); }
    h ^= h >> 29;
    h
}

/// A settler's portrait: their people's race, their age, and the marks of their past.
pub fn of_settler(s: &crate::colony::Settler, h: Option<&crate::history::world_state::WorldHistory>, wounded_in: Option<(String, u64)>) -> Portrait {
    let race = s.past.as_ref().and_then(|p| p.people).and_then(|f| h.and_then(|h| h.factions.get(&f)))
        .and_then(|f| h.and_then(|h| h.races.get(&f.race_id))).map(|r| r.base_type.clone()).unwrap_or(RaceType::Human);
    let age = s.past.as_ref().map_or(30, |p| p.age);
    let k = hash(&s.name, 0);
    let pick = |salt: u64, n: u64| (hash(&s.name, salt) % n) as u8;
    let skins: &[[f32; 3]] = match race {
        RaceType::Orc => &[[120.0, 150.0, 96.0], [104.0, 136.0, 88.0], [136.0, 156.0, 104.0]],
        RaceType::Goblin => &[[150.0, 160.0, 100.0], [130.0, 150.0, 90.0]],
        RaceType::Elf | RaceType::Fey => &[[236.0, 220.0, 200.0], [226.0, 206.0, 184.0], [210.0, 190.0, 170.0]],
        RaceType::Undead => &[[214.0, 210.0, 192.0], [158.0, 170.0, 178.0], [176.0, 186.0, 150.0]],
        RaceType::Reptilian => &[[150.0, 170.0, 120.0], [170.0, 150.0, 100.0]],
        _ => &[[232.0, 196.0, 164.0], [210.0, 170.0, 136.0], [176.0, 130.0, 96.0], [130.0, 92.0, 66.0], [240.0, 210.0, 180.0]],
    };
    let hairs: [[f32; 3]; 6] = [[40.0, 30.0, 24.0], [96.0, 62.0, 36.0], [170.0, 120.0, 60.0], [200.0, 170.0, 100.0], [140.0, 50.0, 30.0], [70.0, 52.0, 40.0]];
    let dresses: [[f32; 3]; 6] = [[90.0, 110.0, 140.0], [150.0, 66.0, 52.0], [92.0, 120.0, 82.0], [170.0, 140.0, 70.0], [116.0, 84.0, 130.0], [120.0, 100.0, 80.0]];
    // Marks from the past: a scar from a named battle (a veteran) or the night of the raid; grey
    // after the loss of a home; an eye lost by those who fought hardest.
    let battle = s.past.as_ref().and_then(|p| p.lines.iter().find(|(t, _)| t.starts_with("Fought at")).map(|(t, e)| (t.trim_end_matches('.').replacen("Fought at", "From", 1), *e)));
    let scar = wounded_in.map(|(w, _)| (w, None)).or(battle);
    let lost_home = s.past.as_ref().map_or(false, |p| p.calling.starts_with("a survivor") || p.calling.starts_with("a refugee"));
    let undead = matches!(race, RaceType::Undead);
    // The persona's colours, so the picture shows what the page says (`persona.rs`).
    let pc = &s.persona;
    let hair_named = crate::persona::colour(&pc.hair);
    let mut hair = hair_named.unwrap_or(if undead { [[200.0, 200.0, 196.0], [120.0, 110.0, 100.0], [60.0, 56.0, 54.0]][(k % 3) as usize] } else { hairs[(k % 6) as usize] });
    let greying = pc.hair_at(age);
    let grey = if hair_named.is_some() { greying != pc.hair } else { age >= 50 } || (lost_home && age >= 35);
    if grey { hair = if greying == "white" { [222.0, 220.0, 212.0] } else { [176.0, 172.0, 166.0] }; }
    let beard = if hair_named.is_some() {
        if pc.beard && age >= 16 { if matches!(race, RaceType::Dwarf) { 2 + pick(3, 2) } else { 1 + pick(3, 3) } } else { 0 }
    } else {
        match race {
            RaceType::Dwarf => 2 + pick(3, 2),
            RaceType::Elf | RaceType::Fey | RaceType::Undead => 0,
            _ => if age >= 18 { pick(3, 4) } else { 0 },
        }
    };
    let skin = crate::persona::colour(&pc.skin).unwrap_or(skins[(k >> 8) as usize % skins.len()]);
    let hair_style = match pc.hairstyle.as_str() {
        "" => pick(1, 5),
        h if h.contains("braid") => 4,
        h if h.contains("topknot") => 2,
        h if h.contains("shaved") || h == "none" => 3,
        "cropped" => 0,
        h if h.contains("loose") || h.contains("long") || h.contains("woven") || h.contains("wild") => 1,
        _ => if pc.looks.iter().any(|l| l.0 == "hair_length" && l.1 >= 4) { 1 } else { 0 },
    };
    // Eyes in their colour once the face is big enough to tell (the ink still outlines them).
    let eyes = crate::persona::colour(&pc.eyes).map(|c| mix(c, INK, 0.35)).unwrap_or(INK);
    Portrait {
        skin, hair, dress: dresses[(k >> 16) as usize % 6],
        hair_style,
        beard,
        ears_pointed: matches!(race, RaceType::Elf | RaceType::Fey | RaceType::Goblin),
        tusks: matches!(race, RaceType::Orc),
        old: age >= 55, grey,
        eye_patch: scar.is_some() && pick(7, 3) == 0,
        scar,
        // The face's shape follows the persona: a square or jutting chin, a slight build.
        head: {
            let gap = |n: &str| pc.looks.iter().find(|l| l.0 == n).map(|l| l.1);
            match (gap("chin"), gap("build")) {
                (Some(c), _) if c >= 4 => 1,
                (_, Some(b)) if b <= 1 => 2,
                (Some(c), _) if c == 0 => 2,
                _ => pick(5, 3),
            }
        },
        headgear: match pick(9, 7) { 0 | 1 | 2 => 0, 3 => 1, 4 => 2, 5 => 3, _ => 4 },
        eyes: if undead { [120.0, 200.0, 220.0] } else { eyes },
    }
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] { [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t] }
fn pack(c: [f32; 3]) -> u32 { ((c[0].clamp(0.0, 255.0) as u32) << 16) | ((c[1].clamp(0.0, 255.0) as u32) << 8) | c[2].clamp(0.0, 255.0) as u32 }

/// Draw `p` in a `size` x `size` square with its top-left at (x0, y0).
pub fn draw(buf: &mut [u32], w: usize, h: usize, x0: i64, y0: i64, size: usize, p: &Portrait) {
    let n = size as f32;
    // Signed shape tests in unit coordinates (u right, v down, head centred at 0.5, 0.45).
    let head = |u: f32, v: f32| -> f32 {
        let (rx, ry) = match p.head { 1 => (0.27, 0.31), 2 => (0.22, 0.33), _ => (0.25, 0.32) };
        let (dx, dy) = ((u - 0.5) / rx, (v - 0.45) / ry);
        let sq = if p.head == 1 { dx.abs().powf(3.0) + dy.abs().powf(3.0) } else { dx * dx + dy * dy };
        sq - 1.0
    };
    for py in 0..size as i64 {
        for px in 0..size as i64 {
            let (sx, sy) = (x0 + px, y0 + py);
            if sx < 0 || sy < 0 || sx >= w as i64 || sy >= h as i64 { continue; }
            let (u, v) = ((px as f32 + 0.5) / n, (py as f32 + 0.5) / n);
            let mut c: Option<[f32; 3]> = None;
            // Shoulders and dress.
            let shoulders = v > 0.78 && ((u - 0.5) / 0.42).powi(2) + ((v - 1.05) / 0.3).powi(2) < 1.0;
            if shoulders { c = Some(p.dress); if v < 0.84 || ((u - 0.5) / 0.42).powi(2) + ((v - 1.05) / 0.3).powi(2) > 0.85 { c = Some(mix(p.dress, INK, 0.6)); } }
            // Neck.
            if (u - 0.5).abs() < 0.08 && v > 0.7 && v < 0.82 { c = Some(mix(p.skin, INK, 0.12)); }
            // Ears.
            let ear_l = ((u - 0.25) / 0.05).powi(2) + ((v - 0.47) / 0.07).powi(2) < 1.0;
            let ear_r = ((u - 0.75) / 0.05).powi(2) + ((v - 0.47) / 0.07).powi(2) < 1.0;
            let point = p.ears_pointed && ((u < 0.25 && u > 0.14 && (v - 0.42).abs() < (u - 0.14) * 0.9) || (u > 0.75 && u < 0.86 && (v - 0.42).abs() < (0.86 - u) * 0.9));
            if ear_l || ear_r || point { c = Some(p.skin); }
            let hd = head(u, v);
            if hd < 0.0 {
                let mut col = p.skin;
                if p.old && ((v - 0.36).abs() < 0.006 || (v - 0.39).abs() < 0.006) && (u - 0.5).abs() < 0.12 { col = mix(col, INK, 0.4); }
                // Eyes (one patched), brows, nose, mouth.
                for (ex, left) in [(0.41f32, true), (0.59, false)] {
                    let eye = ((u - ex) / 0.035).powi(2) + ((v - 0.44) / 0.025).powi(2) < 1.0;
                    if p.eye_patch && left && ((u - ex) / 0.07).powi(2) + ((v - 0.44) / 0.05).powi(2) < 1.0 { col = INK; }
                    else if eye { col = p.eyes; }
                    if (v - 0.395).abs() < 0.012 && (u - ex).abs() < 0.06 { col = mix(p.hair, INK, 0.5); }
                }
                if p.eye_patch && (v - (0.38 + (u - 0.3) * 0.25)).abs() < 0.008 && u > 0.28 && u < 0.72 { col = INK; }
                if (u - 0.5).abs() < 0.008 && v > 0.45 && v < 0.55 { col = mix(col, INK, 0.6); }
                if (v - 0.62).abs() < 0.008 && (u - 0.5).abs() < 0.06 { col = mix(col, INK, 0.75); }
                if p.tusks && ((u - 0.44).abs() < 0.012 || (u - 0.56).abs() < 0.012) && v > 0.58 && v < 0.63 { col = [236.0, 228.0, 206.0]; }
                // The scar: a pale line across the right cheek with stitch marks.
                if p.scar.is_some() {
                    let t = (u - 0.56) / 0.14;
                    if (0.0..=1.0).contains(&t) && (v - (0.48 + t * 0.12)).abs() < 0.009 { col = [200.0, 120.0, 110.0]; }
                }
                // Beard.
                let beard = match p.beard {
                    1 => v > 0.58 && hd < -0.0 && ((u - 0.5) / 0.22).powi(2) + ((v - 0.6) / 0.2).powi(2) < 1.0 && v > 0.56,
                    2 | 3 => v > 0.55 && ((u - 0.5) / 0.24).powi(2) + ((v - 0.66) / 0.26).powi(2) < 1.0,
                    _ => false,
                };
                if beard { col = mix(p.hair, INK, 0.1); }
                c = Some(col);
            }
            // Long beards fall below the chin; a forked one parts.
            if p.beard >= 2 && hd >= 0.0 && v > 0.7 && v < 0.92 && (u - 0.5).abs() < 0.14 - (v - 0.7) * 0.4 && !(p.beard == 3 && (u - 0.5).abs() < 0.02 && v > 0.82) {
                c = Some(mix(p.hair, INK, 0.1));
            }
            // Hair.
            let crown = ((u - 0.5) / 0.27).powi(2) + ((v - 0.36) / 0.25).powi(2) < 1.0 && v < 0.37;
            let hair = match p.hair_style {
                0 => crown,
                1 => crown || ((u < 0.27 || u > 0.73) && v > 0.3 && v < 0.72 && (u - 0.5).abs() < 0.3),
                2 => crown || (((u - 0.5) / 0.08).powi(2) + ((v - 0.1) / 0.07).powi(2) < 1.0),
                4 => crown || (((u - 0.24).abs() < 0.04 || (u - 0.76).abs() < 0.04) && v > 0.35 && v < 0.8),
                _ => false,
            };
            if hair { c = Some(if (px + py) % 3 == 0 { mix(p.hair, INK, 0.25) } else { p.hair }); }
            // Headgear over the hair.
            let dark = mix(p.dress, INK, 0.35);
            match p.headgear {
                1 => {
                    // A hood: a cowl round the head down to the shoulders, the face open.
                    let outer = ((u - 0.5) / 0.33).powi(2) + ((v - 0.44) / 0.38).powi(2) < 1.0 && v < 0.8;
                    if outer && hd >= -0.05 { c = Some(if hd > 0.0 && hd < 0.08 { INK } else { dark }); }
                }
                2 => {
                    let cap = ((u - 0.5) / 0.28).powi(2) + ((v - 0.3) / 0.16).powi(2) < 1.0 && v < 0.3;
                    let band = (v - 0.3).abs() < 0.025 && (u - 0.5).abs() < 0.28;
                    if cap { c = Some(dark); }
                    if band { c = Some(mix(dark, INK, 0.5)); }
                }
                3 => { if (v - 0.27).abs() < 0.018 && (u - 0.5).abs() < 0.24 && hd < 0.1 { c = Some([200.0, 160.0, 70.0]); } }
                4 => {
                    let dome = ((u - 0.5) / 0.28).powi(2) + ((v - 0.33) / 0.2).powi(2) < 1.0 && v < 0.36;
                    let nose = (u - 0.5).abs() < 0.02 && v >= 0.33 && v < 0.5;
                    if dome || nose { c = Some(if (px * 3 + py) % 5 == 0 { [120.0, 120.0, 116.0] } else { [150.0, 150.0, 146.0] }); }
                }
                _ => {}
            }
            if let Some(col) = c {
                // Ink the outline: a pixel whose neighbour is outside every shape.
                let inside = |du: f32, dv: f32| head(u + du, v + dv) < 0.0;
                let d = 1.0 / n;
                let edge = hd < 0.0 && (!inside(d, 0.0) || !inside(-d, 0.0) || !inside(0.0, d) || !inside(0.0, -d));
                buf[sy as usize * w + sx as usize] = pack(if edge { INK } else { col });
            }
        }
    }
}
