//! Ecology: forests, farmland and wild animal populations on the world grid, stepped once a
//! year alongside history.
//!
//! Every land tile carries a forest cover, a farmland fraction and a density (0..1, relative to
//! the best habitat) for each species in `SPECIES`. Each year:
//! - settlements press on the land around them (`pressure`): fields spread over fertile ground
//!   near growing towns, forests are logged and cleared for those fields, and wild game is
//!   hunted; abandoned fields go back to grass and forest regrows where it once stood;
//! - herbivores grow logistically toward a carrying capacity set by biome, forest cover,
//!   farmland and how much they tolerate people; predators are limited by the prey they can
//!   catch and in turn thin it out;
//! - animals spread into neighbouring tiles with room for them, which is how herds migrate and
//!   how wolves find their way back to a ruin once its people are gone.
//! Notable changes around settlements are written into the chronicle.
//!
//! The step uses no randomness, so adding it leaves the rest of the history unchanged.

use crate::history::det::HashMap;

use serde::{Deserialize, Serialize};

use crate::biomes::ExtendedBiome;
use crate::history::events::types::{Event, EventType};
use crate::history::world_state::WorldHistory;
use crate::history::{EntityId, EventId};
use crate::history::creatures::anatomy::{BodyPartSpecial, CreatureSize, MagicAbility};
use crate::map_export::{get_biome_family, BiomeFamily};
use crate::world::WorldData;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trophic {
    Grazer,
    Predator,
    /// Eats plants and prey: supported by habitat, helped (not limited) by prey.
    Omnivore,
}

/// What a species needs from a tile.
pub struct Habitat {
    pub biome: ExtendedBiome,
    pub family: BiomeFamily,
    pub temp_c: f32,
    pub elevation_m: f32,
    pub forest: f32,
    pub farmland: f32,
}

pub struct Species {
    pub name: &'static str,
    pub plural: &'static str,
    pub trophic: Trophic,
    /// Yearly intrinsic growth rate.
    pub growth: f32,
    /// Fraction of the population that moves to neighbouring tiles each year.
    pub spread: f32,
    /// 0 = vanishes wherever people are, 1 = unbothered by them.
    pub tolerance: f32,
    /// Indices into `SPECIES` of what a predator or omnivore eats.
    pub prey: &'static [usize],
    pub habitat: fn(&Habitat) -> f32,
}

const DEER: usize = 0;
const CARIBOU: usize = 1;
const BOAR: usize = 2;
const AUROCHS: usize = 3;
const ANTELOPE: usize = 4;
const IBEX: usize = 5;
pub const WOLF: usize = 6;
pub const BEAR: usize = 7;
pub const LION: usize = 8;

pub const SPECIES: [Species; 9] = [
    Species {
        name: "red deer", plural: "red deer", trophic: Trophic::Grazer, growth: 0.35, spread: 0.10, tolerance: 0.35, prey: &[],
        habitat: |h| {
            let base = match h.family {
                BiomeFamily::TemperateWet => 1.0,
                BiomeFamily::Boreal => 0.6,
                BiomeFamily::TemperateDry => 0.45,
                BiomeFamily::Mountain | BiomeFamily::Wetland => 0.3,
                BiomeFamily::Tropical => 0.15,
                _ => 0.0,
            };
            // Woodland edge animals: some cover, some open ground.
            base * (0.45 + 0.55 * (4.0 * h.forest * (1.0 - h.forest)).max(h.forest * 0.6)) * (1.0 - 0.5 * h.farmland)
        },
    },
    Species {
        name: "caribou", plural: "caribou", trophic: Trophic::Grazer, growth: 0.25, spread: 0.35, tolerance: 0.2, prey: &[],
        habitat: |h| {
            let base = match h.family {
                BiomeFamily::Polar if h.temp_c > -18.0 => 1.0,
                BiomeFamily::Boreal => 0.7,
                BiomeFamily::Mountain if h.temp_c < 0.0 => 0.4,
                _ => 0.0,
            };
            base * (1.0 - 0.8 * h.farmland)
        },
    },
    Species {
        name: "wild boar", plural: "wild boar", trophic: Trophic::Grazer, growth: 0.45, spread: 0.08, tolerance: 0.55, prey: &[],
        habitat: |h| {
            let base = match h.family {
                BiomeFamily::TemperateWet | BiomeFamily::Tropical => 0.85,
                BiomeFamily::Wetland => 0.5,
                BiomeFamily::Boreal | BiomeFamily::TemperateDry => 0.3,
                _ => 0.0,
            };
            // Forest animals that raid crops.
            base * (0.3 + 0.7 * h.forest) * (1.0 + 0.3 * h.farmland)
        },
    },
    Species {
        name: "aurochs", plural: "aurochs", trophic: Trophic::Grazer, growth: 0.2, spread: 0.12, tolerance: 0.1, prey: &[],
        habitat: |h| {
            let base = match h.family {
                BiomeFamily::TemperateDry => 1.0,
                BiomeFamily::TemperateWet => 0.5,
                BiomeFamily::Wetland => 0.4,
                BiomeFamily::Boreal => 0.2,
                _ => 0.0,
            };
            base * (1.0 - 0.7 * h.forest) * (1.0 - 0.9 * h.farmland)
        },
    },
    Species {
        name: "antelope", plural: "antelope", trophic: Trophic::Grazer, growth: 0.35, spread: 0.25, tolerance: 0.3, prey: &[],
        habitat: |h| {
            let base = if h.biome == ExtendedBiome::Savanna {
                1.0
            } else {
                match h.family {
                    BiomeFamily::Arid => 0.35,
                    BiomeFamily::TemperateDry if h.temp_c > 12.0 => 0.5,
                    BiomeFamily::Tropical => 0.2,
                    _ => 0.0,
                }
            };
            base * (1.0 - 0.6 * h.forest) * (1.0 - 0.7 * h.farmland)
        },
    },
    Species {
        name: "ibex", plural: "ibex", trophic: Trophic::Grazer, growth: 0.2, spread: 0.06, tolerance: 0.4, prey: &[],
        habitat: |h| {
            let high = ((h.elevation_m - 1200.0) / 1300.0).clamp(0.0, 1.0);
            let base = if h.family == BiomeFamily::Mountain { 0.6 + 0.4 * high } else { high * 0.8 };
            if h.family == BiomeFamily::Polar && h.temp_c < -15.0 { 0.0 } else { base * (1.0 - 0.5 * h.forest) }
        },
    },
    Species {
        name: "wolf", plural: "wolves", trophic: Trophic::Predator, growth: 0.3, spread: 0.25, tolerance: 0.05,
        prey: &[DEER, CARIBOU, BOAR, AUROCHS, IBEX],
        habitat: |h| match h.family {
            BiomeFamily::Boreal => 1.0,
            BiomeFamily::TemperateWet | BiomeFamily::TemperateDry => 0.8,
            BiomeFamily::Polar if h.temp_c > -20.0 => 0.6,
            BiomeFamily::Mountain => 0.6,
            BiomeFamily::Wetland => 0.4,
            _ => 0.0,
        },
    },
    Species {
        name: "bear", plural: "bears", trophic: Trophic::Omnivore, growth: 0.12, spread: 0.1, tolerance: 0.0,
        prey: &[DEER, BOAR, CARIBOU],
        habitat: |h| {
            let base = match h.family {
                BiomeFamily::Boreal => 1.0,
                BiomeFamily::TemperateWet => 0.8,
                BiomeFamily::Mountain => 0.5,
                BiomeFamily::Polar if h.temp_c > -12.0 => 0.3,
                _ => 0.0,
            };
            base * (0.2 + 0.8 * h.forest)
        },
    },
    Species {
        name: "lion", plural: "lions", trophic: Trophic::Predator, growth: 0.25, spread: 0.15, tolerance: 0.05,
        prey: &[ANTELOPE, AUROCHS, BOAR],
        habitat: |h| {
            if h.biome == ExtendedBiome::Savanna {
                1.0
            } else {
                match h.family {
                    BiomeFamily::Tropical => 0.35,
                    BiomeFamily::Arid => 0.3,
                    BiomeFamily::TemperateDry if h.temp_c > 14.0 => 0.4,
                    _ => 0.0,
                }
            }
        },
    },
];

/// Chronicle notes already written per settlement (bit flags), so each is told once.
const NOTE_FOREST_CLEARED: u8 = 1;
const NOTE_GAME_SCARCE: u8 = 2;
const NOTE_WILD_RETURNED: u8 = 4;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Ecology {
    pub width: usize,
    pub height: usize,
    /// Tree cover 0..1 now, and what the climate would grow without people.
    pub forest: Vec<f32>,
    pub forest_potential: Vec<f32>,
    /// Fraction of the tile under fields.
    pub farmland: Vec<f32>,
    /// Density 0..1 per species (index into `SPECIES`) per tile.
    pub fauna: Vec<Vec<f32>>,
    /// Totals at the start, for reporting change.
    pub initial_totals: Vec<f32>,
    pub initial_forest: f32,
    pub notes: HashMap<u64, u8>,
    /// Landscapes history has scarred: bone fields, ash, titan bones, crystal, overgrown ruins.
    pub scars: Vec<Scar>,
    /// Chronicle events already scanned for scar causes.
    pub scanned_events: usize,
    /// Battles fought per site, and the lair event of each legendary creature.
    pub battles_at: HashMap<(usize, usize), u32>,
    pub lair_events: HashMap<u64, EventId>,
    /// Human pressure 0..1 from the last step (derived; rebuilt each step).
    #[serde(skip)]
    pub pressure: Vec<f32>,
    /// Biome per tile where a scar overrides the world's (derived from `scars`).
    #[serde(skip)]
    overrides: HashMap<usize, ExtendedBiome>,
    /// Years each tile has lain in the Shadow's blight (not saved: a loaded history is finished).
    #[serde(skip)]
    blighted_years: Vec<u16>,
}

/// What left a scar on the land (one scar per source).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScarSource {
    Battlefield(usize, usize),
    Lair(u64),
    Carcass(u64),
    FallenTower(u64),
    Overgrown(u64),
    /// Land the Shadow held in blight for a generation (appended: older saves still decode).
    Blight(usize, usize),
}

/// A patch of land turned into an anomalous biome by something that happened there.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Scar {
    pub source: ScarSource,
    pub x: usize,
    pub y: usize,
    pub radius: f32,
    pub biome: ExtendedBiome,
    /// The chronicle entry describing the change (itself linked to its cause).
    pub event: EventId,
}

fn forest_potential(biome: ExtendedBiome) -> f32 {
    use ExtendedBiome::*;
    match biome {
        BorealForest | SubalpineForest | TemperateForest | MontaneForest | TemperateRainforest
        | CloudForest | TropicalForest | TropicalRainforest | AncientGrove => 0.9,
        MushroomForest | BioluminescentForest | CrystalForest | DeadForest | PetrifiedForest => 0.0,
        Savanna => 0.15,
        MonsoonForest => 0.75,
        MediterraneanShrubland => 0.3,
        Foothills => 0.3,
        Swamp | Marsh | Bog | MangroveSaltmarsh => 0.3,
        _ => match get_biome_family(biome).0 {
            BiomeFamily::Boreal => 0.75,
            BiomeFamily::TemperateWet => 0.65,
            BiomeFamily::Tropical => 0.8,
            BiomeFamily::TemperateDry => 0.08,
            _ => 0.0,
        },
    }
}

impl Ecology {
    pub fn new(world: &WorldData) -> Self {
        let (w, h) = (world.width, world.height);
        let n = w * h;
        let mut forest_pot = vec![0.0; n];
        for i in 0..n {
            let (x, y) = (i % w, i / w);
            if *world.heightmap.get(x, y) > 0.0 {
                forest_pot[i] = forest_potential(*world.biomes.get(x, y));
            }
        }
        let mut eco = Self {
            width: w,
            height: h,
            forest: forest_pot.clone(),
            forest_potential: forest_pot,
            farmland: vec![0.0; n],
            fauna: vec![vec![0.0; n]; SPECIES.len()],
            initial_totals: Vec::new(),
            initial_forest: 0.0,
            notes: HashMap::default(),
            scars: Vec::new(),
            scanned_events: 0,
            battles_at: HashMap::default(),
            lair_events: HashMap::default(),
            pressure: vec![0.0; n],
            overrides: HashMap::default(),
            blighted_years: Vec::new(),
        };
        // Start every species at 80% of its undisturbed carrying capacity.
        let prey_k = eco.capacities(world);
        for (s, k) in prey_k.iter().enumerate() {
            eco.fauna[s] = k.iter().map(|v| v * 0.8).collect();
        }
        eco.initial_totals = eco.fauna.iter().map(|f| f.iter().sum()).collect();
        eco.initial_forest = eco.forest.iter().sum();
        eco
    }

    fn habitat(&self, world: &WorldData, i: usize) -> Option<Habitat> {
        let (x, y) = (i % self.width, i / self.width);
        let e = *world.heightmap.get(x, y);
        if e <= 0.0 { return None; }
        let biome = self.overrides.get(&i).copied().unwrap_or(*world.biomes.get(x, y));
        Some(Habitat {
            biome,
            family: get_biome_family(biome).0,
            temp_c: *world.temperature.get(x, y),
            elevation_m: e,
            forest: self.forest[i],
            farmland: self.farmland[i],
        })
    }

    /// Carrying capacity per species per tile under the current land cover and pressure.
    /// Predators' capacities here are habitat only; prey limits them in `step`.
    fn capacities(&self, world: &WorldData) -> Vec<Vec<f32>> {
        let n = self.width * self.height;
        let mut k = vec![vec![0.0f32; n]; SPECIES.len()];
        for i in 0..n {
            let Some(h) = self.habitat(world, i) else { continue };
            let p = self.pressure.get(i).copied().unwrap_or(0.0);
            for (s, sp) in SPECIES.iter().enumerate() {
                k[s][i] = ((sp.habitat)(&h) * (1.0 - p * (1.0 - sp.tolerance))).clamp(0.0, 1.0);
            }
        }
        k
    }

    /// Undisturbed habitat quality (no people) for one species at one tile.
    pub fn wild_capacity(&self, world: &WorldData, s: usize, i: usize) -> f32 {
        self.habitat(world, i).map(|h| (SPECIES[s].habitat)(&Habitat { forest: self.forest_potential[i], farmland: 0.0, ..h })).unwrap_or(0.0)
    }

    fn idx(&self, x: i64, y: i64) -> Option<usize> {
        if y < 0 || y >= self.height as i64 { return None; }
        Some(y as usize * self.width + x.rem_euclid(self.width as i64) as usize)
    }

    /// Tiles within `r` of (x, y) with their distance.
    fn disc(&self, x: usize, y: usize, r: f32) -> Vec<(usize, f32)> {
        let ri = r.ceil() as i64;
        let mut out = Vec::new();
        for dy in -ri..=ri {
            for dx in -ri..=ri {
                let d = ((dx * dx + dy * dy) as f32).sqrt();
                if d > r { continue; }
                if let Some(i) = self.idx(x as i64 + dx, y as i64 + dy) { out.push((i, d)); }
            }
        }
        out
    }

    /// Sum of a field over a disc.
    pub fn sum_near(&self, field: &[f32], x: usize, y: usize, r: f32) -> f32 {
        self.disc(x, y, r).iter().map(|&(i, _)| field[i]).sum()
    }

    /// The most abundant species at a tile (density above `min`), most abundant first.
    pub fn wildlife_at(&self, x: usize, y: usize, min: f32) -> Vec<(&'static str, f32)> {
        let i = y * self.width + x;
        let mut v: Vec<(&'static str, f32)> = SPECIES.iter().enumerate()
            .map(|(s, sp)| (sp.plural, self.fauna[s][i]))
            .filter(|&(_, d)| d >= min)
            .collect();
        v.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap());
        v
    }

    /// One-paragraph report: land cover change and each species against its starting numbers.
    pub fn summary(&self) -> String {
        let forest: f32 = self.forest.iter().sum();
        let farms = self.farmland.iter().filter(|&&f| f > 0.35).count();
        let mut s = format!(
            "Ecology: forest cover {:.0}% of original, {} tiles farmed\n ",
            100.0 * forest / self.initial_forest.max(1e-6),
            farms
        );
        for (k, sp) in SPECIES.iter().enumerate() {
            let now: f32 = self.fauna[k].iter().sum();
            let pct = 100.0 * now / self.initial_totals.get(k).copied().unwrap_or(1.0).max(1e-6);
            s.push_str(&format!(" {} {:.0}%", sp.plural, pct));
        }
        let mut kinds: Vec<(String, usize)> = Vec::new();
        for scar in &self.scars {
            let k = format!("{:?}", scar.biome);
            match kinds.iter_mut().find(|e| e.0 == k) { Some(e) => e.1 += 1, None => kinds.push((k, 1)) }
        }
        if !kinds.is_empty() {
            s.push_str("\n  scars:");
            for (k, n) in kinds { s.push_str(&format!(" {} {}", n, k)); }
        }
        s
    }
}

/// Advance the ecology by one year and write notable changes into the chronicle.
pub fn step_year(history: &mut WorldHistory, world: &WorldData) {
    let Some(mut eco) = history.ecology.take() else { return };
    let (w, h) = (eco.width, eco.height);
    let n = w * h;
    let res = world.resources();

    // --- Human pressure and wanted farmland around living settlements ----------------------
    let mut pressure = vec![0.0f32; n];
    let mut farm_target = vec![0.0f32; n];
    for s in history.settlements.values().filter(|s| !s.is_destroyed()) {
        let (x, y) = s.location;
        if x >= w || y >= h { continue; }
        let pop = s.population as f32;
        // Fields: the settlement's own tile for a town, a couple of tiles around a big city.
        let fields_r = 0.3 + (pop / 4000.0).sqrt();
        let reach = fields_r + 2.5;
        let weight = (pop / 2000.0).min(1.0);
        for (i, d) in eco.disc(x, y, reach) {
            pressure[i] += weight * (-(d / (fields_r + 1.5)).powi(2)).exp();
            if d <= fields_r + 0.5 {
                let fert = *res.fertility.get(i % w, i / w);
                let suit = ((fert - 0.1) / 0.5).clamp(0.0, 1.0);
                let edge = (fields_r + 0.5 - d).clamp(0.0, 1.0);
                farm_target[i] = farm_target[i].max(suit * edge * 0.9);
            }
        }
    }
    for p in pressure.iter_mut() { *p = p.min(1.0); }

    // --- Land cover --------------------------------------------------------------------------
    for i in 0..n {
        let target = farm_target[i];
        let f = &mut eco.farmland[i];
        if target > *f { *f += 0.15 * (target - *f); } else { *f -= 0.05 * (*f - target); }
        let cap = eco.forest_potential[i] * (1.0 - eco.farmland[i]);
        let tree = &mut eco.forest[i];
        *tree -= 0.03 * pressure[i] * *tree; // logging
        if *tree > cap {
            *tree = cap; // cleared for fields
        } else {
            *tree += 0.03 * (cap - *tree) * (1.0 - pressure[i]); // regrowth
        }
    }
    eco.pressure = pressure;

    // --- Animals -----------------------------------------------------------------------------
    let k = eco.capacities(world);
    let mut next = eco.fauna.clone();
    // Prey available to each predator (diet-weighted), and the predation load on each prey.
    let mut load = vec![vec![0.0f32; n]; SPECIES.len()];
    for (s, sp) in SPECIES.iter().enumerate() {
        if sp.trophic == Trophic::Grazer { continue; }
        for i in 0..n {
            let p = eco.fauna[s][i];
            if p <= 0.0 { continue; }
            for &q in sp.prey { load[q][i] += p; }
        }
    }
    for (s, sp) in SPECIES.iter().enumerate() {
        for i in 0..n {
            let cur = eco.fauna[s][i];
            let cap = match sp.trophic {
                Trophic::Grazer => k[s][i],
                Trophic::Predator | Trophic::Omnivore => {
                    let food: f32 = sp.prey.iter().map(|&q| eco.fauna[q][i]).sum::<f32>();
                    let fed = (food / 0.6).min(1.0);
                    k[s][i] * if sp.trophic == Trophic::Predator { fed } else { 0.6 + 0.4 * fed }
                }
            };
            let mut v = if cap < 1e-3 {
                cur * 0.6
            } else {
                cur + sp.growth * cur * (1.0 - cur / cap)
            };
            if sp.trophic == Trophic::Grazer {
                v -= 0.15 * load[s][i] * cur; // predation
                v -= 0.4 * eco.pressure[i] * (1.0 - sp.tolerance) * cur; // hunting
            }
            next[s][i] = v.clamp(0.0, 1.0);
        }
    }
    // Spread into neighbouring tiles with free room (migration and recolonisation).
    for (s, sp) in SPECIES.iter().enumerate() {
        let src = next[s].clone();
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                let out = src[i] * sp.spread;
                if out < 1e-5 { continue; }
                let mut nb = [(0usize, 0.0f32); 4];
                let mut total = 0.0;
                for (m, (dx, dy)) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)].into_iter().enumerate() {
                    if let Some(j) = eco.idx(x as i64 + dx, y as i64 + dy) {
                        let room = (k[s][j] - src[j]).max(0.0);
                        nb[m] = (j, room);
                        total += room;
                    }
                }
                if total <= 0.0 { continue; }
                let moved = out.min(total);
                next[s][i] -= moved;
                for (j, room) in nb {
                    if room > 0.0 { next[s][j] += moved * room / total; }
                }
            }
        }
    }
    eco.fauna = next;

    record_notes(history, world, &mut eco);
    update_scars(history, world, &mut eco);
    history.ecology = Some(eco);
}

/// Chronicle the visible consequences around settlements, once each.
fn record_notes(history: &mut WorldHistory, world: &WorldData, eco: &mut Ecology) {
    let date = history.current_date;
    let mut events = Vec::new();
    for s in history.settlements.values() {
        let (x, y) = s.location;
        if x >= eco.width || y >= eco.height { continue; }
        let flags = eco.notes.get(&s.id.0).copied().unwrap_or(0);
        if !s.is_destroyed() {
            if flags & NOTE_FOREST_CLEARED == 0 {
                let pot = eco.sum_near(&eco.forest_potential, x, y, 3.0);
                let now = eco.sum_near(&eco.forest, x, y, 3.0);
                if pot > 4.0 && now < 0.4 * pot {
                    events.push((s.id, s.faction, (x, y), NOTE_FOREST_CLEARED, EventType::ForestCleared,
                        format!("The woods around {} are felled", s.name),
                        format!("Generations of logging and new fields have cleared the forests around {}.", s.name)));
                }
            }
            if flags & NOTE_GAME_SCARCE == 0 {
                let disc = eco.disc(x, y, 3.0);
                let (mut wild, mut now) = (0.0, 0.0);
                for (sp, spec) in SPECIES.iter().enumerate() {
                    if spec.trophic != Trophic::Grazer { continue; }
                    for &(i, _) in &disc {
                        wild += eco.wild_capacity(world, sp, i);
                        now += eco.fauna[sp][i];
                    }
                }
                if wild > 3.0 && now < 0.3 * wild {
                    events.push((s.id, s.faction, (x, y), NOTE_GAME_SCARCE, EventType::GameScarce,
                        format!("Game grows scarce near {}", s.name),
                        format!("The hunters of {} must range ever further; the herds that once grazed nearby are gone.", s.name)));
                }
            }
        } else if flags & NOTE_WILD_RETURNED == 0 {
            let abandoned_years = s.destroyed.map(|d| date.year.saturating_sub(d.year)).unwrap_or(0);
            if abandoned_years >= 5 {
                let i = y * eco.width + x;
                for sp in [WOLF, BEAR, LION] {
                    let wild = eco.wild_capacity(world, sp, i);
                    if wild > 0.3 && eco.fauna[sp][i] > 0.5 * wild {
                        let name = SPECIES[sp].plural;
                        let title = format!("{} return to the ruins of {}", capitalize(name), s.name);
                        events.push((s.id, s.faction, (x, y), NOTE_WILD_RETURNED, EventType::WildlifeReturned,
                            title,
                            format!("{} years after it fell, {} den among the ruins of {}.", abandoned_years, name, s.name)));
                        break;
                    }
                }
            }
        }
    }
    for (sid, faction, (x, y), note, kind, title, desc) in events {
        *eco.notes.entry(sid.0).or_insert(0) |= note;
        let id = history.id_generators.next_event();
        let ev = Event::new(id, kind, date, title, desc)
            .at_location(x, y)
            .with_faction(faction)
            .with_participant(EntityId::Settlement(sid));
        history.chronicle.record(ev);
        history.tile_history.record_event(x, y, id);
    }
}

/// Biomes that only make sense as the result of something happening (magic, monsters, war).
/// World generation rolls some of them at random; with a history they are removed and instead
/// grow out of events (`update_scars`).
pub fn is_caused_biome(b: ExtendedBiome) -> bool {
    use ExtendedBiome::*;
    matches!(
        b,
        CrystalForest | BioluminescentForest | MushroomForest | AcidLake | BioluminescentWater
            | CrystalWasteland | TitanBones | CoralPlateau | FloatingStones | Shadowfen | PrismaticPools
            | AuroraWastes | SingingDunes | GlassDesert | EtherealMist | StarfallCrater | LeyNexus
            | WhisperingStones | SpiritMarsh | RazorPeaks | ColossalHive | BoneFields | CarnivorousBog
            | FungalBloom | MirrorLake | VoidScar | SiliconGrove | SporeWastes | BleedingStone
            | HollowEarth | CyclopeanRuins | BuriedTemple | OvergrownCitadel | DarkTower | DeadForest
    )
}

impl Ecology {
    /// Replace randomly placed anomaly biomes with the most common natural biome around them
    /// (searching outward), so that anomalies only appear where history puts them.
    pub fn naturalize(world: &mut WorldData) -> usize {
        let (w, h) = (world.width, world.height);
        let old = world.biomes.clone();
        let mut changed = 0;
        for y in 0..h {
            for x in 0..w {
                let b = *old.get(x, y);
                if !is_caused_biome(b) { continue; }
                let land = *world.heightmap.get(x, y) > 0.0;
                let mut pick = None;
                'search: for r in 1..=12i64 {
                    let mut counts: Vec<(ExtendedBiome, u32)> = Vec::new();
                    for dy in -r..=r {
                        for dx in -r..=r {
                            if dx.abs() != r && dy.abs() != r { continue; }
                            let ny = y as i64 + dy;
                            if ny < 0 || ny >= h as i64 { continue; }
                            let nx = (x as i64 + dx).rem_euclid(w as i64) as usize;
                            let nb = *old.get(nx, ny as usize);
                            if is_caused_biome(nb) || (*world.heightmap.get(nx, ny as usize) > 0.0) != land { continue; }
                            match counts.iter_mut().find(|c| c.0 == nb) {
                                Some(c) => c.1 += 1,
                                None => counts.push((nb, 1)),
                            }
                        }
                    }
                    if let Some(&(nb, _)) = counts.iter().max_by_key(|c| c.1) {
                        pick = Some(nb);
                        break 'search;
                    }
                }
                let natural = pick.unwrap_or(if land { ExtendedBiome::TemperateGrassland } else { ExtendedBiome::Ocean });
                *world.biomes.get_mut(x, y) = natural;
                changed += 1;
            }
        }
        changed
    }

    fn rebuild_overrides(&mut self, world: &WorldData) {
        self.overrides.clear();
        for scar in &self.scars {
            for (i, d) in self.disc(scar.x, scar.y, scar.radius) {
                let (x, y) = (i % self.width, i / self.width);
                if *world.heightmap.get(x, y) <= 0.0 { continue; }
                // Ragged edge: outer tiles are kept only on some hashes.
                let edge = d / scar.radius.max(0.5);
                let hsh = ((x as u64).wrapping_mul(0x9E37_79B9) ^ (y as u64).wrapping_mul(0x85EB_CA6B)) % 100;
                if edge > 0.7 && hsh as f32 > 100.0 * (1.0 - edge) * 2.5 { continue; }
                self.overrides.insert(i, scar.biome);
            }
        }
        for (&i, &b) in &self.overrides {
            if !matches!(b, ExtendedBiome::OvergrownCitadel) { self.forest_potential[i] = 0.0; }
        }
    }

    /// Write the scars into the world's biome map (after history, before drawing).
    pub fn apply_scars(&mut self, world: &mut WorldData) {
        self.rebuild_overrides(world);
        for (&i, &b) in &self.overrides {
            *world.biomes.get_mut(i % self.width, i / self.width) = b;
        }
    }

    /// The scar covering a tile, if any.
    pub fn scar_at(&self, x: usize, y: usize) -> Option<&Scar> {
        let i = y * self.width + x;
        let b = self.overrides.get(&i)?;
        self.scars.iter()
            .filter(|s| s.biome == *b)
            .min_by(|a, c| {
                let d = |s: &Scar| (s.x as f32 - x as f32).powi(2) + (s.y as f32 - y as f32).powi(2);
                d(a).partial_cmp(&d(c)).unwrap()
            })
    }
}

/// Grow anomalies out of what happened: battles leave bone fields, slain giants leave titan
/// bones, dragons scorch their lairs to ash, fallen towers leave crystal wastes and old capitals
/// are swallowed by the forest. Each is chronicled once, linked to its cause.
fn update_scars(history: &mut WorldHistory, world: &WorldData, eco: &mut Ecology) {
    use crate::history::objects::monuments::MonumentType;
    use crate::history::civilizations::settlement::SettlementType;
    let date = history.current_date;
    let mut new_scars: Vec<(Scar, String, String, Option<EventId>, Vec<EntityId>)> = Vec::new();
    let has_scar = |eco: &Ecology, src: ScarSource| eco.scars.iter().any(|s| s.source == src);

    // New chronicle entries since last year.
    let start = eco.scanned_events.min(history.chronicle.events.len());
    for ev in &history.chronicle.events[start..] {
        match ev.event_type {
            EventType::LairEstablished => {
                for p in &ev.primary_participants {
                    if let EntityId::LegendaryCreature(c) = p { eco.lair_events.insert(c.0, ev.id); }
                }
            }
            EventType::BattleFought => {
                let Some(site) = ev.location else { continue };
                let n = eco.battles_at.entry(site).or_insert(0);
                *n += 1;
                let n = *n;
                let src = ScarSource::Battlefield(site.0, site.1);
                if let Some(s) = eco.scars.iter_mut().find(|s| s.source == src) {
                    s.radius = (0.8 + 0.12 * n as f32).min(2.4);
                } else if n >= 6 && !new_scars.iter().any(|s| s.0.source == src) {
                    let place = history.settlements.values().find(|s| s.location == site).map(|s| s.name.clone()).unwrap_or_else(|| "the border".into());
                    new_scars.push((
                        Scar { source: src, x: site.0, y: site.1, radius: 1.2, biome: ExtendedBiome::BoneFields, event: EventId(0) },
                        format!("The bone fields of {}", place),
                        format!("So many battles have been fought before {} that the fields are white with the bones of the fallen.", place),
                        Some(ev.id),
                        Vec::new(),
                    ));
                }
            }
            EventType::CreatureSlain => {
                for p in &ev.primary_participants {
                    let EntityId::LegendaryCreature(cid) = p else { continue };
                    let Some(c) = history.legendary_creatures.get(cid) else { continue };
                    let Some((x, y)) = c.lair_location else { continue };
                    let size = history.creature_species.get(&c.species_id).map(|s| s.size);
                    let giant = matches!(size, Some(CreatureSize::Gargantuan | CreatureSize::Colossal))
                        || (matches!(size, Some(CreatureSize::Huge)) && c.size_multiplier > 2.4);
                    if !giant || has_scar(eco, ScarSource::Carcass(cid.0)) { continue; }
                    new_scars.push((
                        Scar { source: ScarSource::Carcass(cid.0), x, y, radius: 0.8 + 0.3 * c.size_multiplier, biome: ExtendedBiome::TitanBones, event: EventId(0) },
                        format!("The bones of {}", c.full_name()),
                        format!("The vast skeleton of {} still lies where it fell, its ribs taller than towers.", c.full_name()),
                        Some(ev.id),
                        vec![EntityId::LegendaryCreature(*cid)],
                    ));
                }
            }
            EventType::MonumentDestroyed => {
                let Some(m) = history.monuments.values().find(|m| m.destruction_event == Some(ev.id)) else { continue };
                if m.monument_type != MonumentType::Tower || has_scar(eco, ScarSource::FallenTower(m.id.0)) { continue; }
                let (x, y) = m.location;
                new_scars.push((
                    Scar { source: ScarSource::FallenTower(m.id.0), x, y, radius: 1.4, biome: ExtendedBiome::CrystalWasteland, event: EventId(0) },
                    format!("The shards of {}", m.name),
                    format!("When {} fell, the power bound in its stones burst out; crystal has grown over the land around it ever since.", m.name),
                    Some(ev.id),
                    vec![EntityId::Monument(m.id)],
                ));
            }
            _ => {}
        }
    }
    eco.scanned_events = history.chronicle.events.len();

    // Great beasts that have held a lair for two generations mark the land with their power:
    // elemental fury burns it to ash, necromancy kills the woods, spellcraft crystallises them.
    for c in history.legendary_creatures.values() {
        if !c.is_alive() { continue; }
        let Some((x, y)) = c.lair_location else { continue };
        let Some(sp) = history.creature_species.get(&c.species_id) else { continue };
        let huge = matches!(sp.size, CreatureSize::Huge | CreatureSize::Gargantuan | CreatureSize::Colossal) && c.size_multiplier > 2.0;
        let fire = sp.body_parts.iter().any(|b| b.specials.contains(&BodyPartSpecial::FireBreathing));
        let power = if fire || c.unique_abilities.contains(&MagicAbility::ElementalControl) {
            Some((ExtendedBiome::Ashlands, "ashlands", "has burned the country around its lair; nothing grows there now but ash"))
        } else if c.unique_abilities.contains(&MagicAbility::Necromancy) {
            Some((ExtendedBiome::DeadForest, "dead woods", "has drained the life from the woods around its lair; the trees stand grey and dead"))
        } else if c.unique_abilities.contains(&MagicAbility::Spellcasting) {
            Some((ExtendedBiome::CrystalForest, "crystal wood", "has soaked the land around its lair in sorcery; the trees have turned to crystal"))
        } else {
            None
        };
        let (Some((biome, what, did)), true) = (power, huge) else { continue };
        let held = c.birth_date.map(|b| date.year.saturating_sub(b.year)).unwrap_or(0);
        let src = ScarSource::Lair(c.id.0);
        if let Some(s) = eco.scars.iter_mut().find(|s| s.source == src) {
            s.radius = (1.0 + held as f32 / 80.0).min(2.6);
        } else if held >= 40 {
            new_scars.push((
                Scar { source: src, x, y, radius: 1.5, biome, event: EventId(0) },
                format!("The {} of {}", what, c.full_name()),
                format!("For {} years {} {}.", held, c.full_name(), did),
                eco.lair_events.get(&c.id.0).copied(),
                vec![EntityId::LegendaryCreature(c.id)],
            ));
        }
    }

    // Towers left standing in a razed town fall within a generation, and the power bound in
    // their stones grows crystal over the land.
    for m in history.monuments.values() {
        if m.monument_type != MonumentType::Tower || has_scar(eco, ScarSource::FallenTower(m.id.0)) { continue; }
        let Some(town) = history.settlements.values().find(|s| s.location == m.location && s.destroyed.is_some()) else { continue };
        let fell = town.destroyed.map(|d| d.year).unwrap_or(date.year);
        if date.year.saturating_sub(fell) < 20 { continue; }
        let (x, y) = m.location;
        let cause = history.chronicle.events.iter().rev()
            .find(|e| e.event_type == EventType::SettlementDestroyed && e.primary_participants.contains(&EntityId::Settlement(town.id)))
            .map(|e| e.id);
        new_scars.push((
            Scar { source: ScarSource::FallenTower(m.id.0), x, y, radius: 1.4, biome: ExtendedBiome::CrystalWasteland, event: EventId(0) },
            format!("The fall of {}", m.name),
            format!("{} stood alone over the ruins of {} until it fell; crystal has grown over the land around it ever since.", m.name, town.name),
            cause,
            vec![EntityId::Monument(m.id), EntityId::Settlement(town.id)],
        ));
    }

    // Towns and greater places a century and more in ruins are swallowed by the wild.
    for s in history.settlements.values() {
        let Some(fell) = s.destroyed else { continue };
        if matches!(s.settlement_type, SettlementType::Village) { continue; }
        if date.year.saturating_sub(fell.year) < 100 || has_scar(eco, ScarSource::Overgrown(s.id.0)) { continue; }
        let (x, y) = s.location;
        let i = y * eco.width + x;
        let forested = eco.forest_potential.get(i).copied().unwrap_or(0.0) > 0.3;
        let biome = if forested { ExtendedBiome::OvergrownCitadel } else { ExtendedBiome::CyclopeanRuins };
        let cause = history.chronicle.events.iter().rev()
            .find(|e| e.event_type == EventType::SettlementDestroyed && e.primary_participants.contains(&EntityId::Settlement(s.id)))
            .map(|e| e.id);
        new_scars.push((
            Scar { source: ScarSource::Overgrown(s.id.0), x, y, radius: 0.6, biome, event: EventId(0) },
            if forested { format!("The forest takes {}", s.name) } else { format!("The ruins of {} crumble", s.name) },
            if forested {
                format!("A century after its fall, trees grow through the halls of {}.", s.name)
            } else {
                format!("A century after its fall, only great tumbled stones remain of {}.", s.name)
            },
            cause,
            vec![EntityId::Settlement(s.id)],
        ));
    }

    blight_scars(history, world, eco, &mut new_scars);

    if new_scars.is_empty() {
        if !eco.scars.is_empty() { eco.rebuild_overrides(world); }
        return;
    }
    for (mut scar, title, desc, cause, who) in new_scars {
        let id = history.id_generators.next_event();
        let mut ev = Event::new(id, EventType::LandScarred, date, title, desc).at_location(scar.x, scar.y);
        if let Some(c) = cause { ev = ev.caused_by(c); }
        for p in who { ev = ev.with_participant(p); }
        history.chronicle.record(ev);
        history.tile_history.record_event(scar.x, scar.y, id);
        scar.event = id;
        eco.scars.push(scar);
    }
    eco.scanned_events = history.chronicle.events.len();
    eco.rebuild_overrides(world);
}

/// Years of blight after which the Shadow's land dies for good.
const BLIGHT_YEARS: u16 = 30;

/// Land the Shadow has held in blight (`shadow::BLIGHT`) for `BLIGHT_YEARS` dies: forest becomes
/// dead woods, the rest ashlands. A few patches a year at most, each caused by the Shadow's
/// latest deed; they stay after the Shadow is broken.
fn blight_scars(
    history: &WorldHistory,
    world: &WorldData,
    eco: &mut Ecology,
    new_scars: &mut Vec<(Scar, String, String, Option<EventId>, Vec<EntityId>)>,
) {
    let Some(shadow) = history.shadow.as_ref() else { return };
    let (w, h) = (eco.width, eco.height);
    if shadow.width != w || shadow.height != h { return; }
    if eco.blighted_years.len() != w * h { eco.blighted_years = vec![0; w * h]; }
    let mut ripe: Vec<usize> = Vec::new();
    for i in 0..w * h {
        if shadow.corruption[i] >= crate::history::shadow::BLIGHT && *world.heightmap.get(i % w, i / w) > 0.0 {
            eco.blighted_years[i] = eco.blighted_years[i].saturating_add(1);
            if eco.blighted_years[i] >= BLIGHT_YEARS && !eco.overrides.contains_key(&i) { ripe.push(i); }
        } else {
            eco.blighted_years[i] = 0;
        }
    }
    const RADIUS: f32 = 2.2;
    let mut made = 0;
    for i in ripe {
        if made >= 3 { break; }
        let (x, y) = (i % w, i / w);
        let near = |sx: usize, sy: usize| {
            let (dx, dy) = (sx as f32 - x as f32, sy as f32 - y as f32);
            dx * dx + dy * dy < (RADIUS * 1.6) * (RADIUS * 1.6)
        };
        if eco.scars.iter().chain(new_scars.iter().map(|s| &s.0)).any(|s| matches!(s.source, ScarSource::Blight(..)) && near(s.x, s.y)) {
            continue;
        }
        let wooded = eco.forest_potential[i] >= 0.5;
        let biome = if wooded { ExtendedBiome::DeadForest } else { ExtendedBiome::Ashlands };
        let place = nearest_place_name(history, x, y);
        let (title, desc) = if wooded {
            (
                format!("The woods die near {}", place),
                format!("After {} years under {}, the woods near {} stand grey and leafless; nothing grows back.", BLIGHT_YEARS, shadow.name, place),
            )
        } else {
            (
                format!("The land near {} turns to ash", place),
                format!("After {} years under {}, the land near {} is ash and cinders, and no grass returns.", BLIGHT_YEARS, shadow.name, place),
            )
        };
        new_scars.push((
            Scar { source: ScarSource::Blight(x, y), x, y, radius: RADIUS, biome, event: EventId(0) },
            title,
            desc,
            Some(shadow.last_deed),
            vec![EntityId::Faction(shadow.faction)],
        ));
        made += 1;
    }
}

fn nearest_place_name(history: &WorldHistory, x: usize, y: usize) -> String {
    history
        .settlements
        .values()
        .min_by_key(|s| {
            let (dx, dy) = (s.location.0 as i64 - x as i64, s.location.1 as i64 - y as i64);
            (dx * dx + dy * dy, s.id.0)
        })
        .map(|s| s.name.clone())
        .unwrap_or_else(|| "the Shadow's seat".into())
}

fn capitalize(s: &str) -> String {
    let mut c = s.chars();
    match c.next() {
        Some(f) => f.to_uppercase().collect::<String>() + c.as_str(),
        None => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::history::civilizations::settlement::{Settlement, SettlementType};
    use crate::history::config::HistoryConfig;
    use crate::history::time::Date;
    use crate::history::{FactionId, SettlementId};
    use crate::plates::PlateId;
    use crate::scale::MapScale;
    use crate::seeds::WorldSeeds;
    use crate::tilemap::Tilemap;
    use crate::water_bodies::WaterBodyId;
    use crate::seasons::Season;

    /// A wet temperate forest, 64x32, with an ocean row at the top.
    fn forest_world() -> WorldData {
        let (w, h) = (64, 32);
        let mut heightmap = Tilemap::new_with(w, h, 300.0);
        let mut biomes = Tilemap::new_with(w, h, ExtendedBiome::TemperateForest);
        for x in 0..w {
            *biomes.get_mut(x, 0) = ExtendedBiome::Ocean;
            *heightmap.get_mut(x, 0) = -100.0;
        }
        WorldData::new(
            WorldSeeds::from_master(7), MapScale::new(1.0), heightmap,
            Tilemap::new_with(w, h, 12.0), Tilemap::new_with(w, h, 0.6),
            biomes, Tilemap::new_with(w, h, 0.0), Tilemap::new_with(w, h, PlateId(0)), Vec::new(),
            None, Tilemap::new_with(w, h, WaterBodyId::NONE), Vec::new(), Tilemap::new_with(w, h, 0.0),
            None, None,
        )
    }

    fn near_far(eco: &Ecology, field: &[f32]) -> (f32, f32) {
        (eco.sum_near(field, 20, 16, 2.0) / 13.0, eco.sum_near(field, 52, 16, 2.0) / 13.0)
    }

    #[test]
    fn towns_clear_land_and_wildlife_returns_to_ruins() {
        let world = forest_world();
        let mut history = WorldHistory::new(HistoryConfig::default(), 64, 32, Date::new(1, Season::Spring));
        history.ecology = Some(Ecology::new(&world));
        let sid = SettlementId(1);
        let mut town = Settlement::new(sid, "Testford".into(), SettlementType::City, (20, 16), FactionId(0), Date::new(1, Season::Spring), Vec::new());
        town.population = 2500;
        history.settlements.insert(sid, town);

        for year in 1..=80 {
            history.current_date = Date::new(year, Season::Spring);
            step_year(&mut history, &world);
        }
        let eco = history.ecology.as_ref().unwrap();
        let (forest_near, forest_far) = near_far(eco, &eco.forest);
        let (deer_near, deer_far) = near_far(eco, &eco.fauna[DEER]);
        let (wolf_near, wolf_far) = near_far(eco, &eco.fauna[WOLF]);
        assert!(forest_near < 0.4 * forest_far, "forest near {forest_near} far {forest_far}");
        assert!(deer_near < 0.5 * deer_far, "deer near {deer_near} far {deer_far}");
        assert!(wolf_far > 0.2 && wolf_near < 0.3 * wolf_far, "wolves near {wolf_near} far {wolf_far}");
        assert!(eco.farmland[16 * 64 + 20] > 0.4 || forest_near < 0.2, "the town should farm or fell its surroundings");
        let told = |k: EventType| history.chronicle.events.iter().any(|e| e.event_type == k);
        assert!(told(EventType::ForestCleared) || told(EventType::GameScarce));

        // Abandon the town: fields go back to the wild and the wolves come back.
        history.settlements.get_mut(&sid).unwrap().destroyed = Some(Date::new(80, Season::Spring));
        for year in 81..=200 {
            history.current_date = Date::new(year, Season::Spring);
            step_year(&mut history, &world);
        }
        let eco = history.ecology.as_ref().unwrap();
        let (forest_near, forest_far) = near_far(eco, &eco.forest);
        let (wolf_near, wolf_far) = near_far(eco, &eco.fauna[WOLF]);
        assert!(forest_near > 0.6 * forest_far, "forest regrows: near {forest_near} far {forest_far}");
        assert!(wolf_near > 0.5 * wolf_far, "wolves return: near {wolf_near} far {wolf_far}");
        assert!(told_after(&history, EventType::WildlifeReturned));
    }

    #[test]
    fn battles_leave_bone_fields_and_naturalize_removes_dice_anomalies() {
        let mut world = forest_world();
        *world.biomes.get_mut(40, 10) = ExtendedBiome::BoneFields;
        assert_eq!(Ecology::naturalize(&mut world), 1);
        assert_eq!(*world.biomes.get(40, 10), ExtendedBiome::TemperateForest);

        let mut history = WorldHistory::new(HistoryConfig::default(), 64, 32, Date::new(1, Season::Spring));
        history.ecology = Some(Ecology::new(&world));
        for year in 1..=8u32 {
            history.current_date = Date::new(year, Season::Spring);
            let id = history.id_generators.next_event();
            history.chronicle.record(Event::new(id, EventType::BattleFought, history.current_date, "Battle".into(), String::new()).at_location(30, 20));
            step_year(&mut history, &world);
        }
        let mut eco = history.ecology.take().unwrap();
        assert_eq!(eco.scars.len(), 1, "six battles at one site make a bone field");
        let scar = eco.scars[0].clone();
        let told = history.chronicle.events.iter().find(|e| e.id == scar.event).unwrap();
        assert_eq!(told.event_type, EventType::LandScarred);
        assert!(told.triggered_by.is_some(), "the scar links to the battle that caused it");
        eco.apply_scars(&mut world);
        assert_eq!(*world.biomes.get(30, 20), ExtendedBiome::BoneFields);
        assert!(eco.scar_at(30, 20).is_some());
    }

    fn told_after(history: &WorldHistory, k: EventType) -> bool {
        history.chronicle.events.iter().any(|e| e.event_type == k)
    }

    #[test]
    fn predators_follow_their_prey() {
        let world = forest_world();
        let mut eco = Ecology::new(&world);
        // Wipe out the prey on one half: wolves there must decline.
        for i in 0..eco.width * eco.height {
            if i % eco.width < 32 {
                for s in [DEER, BOAR, CARIBOU, AUROCHS, IBEX] { eco.fauna[s][i] = 0.0; }
            }
        }
        let mut history = WorldHistory::new(HistoryConfig::default(), 64, 32, Date::new(1, Season::Spring));
        history.ecology = Some(eco);
        for _ in 0..3 { step_year(&mut history, &world); }
        let eco = history.ecology.as_ref().unwrap();
        let (hungry, fed) = (eco.fauna[WOLF][16 * 64 + 10], eco.fauna[WOLF][16 * 64 + 50]);
        assert!(hungry < 0.6 * fed, "hungry wolves {hungry} vs fed {fed}");
    }
}
