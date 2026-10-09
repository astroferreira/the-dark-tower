//! Arms for every realm, drawn in ink from what the realm is.
//!
//! A realm used to be a coloured square. Its arms now come from the people and the place: the
//! race sets the shield's shape, a hash of the realm its field, division and tinctures (a metal
//! charge on a colour, the old rule), the seat's geography the charge (a wavy bar for a river, a
//! mount for high ground, a tree for forest, waves for a coast, a sun for desert, else a star),
//! and the Shadow's realm bears a red eye on sable. Drawn per pixel from shape masks with an ink
//! outline, so they read at 24 px and at 96.

use crate::history::entities::races::RaceType;
use crate::history::world_state::WorldHistory;
use crate::history::FactionId;
use crate::world::WorldData;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape { Heater, Square, Kite, Round, Banner }
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Division { Plain, PerPale, PerFess, PerBend, Chevron, Quarterly }
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Charge { River, Mount, Tree, Waves, Sun, Star, Eye }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Arms {
    pub shape: Shape,
    pub field: u32,
    pub second: u32,
    pub division: Division,
    pub charge: Charge,
    pub metal: u32,
}

const INK: u32 = 0x0030_2418;
/// Colours (muted, inked) and metals.
const COLOURS: [u32; 5] = [0x009A_2A1E, 0x003E_5C80, 0x004E_6E3A, 0x006A_4A78, 0x0038_2E28];
const METALS: [u32; 2] = [0x00C8_A24A, 0x00EC_E4D0];

fn hash(x: u64) -> u64 {
    let mut h = x.wrapping_mul(0x9E37_79B9_7F4A_7C15);
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 29)
}

/// The arms of a realm.
pub fn arms_of(world: &WorldData, h: &WorldHistory, f: FactionId) -> Arms {
    let fac = h.factions.get(&f);
    let race = fac.and_then(|x| h.races.get(&x.race_id)).map(|r| r.base_type.clone());
    let k = hash(f.0 as u64 ^ world.seed());
    let shape = match race {
        Some(RaceType::Dwarf | RaceType::Construct | RaceType::Giant) => Shape::Square,
        Some(RaceType::Elf | RaceType::Fey) => Shape::Kite,
        Some(RaceType::Orc | RaceType::Goblin | RaceType::Beastfolk | RaceType::Halfling) => Shape::Round,
        Some(RaceType::Undead | RaceType::Elemental) => Shape::Banner,
        _ => Shape::Heater,
    };
    let shadow = h.shadow.as_ref().map_or(false, |s| s.faction == f && !s.is_broken());
    if shadow {
        return Arms { shape, field: 0x0022_1C1A, second: 0x0022_1C1A, division: Division::Plain, charge: Charge::Eye, metal: 0x00B0_2A1A };
    }
    let field = COLOURS[(k % COLOURS.len() as u64) as usize];
    let second = METALS[((k >> 8) % 2) as usize];
    let metal = METALS[((k >> 9) % 2) as usize];
    let division = match (k >> 16) % 6 { 0 => Division::Plain, 1 => Division::PerPale, 2 => Division::PerFess, 3 => Division::PerBend, 4 => Division::Chevron, _ => Division::Quarterly };
    // The charge: what lies at the seat.
    let seat = fac.and_then(|x| x.capital).and_then(|c| h.settlements.get(&c)).map(|t| t.location);
    let charge = match seat {
        Some((x, y)) => {
            let (w, ht) = (world.width, world.height);
            let near = |dx: i64, dy: i64| (((x as i64 + dx).rem_euclid(w as i64)) as usize, (y as i64 + dy).clamp(0, ht as i64 - 1) as usize);
            let around: Vec<(usize, usize)> = (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (dx, dy))).map(|(dx, dy)| near(dx, dy)).collect();
            let biome = format!("{:?}", world.biomes.get(x, y));
            if around.iter().any(|&(i, j)| *world.heightmap.get(i, j) < 0.0) { Charge::Waves }
            else if world.river_network.as_ref().map_or(false, |rn| around.iter().any(|&(i, j)| rn.has_significant_flow(i, j))) { Charge::River }
            else if *world.heightmap.get(x, y) > 1000.0 { Charge::Mount }
            else if biome.contains("Forest") || biome.contains("Jungle") || biome.contains("Taiga") { Charge::Tree }
            else if biome.contains("Desert") || biome.contains("Savanna") { Charge::Sun }
            else { Charge::Star }
        }
        None => Charge::Star,
    };
    Arms { shape, field, second, division, charge, metal }
}

/// Whether (u, v) in [0,1]^2 (v down) is inside the shield.
fn inside(shape: Shape, u: f32, v: f32) -> bool {
    let (x, y) = (u - 0.5, v);
    match shape {
        Shape::Heater => if y < 0.55 { x.abs() <= 0.45 } else { let t = (y - 0.55) / 0.45; x.abs() <= 0.45 * (1.0 - t * t).max(0.0).sqrt() },
        Shape::Square => x.abs() <= 0.45 && y <= 0.92 && !(y > 0.8 && x.abs() < 0.08),
        Shape::Kite => { let t = y; x.abs() <= 0.42 * if t < 0.3 { (t / 0.3).sqrt() } else { 1.0 - (t - 0.3) / 0.7 } },
        Shape::Round => (x * x + (y - 0.5) * (y - 0.5)) <= 0.2,
        Shape::Banner => x.abs() <= 0.4 && (y <= 0.75 || (y - 0.75) < 0.25 * (x.abs() / 0.4)),
    }
}

fn division_second(d: Division, u: f32, v: f32) -> bool {
    match d {
        Division::Plain => false,
        Division::PerPale => u > 0.5,
        Division::PerFess => v > 0.5,
        Division::PerBend => v > u,
        Division::Chevron => v > 0.75 - (u - 0.5).abs() * 0.9 && v < 0.95 - (u - 0.5).abs() * 0.9,
        Division::Quarterly => (u > 0.5) != (v > 0.5),
    }
}

fn charge_mask(c: Charge, u: f32, v: f32) -> bool {
    let (x, y) = (u - 0.5, v - 0.45);
    match c {
        Charge::River => { let wave = 0.05 * (u * 18.0).sin(); (y - wave).abs() < 0.07 }
        Charge::Mount => y > -0.05 && y < 0.25 && x.abs() < (y + 0.05) * 1.2,
        Charge::Tree => (x * x + (y + 0.05) * (y + 0.05) < 0.03) || (x.abs() < 0.035 && y > 0.0 && y < 0.25),
        Charge::Waves => (0..3).any(|k| { let yy = y + 0.12 - k as f32 * 0.12; (yy - 0.035 * (u * 20.0).sin()).abs() < 0.03 }),
        Charge::Sun => { let r = (x * x + y * y).sqrt(); let a = y.atan2(x); r < 0.1 || (r < 0.2 && (a * 8.0).cos() > 0.6) }
        Charge::Star => { let r = (x * x + y * y).sqrt(); let a = y.atan2(x) + std::f32::consts::FRAC_PI_2; r < 0.08 + 0.1 * ((a * 5.0 / 2.0).cos().abs().powf(6.0)) }
        Charge::Eye => (x / 0.24).powi(2) + (y / 0.11).powi(2) < 1.0,
    }
}

/// Draw `arms` with its top-left at (x0, y0), `size` px tall (width ~ 0.85 size).
pub fn draw(buf: &mut [u32], w: usize, h: usize, x0: i64, y0: i64, size: usize, arms: &Arms) {
    let sw = (size as f32 * 0.85) as i64;
    let sz = size as i64;
    let at = |px: i64, py: i64| -> Option<(f32, f32)> { if px < 0 || py < 0 || px >= sw || py >= sz { None } else { Some(((px as f32 + 0.5) / sw as f32, (py as f32 + 0.5) / sz as f32)) } };
    let ins = |px: i64, py: i64| at(px, py).map_or(false, |(u, v)| inside(arms.shape, u, v));
    for py in 0..sz {
        for px in 0..sw {
            let (sx, sy) = (x0 + px, y0 + py);
            if sx < 0 || sy < 0 || sx >= w as i64 || sy >= h as i64 { continue; }
            if !ins(px, py) { continue; }
            let (u, v) = at(px, py).unwrap();
            let edge = !ins(px - 1, py) || !ins(px + 1, py) || !ins(px, py - 1) || !ins(px, py + 1);
            let mut c = if division_second(arms.division, u, v) { arms.second } else { arms.field };
            if charge_mask(arms.charge, u, v) {
                c = arms.metal;
                if arms.charge == Charge::Eye {
                    let (x, y) = (u - 0.5, v - 0.45);
                    if (x * x + y * y).sqrt() < 0.05 { c = INK; }
                }
                // Ink the charge's edge.
                let d = 1.0 / size as f32;
                if [(d, 0.0), (-d, 0.0), (0.0, d), (0.0, -d)].iter().any(|(du, dv)| !charge_mask(arms.charge, u + du * 1.2, v + dv * 1.2)) { c = INK; }
            }
            buf[sy as usize * w + sx as usize] = if edge { INK } else { c };
        }
    }
}

/// A sheet of every living realm's arms at 24 and 96 px with their names (`--arms-sheet`).
pub fn save_sheet(world: &WorldData, h: &WorldHistory, path: &str) -> Result<usize, Box<dyn std::error::Error>> {
    let mut realms: Vec<_> = h.factions.values().filter(|f| f.is_active()).collect();
    realms.sort_by_key(|f| f.id);
    let cols = 4usize;
    let (cw, ch) = (300usize, 130usize);
    let rows = (realms.len() + cols - 1) / cols;
    let (w, ht) = (cols * cw, rows.max(1) * ch);
    let mut buf = vec![0x00EA_DEC4u32; w * ht];
    for (k, f) in realms.iter().enumerate() {
        let (x, y) = ((k % cols) * cw + 10, (k / cols) * ch + 10);
        let a = arms_of(world, h, f.id);
        draw(&mut buf, w, ht, x as i64, y as i64, 96, &a);
        draw(&mut buf, w, ht, (x + 96) as i64, (y + 72) as i64, 24, &a);
        super::text::draw_fell(&mut buf, w, ht, (x + 126) as i64, (y + 10) as i64, &super::ui::truncate(&super::ui::ascii(&f.name), 22), INK, 1, true);
        super::text::draw_fell(&mut buf, w, ht, (x + 126) as i64, (y + 24) as i64, &format!("{:?}, {:?}", a.charge, a.shape), 0x0080_6A52, 1, false);
    }
    image::RgbImage::from_fn(w as u32, ht as u32, |x, y| { let p = buf[y as usize * w + x as usize]; image::Rgb([(p >> 16) as u8, (p >> 8) as u8, p as u8]) }).save(path)?;
    Ok(realms.len())
}
