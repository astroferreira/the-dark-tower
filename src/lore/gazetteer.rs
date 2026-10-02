//! Named geography: finds the world's features and names them in the language of whoever
//! lives (or lived) there.
//!
//! Features: rivers (main stems traced from their mouths), mountain ranges and their highest
//! peaks, lakes, seas and gulfs, continents and islands, and large forests, deserts, marshes,
//! plains and ice fields. Each feature's name comes from the naming style of the culture that
//! holds most of it (current owner, else former owner); unclaimed features get names in an
//! "ancient" style tied to their landmass, so nameless wilderness still sounds consistent.

use std::collections::{HashMap, VecDeque};

use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::biomes::ExtendedBiome;
use crate::history::naming::generator::NameGenerator;
use crate::history::naming::styles::{NamingArchetype, NamingStyle};
use crate::history::world_state::WorldHistory;
use crate::history::{FactionId, NamingStyleId};
use crate::tilemap::Tilemap;
use crate::world::WorldData;

pub const NONE: u32 = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FeatureKind {
    Ocean,
    Sea,
    Gulf,
    Lake,
    River,
    Continent,
    Island,
    MountainRange,
    Peak,
    Forest,
    Jungle,
    Desert,
    Marsh,
    Plains,
    Tundra,
    IceField,
}

impl FeatureKind {
    /// Rough importance for label ordering (higher shows first / at lower zoom).
    pub fn rank(self) -> u32 {
        use FeatureKind::*;
        match self {
            Ocean => 100,
            Continent => 95,
            Sea => 80,
            MountainRange => 70,
            Desert | IceField => 60,
            Forest | Jungle | Tundra | Plains => 55,
            Gulf => 50,
            River => 45,
            Island => 40,
            Lake => 35,
            Marsh => 30,
            Peak => 25,
        }
    }

    pub fn is_water(self) -> bool {
        matches!(self, FeatureKind::Ocean | FeatureKind::Sea | FeatureKind::Gulf | FeatureKind::Lake | FeatureKind::River)
    }
}

#[derive(Clone, Debug)]
pub struct Feature {
    pub id: u32,
    pub kind: FeatureKind,
    pub name: String,
    /// Faction whose culture named it (None = ancient / unclaimed).
    pub named_by: Option<FactionId>,
    /// Tiles in the feature (river: tiles along the main stem).
    pub size: usize,
    /// Where to put the label (world tile).
    pub anchor: (usize, usize),
    /// Main river course (river only), mouth first.
    pub path: Vec<(usize, usize)>,
    /// Highest elevation (peaks and ranges), metres.
    pub height_m: f32,
}

/// Per-tile feature membership, by layer.
pub struct Gazetteer {
    pub features: Vec<Feature>,
    /// Ocean / sea / gulf / lake / river.
    pub water: Tilemap<u32>,
    /// Mountain range.
    pub relief: Tilemap<u32>,
    /// Forest / desert / marsh / plains / tundra / ice field.
    pub region: Tilemap<u32>,
    /// Continent or island.
    pub landmass: Tilemap<u32>,
}

impl Gazetteer {
    pub fn feature(&self, id: u32) -> Option<&Feature> {
        if id == NONE { None } else { self.features.get(id as usize) }
    }

    /// Human-readable description of where a tile is, most specific first.
    pub fn describe(&self, x: usize, y: usize) -> String {
        let mut parts = Vec::new();
        for layer in [&self.water, &self.relief, &self.region, &self.landmass] {
            if let Some(f) = self.feature(*layer.get(x, y)) {
                parts.push(f.name.clone());
            }
        }
        parts.join(", ")
    }

    pub fn count(&self, kind: FeatureKind) -> usize {
        self.features.iter().filter(|f| f.kind == kind).count()
    }
}

// ---------------------------------------------------------------------------------------------
// Naming
// ---------------------------------------------------------------------------------------------

struct Namer<'a> {
    history: Option<&'a WorldHistory>,
    styles: HashMap<NamingArchetype, NamingStyle>,
    used: std::collections::HashSet<String>,
    seed: u64,
}

impl<'a> Namer<'a> {
    fn new(history: Option<&'a WorldHistory>, seed: u64) -> Self {
        let styles = NamingArchetype::all()
            .iter()
            .map(|&a| (a, NamingStyle::from_archetype(NamingStyleId(0), a)))
            .collect();
        Self { history, styles, used: Default::default(), seed }
    }

    /// The faction that holds most of `tiles` now, else the one that held most in the past.
    fn owner(&self, tiles: &[(usize, usize)]) -> Option<FactionId> {
        let h = self.history?;
        let mut now: HashMap<FactionId, usize> = HashMap::new();
        let mut past: HashMap<FactionId, usize> = HashMap::new();
        let step = (tiles.len() / 400).max(1);
        for &(x, y) in tiles.iter().step_by(step) {
            let t = h.tile_history.get(x, y);
            if let Some(f) = t.current_owner { *now.entry(f).or_insert(0) += 1; }
            if let Some(r) = t.ownership.first() { *past.entry(r.faction).or_insert(0) += 1; }
        }
        // A culture names a feature once it holds a meaningful share of it.
        let need = (tiles.len().div_ceil(step) / 10).max(1);
        let best = |m: &HashMap<FactionId, usize>| m.iter().filter(|(_, &c)| c >= need).max_by_key(|(f, &c)| (c, f.0)).map(|(f, _)| *f);
        best(&now).or_else(|| best(&past))
    }

    fn archetype(&self, faction: Option<FactionId>, fallback_key: u64) -> NamingArchetype {
        if let (Some(h), Some(fid)) = (self.history, faction) {
            if let Some(race) = h.factions.get(&fid).and_then(|f| h.races.get(&f.race_id)) {
                return race.base_type.default_naming_archetype();
            }
        }
        // Unclaimed land: an old tongue chosen per landmass so neighbours sound alike.
        let all = NamingArchetype::all();
        let pick = [NamingArchetype::Ancient, NamingArchetype::Flowing, NamingArchetype::Harsh, NamingArchetype::Mystical];
        let a = pick[(fallback_key % pick.len() as u64) as usize];
        if all.contains(&a) { a } else { all[0] }
    }

    /// A unique proper name in the given style.
    fn proper(&mut self, archetype: NamingArchetype, key: u64, avoid: &[&str]) -> String {
        let style = &self.styles[&archetype];
        let mut rng = ChaCha8Rng::seed_from_u64(self.seed ^ key.wrapping_mul(0x9E37_79B9_7F4A_7C15));
        for _ in 0..40 {
            let n = NameGenerator::place_name(style, &mut rng);
            let len = n.chars().count();
            let lower = n.to_lowercase();
            // Readable (3-10 letters, no triple vowels) and not repeating the feature's own
            // word ("Mount Basaltmount", "Fooforestwood").
            let clunky = lower.as_bytes().windows(3).any(|w| w.iter().all(|c| b"aeiouy".contains(c)));
            let redundant = avoid.iter().any(|a| lower.contains(a));
            if (3..=10).contains(&len) && !clunky && !redundant && self.used.insert(n.clone()) {
                return n;
            }
        }
        let n = format!("{}{}", NameGenerator::personal_name(style, &mut rng), self.used.len());
        self.used.insert(n.clone());
        n
    }

    fn name(&mut self, kind: FeatureKind, tiles: &[(usize, usize)], landmass_key: u64, key: u64) -> (String, Option<FactionId>) {
        let owner = self.owner(tiles);
        let arch = self.archetype(owner, landmass_key);
        let avoid: &[&str] = match kind {
            FeatureKind::Peak | FeatureKind::MountainRange => &["mount", "peak", "spire", "range"],
            FeatureKind::Forest | FeatureKind::Jungle => &["wood", "forest", "grove"],
            FeatureKind::Lake => &["lake", "mere"],
            FeatureKind::Gulf => &["bay", "gulf"],
            FeatureKind::Island => &["isle"],
            FeatureKind::River => &["river"],
            _ => &[],
        };
        let p = self.proper(arch, key, avoid);
        let mut rng = ChaCha8Rng::seed_from_u64(key ^ 0xA5A5);
        use rand::Rng;
        let alt: bool = rng.gen();
        let full = match kind {
            FeatureKind::Ocean => format!("the {p} Ocean"),
            FeatureKind::Sea => format!("the {p} Sea"),
            FeatureKind::Gulf => if alt { format!("the Gulf of {p}") } else { format!("{p} Bay") },
            FeatureKind::Lake => if alt { format!("Lake {p}") } else { format!("{p} Mere") },
            FeatureKind::River => if alt { format!("the {p} River") } else { format!("the River {p}") },
            FeatureKind::Continent => p,
            FeatureKind::Island => if alt { format!("{p} Isle") } else { format!("the Isle of {p}") },
            FeatureKind::MountainRange => if alt { format!("the {p} Mountains") } else { format!("the {p} Range") },
            FeatureKind::Peak => format!("Mount {p}"),
            FeatureKind::Forest => if alt { format!("the {p} Forest") } else { format!("{p}wood") },
            FeatureKind::Jungle => format!("the {p} Jungle"),
            FeatureKind::Desert => if alt { format!("the {p} Desert") } else { format!("the {p} Wastes") },
            FeatureKind::Marsh => if alt { format!("the {p} Marshes") } else { format!("the {p} Fens") },
            FeatureKind::Plains => if alt { format!("the Plains of {p}") } else { format!("the {p} Steppe") },
            FeatureKind::Tundra => format!("the {p} Barrens"),
            FeatureKind::IceField => format!("the {p} Icefield"),
        };
        (full, owner)
    }
}

// ---------------------------------------------------------------------------------------------
// Detection helpers
// ---------------------------------------------------------------------------------------------

fn components(w: usize, h: usize, member: impl Fn(usize, usize) -> bool, diagonal: bool) -> Vec<Vec<(usize, usize)>> {
    let mut seen = vec![false; w * h];
    let mut out = Vec::new();
    for sy in 0..h {
        for sx in 0..w {
            if seen[sy * w + sx] || !member(sx, sy) { continue; }
            let mut comp = Vec::new();
            let mut q = VecDeque::from([(sx, sy)]);
            seen[sy * w + sx] = true;
            while let Some((x, y)) = q.pop_front() {
                comp.push((x, y));
                for dy in -1i64..=1 {
                    for dx in -1i64..=1 {
                        if (dx == 0 && dy == 0) || (!diagonal && dx != 0 && dy != 0) { continue; }
                        let ny = y as i64 + dy;
                        if ny < 0 || ny >= h as i64 { continue; }
                        let nx = (x as i64 + dx).rem_euclid(w as i64) as usize;
                        let ny = ny as usize;
                        if !seen[ny * w + nx] && member(nx, ny) {
                            seen[ny * w + nx] = true;
                            q.push_back((nx, ny));
                        }
                    }
                }
            }
            out.push(comp);
        }
    }
    out
}

/// Label anchor: the member tile closest to the component's (wrap-aware) centroid.
fn anchor(tiles: &[(usize, usize)], w: usize) -> (usize, usize) {
    let (mut sx, mut sy, mut cx, mut cz) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    for &(x, y) in tiles {
        let a = x as f64 / w as f64 * std::f64::consts::TAU;
        cx += a.cos();
        cz += a.sin();
        sy += y as f64;
        sx += 1.0;
    }
    let ax = (cz.atan2(cx).rem_euclid(std::f64::consts::TAU) / std::f64::consts::TAU * w as f64) as usize % w;
    let ay = (sy / sx) as usize;
    *tiles
        .iter()
        .min_by_key(|&&(x, y)| {
            let dx = (x as i64 - ax as i64).abs();
            let dx = dx.min(w as i64 - dx);
            dx * dx + (y as i64 - ay as i64).pow(2)
        })
        .unwrap()
}

fn key_of(kind: FeatureKind, a: (usize, usize)) -> u64 {
    ((kind.rank() as u64) << 48) ^ ((a.0 as u64) << 24) ^ a.1 as u64
}

#[derive(Clone, Copy, PartialEq)]
enum RegionClass { Forest, Jungle, Desert, Marsh, Plains, Tundra, Ice }

fn region_class(b: ExtendedBiome) -> Option<RegionClass> {
    use ExtendedBiome::*;
    Some(match b {
        TemperateForest | TemperateRainforest | BorealForest | MontaneForest | SubalpineForest
        | CloudForest | AncientGrove | DeadForest | PetrifiedForest | MushroomForest
        | BioluminescentForest | CrystalForest => RegionClass::Forest,
        TropicalForest | TropicalRainforest => RegionClass::Jungle,
        Desert | SaltFlats | SingingDunes | GlassDesert | Ashlands | VolcanicWasteland => RegionClass::Desert,
        Swamp | Marsh | Bog | MangroveSaltmarsh | Shadowfen => RegionClass::Marsh,
        TemperateGrassland | Savanna | AlpineMeadow | Paramo => RegionClass::Plains,
        Tundra | AlpineTundra | AuroraWastes => RegionClass::Tundra,
        Ice | SnowyPeaks => RegionClass::Ice,
        _ => return None,
    })
}

// ---------------------------------------------------------------------------------------------
// Build
// ---------------------------------------------------------------------------------------------

/// Minimum sizes (in world tiles, scaled from a 512x256 map).
struct Thresholds { continent: usize, island: usize, range: usize, region: usize, lake: usize, river: usize, gulf: usize, sea: usize }

pub fn build_gazetteer(world: &WorldData, history: Option<&WorldHistory>, seed: u64) -> Gazetteer {
    let (w, h) = (world.width, world.height);
    let area = (w * h) as f32 / (512.0 * 256.0);
    let lin = area.sqrt();
    let th = Thresholds {
        continent: (1500.0 * area) as usize,
        island: 6,
        range: (25.0 * area).max(6.0) as usize,
        region: (60.0 * area).max(12.0) as usize,
        lake: 3,
        river: (14.0 * lin).max(6.0) as usize,
        gulf: (40.0 * area).max(10.0) as usize,
        sea: (6000.0 * area) as usize,
    };
    let hm = &world.heightmap;
    let is_lake = |x: usize, y: usize| world.water_body_map.get(x, y).is_lake();
    // Erosion carves river channels below sea level: one-tile-wide strips of shallow water
    // (not part of any 2x2 all-water block) are channels, i.e. land with a river in it.
    let raw_water: Vec<bool> = (0..w * h).map(|i| *hm.get(i % w, i / w) <= 0.0 || is_lake(i % w, i / w)).collect();
    let wet_at = |x: i64, y: i64| y >= 0 && y < h as i64 && raw_water[y as usize * w + x.rem_euclid(w as i64) as usize];
    let channel: Vec<bool> = (0..w * h)
        .map(|i| {
            let (x, y) = ((i % w) as i64, (i / w) as i64);
            raw_water[i]
                && *hm.get(i % w, i / w) > -300.0
                && ![(0, 0), (-1, 0), (0, -1), (-1, -1)].iter().any(|&(ox, oy)| {
                    wet_at(x + ox, y + oy) && wet_at(x + ox + 1, y + oy) && wet_at(x + ox, y + oy + 1) && wet_at(x + ox + 1, y + oy + 1)
                })
        })
        .collect();
    let land = |x: usize, y: usize| !raw_water[y * w + x] || channel[y * w + x];

    let mut namer = Namer::new(history, seed);
    let mut g = Gazetteer {
        features: Vec::new(),
        water: Tilemap::new_with(w, h, NONE),
        relief: Tilemap::new_with(w, h, NONE),
        region: Tilemap::new_with(w, h, NONE),
        landmass: Tilemap::new_with(w, h, NONE),
    };
    let mut add = |g: &mut Gazetteer, namer: &mut Namer, kind: FeatureKind, tiles: &[(usize, usize)], landmass_key: u64, height_m: f32, path: Vec<(usize, usize)>| -> u32 {
        let a = if path.is_empty() { anchor(tiles, w) } else { path[path.len() / 2] };
        let (name, named_by) = namer.name(kind, tiles, landmass_key, key_of(kind, a));
        let id = g.features.len() as u32;
        g.features.push(Feature { id, kind, name, named_by, size: tiles.len(), anchor: a, path, height_m });
        id
    };

    // Landmasses first: their ids key the fallback language of everything on them.
    let masses = components(w, h, |x, y| land(x, y), true);
    for m in &masses {
        if m.len() < th.island { continue; }
        let kind = if m.len() >= th.continent { FeatureKind::Continent } else { FeatureKind::Island };
        let a = anchor(m, w);
        let id = add(&mut g, &mut namer, kind, m, (a.0 * 31 + a.1) as u64, 0.0, Vec::new());
        for &(x, y) in m { g.landmass.set(x, y, id); }
    }
    let lm_key = |g: &Gazetteer, x: usize, y: usize| -> u64 {
        let id = *g.landmass.get(x, y);
        if id == NONE { (x / 64 * 7 + y / 64) as u64 } else { let a = g.features[id as usize].anchor; (a.0 * 31 + a.1) as u64 }
    };

    // Mountain ranges and their highest peaks.
    let mountain = |x: usize, y: usize| land(x, y) && *hm.get(x, y) > 1800.0;
    for comp in components(w, h, mountain, true) {
        if comp.len() < th.range { continue; }
        let &(px, py) = comp.iter().max_by(|a, b| hm.get(a.0, a.1).partial_cmp(hm.get(b.0, b.1)).unwrap()).unwrap();
        let peak_h = *hm.get(px, py);
        let k = lm_key(&g, px, py);
        let id = add(&mut g, &mut namer, FeatureKind::MountainRange, &comp, k, peak_h, Vec::new());
        for &(x, y) in &comp { g.relief.set(x, y, id); }
        let pid = add(&mut g, &mut namer, FeatureKind::Peak, &[(px, py)], k, peak_h, vec![(px, py)]);
        let _ = pid;
    }

    // Biome regions.
    let classes: Vec<Option<RegionClass>> = (0..w * h).map(|i| if land(i % w, i / w) { region_class(*world.biomes.get(i % w, i / w)) } else { None }).collect();
    for class in [RegionClass::Forest, RegionClass::Jungle, RegionClass::Desert, RegionClass::Marsh, RegionClass::Plains, RegionClass::Tundra, RegionClass::Ice] {
        for comp in components(w, h, |x, y| classes[y * w + x] == Some(class), true) {
            let min = if class == RegionClass::Marsh { th.region / 3 } else { th.region };
            if comp.len() < min { continue; }
            let kind = match class {
                RegionClass::Forest => FeatureKind::Forest,
                RegionClass::Jungle => FeatureKind::Jungle,
                RegionClass::Desert => FeatureKind::Desert,
                RegionClass::Marsh => FeatureKind::Marsh,
                RegionClass::Plains => FeatureKind::Plains,
                RegionClass::Tundra => FeatureKind::Tundra,
                RegionClass::Ice => FeatureKind::IceField,
            };
            let a = anchor(&comp, w);
            let k = lm_key(&g, a.0, a.1);
            let id = add(&mut g, &mut namer, kind, &comp, k, 0.0, Vec::new());
            for &(x, y) in &comp { g.region.set(x, y, id); }
        }
    }

    // Lakes.
    for body in &world.water_bodies {
        if !body.id.is_lake() || body.tile_count < th.lake { continue; }
        let (x0, y0, x1, y1) = body.bounds;
        let mut tiles = Vec::new();
        for y in y0..=y1.min(h - 1) {
            for x in x0..=x1.min(w - 1) {
                if *world.water_body_map.get(x, y) == body.id { tiles.push((x, y)); }
            }
        }
        if tiles.is_empty() { continue; }
        let a = anchor(&tiles, w);
        let k = lm_key(&g, a.0, a.1);
        let id = add(&mut g, &mut namer, FeatureKind::Lake, &tiles, k, 0.0, Vec::new());
        for &(x, y) in &tiles { g.water.set(x, y, id); }
    }

    // Rivers: D8 main stems traced upstream from their mouths, longest first. Tributaries long
    // enough get their own names.
    if let Some(flow) = &world.flow_accumulation {
        let river = |x: usize, y: usize| (channel[y * w + x] || land(x, y)) && *flow.get(x, y) >= 50.0;
        let wet = |x: usize, y: usize| !land(x, y);
        // Downstream of each river tile: steepest descent among neighbours.
        let mut down = vec![NONE as usize; w * h];
        for y in 0..h {
            for x in 0..w {
                if !river(x, y) { continue; }
                let e = *hm.get(x, y);
                let mut best = (0.0f32, None);
                for dy in -1i64..=1 {
                    for dx in -1i64..=1 {
                        if dx == 0 && dy == 0 { continue; }
                        let ny = y as i64 + dy;
                        if ny < 0 || ny >= h as i64 { continue; }
                        let (nx, ny) = ((x as i64 + dx).rem_euclid(w as i64) as usize, ny as usize);
                        let ne = if wet(nx, ny) { -1.0e6 } else { *hm.get(nx, ny) };
                        let d = if dx != 0 && dy != 0 { 1.414 } else { 1.0 };
                        let s = (e - ne) / d;
                        if s > best.0 { best = (s, Some(ny * w + nx)); }
                    }
                }
                if let Some(j) = best.1 { down[y * w + x] = j; }
            }
        }
        let mut ups: Vec<Vec<usize>> = vec![Vec::new(); w * h];
        for i in 0..w * h {
            let j = down[i];
            if j != NONE as usize && river(j % w, j / w) { ups[j].push(i); }
        }
        // Mouths: river tiles draining into water (or nowhere).
        let mut mouths: Vec<usize> = (0..w * h)
            .filter(|&i| river(i % w, i / w) && (down[i] == NONE as usize || !river(down[i] % w, down[i] / w)))
            .collect();
        mouths.sort_by(|&a, &b| flow.get(b % w, b / w).partial_cmp(flow.get(a % w, a / w)).unwrap());
        let mut claimed = vec![false; w * h];
        let mut stack: Vec<usize> = mouths;
        while let Some(start) = stack.pop() {
            if claimed[start] { continue; }
            // Follow the largest tributary upstream; others become branch starts.
            let mut path = vec![start];
            claimed[start] = true;
            let mut cur = start;
            loop {
                let mut nexts: Vec<usize> = ups[cur].iter().copied().filter(|&u| !claimed[u]).collect();
                if nexts.is_empty() { break; }
                nexts.sort_by(|&a, &b| flow.get(b % w, b / w).partial_cmp(flow.get(a % w, a / w)).unwrap());
                for &other in &nexts[1..] { stack.push(other); }
                cur = nexts[0];
                claimed[cur] = true;
                path.push(cur);
            }
            if path.len() < th.river { continue; }
            let tiles: Vec<(usize, usize)> = path.iter().map(|&i| (i % w, i / w)).collect();
            let k = lm_key(&g, tiles[0].0, tiles[0].1);
            let id = add(&mut g, &mut namer, FeatureKind::River, &tiles, k, 0.0, tiles.clone());
            for &(x, y) in &tiles { g.water.set(x, y, id); }
        }
    }

    // Seas: ocean tiles split into enclosed gulfs (most directions hit land nearby) and open
    // water; open water is divided into seas around points far from land.
    let ocean = |x: usize, y: usize| raw_water[y * w + x] && !channel[y * w + x] && !is_lake(x, y);
    let ray = (12.0 * lin).max(6.0) as i64;
    let enclosed: Vec<bool> = (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            if !ocean(x, y) { return false; }
            let mut blocked = 0;
            for k in 0..16 {
                let a = k as f64 / 16.0 * std::f64::consts::TAU;
                let (dx, dy) = (a.cos(), a.sin());
                let hit = (1..=ray).any(|r| {
                    let yy = y as i64 + (dy * r as f64).round() as i64;
                    if yy < 0 || yy >= h as i64 { return false; }
                    let xx = (x as i64 + (dx * r as f64).round() as i64).rem_euclid(w as i64) as usize;
                    !ocean(xx, yy as usize)
                });
                if hit { blocked += 1; }
            }
            blocked >= 11
        })
        .collect();
    for comp in components(w, h, |x, y| enclosed[y * w + x], false) {
        if comp.len() < th.gulf { continue; }
        let a = anchor(&comp, w);
        let k = lm_key(&g, a.0, a.1);
        // Gulfs are named by the coasts around them.
        let coast: Vec<(usize, usize)> = comp.iter().flat_map(|&(x, y)| {
            [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)].into_iter().filter_map(move |(dx, dy)| {
                let ny = y as i64 + dy;
                if ny < 0 || ny >= h as i64 { return None; }
                Some(((x as i64 + dx).rem_euclid(w as i64) as usize, ny as usize))
            })
        }).filter(|&(x, y)| land(x, y)).collect();
        let naming_tiles = if coast.is_empty() { comp.clone() } else { coast };
        let id = {
            let a2 = anchor(&comp, w);
            let (name, named_by) = namer.name(FeatureKind::Gulf, &naming_tiles, k, key_of(FeatureKind::Gulf, a2));
            let id = g.features.len() as u32;
            g.features.push(Feature { id, kind: FeatureKind::Gulf, name, named_by, size: comp.len(), anchor: a2, path: Vec::new(), height_m: 0.0 });
            id
        };
        let _ = a;
        for &(x, y) in &comp { g.water.set(x, y, id); }
    }
    // Open water: multi-source BFS from seeds far from land.
    let mut dist = vec![i32::MAX; w * h];
    let mut q = VecDeque::new();
    for y in 0..h {
        for x in 0..w {
            if !ocean(x, y) { dist[y * w + x] = 0; q.push_back((x, y)); }
        }
    }
    while let Some((x, y)) = q.pop_front() {
        let d = dist[y * w + x];
        for (dx, dy) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
            let ny = y as i64 + dy;
            if ny < 0 || ny >= h as i64 { continue; }
            let (nx, ny) = ((x as i64 + dx).rem_euclid(w as i64) as usize, ny as usize);
            if dist[ny * w + nx] > d + 1 { dist[ny * w + nx] = d + 1; q.push_back((nx, ny)); }
        }
    }
    let open = |x: usize, y: usize| ocean(x, y) && *g.water.get(x, y) == NONE;
    let spacing = (110.0 * lin) as i64;
    let mut seeds: Vec<(usize, usize)> = Vec::new();
    let mut cands: Vec<(usize, usize)> = (0..w * h).map(|i| (i % w, i / w)).filter(|&(x, y)| open(x, y)).collect();
    cands.sort_by_key(|&(x, y)| std::cmp::Reverse(dist[y * w + x]));
    for &(x, y) in &cands {
        if dist[y * w + x] < 3 { break; }
        let far = seeds.iter().all(|&(sx, sy)| {
            let dx = (x as i64 - sx as i64).abs();
            let dx = dx.min(w as i64 - dx);
            dx * dx + (y as i64 - sy as i64).pow(2) >= spacing * spacing
        });
        if far { seeds.push((x, y)); }
    }
    let mut owner_seed = vec![usize::MAX; w * h];
    let mut q: VecDeque<(usize, usize)> = VecDeque::new();
    for (k, &(x, y)) in seeds.iter().enumerate() { owner_seed[y * w + x] = k; q.push_back((x, y)); }
    while let Some((x, y)) = q.pop_front() {
        let s = owner_seed[y * w + x];
        for (dx, dy) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
            let ny = y as i64 + dy;
            if ny < 0 || ny >= h as i64 { continue; }
            let (nx, ny) = ((x as i64 + dx).rem_euclid(w as i64) as usize, ny as usize);
            if open(nx, ny) && owner_seed[ny * w + nx] == usize::MAX { owner_seed[ny * w + nx] = s; q.push_back((nx, ny)); }
        }
    }
    let mut groups: Vec<Vec<(usize, usize)>> = vec![Vec::new(); seeds.len()];
    for i in 0..w * h {
        if owner_seed[i] != usize::MAX { groups[owner_seed[i]].push((i % w, i / w)); }
    }
    for (k, tiles) in groups.into_iter().enumerate() {
        if tiles.len() < th.sea / 20 { continue; }
        let kind = if tiles.len() >= th.sea { FeatureKind::Ocean } else { FeatureKind::Sea };
        // Seas near a culture's coast take its language; open ocean keeps the old tongue.
        let coastal: Vec<(usize, usize)> = tiles.iter().copied().filter(|&(x, y)| dist[y * w + x] <= 3).collect();
        let naming_tiles: Vec<(usize, usize)> = coastal.iter().flat_map(|&(x, y)| {
            [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)].into_iter().filter_map(move |(dx, dy)| {
                let ny = y as i64 + dy;
                if ny < 0 || ny >= h as i64 { return None; }
                Some(((x as i64 + dx).rem_euclid(w as i64) as usize, ny as usize))
            })
        }).filter(|&(x, y)| land(x, y)).collect();
        let a = seeds[k];
        let lk = (a.0 / 97 * 13 + a.1 / 61) as u64;
        let (name, named_by) = if kind == FeatureKind::Sea && !naming_tiles.is_empty() {
            namer.name(kind, &naming_tiles, lk, key_of(kind, a))
        } else {
            namer.name(kind, &[], lk, key_of(kind, a))
        };
        let id = g.features.len() as u32;
        g.features.push(Feature { id, kind, name, named_by, size: tiles.len(), anchor: a, path: Vec::new(), height_m: 0.0 });
        for &(x, y) in &tiles { g.water.set(x, y, id); }
    }
    g
}
