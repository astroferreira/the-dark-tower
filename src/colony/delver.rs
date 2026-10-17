//! Real-time ARPG delving and nomadic company transport (`Colony::arpg`).
//!
//! Bridges the autonomous surface colony (`src/colony/`) with direct player-controlled
//! underground exploration and technology-limited nomadic camp relocation:
//! - Press `E` (or click "Delve") to step into a settler (`Delver`) with up to two companion
//!   settlers following.
//! - Move smoothly in sub-cell coordinates with `WASD`, aim your lantern cone and weapon with
//!   the mouse, `Space` to dodge-dash, `Left-Click` to slash foes in an arc or swing your pick
//!   at rock/ore walls (cracking and breaking them open into physical loot drops), `Right-Click`
//!   to raise your shield and parry lunges, `G` to hurl a javelin/stone, `F` to plant a wall
//!   torch, and `>` / `<` to climb or cut stairs down into caves, mines, tombs, lairs, and caverns.
//! - Carry loot back to the campfire in limited pack slots so your smiths and carpenters can
//!   smelt bars, forge weapons/tools, and build higher-tier transport (`TransportTier`).
//! - Press `K` ("Strike Camp") to pack what fits within your transport technology's carry
//!   capacity (`PackedCompany`) and march across the Region map to pitch camp at a new site.

use super::*;
use crate::local::{Material, Plant, Shape};
use crate::local::places::{PlaceKind, UnderPlace};

/// Technology tier limiting both personal delve packs and camp-wide carry capacity when striking camp.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum TransportTier {
    /// Tier 0: Woven packs and shoulder slings.
    Packs = 0,
    /// Tier 1: Timber travois and sledges dragged by hand (2 logs).
    Travois = 1,
    /// Tier 2: Iron-rimmed handcarts and wheelbarrows (2 logs + 1 metal bar).
    Handcart = 2,
    /// Tier 3: Pack beasts and harness frames (2 logs + 1 metal bar + beast/hide).
    PackBeast = 3,
    /// Tier 4: Heavy expedition wagon (4 logs + 3 metal bars).
    Wagon = 4,
}

impl Default for TransportTier {
    fn default() -> Self { TransportTier::Packs }
}

impl TransportTier {
    pub fn name(self) -> &'static str {
        match self {
            TransportTier::Packs => "Tier 0: Backs & Packs",
            TransportTier::Travois => "Tier 1: Timber Travois",
            TransportTier::Handcart => "Tier 2: Iron-Rimmed Handcarts",
            TransportTier::PackBeast => "Tier 3: Pack Mule Train",
            TransportTier::Wagon => "Tier 4: Expedition Wagon",
        }
    }

    pub fn short_name(self) -> &'static str {
        match self {
            TransportTier::Packs => "Packs",
            TransportTier::Travois => "Travois",
            TransportTier::Handcart => "Handcarts",
            TransportTier::PackBeast => "Pack Train",
            TransportTier::Wagon => "Wagon",
        }
    }

    pub fn subtitle(self) -> &'static str {
        match self {
            TransportTier::Packs => "5 delve slots · 2 carry/settler",
            TransportTier::Travois => "7 delve slots · 4 carry/settler",
            TransportTier::Handcart => "10 delve slots · 6 carry/settler",
            TransportTier::PackBeast => "13 delve slots · 8 carry/settler",
            TransportTier::Wagon => "16 delve slots · 11 carry/settler",
        }
    }

    /// Personal + companion pack capacity while delving underground.
    pub fn delve_pack_slots(self, companions: usize) -> usize {
        let base = match self {
            TransportTier::Packs => 5,
            TransportTier::Travois => 7,
            TransportTier::Handcart => 10,
            TransportTier::PackBeast => 13,
            TransportTier::Wagon => 16,
        };
        base + companions * 2
    }

    /// Camp-wide carry slots per living settler when striking camp.
    pub fn camp_slots_per_settler(self) -> usize {
        match self {
            TransportTier::Packs => 2,
            TransportTier::Travois => 4,
            TransportTier::Handcart => 6,
            TransportTier::PackBeast => 8,
            TransportTier::Wagon => 11,
        }
    }

    /// Flat bonus slots from carts/wagons when striking camp.
    pub fn camp_bonus_slots(self) -> usize {
        match self {
            TransportTier::Packs => 2,
            TransportTier::Travois => 4,
            TransportTier::Handcart => 8,
            TransportTier::PackBeast => 14,
            TransportTier::Wagon => 24,
        }
    }

    pub fn next(self) -> Option<TransportTier> {
        match self {
            TransportTier::Packs => Some(TransportTier::Travois),
            TransportTier::Travois => Some(TransportTier::Handcart),
            TransportTier::Handcart => Some(TransportTier::PackBeast),
            TransportTier::PackBeast => Some(TransportTier::Wagon),
            TransportTier::Wagon => None,
        }
    }

    /// (logs, metal_bars) required to build this tier at the camp.
    pub fn craft_cost(self) -> (u32, u32) {
        match self {
            TransportTier::Packs => (0, 0),
            TransportTier::Travois => (2, 0),
            TransportTier::Handcart => (2, 1),
            TransportTier::PackBeast => (2, 2),
            TransportTier::Wagon => (4, 3),
        }
    }

    pub fn cost_words(self) -> &'static str {
        match self {
            TransportTier::Packs => "starter gear",
            TransportTier::Travois => "2 logs",
            TransportTier::Handcart => "2 logs, 1 metal bar (or Old Mine blueprint)",
            TransportTier::PackBeast => "2 logs, 2 metal bars (or Lair blueprint)",
            TransportTier::Wagon => "4 logs, 3 metal bars (or Halls blueprint)",
        }
    }
}

/// A physical item carried in the delver's pack or lying on the dungeon floor.
#[derive(Clone, Debug)]
pub enum PackItem {
    Ore(String),
    Gem(String),
    Bars(String, u32),
    Relic(String),
    Weapon { kind: String, material: String, quality: f32 },
    Provisions(u32),
    Hide { beast: String, pieces: u32 },
    Blueprint(TransportTier),
    Tablet { title: String, tile: (usize, usize), note: String },
}

impl PackItem {
    pub fn label(&self) -> String {
        match self {
            PackItem::Ore(k) => format!("{} ore", k),
            PackItem::Gem(g) => format!("uncut {}", g),
            PackItem::Bars(m, n) => if *n == 1 { format!("{} bar", m) } else { format!("{} {} bars", n, m) },
            PackItem::Relic(r) => r.clone(),
            PackItem::Weapon { kind, .. } => kind.clone(),
            PackItem::Provisions(n) => format!("{} rations", n),
            PackItem::Hide { beast, .. } => format!("hide of {}", beast),
            PackItem::Blueprint(t) => format!("{} plans", t.short_name()),
            PackItem::Tablet { title, .. } => title.clone(),
        }
    }

    pub fn color(&self) -> [f32; 3] {
        match self {
            PackItem::Ore(k) | PackItem::Bars(k, _) => match k.as_str() {
                "gold" => [232.0, 190.0, 64.0],
                "copper" => [206.0, 124.0, 76.0],
                "silver" => [200.0, 206.0, 214.0],
                "adamantine" => [100.0, 210.0, 200.0],
                _ => [156.0, 162.0, 172.0],
            },
            PackItem::Gem(_) => [120.0, 210.0, 160.0],
            PackItem::Relic(_) => [238.0, 194.0, 68.0],
            PackItem::Weapon { .. } => [190.0, 196.0, 208.0],
            PackItem::Provisions(_) => [196.0, 150.0, 92.0],
            PackItem::Hide { .. } => [164.0, 118.0, 82.0],
            PackItem::Blueprint(_) => [110.0, 168.0, 218.0],
            PackItem::Tablet { .. } => [216.0, 160.0, 110.0],
        }
    }
}

/// A physical loot drop lying at `(x, y, z)` in the world.
#[derive(Clone, Debug)]
pub struct LootDrop {
    pub x: f32,
    pub y: f32,
    pub z: i32,
    pub item: PackItem,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FoeKind {
    CaveSpider,
    BlindStalker,
    RockCrawler,
    OutlawMiner,
    RisenGuard,
    BarrowWight,
    DeepAutomaton,
    LairBoss,
}

impl FoeKind {
    pub fn beast_look_name(self, custom_name: &str) -> &str {
        match self {
            FoeKind::CaveSpider => "cave spider",
            FoeKind::BlindStalker => "cave crocodile",
            FoeKind::RockCrawler => "giant rat",
            FoeKind::OutlawMiner => "outlaw",
            FoeKind::RisenGuard => "risen dead",
            FoeKind::BarrowWight => "risen dead",
            FoeKind::DeepAutomaton => "bronze colossus",
            FoeKind::LairBoss => custom_name,
        }
    }

    pub fn is_humanoid(self) -> bool {
        matches!(self, FoeKind::OutlawMiner | FoeKind::RisenGuard | FoeKind::BarrowWight)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum FoeState {
    Idle,
    Chase,
    Windup { timer: f32, dx: f32, dy: f32 },
    Lunge { timer: f32, dx: f32, dy: f32 },
    Staggered(f32),
}

/// Real-time enemy inside an underground place or cavern.
#[derive(Clone, Debug)]
pub struct DelveFoe {
    pub id: u32,
    pub name: String,
    pub kind: FoeKind,
    pub x: f32,
    pub y: f32,
    pub z: i32,
    pub home: (f32, f32, i32),
    pub vx: f32,
    pub vy: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub damage: f32,
    pub speed: f32,
    pub size: f32,
    pub state: FoeState,
    pub hit_flash: f32,
    pub attack_cd: f32,
    pub place_idx: Option<usize>,
    pub drops: Vec<PackItem>,
}

/// Visual transient effects for ARPG combat and mining.
#[derive(Clone, Debug)]
pub enum DelveFx {
    SlashArc {
        x: f32,
        y: f32,
        z: i32,
        angle: f32,
        radius: f32,
        timer: f32,
        max_timer: f32,
        pick: bool,
    },
    Spark {
        x: f32,
        y: f32,
        vx: f32,
        vy: f32,
        z: i32,
        color: [f32; 3],
        timer: f32,
    },
    FloatingText {
        x: f32,
        y: f32,
        z: i32,
        text: String,
        color: u32,
        timer: f32,
    },
}

/// Thrown stone or javelin projectile.
#[derive(Clone, Debug)]
pub struct Projectile {
    pub x: f32,
    pub y: f32,
    pub z: i32,
    pub vx: f32,
    pub vy: f32,
    pub damage: f32,
    pub timer: f32,
    pub from_player: bool,
}

/// The player-controlled settler during a real-time delve.
#[derive(Clone, Debug)]
pub struct Delver {
    pub who: usize,
    pub companions: Vec<usize>,
    pub x: f32,
    pub y: f32,
    pub z: i32,
    pub vx: f32,
    pub vy: f32,
    pub aim_angle: f32,
    pub hp: f32,
    pub max_hp: f32,
    pub stamina: f32,
    pub max_stamina: f32,
    pub dash_timer: f32,
    pub dash_cd: f32,
    pub dash_dir: (f32, f32),
    pub swing_timer: f32,
    pub swing_cd: f32,
    pub swing_angle: f32,
    pub swing_pick: bool,
    pub blocking: bool,
    pub throw_cd: f32,
    pub hurt_cd: f32,
    pub comp_attack_cd: [f32; 2],
    pub pack: Vec<PackItem>,
}

/// Per-frame real-time input fed into `Colony::step_arpg`.
#[derive(Clone, Copy, Debug, Default)]
pub struct ArpgInput {
    pub move_x: f32,
    pub move_y: f32,
    pub aim_world: (f32, f32),
    pub primary_down: bool,
    pub primary_clicked: bool,
    pub secondary_down: bool,
    pub throw_pressed: bool,
    pub dash_pressed: bool,
    pub torch_pressed: bool,
    pub stair_down_pressed: bool,
    pub stair_up_pressed: bool,
}

/// All ARPG delving and transport state owned by `Colony`.
#[derive(Clone, Debug, Default)]
pub struct ArpgState {
    pub delver: Option<Delver>,
    pub transport: TransportTier,
    pub unlocked_tiers: Vec<TransportTier>,
    pub populated_places: bool,
    pub foes: Vec<DelveFoe>,
    pub loot: Vec<LootDrop>,
    pub wall_damage: Vec<((u16, u16, i32), f32)>,
    pub torches: Vec<((u16, u16), i32)>,
    pub fx: Vec<DelveFx>,
    pub projectiles: Vec<Projectile>,
    pub region_clues: Vec<((usize, usize), String)>,
    pub strike_camp_open: bool,
    pub next_foe_id: u32,
}

/// A company packed up to march across the Region map to a new embark site.
#[derive(Clone, Debug)]
pub struct PackedCompany {
    pub name: Option<String>,
    pub day: u64,
    pub settlers: Vec<Settler>,
    pub transport: TransportTier,
    pub unlocked_tiers: Vec<TransportTier>,
    pub arms: Vec<militia::Arm>,
    pub armour: Vec<armour::Armour>,
    pub tools: Option<(String, u64)>,
    pub smith_skills: crate::history::det::HashMap<usize, f32>,
    pub bars: Vec<(String, u32)>,
    pub ore: Vec<(String, u32)>,
    pub gems: Vec<(String, u32)>,
    pub treasures: Vec<String>,
    pub works: Vec<craft::Work>,
    pub rations: u32,
    pub logs: u32,
    pub stones: u32,
    pub region_clues: Vec<((usize, usize), String)>,
    pub slots_used: usize,
    pub slots_max: usize,
    pub summary_lines: Vec<String>,
}

impl PackedCompany {
    pub fn origin_name(&self) -> String {
        self.name.clone().unwrap_or_else(|| "the camp".to_string())
    }
}

/// Manifest preview for the Strike Camp modal.
#[derive(Clone, Debug)]
pub struct StrikeManifest {
    pub transport: TransportTier,
    pub slots_used: usize,
    pub slots_max: usize,
    /// (Category label, count/detail, slots taken, fits_in_transport)
    pub rows: Vec<(String, String, usize, bool)>,
    pub left_behind: Vec<String>,
}

impl Colony {
    /// Whether the player is currently controlling a settler in ARPG Delver Mode.
    pub fn is_delving(&self) -> bool {
        self.arpg.delver.is_some()
    }

    /// Whether settler `i` is under ARPG control (either the delver or one of their active companions).
    pub fn is_arpg_controlled(&self, i: usize) -> bool {
        self.arpg.delver.as_ref().map_or(false, |d| d.who == i || d.companions.contains(&i))
    }

    /// Maximum pack slots available during a delve for the current transport tier + companions.
    pub fn max_delve_pack(&self) -> usize {
        let comps = self.arpg.delver.as_ref().map_or(0, |d| d.companions.len());
        self.arpg.transport.delve_pack_slots(comps)
    }

    /// Maximum camp carry slots when striking camp to relocate across the Region.
    pub fn max_camp_carry_slots(&self) -> usize {
        let alive = self.alive().max(1);
        let t = self.arpg.transport;
        alive * t.camp_slots_per_settler() + t.camp_bonus_slots()
    }

    /// Check if the colony can upgrade its `TransportTier` right now.
    pub fn can_upgrade_transport(&self) -> Option<(TransportTier, bool)> {
        let next = self.arpg.transport.next()?;
        if self.arpg.unlocked_tiers.contains(&next) {
            return Some((next, true));
        }
        let (need_logs, need_bars) = next.craft_cost();
        let logs = self.logs_stored();
        let bars: u32 = self.industry.bars.iter().map(|b| b.1).sum();
        Some((next, logs >= need_logs && bars >= need_bars))
    }

    /// Upgrade to the next `TransportTier`, spending logs/bars (or free if blueprint unlocked).
    pub fn upgrade_transport(&mut self) -> Result<String, String> {
        let Some(next) = self.arpg.transport.next() else {
            return Err("Already at maximum transport technology (Expedition Wagon).".into());
        };
        if !self.arpg.unlocked_tiers.contains(&next) {
            let (need_logs, need_bars) = next.craft_cost();
            let logs = self.logs_stored();
            let bars: u32 = self.industry.bars.iter().map(|b| b.1).sum();
            if logs < need_logs || bars < need_bars {
                return Err(format!(
                    "Need {} to build {} (have {} logs, {} metal bars; or find its blueprint underground).",
                    next.cost_words(),
                    next.name(),
                    logs,
                    bars
                ));
            }
            for _ in 0..need_logs {
                if let Some(k) = self.items.iter().position(|it| it.stored && it.kind == ItemKind::Log) {
                    self.items.remove(k);
                    self.fix_refs_pub(k);
                }
            }
            let mut rem = need_bars;
            for b in &mut self.industry.bars {
                let take = b.1.min(rem);
                b.1 -= take;
                rem -= take;
                if rem == 0 { break; }
            }
            self.industry.bars.retain(|b| b.1 > 0);
        }
        self.arpg.transport = next;
        let msg = format!(
            "Built {}! Delve pack: {} slots; Strike Camp capacity: {} slots.",
            next.name(),
            self.max_delve_pack(),
            self.max_camp_carry_slots()
        );
        self.note(msg.clone());
        Ok(msg)
    }

    /// Ensure this embark has at least one accessible underground place with a surface mouth,
    /// and populate all underground places with real-time ARPG foes, ore seams, and loot caches.
    pub fn ensure_arpg_places(&mut self) {
        if self.arpg.populated_places {
            return;
        }
        self.arpg.populated_places = true;

        // If the embark generated without a surface-accessible place, carve a multi-level Cave & Old Seam near camp.
        let has_mouth = self.map.places.iter().any(|p| p.mouth.is_some());
        if !has_mouth {
            self.carve_guaranteed_delve();
        }

        // Reveal all surface mouths on the compass so the delver knows where to go.
        for (k, pl) in self.map.places.iter().enumerate() {
            if pl.mouth.is_some() && !self.mouths_seen.contains(&k) {
                self.mouths_seen.push(k);
            }
        }

        // Populate every place with real-time foes and floor loot drops.
        let num_places = self.map.places.len();
        for p_idx in 0..num_places {
            let kind = self.map.places[p_idx].kind;
            let pname = self.map.places[p_idx].name.clone();
            let cells = self.map.places[p_idx].cells.clone();
            if cells.len() < 4 {
                continue;
            }
            // Place ore veins in walls adjacent to the deeper half of the place so pickaxe mining is rewarding.
            let mid = cells.len() / 2;
            let ores = [
                crate::history::civilizations::economy::ResourceType::Iron,
                crate::history::civilizations::economy::ResourceType::Copper,
                crate::history::civilizations::economy::ResourceType::Gold,
            ];
            for (ci, &(pos, z)) in cells.iter().enumerate().skip(mid / 2) {
                if ci % 3 == 0 {
                    let ore_kind = ores[(ci / 3 + p_idx) % ores.len()];
                    for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                        let (wx, wy) = (pos.0 as i32 + dx, pos.1 as i32 + dy);
                        if wx >= 1 && wy >= 1 && (wx as usize) < self.map.width - 1 && (wy as usize) < self.map.height - 1 && (z + 1) >= 0 && (z as usize + 1) < self.map.depth {
                            let idx = self.map.idx(wx as usize, wy as usize, (z + 1) as usize);
                            if self.map.cells[idx].shape == Shape::Wall && matches!(self.map.cells[idx].material, Material::Rock(_)) {
                                self.map.cells[idx].material = Material::Ore(ore_kind);
                                break;
                            }
                        }
                    }
                }
            }

            // Spawn enemies along the cells of the place.
            let step = (cells.len() / 4).max(3);
            for (idx, &(pos, z)) in cells.iter().enumerate().skip(3).step_by(step) {
                if idx + 1 >= cells.len() {
                    break;
                }
                let (fkind, fname, hp, dmg, spd, sz, drops) = match kind {
                    PlaceKind::Cave | PlaceKind::Cavern => {
                        if idx % 2 == 0 {
                            (
                                FoeKind::CaveSpider,
                                "a giant cave spider".to_string(),
                                42.0,
                                9.0,
                                3.8,
                                1.1,
                                vec![PackItem::Ore("iron".into()), PackItem::Gem("emerald".into())],
                            )
                        } else {
                            (
                                FoeKind::BlindStalker,
                                "a blind cave stalker".to_string(),
                                58.0,
                                12.0,
                                3.3,
                                1.3,
                                vec![PackItem::Ore("copper".into()), PackItem::Hide { beast: "cave stalker".into(), pieces: 2 }],
                            )
                        }
                    }
                    PlaceKind::OldMine => (
                        FoeKind::OutlawMiner,
                        "a delving brigand".to_string(),
                        52.0,
                        11.0,
                        3.5,
                        1.0,
                        vec![PackItem::Bars("iron".into(), 1), PackItem::Ore("iron".into())],
                    ),
                    PlaceKind::Tomb => (
                        FoeKind::RisenGuard,
                        "a barrow-draugr".to_string(),
                        65.0,
                        13.0,
                        3.1,
                        1.1,
                        vec![PackItem::Bars("gold".into(), 1), PackItem::Gem("ruby".into())],
                    ),
                    PlaceKind::Lair => (
                        FoeKind::RockCrawler,
                        "a lair whelp".to_string(),
                        46.0,
                        10.0,
                        3.9,
                        1.0,
                        vec![PackItem::Hide { beast: "lair beast".into(), pieces: 1 }, PackItem::Ore("iron".into())],
                    ),
                    PlaceKind::Halls => (
                        FoeKind::DeepAutomaton,
                        "a bronze warder".to_string(),
                        80.0,
                        15.0,
                        2.9,
                        1.4,
                        vec![PackItem::Bars("copper".into(), 2), PackItem::Ore("adamantine".into())],
                    ),
                };
                let id = self.arpg.next_foe_id;
                self.arpg.next_foe_id += 1;
                self.arpg.foes.push(DelveFoe {
                    id,
                    name: fname,
                    kind: fkind,
                    x: pos.0 as f32 + 0.5,
                    y: pos.1 as f32 + 0.5,
                    z,
                    home: (pos.0 as f32 + 0.5, pos.1 as f32 + 0.5, z),
                    vx: 0.0,
                    vy: 0.0,
                    hp,
                    max_hp: hp,
                    damage: dmg,
                    speed: spd,
                    size: sz,
                    state: FoeState::Idle,
                    hit_flash: 0.0,
                    attack_cd: 0.0,
                    place_idx: Some(p_idx),
                    drops,
                });
            }

            // Boss / Champion at the far end of the place, plus floor treasure caches!
            if let Some(&(end_pos, end_z)) = cells.last() {
                let (ex, ey) = (end_pos.0 as f32 + 0.5, end_pos.1 as f32 + 0.5);
                let clue_tile = (
                    (self.map.world_tile.0 + 3 + p_idx * 2) % self.world_width.max(1),
                    (self.map.world_tile.1 + 2).min(255),
                );
                let (bkind, bname, bhp, bdmg, bspd, bsz, bdrops) = match kind {
                    PlaceKind::Lair => {
                        let boss_name = pname.strip_prefix("The lair of ").unwrap_or(&pname).to_string();
                        (
                            FoeKind::LairBoss,
                            boss_name.clone(),
                            145.0,
                            19.0,
                            3.6,
                            1.85,
                            vec![
                                PackItem::Relic(format!("The Crown of {}", boss_name)),
                                PackItem::Blueprint(TransportTier::PackBeast),
                                PackItem::Hide { beast: boss_name, pieces: 4 },
                                PackItem::Bars("iron".into(), 2),
                            ],
                        )
                    }
                    PlaceKind::Tomb => (
                        FoeKind::BarrowWight,
                        format!("Wight of {}", pname.strip_prefix("The tomb of ").unwrap_or(&pname)),
                        115.0,
                        16.0,
                        3.3,
                        1.35,
                        vec![
                            PackItem::Weapon {
                                kind: "an ancient iron-headed spear".into(),
                                material: "iron".into(),
                                quality: 1.25,
                            },
                            PackItem::Relic(format!("The Signet of {}", pname)),
                            PackItem::Tablet {
                                title: "Carved Way-Tablet".into(),
                                tile: clue_tile,
                                note: format!("Points to ancient halls at tile ({}, {})", clue_tile.0, clue_tile.1),
                            },
                        ],
                    ),
                    PlaceKind::OldMine => (
                        FoeKind::OutlawMiner,
                        "the Mine-Boss".to_string(),
                        95.0,
                        14.0,
                        3.4,
                        1.25,
                        vec![
                            PackItem::Blueprint(TransportTier::Handcart),
                            PackItem::Bars("iron".into(), 2),
                            PackItem::Ore("iron".into()),
                        ],
                    ),
                    PlaceKind::Halls => (
                        FoeKind::DeepAutomaton,
                        "The Forgemaster Colossus".to_string(),
                        160.0,
                        21.0,
                        3.0,
                        1.9,
                        vec![
                            PackItem::Blueprint(TransportTier::Wagon),
                            PackItem::Weapon {
                                kind: "a masterwork adamantine spear".into(),
                                material: "adamantine".into(),
                                quality: 1.4,
                            },
                            PackItem::Bars("adamantine".into(), 2),
                        ],
                    ),
                    PlaceKind::Cave | PlaceKind::Cavern => (
                        FoeKind::CaveSpider,
                        "the Broodmother Spider".to_string(),
                        105.0,
                        15.0,
                        3.7,
                        1.6,
                        vec![
                            PackItem::Blueprint(TransportTier::Travois),
                            PackItem::Gem("diamond".into()),
                            PackItem::Ore("iron".into()),
                            PackItem::Tablet {
                                title: "Surveyor's Slate".into(),
                                tile: clue_tile,
                                note: format!("Marks a rich delve at tile ({}, {})", clue_tile.0, clue_tile.1),
                            },
                        ],
                    ),
                };
                let id = self.arpg.next_foe_id;
                self.arpg.next_foe_id += 1;
                self.arpg.foes.push(DelveFoe {
                    id,
                    name: bname,
                    kind: bkind,
                    x: ex,
                    y: ey,
                    z: end_z,
                    home: (ex, ey, end_z),
                    vx: 0.0,
                    vy: 0.0,
                    hp: bhp,
                    max_hp: bhp,
                    damage: bdmg,
                    speed: bspd,
                    size: bsz,
                    state: FoeState::Idle,
                    hit_flash: 0.0,
                    attack_cd: 0.0,
                    place_idx: Some(p_idx),
                    drops: bdrops,
                });

                // Also scatter 2-3 immediate loot pickups near the entrance/midway so early delves are immediately rewarding.
                if let Some(&(mid_pos, mid_z)) = cells.get(cells.len() / 3) {
                    self.arpg.loot.push(LootDrop {
                        x: mid_pos.0 as f32 + 0.5,
                        y: mid_pos.1 as f32 + 0.5,
                        z: mid_z,
                        item: PackItem::Ore("iron".into()),
                    });
                }
                if let Some(&(ch_pos, ch_z)) = cells.get(cells.len() * 2 / 3) {
                    self.arpg.loot.push(LootDrop {
                        x: ch_pos.0 as f32 + 0.5,
                        y: ch_pos.1 as f32 + 0.5,
                        z: ch_z,
                        item: PackItem::Gem("obsidian".into()),
                    });
                }
            }
        }
    }

    /// Carve a guaranteed multi-level sinkhole cave & old mine seam 26 cells from the camp if the map had none.
    fn carve_guaranteed_delve(&mut self) {
        let (cx, cy) = (self.camp.0 as i32, self.camp.1 as i32);
        let (w, h) = (self.map.width as i32, self.map.height as i32);
        let mut chosen = (cx + 22, cy);
        for &(dx, dy) in &[(22, 0), (-22, 0), (0, 22), (0, -22), (18, 18), (-18, 18)] {
            let (tx, ty) = ((cx + dx).clamp(10, w - 16), (cy + dy).clamp(10, h - 16));
            if nav::passable(&self.map, (tx as u16, ty as u16)) {
                chosen = (tx, ty);
                break;
            }
        }
        let (mx, my) = chosen;
        let z0 = self.map.surface_z[my as usize * self.map.width + mx as usize];
        if z0 < 8 {
            return;
        }
        let mut walk: Vec<((u16, u16), i32)> = Vec::new();
        // Cut a stair shaft 4 levels down from the surface mouth, then a passage and two chambers.
        for z in (z0 - 4..z0).rev() {
            let k = self.map.idx(mx as usize, my as usize, (z + 1) as usize);
            self.map.cells[k].shape = Shape::Stair;
            self.map.cells[k].plant = Plant::None;
            self.map.cells[k].boulder = false;
            let f = self.map.idx(mx as usize, my as usize, z as usize);
            if self.map.cells[f].shape == Shape::Wall {
                self.map.cells[f].shape = Shape::Stair;
            }
            walk.push(((mx as u16, my as u16), z));
        }
        let zb = z0 - 4;
        for step in 1..=10 {
            let px = (mx + step).clamp(4, w - 5);
            for dy in -1i32..=1 {
                if step < 4 && dy != 0 {
                    continue;
                }
                let py = (my + dy).clamp(4, h - 5);
                let head = self.map.idx(px as usize, py as usize, (zb + 1) as usize);
                self.map.cells[head].shape = Shape::Empty;
                self.map.cells[head].material = Material::Air;
                let floor = self.map.idx(px as usize, py as usize, zb as usize);
                self.map.cells[floor].shape = Shape::Floor;
                walk.push(((px as u16, py as u16), zb));
            }
        }
        // Second deeper level (zb - 3) via a stair at the end of the first chamber.
        let (sx, sy) = ((mx + 10).clamp(4, w - 8), my);
        for z in (zb - 3..zb).rev() {
            let k = self.map.idx(sx as usize, sy as usize, (z + 1) as usize);
            self.map.cells[k].shape = Shape::Stair;
            let f = self.map.idx(sx as usize, sy as usize, z as usize);
            self.map.cells[f].shape = Shape::Stair;
            walk.push(((sx as u16, sy as u16), z));
        }
        let z2 = zb - 3;
        for dx in 1..=5 {
            for dy in -2i32..=2 {
                let (px, py) = ((sx + dx).clamp(4, w - 5), (sy + dy).clamp(4, h - 5));
                let head = self.map.idx(px as usize, py as usize, (z2 + 1) as usize);
                self.map.cells[head].shape = Shape::Empty;
                self.map.cells[head].material = Material::Air;
                let floor = self.map.idx(px as usize, py as usize, z2 as usize);
                self.map.cells[floor].shape = Shape::Floor;
                walk.push(((px as u16, py as u16), z2));
            }
        }
        self.map.places.push(UnderPlace {
            kind: PlaceKind::OldMine,
            name: "The Old Iron Seam".into(),
            cause: "an old gallery cut into the hill before the camp was made".into(),
            contents: vec![
                "rich veins of iron and copper in the rock".into(),
                "abandoned tools and a handcart blueprint".into(),
            ],
            cells: walk,
            mouth: Some((mx as u16, my as u16)),
            found: false,
        });
    }

    /// Enter or exit ARPG Delver Mode (`E` key or UI button).
    pub fn toggle_delver(&mut self, preferred: Option<usize>) -> Result<String, String> {
        if let Some(d) = self.arpg.delver.take() {
            let name = self.settlers.get(d.who).map(|s| s.name.clone()).unwrap_or_default();
            // Return settler and companions to autonomous camp AI.
            if let Some(s) = self.settlers.get_mut(d.who) {
                s.job = Job::Idle;
                s.why = "Back from the delve".into();
            }
            for c in d.companions {
                if let Some(s) = self.settlers.get_mut(c) {
                    s.job = Job::Idle;
                    s.why = "Back from the delve".into();
                }
            }
            return Ok(format!("{} steps back into the company's ranks (autonomous camp mode).", name));
        }

        self.ensure_arpg_places();

        // Pick the preferred settler if alive, else the best fighter in the company.
        let who = preferred
            .filter(|&i| self.settlers.get(i).map_or(false, |s| s.alive))
            .or_else(|| {
                (0..self.settlers.len())
                    .filter(|&i| self.settlers[i].alive && self.settlers[i].guest_until == 0)
                    .max_by(|&a, &b| self.fight_skill(a).total_cmp(&self.fight_skill(b)))
            })
            .ok_or_else(|| "No living settler to control.".to_string())?;

        // Release any current job they held.
        self.release(who);
        let p3 = self.here3(who);
        self.settlers[who].z = p3.2;
        self.settlers[who].why = "Leading the delve (player)".into();

        // Pick up to 2 healthy adult companions to follow with torches & spears.
        let mut companions: Vec<usize> = (0..self.settlers.len())
            .filter(|&i| {
                i != who
                    && self.settlers[i].alive
                    && self.settlers[i].guest_until == 0
                    && self.settlers[i].ill_until <= self.clock.tick
                    && self.settlers[i].past.as_ref().map_or(true, |p| p.age >= 14)
            })
            .collect();
        companions.sort_by(|&a, &b| self.fight_skill(b).total_cmp(&self.fight_skill(a)));
        companions.truncate(2);
        for &c in &companions {
            self.release(c);
            self.settlers[c].why = format!("Following {} into the delve", self.settlers[who].name);
        }

        let armour_bonus = if self.armour_of(who).is_some() { 35.0 } else { 0.0 };
        let max_hp = 100.0 + armour_bonus;
        let delver = Delver {
            who,
            companions: companions.clone(),
            x: p3.0 as f32 + 0.5,
            y: p3.1 as f32 + 0.5,
            z: p3.2,
            vx: 0.0,
            vy: 0.0,
            aim_angle: 0.0,
            hp: max_hp,
            max_hp,
            stamina: 100.0,
            max_stamina: 100.0,
            dash_timer: 0.0,
            dash_cd: 0.0,
            dash_dir: (1.0, 0.0),
            swing_timer: 0.0,
            swing_cd: 0.0,
            swing_angle: 0.0,
            swing_pick: false,
            blocking: false,
            throw_cd: 0.0,
            hurt_cd: 0.0,
            comp_attack_cd: [0.0; 2],
            pack: Vec::new(),
        };
        self.arpg.delver = Some(delver);
        self.reveal_around_delver();

        // Describe the nearest cave/mine/tomb mouth so the player knows right where to walk.
        let mouth_hint = self
            .nearest_place_mouth((p3.0, p3.1))
            .map(|(idx, m, dist)| {
                let dir = compass_dir((p3.0 as i32, p3.1 as i32), (m.0 as i32, m.1 as i32));
                format!(
                    " | Nearest entrance: {} ({} cells {} at {},{})",
                    self.map.places[idx].name, dist, dir, m.0, m.1
                )
            })
            .unwrap_or_default();

        let comp_names: Vec<String> = companions.iter().map(|&c| self.settlers[c].name.clone()).collect();
        let with = if comp_names.is_empty() {
            String::new()
        } else {
            format!(" with {}", comp_names.join(" & "))
        };
        Ok(format!(
            "DELVER MODE: Controlling {}{} (WASD move, L-Click slash/mine, R-Click block, Space dash, </> stairs, F torch, K strike camp){}",
            self.settlers[who].name, with, mouth_hint
        ))
    }

    /// Nearest underground place mouth from `from`: `(place_index, mouth_pos, distance_cells)`.
    pub fn nearest_place_mouth(&self, from: Pos) -> Option<(usize, Pos, i32)> {
        self.map
            .places
            .iter()
            .enumerate()
            .filter_map(|(k, p)| {
                let m = p.mouth?;
                let d = (((m.0 as i32 - from.0 as i32).pow(2) + (m.1 as i32 - from.1 as i32).pow(2)) as f32).sqrt() as i32;
                Some((k, m, d))
            })
            .min_by_key(|&(_, _, d)| d)
    }

    /// Weapon name and base damage of settler `who`.
    pub fn delver_weapon(&self, who: usize) -> (String, f32) {
        if let Some(arm) = self.arm_of(who) {
            let base = if arm.material.contains("adamantine") {
                56.0
            } else if arm.material.contains("iron") {
                38.0
            } else if arm.material.contains("copper") {
                28.0
            } else if arm.material.contains("obsidian") {
                30.0
            } else {
                22.0
            };
            return (arm.kind.clone(), base * arm.quality.max(0.8));
        }
        if let Some((metal, _)) = &self.industry.tools {
            let dmg = if metal == "iron" { 30.0 } else { 24.0 };
            return (format!("a {} pick & axe", metal), dmg);
        }
        ("a heavy stone-headed pick".to_string(), 18.0)
    }

    /// Step the real-time ARPG simulation by `dt` seconds (~1/60s).
    /// Returns an optional status message if something notable happened this frame.
    pub fn step_arpg(&mut self, dt: f32, input: ArpgInput) -> Option<String> {
        let dt = dt.clamp(0.001, 0.1);
        let mut status_out: Option<String> = None;

        // Tick transient visual effects.
        for fx in &mut self.arpg.fx {
            match fx {
                DelveFx::SlashArc { timer, .. } => *timer -= dt,
                DelveFx::Spark { x, y, vx, vy, timer, .. } => {
                    *x += *vx * dt;
                    *y += *vy * dt;
                    *timer -= dt;
                }
                DelveFx::FloatingText { y, timer, .. } => {
                    *y -= 1.2 * dt;
                    *timer -= dt;
                }
            }
        }
        self.arpg.fx.retain(|fx| match fx {
            DelveFx::SlashArc { timer, .. }
            | DelveFx::Spark { timer, .. }
            | DelveFx::FloatingText { timer, .. } => *timer > 0.0,
        });

        let Some(mut d) = self.arpg.delver.take() else {
            return None;
        };
        if !self.settlers.get(d.who).map_or(false, |s| s.alive) {
            return Some("The delver has fallen!".into());
        }

        // Update aim angle toward mouse cursor.
        let (mdx, mdy) = (input.aim_world.0 - d.x, input.aim_world.1 - d.y);
        if mdx.abs() > 0.001 || mdy.abs() > 0.001 {
            d.aim_angle = mdy.atan2(mdx);
        }

        // Cooldowns & stamina regeneration.
        d.dash_cd = (d.dash_cd - dt).max(0.0);
        d.swing_cd = (d.swing_cd - dt).max(0.0);
        d.swing_timer = (d.swing_timer - dt).max(0.0);
        d.throw_cd = (d.throw_cd - dt).max(0.0);
        d.hurt_cd = (d.hurt_cd - dt).max(0.0);
        for cd in &mut d.comp_attack_cd {
            *cd = (*cd - dt).max(0.0);
        }
        d.blocking = input.secondary_down && d.dash_timer <= 0.0 && d.swing_timer <= 0.0;
        let stam_rate = if d.blocking { 8.0 } else { 24.0 };
        d.stamina = (d.stamina + stam_rate * dt).min(d.max_stamina);

        // Passive healing near the surface campfire.
        let camp_z = self.camp_z();
        let dist_camp = ((d.x - self.camp.0 as f32).powi(2) + (d.y - self.camp.1 as f32).powi(2)).sqrt();
        if (d.z - camp_z).abs() <= 1 && dist_camp <= 5.0 {
            d.hp = (d.hp + 12.0 * dt).min(d.max_hp);
            // Unload pack at the campfire!
            if !d.pack.is_empty() {
                let msg = self.unload_delver_pack(&mut d);
                status_out = Some(msg);
            }
        }

        // Dodge-dash (`Space`).
        if input.dash_pressed && d.dash_cd <= 0.0 && d.stamina >= 25.0 {
            d.stamina -= 25.0;
            d.dash_timer = 0.18;
            d.dash_cd = 0.55;
            let len = (input.move_x * input.move_x + input.move_y * input.move_y).sqrt();
            d.dash_dir = if len > 0.1 {
                (input.move_x / len, input.move_y / len)
            } else {
                (d.aim_angle.cos(), d.aim_angle.sin())
            };
            for k in 0..5 {
                let a = d.aim_angle + (k as f32 - 2.0) * 0.25;
                self.arpg.fx.push(DelveFx::Spark {
                    x: d.x,
                    y: d.y,
                    vx: -a.cos() * 3.0,
                    vy: -a.sin() * 3.0,
                    z: d.z,
                    color: [220.0, 210.0, 185.0],
                    timer: 0.22,
                });
            }
        }

        // Movement speed (encumbrance slows you down slightly as pack fills).
        let max_pack = self.arpg.transport.delve_pack_slots(d.companions.len()).max(1);
        let enc_frac = (d.pack.len() as f32 / max_pack as f32).clamp(0.0, 1.2);
        let enc_mult = 1.0 - 0.22 * enc_frac;
        let (vx, vy) = if d.dash_timer > 0.0 {
            d.dash_timer = (d.dash_timer - dt).max(0.0);
            (d.dash_dir.0 * 14.5, d.dash_dir.1 * 14.5)
        } else {
            let len = (input.move_x * input.move_x + input.move_y * input.move_y).sqrt();
            let (mx, my) = if len > 1.0 {
                (input.move_x / len, input.move_y / len)
            } else {
                (input.move_x, input.move_y)
            };
            let base_spd = if d.blocking { 2.8 } else { 5.6 } * enc_mult;
            (mx * base_spd, my * base_spd)
        };
        d.vx = vx;
        d.vy = vy;

        // Apply smooth sub-cell movement with 3D step-up/step-down collision.
        self.move_delver_smooth(&mut d, vx * dt, vy * dt);

        // Stair climbing (`<` / `>`) or cutting a shaft straight down when pressing `>` underground.
        if input.stair_up_pressed || input.stair_down_pressed {
            let (cx, cy) = (d.x as usize, d.y as usize);
            if input.stair_up_pressed {
                // Check for stair overhead or adjacent stair.
                if let Some((sx, sy, sz)) = self.find_stair_near(cx, cy, d.z, true) {
                    d.x = sx as f32 + 0.5;
                    d.y = sy as f32 + 0.5;
                    d.z = sz;
                    status_out = Some(format!("Climbed up to level {} ({} from surface).", d.z, d.z - camp_z));
                } else if d.z < self.map.surface_z[cy * self.map.width + cx] {
                    // Cut an upward stair shaft through the rock overhead!
                    if (d.z as usize + 2) < self.map.depth {
                        let _ = self.dig_cell((cx as u16, cy as u16), d.z, true);
                        let _ = self.dig_cell((cx as u16, cy as u16), d.z + 1, true);
                        d.z += 1;
                        status_out = Some(format!("Cut a stair upward to level {}!", d.z));
                    }
                }
            } else if input.stair_down_pressed {
                if let Some((sx, sy, sz)) = self.find_stair_near(cx, cy, d.z, false) {
                    d.x = sx as f32 + 0.5;
                    d.y = sy as f32 + 0.5;
                    d.z = sz;
                    status_out = Some(format!("Descended to level {} ({} below camp).", d.z, camp_z - d.z));
                } else if d.z >= 3 {
                    // Cut a stair shaft straight down into the rock underfoot!
                    let _ = self.dig_cell((cx as u16, cy as u16), d.z, true);
                    if let Some(mat) = self.dig_cell((cx as u16, cy as u16), d.z - 1, true) {
                        self.spawn_dig_loot(cx as u16, cy as u16, d.z - 1, mat);
                    }
                    d.z -= 1;
                    status_out = Some(format!("Cut a stair shaft down to level {} ({} below camp)!", d.z, camp_z - d.z));
                }
            }
        }

        // Plant a wall torch (`F`).
        if input.torch_pressed {
            let p = (d.x as u16, d.y as u16);
            if !self.arpg.torches.contains(&(p, d.z)) {
                self.arpg.torches.push((p, d.z));
                status_out = Some(format!("Set a blazing torch at ({}, {}, z={}).", p.0, p.1, d.z));
            }
        }

        // Ranged javelin/stone throw (`G`).
        if input.throw_pressed && d.throw_cd <= 0.0 && d.stamina >= 15.0 {
            d.stamina -= 15.0;
            d.throw_cd = 0.65;
            let (_, w_dmg) = self.delver_weapon(d.who);
            self.arpg.projectiles.push(Projectile {
                x: d.x + d.aim_angle.cos() * 0.5,
                y: d.y + d.aim_angle.sin() * 0.5,
                z: d.z,
                vx: d.aim_angle.cos() * 13.0,
                vy: d.aim_angle.sin() * 13.0,
                damage: w_dmg * 0.75,
                timer: 0.9,
                from_player: true,
            });
        }

        // Left-Click: Weapon Slash (if enemies in arc or open floor) OR Pickaxe Wall Strike!
        if (input.primary_clicked || input.primary_down) && d.swing_cd <= 0.0 {
            d.swing_cd = 0.34;
            d.swing_timer = 0.18;
            d.swing_angle = d.aim_angle;
            if let Some(msg) = self.perform_delver_swing(&mut d) {
                status_out = Some(msg);
            }
        }

        // Pick up any floor loot drops within 1.15 cells if pack has space.
        let max_slots = self.arpg.transport.delve_pack_slots(d.companions.len());
        let mut picked_up = Vec::new();
        self.arpg.loot.retain(|drop| {
            if (drop.z - d.z).abs() <= 1 && ((drop.x - d.x).powi(2) + (drop.y - d.y).powi(2)).sqrt() <= 1.25 {
                if d.pack.len() < max_slots {
                    picked_up.push(drop.item.label());
                    d.pack.push(drop.item.clone());
                    return false;
                }
            }
            true
        });
        if !picked_up.is_empty() {
            let txt = format!("+{}", picked_up.join(", +"));
            self.arpg.fx.push(DelveFx::FloatingText {
                x: d.x,
                y: d.y - 0.6,
                z: d.z,
                text: txt.clone(),
                color: 0x00D8_A838,
                timer: 1.1,
            });
            status_out = Some(format!(
                "Picked up {} (pack: {}/{} slots — return to campfire to unload).",
                picked_up.join(", "),
                d.pack.len(),
                max_slots
            ));
        }

        // Sync controlled settler's discrete position & level, and update AI companions.
        self.settlers[d.who].pos = (
            (d.x as usize).min(self.map.width - 1) as u16,
            (d.y as usize).min(self.map.height - 1) as u16,
        );
        self.settlers[d.who].z = d.z;
        self.settlers[d.who].path.clear();
        self.settlers[d.who].stride = 0;

        self.step_companions(&mut d, dt);
        if let Some(msg) = self.step_projectiles_and_foes(&mut d, dt) {
            status_out = Some(msg);
        }

        // Check if the delver just discovered an underground place.
        let mut found_notes = Vec::new();
        for (k, pl) in self.map.places.iter_mut().enumerate() {
            if !self.places_found.contains(&k) {
                let near = pl.cells.iter().any(|&(c, z)| {
                    (z - d.z).abs() <= 1 && (c.0 as f32 + 0.5 - d.x).abs() <= 2.5 && (c.1 as f32 + 0.5 - d.y).abs() <= 2.5
                });
                if near {
                    pl.found = true;
                    self.places_found.push(k);
                    let line = format!(
                        "{} enters {}: {}!",
                        self.settlers[d.who].name, pl.name, pl.cause
                    );
                    found_notes.push(line);
                }
            }
        }
        for line in found_notes {
            self.note(line.clone());
            status_out = Some(line);
        }

        self.arpg.delver = Some(d);
        self.reveal_around_delver();
        status_out
    }

    /// Move the delver smoothly in `(dx, dy)` while handling 3D step-up/step-down and wall sliding.
    fn move_delver_smooth(&self, d: &mut Delver, dx: f32, dy: f32) {
        let try_pos = |nx: f32, ny: f32, cur_z: i32| -> Option<i32> {
            if nx < 2.0 || ny < 2.0 || nx >= (self.map.width - 2) as f32 || ny >= (self.map.height - 2) as f32 {
                return None;
            }
            let (cx, cy) = (nx as usize, ny as usize);
            // Same level first.
            if nav::standable(&self.map, cx, cy, cur_z) {
                return Some(cur_z);
            }
            // Step down 1 level (e.g., descending a cave tunnel, ramp, or stair).
            if cur_z >= 1 && nav::standable(&self.map, cx, cy, cur_z - 1) {
                return Some(cur_z - 1);
            }
            // Step up 1 level (e.g., climbing a cave step, ramp, or hill).
            if (cur_z as usize + 2) < self.map.depth && nav::standable(&self.map, cx, cy, cur_z + 1) {
                return Some(cur_z + 1);
            }
            // Surface fallback if walking on the surface.
            let sz = self.map.surface_z[cy * self.map.width + cx];
            if (sz - cur_z).abs() <= 2 && nav::standable(&self.map, cx, cy, sz) {
                return Some(sz);
            }
            None
        };

        if let Some(nz) = try_pos(d.x + dx, d.y + dy, d.z) {
            d.x += dx;
            d.y += dy;
            d.z = nz;
        } else if let Some(nz) = try_pos(d.x + dx, d.y, d.z) {
            d.x += dx;
            d.z = nz;
        } else if let Some(nz) = try_pos(d.x, d.y + dy, d.z) {
            d.y += dy;
            d.z = nz;
        }
    }

    fn find_stair_near(&self, cx: usize, cy: usize, z: i32, up: bool) -> Option<(usize, usize, i32)> {
        let target_z = if up { z + 1 } else { z - 1 };
        if target_z < 1 || (target_z as usize + 1) >= self.map.depth {
            return None;
        }
        for (dx, dy) in [(0i32, 0i32), (1, 0), (-1, 0), (0, 1), (0, -1)] {
            let (nx, ny) = (cx as i32 + dx, cy as i32 + dy);
            if nx < 1 || ny < 1 || nx as usize >= self.map.width - 1 || ny as usize >= self.map.height - 1 {
                continue;
            }
            let (ux, uy) = (nx as usize, ny as usize);
            let c_here = self.map.cell(ux, uy, z as usize);
            let c_above = self.map.cell(ux, uy, (z + 1) as usize);
            let c_target = self.map.cell(ux, uy, target_z as usize);
            if c_here.shape == Shape::Stair
                || c_above.shape == Shape::Stair
                || c_target.shape == Shape::Stair
                || nav::standable(&self.map, ux, uy, target_z)
            {
                if nav::standable(&self.map, ux, uy, target_z) {
                    return Some((ux, uy, target_z));
                }
            }
        }
        None
    }

    /// Execute a Left-Click swing: attacks foes in front of the delver, or strikes an adjacent rock/ore wall with a pickaxe.
    fn perform_delver_swing(&mut self, d: &mut Delver) -> Option<String> {
        let (_, base_dmg) = self.delver_weapon(d.who);
        let skill_mult = 1.0 + 0.5 * self.fight_skill(d.who);
        let dmg = base_dmg * skill_mult;

        // Check for foes within melee reach (2.35 cells) in a 130-degree forward arc.
        let mut hit_any = false;
        let mut slain_msg = None;
        let mut new_drops = Vec::new();
        for foe in &mut self.arpg.foes {
            if (foe.z - d.z).abs() > 1 {
                continue;
            }
            let (fdx, fdy) = (foe.x - d.x, foe.y - d.y);
            let dist = (fdx * fdx + fdy * fdy).sqrt();
            if dist > 2.35 + foe.size * 0.35 {
                continue;
            }
            let angle_to = fdy.atan2(fdx);
            let diff = angle_diff(angle_to, d.aim_angle).abs();
            if diff <= 1.15 || dist < 0.95 {
                hit_any = true;
                foe.hp -= dmg;
                foe.hit_flash = 0.2;
                foe.state = FoeState::Staggered(0.35);
                let inv = 1.0 / dist.max(0.1);
                foe.vx = fdx * inv * 6.5;
                foe.vy = fdy * inv * 6.5;
                self.arpg.fx.push(DelveFx::FloatingText {
                    x: foe.x,
                    y: foe.y - 0.5,
                    z: foe.z,
                    text: format!("-{:.0}", dmg),
                    color: 0x00C8_281E,
                    timer: 0.75,
                });
                for k in 0..6 {
                    let a = angle_to + (k as f32 - 2.5) * 0.3;
                    self.arpg.fx.push(DelveFx::Spark {
                        x: foe.x,
                        y: foe.y,
                        vx: a.cos() * 4.5,
                        vy: a.sin() * 4.5,
                        z: foe.z,
                        color: [200.0, 48.0, 36.0],
                        timer: 0.25,
                    });
                }
                if foe.hp <= 0.0 {
                    for item in foe.drops.drain(..) {
                        new_drops.push((foe.x, foe.y, foe.z, item));
                    }
                    slain_msg = Some((foe.name.clone(), foe.kind == FoeKind::LairBoss));
                }
            }
        }
        self.arpg.foes.retain(|f| f.hp > 0.0);
        for (lx, ly, lz, item) in new_drops {
            self.arpg.loot.push(LootDrop {
                x: lx + ((self.arpg.loot.len() % 3) as f32 - 1.0) * 0.22,
                y: ly + ((self.arpg.loot.len() / 3 % 3) as f32 - 1.0) * 0.22,
                z: lz,
                item,
            });
        }

        if hit_any {
            d.swing_pick = false;
            self.arpg.fx.push(DelveFx::SlashArc {
                x: d.x,
                y: d.y,
                z: d.z,
                angle: d.aim_angle,
                radius: 2.1,
                timer: 0.16,
                max_timer: 0.16,
                pick: false,
            });
            // Train combat drill on real hits!
            self.settlers[d.who].drill = (self.settlers[d.who].drill + 0.008).min(0.6);
            if let Some((slain, is_boss)) = slain_msg {
                let who_name = self.settlers[d.who].name.clone();
                let line = format!("{} slew {} in the dark!", who_name, slain);
                self.note(line.clone());
                if is_boss {
                    self.settlers[d.who].deeds.push(format!("slew {} in its lair on day {}", slain, self.clock.day()));
                }
                return Some(line);
            }
            return None;
        }

        // Otherwise, check if aiming at an adjacent rock/ore wall cell (`z + 1` body level) to strike with the pickaxe!
        let tx = (d.x + d.aim_angle.cos() * 1.15).floor() as i32;
        let ty = (d.y + d.aim_angle.sin() * 1.15).floor() as i32;
        if tx >= 2 && ty >= 2 && (tx as usize) < self.map.width - 2 && (ty as usize) < self.map.height - 2 && (d.z as usize + 1) < self.map.depth {
            let (ux, uy) = (tx as usize, ty as usize);
            let wall_cell = *self.map.cell(ux, uy, (d.z + 1) as usize);
            if wall_cell.shape == Shape::Wall && wall_cell.material != Material::Magma {
                d.swing_pick = true;
                self.arpg.fx.push(DelveFx::SlashArc {
                    x: d.x,
                    y: d.y,
                    z: d.z,
                    angle: d.aim_angle,
                    radius: 1.4,
                    timer: 0.14,
                    max_timer: 0.14,
                    pick: true,
                });
                let tool_mult = if self.iron_worked() {
                    1.6
                } else if self.industry.tools.is_some() {
                    1.3
                } else {
                    1.0
                };
                let hit_power = match wall_cell.material {
                    Material::Soil | Material::Clay | Material::Sand | Material::Gravel => 0.55,
                    Material::Ore(_) => 0.34,
                    Material::Rock(crate::erosion::materials::RockType::Limestone | crate::erosion::materials::RockType::Sandstone | crate::erosion::materials::RockType::Shale) => 0.38,
                    _ => 0.28,
                } * tool_mult;

                let key = (ux as u16, uy as u16, d.z);
                let cur = match self.arpg.wall_damage.iter_mut().find(|w| w.0 == key) {
                    Some(entry) => {
                        entry.1 += hit_power;
                        entry.1
                    }
                    None => {
                        self.arpg.wall_damage.push((key, hit_power));
                        hit_power
                    }
                };

                let spark_col = match wall_cell.material {
                    Material::Ore(o) => {
                        let c = crate::lore::resource_color(o);
                        [c[0] as f32, c[1] as f32, c[2] as f32]
                    }
                    _ => [190.0, 178.0, 152.0],
                };
                for k in 0..6 {
                    let a = d.aim_angle + std::f32::consts::PI + (k as f32 - 2.5) * 0.35;
                    self.arpg.fx.push(DelveFx::Spark {
                        x: ux as f32 + 0.5,
                        y: uy as f32 + 0.5,
                        vx: a.cos() * 3.8,
                        vy: a.sin() * 3.8,
                        z: d.z,
                        color: spark_col,
                        timer: 0.24,
                    });
                }

                if cur >= 1.0 {
                    self.arpg.wall_damage.retain(|w| w.0 != key);
                    if let Some(mat) = self.dig_cell((ux as u16, uy as u16), d.z, false) {
                        self.settlers[d.who].skill[2] = (self.settlers[d.who].skill[2] + 0.01).min(1.0);
                        return self.spawn_dig_loot(ux as u16, uy as u16, d.z, mat);
                    }
                }
                return None;
            }
        }

        // Empty swing arc in the air.
        d.swing_pick = false;
        self.arpg.fx.push(DelveFx::SlashArc {
            x: d.x,
            y: d.y,
            z: d.z,
            angle: d.aim_angle,
            radius: 2.0,
            timer: 0.15,
            max_timer: 0.15,
            pick: false,
        });
        None
    }

    fn spawn_dig_loot(&mut self, ux: u16, uy: u16, z: i32, mat: Material) -> Option<String> {
        let (lx, ly) = (ux as f32 + 0.5, uy as f32 + 0.5);
        if let Material::Ore(r) = mat {
            let ore_name = format!("{:?}", r).to_lowercase();
            self.arpg.loot.push(LootDrop {
                x: lx,
                y: ly,
                z,
                item: PackItem::Ore(ore_name.clone()),
            });
            return Some(format!("Struck a vein of {} ore!", ore_name));
        }
        // Chance of prising a gem or ore pocket from deep rock.
        let h = crate::history::settlers::hash_pub(self.seed ^ ((ux as u64) << 16) ^ ((uy as u64) << 4) ^ (z as u64), 0x6E35);
        if h % 7 == 0 {
            let gems = ["obsidian", "emerald", "ruby", "sapphire", "diamond"];
            let g = gems[(h as usize / 7) % gems.len()].to_string();
            self.arpg.loot.push(LootDrop {
                x: lx,
                y: ly,
                z,
                item: PackItem::Gem(g.clone()),
            });
            return Some(format!("Prised an uncut {} from the rock!", g));
        } else if h % 11 == 0 {
            self.arpg.loot.push(LootDrop {
                x: lx,
                y: ly,
                z,
                item: PackItem::Ore("iron".into()),
            });
            return Some("Uncovered a pocket of iron ore in the rock!".into());
        }
        None
    }

    /// Update companion positions and let them help strike nearby foes.
    fn step_companions(&mut self, d: &mut Delver, dt: f32) {
        let back_angle = d.aim_angle + std::f32::consts::PI;
        for (idx, &c) in d.companions.iter().enumerate() {
            if !self.settlers.get(c).map_or(false, |s| s.alive) {
                continue;
            }
            let side = if idx == 0 { 0.65 } else { -0.65 };
            let target_x = d.x + (back_angle + side).cos() * 1.45;
            let target_y = d.y + (back_angle + side).sin() * 1.45;
            let cur = self.settlers[c].pos;
            let (mut cx, mut cy) = (cur.0 as f32 + 0.5, cur.1 as f32 + 0.5);
            let dist = ((target_x - cx).powi(2) + (target_y - cy).powi(2)).sqrt();
            if dist > 6.0 || (self.settlers[c].z - d.z).abs() > 1 {
                cx = target_x;
                cy = target_y;
            } else if dist > 0.6 {
                let spd = 5.0 * dt;
                cx += (target_x - cx) / dist * spd.min(dist);
                cy += (target_y - cy) / dist * spd.min(dist);
            }
            let (ux, uy) = (
                (cx as usize).clamp(1, self.map.width - 2),
                (cy as usize).clamp(1, self.map.height - 2),
            );
            if nav::standable(&self.map, ux, uy, d.z) {
                self.settlers[c].pos = (ux as u16, uy as u16);
                self.settlers[c].z = d.z;
            }
            // Companion attacks nearby foes when cooldown is ready.
            if idx < 2 && d.comp_attack_cd[idx] <= 0.0 {
                if let Some(foe) = self.arpg.foes.iter_mut().find(|f| {
                    (f.z - d.z).abs() <= 1 && ((f.x - cx).powi(2) + (f.y - cy).powi(2)).sqrt() <= 2.1
                }) {
                    d.comp_attack_cd[idx] = 0.85;
                    let dmg = 14.0;
                    foe.hp -= dmg;
                    foe.hit_flash = 0.15;
                    self.arpg.fx.push(DelveFx::FloatingText {
                        x: foe.x,
                        y: foe.y - 0.4,
                        z: foe.z,
                        text: format!("-{:.0}", dmg),
                        color: 0x00B8_6028,
                        timer: 0.6,
                    });
                }
            }
        }
    }

    /// Update projectiles and enemy AI (chase, telegraphed wind-up, lunge, shield parry, and drops).
    fn step_projectiles_and_foes(&mut self, d: &mut Delver, dt: f32) -> Option<String> {
        let mut status_out = None;
        let mut new_drops = Vec::new();
        let mut notes = Vec::new();
        let camp_z = self.camp_z();

        // Update projectiles.
        let mut kept_proj = Vec::new();
        for mut p in self.arpg.projectiles.drain(..) {
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.timer -= dt;
            if p.timer <= 0.0 {
                continue;
            }
            let (ux, uy) = (p.x as usize, p.y as usize);
            if ux >= self.map.width || uy >= self.map.height || !nav::standable(&self.map, ux, uy, p.z) {
                continue;
            }
            if p.from_player {
                let mut hit = false;
                for foe in &mut self.arpg.foes {
                    if (foe.z - p.z).abs() <= 1 && ((foe.x - p.x).powi(2) + (foe.y - p.y).powi(2)).sqrt() <= 0.85 + foe.size * 0.3 {
                        foe.hp -= p.damage;
                        foe.hit_flash = 0.2;
                        foe.state = FoeState::Staggered(0.25);
                        self.arpg.fx.push(DelveFx::FloatingText {
                            x: foe.x,
                            y: foe.y - 0.5,
                            z: foe.z,
                            text: format!("-{:.0}", p.damage),
                            color: 0x00C8_281E,
                            timer: 0.7,
                        });
                        if foe.hp <= 0.0 {
                            for item in foe.drops.drain(..) {
                                new_drops.push((foe.x, foe.y, foe.z, item));
                            }
                            status_out = Some(format!("{} felled {} with a thrown shaft!", self.settlers[d.who].name, foe.name));
                        }
                        hit = true;
                        break;
                    }
                }
                if !hit {
                    kept_proj.push(p);
                }
            }
        }
        self.arpg.projectiles = kept_proj;
        self.arpg.foes.retain(|f| f.hp > 0.0);

        // Update foes.
        let armor_mitigation = if self.armour_of(d.who).is_some() { 0.72 } else { 1.0 };
        for foe in &mut self.arpg.foes {
            foe.hit_flash = (foe.hit_flash - dt).max(0.0);
            foe.attack_cd = (foe.attack_cd - dt).max(0.0);

            // Apply knockback velocity decay.
            if foe.vx.abs() > 0.05 || foe.vy.abs() > 0.05 {
                let nx = foe.x + foe.vx * dt;
                let ny = foe.y + foe.vy * dt;
                if nx >= 1.0 && ny >= 1.0 && (nx as usize) < self.map.width - 1 && (ny as usize) < self.map.height - 1 && nav::standable(&self.map, nx as usize, ny as usize, foe.z) {
                    foe.x = nx;
                    foe.y = ny;
                }
                foe.vx *= (1.0 - 10.0 * dt).max(0.0);
                foe.vy *= (1.0 - 10.0 * dt).max(0.0);
            }

            if (foe.z - d.z).abs() > 1 {
                continue;
            }
            let (dx, dy) = (d.x - foe.x, d.y - foe.y);
            let dist = (dx * dx + dy * dy).sqrt();

            match &mut foe.state {
                FoeState::Staggered(t) => {
                    *t -= dt;
                    if *t <= 0.0 {
                        foe.state = FoeState::Chase;
                    }
                }
                FoeState::Idle => {
                    if dist <= 8.5 {
                        foe.state = FoeState::Chase;
                    }
                }
                FoeState::Chase => {
                    if dist > 14.0 {
                        foe.state = FoeState::Idle;
                    } else if dist <= 1.85 + foe.size * 0.25 && foe.attack_cd <= 0.0 {
                        let inv = 1.0 / dist.max(0.1);
                        foe.state = FoeState::Windup {
                            timer: 0.42,
                            dx: dx * inv,
                            dy: dy * inv,
                        };
                    } else if dist > 0.9 {
                        let inv = 1.0 / dist.max(0.1);
                        let (mx, my) = (dx * inv * foe.speed * dt, dy * inv * foe.speed * dt);
                        let (nx, ny) = (foe.x + mx, foe.y + my);
                        if nav::standable(&self.map, nx as usize, ny as usize, foe.z) {
                            foe.x = nx;
                            foe.y = ny;
                        }
                    }
                }
                FoeState::Windup { timer, dx: wdx, dy: wdy } => {
                    *timer -= dt;
                    if *timer <= 0.0 {
                        let (ldx, ldy) = (*wdx, *wdy);
                        foe.state = FoeState::Lunge {
                            timer: 0.22,
                            dx: ldx,
                            dy: ldy,
                        };
                    }
                }
                FoeState::Lunge { timer, dx: ldx, dy: ldy } => {
                    let (ldx_v, ldy_v) = (*ldx, *ldy);
                    *timer -= dt;
                    let rem_timer = *timer;
                    let (nx, ny) = (foe.x + ldx_v * foe.speed * 2.5 * dt, foe.y + ldy_v * foe.speed * 2.5 * dt);
                    if nav::standable(&self.map, nx as usize, ny as usize, foe.z) {
                        foe.x = nx;
                        foe.y = ny;
                    }
                    let hit_dist = ((d.x - foe.x).powi(2) + (d.y - foe.y).powi(2)).sqrt();
                    if hit_dist <= 1.15 + foe.size * 0.25 && d.dash_timer <= 0.0 && d.hurt_cd <= 0.0 {
                        d.hurt_cd = 0.45;
                        foe.attack_cd = 1.1;
                        if d.blocking && d.stamina >= 12.0 {
                            // Shield parry! Staggers the attacker and reduces damage by 80%.
                            d.stamina -= 12.0;
                            let chip = (foe.damage * 0.2 * armor_mitigation).max(1.0);
                            d.hp -= chip;
                            foe.state = FoeState::Staggered(0.85);
                            foe.vx = -ldx_v * 5.0;
                            foe.vy = -ldy_v * 5.0;
                            self.arpg.fx.push(DelveFx::FloatingText {
                                x: d.x,
                                y: d.y - 0.6,
                                z: d.z,
                                text: "PARRIED!".into(),
                                color: 0x0040_88B8,
                                timer: 0.75,
                            });
                            continue;
                        } else {
                            let taken = foe.damage * armor_mitigation;
                            d.hp -= taken;
                            self.arpg.fx.push(DelveFx::FloatingText {
                                x: d.x,
                                y: d.y - 0.6,
                                z: d.z,
                                text: format!("-{:.0}", taken),
                                color: 0x00D0_2020,
                                timer: 0.8,
                            });
                            if d.hp <= 0.0 {
                                // Emergency retreat to the campfire instead of instant permadeath!
                                d.hp = d.max_hp * 0.35;
                                d.x = self.camp.0 as f32 + 0.5;
                                d.y = self.camp.1 as f32 + 0.5;
                                d.z = camp_z;
                                // Drop half of pack where they fell.
                                let drop_n = d.pack.len() / 2;
                                for _ in 0..drop_n {
                                    if let Some(it) = d.pack.pop() {
                                        new_drops.push((foe.x, foe.y, foe.z, it));
                                    }
                                }
                                let msg = format!(
                                    "{} was overwhelmed by {} and dragged back to the campfire by their companions!",
                                    self.settlers[d.who].name, foe.name
                                );
                                notes.push(msg.clone());
                                status_out = Some(msg);
                            }
                        }
                    }
                    if rem_timer <= 0.0 {
                        foe.attack_cd = 0.9;
                        foe.state = FoeState::Chase;
                    }
                }
            }
        }
        for msg in notes {
            self.note(msg);
        }

        for (lx, ly, lz, item) in new_drops {
            self.arpg.loot.push(LootDrop { x: lx, y: ly, z: lz, item });
        }
        status_out
    }

    /// Reveal underground cells around the delver (ambient radius + directional torch cone + placed wall torches).
    pub fn reveal_around_delver(&mut self) {
        let Some(d) = &self.arpg.delver else { return };
        let (dx, dy, dz) = (d.x, d.y, d.z);
        let aim = d.aim_angle;

        let mut to_reveal: Vec<((u16, u16), i32)> = Vec::new();
        for oy in -8i32..=8 {
            for ox in -8i32..=8 {
                let (cx, cy) = (dx as i32 + ox, dy as i32 + oy);
                if cx < 0 || cy < 0 || cx as usize >= self.map.width || cy as usize >= self.map.height {
                    continue;
                }
                let dist = ((ox * ox + oy * oy) as f32).sqrt();
                let in_ambient = dist <= 4.2;
                let angle = (oy as f32).atan2(ox as f32);
                let in_cone = dist <= 8.2 && angle_diff(angle, aim).abs() <= 0.95;
                if in_ambient || in_cone {
                    let p = (cx as u16, cy as u16);
                    for z_off in -1..=1 {
                        to_reveal.push((p, dz + z_off));
                    }
                }
            }
        }
        for &(tp, tz) in &self.arpg.torches {
            if (tz - dz).abs() <= 1 {
                for oy in -4i32..=4 {
                    for ox in -4i32..=4 {
                        if ox * ox + oy * oy <= 18 {
                            let p = (
                                (tp.0 as i32 + ox).max(0) as u16,
                                (tp.1 as i32 + oy).max(0) as u16,
                            );
                            to_reveal.push((p, tz));
                        }
                    }
                }
            }
        }
        for entry in to_reveal {
            if !self.revealed.contains(&entry) {
                self.revealed.push(entry);
            }
        }
    }

    /// Unload everything in the delver's pack at the surface campfire stockpile.
    fn unload_delver_pack(&mut self, d: &mut Delver) -> String {
        let mut counts: Vec<String> = Vec::new();
        let who_name = self.settlers[d.who].name.clone();
        for item in d.pack.drain(..) {
            counts.push(item.label());
            match item {
                PackItem::Ore(kind) => {
                    self.add_ore(&kind, 2);
                    self.ore_found += 2;
                }
                PackItem::Gem(g) => {
                    match self.gems.iter_mut().find(|x| x.0 == g) {
                        Some(x) => x.1 += 1,
                        None => self.gems.push((g, 1)),
                    }
                }
                PackItem::Bars(m, n) => {
                    match self.industry.bars.iter_mut().find(|x| x.0 == m) {
                        Some(x) => x.1 += n,
                        None => self.industry.bars.push((m, n)),
                    }
                }
                PackItem::Relic(title) => {
                    if !self.treasures.contains(&title) {
                        self.treasures.push(title.clone());
                    }
                    self.settlers[d.who]
                        .deeds
                        .push(format!("brought up {} from the dark on day {}", title, self.clock.day()));
                }
                PackItem::Weapon { kind, material, quality } => {
                    self.arms.push(militia::Arm {
                        kind,
                        material,
                        quality,
                        maker: d.who,
                        day: self.clock.day(),
                        holder: Some(d.who),
                    });
                    self.arm_militia();
                }
                PackItem::Provisions(n) => {
                    for _ in 0..n {
                        self.items.push(Item::food(Stuff::Provisions, self.camp, true));
                    }
                }
                PackItem::Hide { beast, pieces } => {
                    self.remains.push((beast.clone(), beast, 2, pieces, "hide".into()));
                }
                PackItem::Blueprint(tier) => {
                    if !self.arpg.unlocked_tiers.contains(&tier) {
                        self.arpg.unlocked_tiers.push(tier);
                    }
                    if tier > self.arpg.transport {
                        self.arpg.transport = tier;
                    }
                }
                PackItem::Tablet { title, tile, note } => {
                    if !self.arpg.region_clues.iter().any(|c| c.0 == tile) {
                        self.arpg.region_clues.push((tile, format!("{}: {}", title, note)));
                    }
                }
            }
        }
        let line = format!(
            "{} unloads at the campfire: {}!",
            who_name,
            counts.join(", ")
        );
        self.note(line.clone());
        self.arpg.fx.push(DelveFx::FloatingText {
            x: d.x,
            y: d.y - 0.8,
            z: d.z,
            text: "UNLOADED AT CAMP!".into(),
            color: 0x0038_8E3C,
            timer: 1.4,
        });
        line
    }

    /// Compute the technology-limited Strike Camp manifest showing what fits in `max_camp_carry_slots()`
    /// and what heavy raw materials will be left behind unless refined first.
    pub fn strike_manifest(&self) -> StrikeManifest {
        let slots_max = self.max_camp_carry_slots();
        let mut slots_used = 0usize;
        let mut rows = Vec::new();
        let mut left_behind = Vec::new();

        let mut add_group = |label: String, detail: String, need_slots: usize| {
            if need_slots == 0 {
                return;
            }
            if slots_used + need_slots <= slots_max {
                slots_used += need_slots;
                rows.push((label, detail, need_slots, true));
            } else {
                let can_take = slots_max.saturating_sub(slots_used);
                if can_take > 0 {
                    slots_used += can_take;
                    rows.push((
                        format!("{} (partial)", label),
                        detail.clone(),
                        can_take,
                         true,
                    ));
                }
                let dropped = need_slots - can_take;
                rows.push((label.clone(), detail, dropped, false));
                left_behind.push(format!("{} ({} slots over capacity)", label, dropped));
            }
        };

        // 1. Weapons & Armour (lightweight: 2 items per slot).
        let gear_n = self.arms.len() + self.armour.len();
        if gear_n > 0 {
            add_group(
                "Forged Arms & Mail".into(),
                format!("{} weapons, {} armour", self.arms.len(), self.armour.len()),
                gear_n.div_ceil(2),
            );
        }

        // 2. Forged Tools (1 slot).
        if let Some((m, _)) = &self.industry.tools {
            add_group("Smith's & Miner's Tools".into(), format!("{} tools", m), 1);
        }

        // 3. Relics, Treasures & Works (1 slot per 2).
        let rel_n = self.treasures.len() + self.works.iter().filter(|w| !w.traded).count();
        if rel_n > 0 {
            add_group(
                "Relics & Crafted Works".into(),
                format!("{} treasures/works", rel_n),
                rel_n.div_ceil(2),
            );
        }

        // 4. Refined Metal Bars & Cut Gems (compact: 2 bars/gems per slot).
        let bars_n: u32 = self.industry.bars.iter().map(|b| b.1).sum();
        let gems_n: u32 = self.gems.iter().map(|g| g.1).sum();
        if bars_n + gems_n > 0 {
            add_group(
                "Refined Bars & Gems".into(),
                format!("{} metal bars, {} gems", bars_n, gems_n),
                ((bars_n + gems_n) as usize).div_ceil(2),
            );
        }

        // 5. Dried Food Rations (4 meals per slot).
        let food = self.food_stored() as usize;
        if food > 0 {
            add_group(
                "Marching Rations".into(),
                format!("{} meals", food),
                food.div_ceil(4),
            );
        }

        // 6. Heavy Unsmelted Ore (1 full slot per load! Smelt into bars first to save space).
        let ore_n = self.ore_total() as usize;
        if ore_n > 0 {
            add_group(
                "Raw Unsmelted Ore (Heavy!)".into(),
                format!("{} loads (smelt into bars to halve weight)", ore_n),
                ore_n,
            );
        }

        // 7. Raw Logs & Rough Stone (1 full slot each — lowest priority).
        let logs = self.logs_stored() as usize;
        let stones = self.items.iter().filter(|it| it.stored && it.kind == ItemKind::Stone).count();
        if logs + stones > 0 {
            add_group(
                "Raw Timber & Stone (Bulky)".into(),
                format!("{} logs, {} stones", logs, stones),
                logs + stones,
            );
        }

        StrikeManifest {
            transport: self.arpg.transport,
            slots_used,
            slots_max,
            rows,
            left_behind,
        }
    }

    /// Pack the company into a `PackedCompany` within `max_camp_carry_slots()` to travel across the Region map.
    pub fn pack_company(&mut self) -> PackedCompany {
        // First unload any active delver's pack so nothing on their back is lost.
        if let Some(mut d) = self.arpg.delver.take() {
            if !d.pack.is_empty() {
                let _ = self.unload_delver_pack(&mut d);
            }
        }
        self.arpg.strike_camp_open = false;

        let slots_max = self.max_camp_carry_slots();
        let mut rem = slots_max;

        // 1. Arms & Armour (2 per slot).
        let arms_cap = rem * 2;
        let arms: Vec<militia::Arm> = self.arms.iter().take(arms_cap).cloned().collect();
        rem = rem.saturating_sub(arms.len().div_ceil(2));
        let armour_cap = rem * 2;
        let armour: Vec<armour::Armour> = self.armour.iter().take(armour_cap).cloned().collect();
        rem = rem.saturating_sub(armour.len().div_ceil(2));

        // 2. Tools (1 slot).
        let tools = if rem >= 1 && self.industry.tools.is_some() {
            rem -= 1;
            self.industry.tools.clone()
        } else {
            None
        };

        // 3. Treasures & Works (2 per slot).
        let tr_cap = rem * 2;
        let treasures: Vec<String> = self.treasures.iter().take(tr_cap).cloned().collect();
        rem = rem.saturating_sub(treasures.len().div_ceil(2));
        let wk_cap = rem * 2;
        let works: Vec<craft::Work> = self.works.iter().filter(|w| !w.traded).take(wk_cap).cloned().collect();
        rem = rem.saturating_sub(works.len().div_ceil(2));

        // 4. Bars & Gems (2 per slot).
        let mut bars = Vec::new();
        let mut bar_items = rem * 2;
        for (m, n) in &self.industry.bars {
            let take = (*n as usize).min(bar_items);
            if take > 0 {
                bars.push((m.clone(), take as u32));
                bar_items -= take;
            }
        }
        let total_bars: u32 = bars.iter().map(|b| b.1).sum();
        rem = rem.saturating_sub((total_bars as usize).div_ceil(2));

        let mut gems = Vec::new();
        let mut gem_items = rem * 2;
        for (g, n) in &self.gems {
            let take = (*n as usize).min(gem_items);
            if take > 0 {
                gems.push((g.clone(), take as u32));
                gem_items -= take;
            }
        }
        let total_gems: u32 = gems.iter().map(|g| g.1).sum();
        rem = rem.saturating_sub((total_gems as usize).div_ceil(2));

        // 5. Food Rations (4 meals per slot, minimum 2 per settler).
        let stored_food = self.food_stored() as usize;
        let food_slots = rem.min(stored_food.div_ceil(4));
        let rations = (food_slots * 4).min(stored_food).max(self.alive() * 2) as u32;
        rem = rem.saturating_sub(food_slots);

        let mut take_slots = |cost_per_item: usize, avail: usize| -> usize {
            if rem == 0 || avail == 0 {
                return 0;
            }
            if cost_per_item == 0 {
                return avail;
            }
            let max_items = rem / cost_per_item;
            let taken = avail.min(max_items);
            rem = rem.saturating_sub(taken * cost_per_item);
            taken
        };

        // 6. Raw Ore (1 slot per load).
        let mut ore = Vec::new();
        for (k, n) in &self.industry.ore {
            let take = take_slots(1, *n as usize);
            if take > 0 {
                ore.push((k.clone(), take as u32));
            }
        }

        // 7. Logs & Stones (1 slot each).
        let logs = take_slots(1, self.logs_stored() as usize) as u32;
        let stored_stones = self.items.iter().filter(|it| it.stored && it.kind == ItemKind::Stone).count();
        let stones = take_slots(1, stored_stones) as u32;

        let slots_used = slots_max - rem;
        let living_settlers: Vec<Settler> = self
            .settlers
            .iter()
            .filter(|s| s.alive && s.guest_until == 0)
            .cloned()
            .collect();

        let mut summary_lines = vec![
            format!("{} delvers with {}", living_settlers.len(), self.arpg.transport.name()),
            format!("Carry slots packed: {} / {}", slots_used, slots_max),
        ];
        if !arms.is_empty() || !armour.is_empty() {
            summary_lines.push(format!("{} arms, {} armour", arms.len(), armour.len()));
        }
        if total_bars > 0 || total_gems > 0 {
            summary_lines.push(format!("{} metal bars, {} gems", total_bars, total_gems));
        }
        summary_lines.push(format!("{} marching rations, {} logs, {} stones", rations, logs, stones));

        PackedCompany {
            name: self.name.clone(),
            day: self.clock.day(),
            settlers: living_settlers,
            transport: self.arpg.transport,
            unlocked_tiers: self.arpg.unlocked_tiers.clone(),
            arms,
            armour,
            tools,
            smith_skills: self.industry.smith.clone(),
            bars,
            ore,
            gems,
            treasures,
            works,
            rations,
            logs,
            stones,
            region_clues: self.arpg.region_clues.clone(),
            slots_used,
            slots_max,
            summary_lines,
        }
    }

    /// Unpack a `PackedCompany` onto this newly founded `Colony` at a new Region site!
    pub fn unpack_company(&mut self, packed: PackedCompany) {
        self.name = packed.name;
        self.clock.tick = (packed.day.saturating_sub(1) + 1) * TICKS_PER_DAY + 6 * 60;
        let camp = self.camp;
        let camp_z = self.camp_z();

        let mut new_settlers = packed.settlers;
        for (i, s) in new_settlers.iter_mut().enumerate() {
            let p = (camp.0 + (i as u16 % 3), camp.1 + (i as u16 / 3) % 3);
            s.pos = if nav::passable(&self.map, p) { p } else { camp };
            s.z = camp_z;
            s.path.clear();
            s.job = Job::Idle;
            s.work_left = 0;
            s.carrying = None;
            s.why = "Pitched a new camp after the march".into();
        }
        if !new_settlers.is_empty() {
            self.settlers = new_settlers;
        }

        self.items.clear();
        for _ in 0..packed.rations {
            self.items.push(Item::food(Stuff::Provisions, camp, true));
        }
        for _ in 0..packed.logs {
            self.items.push(Item::new(ItemKind::Log, camp, true));
        }
        for _ in 0..packed.stones {
            self.items.push(Item::new(ItemKind::Stone, camp, true));
        }

        self.arpg.transport = packed.transport;
        self.arpg.unlocked_tiers = packed.unlocked_tiers;
        self.arpg.region_clues = packed.region_clues;
        self.arms = packed.arms;
        self.armour = packed.armour;
        self.industry.tools = packed.tools;
        self.industry.smith = packed.smith_skills;
        self.industry.bars = packed.bars;
        self.industry.ore = packed.ore;
        for (k, _) in &self.industry.ore {
            if !self.ores.contains(k) {
                self.ores.push(k.clone());
            }
        }
        self.gems = packed.gems;
        self.treasures = packed.treasures;
        self.works = packed.works;
        self.arm_militia();
        self.ensure_arpg_places();

        self.note(format!(
            "The company pitches a new camp at {},{} with {} ({} of {} carry slots brought).",
            camp.0, camp.1, self.arpg.transport.name(), packed.slots_used, packed.slots_max
        ));
    }
}

fn angle_diff(a: f32, b: f32) -> f32 {
    let mut d = (a - b) % std::f32::consts::TAU;
    if d > std::f32::consts::PI {
        d -= std::f32::consts::TAU;
    }
    if d < -std::f32::consts::PI {
        d += std::f32::consts::TAU;
    }
    d
}

fn compass_dir(from: (i32, i32), to: (i32, i32)) -> &'static str {
    let (dx, dy) = (to.0 - from.0, to.1 - from.1);
    if dy.abs() >= dx.abs() * 2 {
        if dy < 0 { "north" } else { "south" }
    } else if dx.abs() >= dy.abs() * 2 {
        if dx < 0 { "west" } else { "east" }
    } else if dy < 0 {
        if dx < 0 { "north-west" } else { "north-east" }
    } else if dx < 0 {
        "south-west"
    } else {
        "south-east"
    }
}
