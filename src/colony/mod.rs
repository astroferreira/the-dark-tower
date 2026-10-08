//! The first colony: seven settlers living on a playable area with no orders from anyone.
//!
//! The thinnest slice of roadmap Update 3. A clock (one tick = one game minute), three needs
//! (hunger, rest, shelter), and a handful of jobs each settler picks for itself by utility:
//! eat, sleep, forage shrubs, fish the river, fell trees, haul what lies about to the camp, and
//! build a hut once there are logs. Every settler keeps one line saying why it is doing what it
//! is doing, and the colony keeps a log of the moments that matter (the first meal from the
//! river, the hut's last wall, a settler going hungry). Deterministic: a ChaCha stream seeded
//! from the world and the site. Built and judged headless with `--sim-snapshot`.

pub mod nav;
pub mod arc;
pub mod projects;
pub mod creatures;
pub mod dig;
pub mod mind;
pub mod mine;
pub mod fight;
pub mod craft;
pub mod trade;
pub mod society;
pub mod dead;
pub mod mood;
pub mod curse;
pub mod relic;
pub mod militia;
pub mod visitors;
pub mod engrave;
pub mod night;
pub mod pets;
pub mod heal;
pub mod family;
pub mod legend;
pub mod ghosts;
pub mod expedition;
pub mod drink;
pub mod traps;
pub mod ageing;
pub mod nobles;
pub mod cavefarm;
pub mod temper;
pub mod tavern;
pub mod books;
pub mod livestock;
pub mod deep;
pub mod explore;
pub mod prisoners;
pub mod regard;
pub mod armour;
pub mod snatch;
pub mod siege;
pub mod weather;
pub mod guild;
pub mod rising;
pub mod liaison;
pub mod recognize;
pub mod respond;
pub mod thieves;
pub mod priest;
pub mod warcall;
pub mod regrow;
pub mod tithe;
pub mod vow;
pub mod kitchen;
pub mod clothes;
pub mod dreams;
pub mod childhood;
pub mod rations;
pub mod delve;
pub mod annals;
pub mod justice;
pub mod cavelife;
pub mod news;
pub mod industry;
pub mod needs;
pub mod voices;
pub mod haunts;
pub mod talk;

use rand::{Rng, SeedableRng};
use rand_chacha::ChaCha8Rng;
use crate::local::{LocalMap, Material, Plant, RoofPlan, Shape};
use nav::Pos;

pub const TICKS_PER_DAY: u64 = 1440;
/// Logs a hut takes.
pub const HUT_LOGS: u32 = 24;
/// Hut footprint (cells, 2 m each): walls on the ring, a door on the south side.
const HUT_W: usize = 6;
const HUT_H: usize = 5;
/// Food the settlers try to keep at the camp, per settler.
const FOOD_PER_SETTLER: u32 = 6;
/// Days a foraged shrub takes to bear again.
const SHRUB_REGROW_DAYS: u64 = 12;
/// How far (cells) settlers look for work.
const WORK_RADIUS: i32 = 60;
/// Side of the squares `nearest_ripe_shrub` files the shrubs in.
const SHRUB_BUCKET: usize = 16;
/// Days in a season of the colony's year (it begins in spring).
pub const SEASON_DAYS: u64 = 30;
/// How far a camp can live on what grows: a forager's round trip there and back is ~4 hours.
pub const FORAGE_RADIUS: i32 = 25;
/// Walking: stride gained a tick against 10 for a plain cell (2 = a cell every 5 minutes).
pub const WALK_PER_TICK: i32 = 2;
/// Hunger a meal takes away (a settler eats about 1.7 meals a day).
const MEAL: f32 = 0.8;
/// Path search budget (nodes).
pub(crate) const PATH_BUDGET: usize = 40_000;

/// Game time.
#[derive(Clone, Copy, Debug, Default)]
pub struct Clock { pub tick: u64 }

impl Clock {
    pub fn day(&self) -> u64 { self.tick / TICKS_PER_DAY + 1 }
    pub fn hour(&self) -> u64 { (self.tick % TICKS_PER_DAY) / 60 }
    pub fn minute(&self) -> u64 { self.tick % 60 }
    pub fn is_night(&self) -> bool { let h = self.hour(); h < 6 || h >= 21 }
    pub fn stamp(&self) -> String { format!("Day {}, {:02}:{:02}", self.day(), self.hour(), self.minute()) }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ItemKind { Log, Food, Stone }

#[derive(Clone, Debug)]
pub struct Item { pub kind: ItemKind, pub at: Pos, pub stored: bool, pub reserved: bool }

/// What a settler is doing.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Job {
    Idle,
    Eat,
    Sleep,
    Forage(Pos),
    Fish(Pos),
    Fell(Pos),
    Haul(usize),
    Build,
    Wander(Pos),
    /// Break stone from a boulder or bare rock (where there is no timber).
    Quarry(Pos),
    /// Dig the cell at the position down to the level (a hall's or a cellar's).
    Dig(Pos, i32),
    /// Stalk the game animal with this id.
    Hunt(u32),
    /// Make something at the workshop (`craft.rs`).
    Craft,
}

impl Job {
    pub fn verb(&self) -> &'static str {
        match self {
            Job::Idle => "idle", Job::Eat => "eating", Job::Sleep => "sleeping", Job::Forage(_) => "foraging",
            Job::Fish(_) => "fishing", Job::Fell(_) => "felling a tree", Job::Haul(_) => "hauling",
            Job::Build => "building", Job::Wander(_) => "wandering", Job::Quarry(_) => "quarrying stone", Job::Dig(..) => "digging", Job::Hunt(_) => "hunting", Job::Craft => "crafting",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Settler {
    pub name: String,
    pub pos: Pos,
    /// The level they stand on (`nav::P3`): the surface's, or a hall's, a stair's, a cavern's.
    pub z: i32,
    pub path: Vec<nav::P3>,
    /// 0 = fed, 1 = starving.
    pub hunger: f32,
    /// 0 = rested, 1 = exhausted.
    pub fatigue: f32,
    /// 0 = sheltered, 1 = chilled through (nights in the open).
    pub exposure: f32,
    pub job: Job,
    /// Minutes of work left once at the job's place.
    pub work_left: u32,
    pub carrying: Option<ItemKind>,
    /// Why this settler is doing what it is doing, in one line.
    pub why: String,
    pub alive: bool,
    /// Ticks spent wanting to move but unable to.
    pub stuck: u32,
    /// After a way that could not be found, the tick to think again (a settler cut off had
    /// searched the whole map every minute).
    pub(crate) retry_at: u64,
    /// Ticks spent starving (hunger at 1).
    pub starving: u32,
    /// Liking for each kind of work (forage, fish, fell, haul, build), 0.75-1.3: settlers
    /// differ, so seven of them don't all do the same thing at once.
    pub taste: [f32; 5],
    /// Who they were before the colony (from the history; None for nameless wanderers).
    pub past: Option<crate::history::settlers::Past>,
    /// Ill from the cold until this tick (0 = well).
    pub ill_until: u64,
    /// Progress toward the next cell of the path (a plain cell costs 10, `WALK_PER_TICK` a tick).
    pub stride: i32,
    /// Skill at each kind of work (forage, fish, fell, haul, build), 0 green .. 1 master: a
    /// green hand takes 1.3x the time, a master 0.7x. Seeded by the past, grown by practice.
    pub skill: [f32; 5],
    /// The trade the camp knows them for (`ROLES`), if any.
    pub role: Option<usize>,
    /// Loads of the hut and the works this settler laid.
    pub loads_laid: u32,
    /// Who they are: body, mind, character, values and likes (`persona.rs`). Strength and
    /// endurance set the pace of heavy work, agility the walk, toughness the cold.
    pub persona: crate::persona::Persona,
    /// Walking progress beyond `WALK_PER_TICK`, from their pace (fractions of a cost unit).
    pub stride_frac: f32,
    /// What they feel: thoughts, stress, a break under way (`mind.rs`).
    pub mind: mind::Mind,
    /// Wounds on body parts (`fight.rs`), kept after they heal (they are the settler's story).
    pub wounds: Vec<fight::Wound>,
    /// An office ("speaks for the camp", `society.rs`) and what their hands made (`craft.rs`).
    pub office: Option<String>,
    pub made: Vec<String>,
    /// Great deeds: "slew Gru the Unending on day 30" (`fight.rs`).
    pub deeds: Vec<String>,
    /// Drilled with the spear (`militia.rs`), 0..0.6.
    pub drill: f32,
    /// Their bed could not be reached: by the fire until this tick.
    pub(crate) bed_blocked_until: u64,
    /// Wed to (`family.rs`).
    pub spouse: Option<usize>,
    /// Away from the camp until this day (an expedition, `expedition.rs`); not alive meanwhile.
    pub away_until: u64,
    /// A spare-hours act under way (`needs.rs`): met when the wander ends.
    pub(crate) need_act: Option<needs::NeedAct>,
    /// The day of their last cup (`drink.rs`).
    pub last_drink: u64,
    /// The last day they ate the cook's supper (`kitchen.rs`).
    pub last_supper: u64,
    /// On rations, whether their last meal was taken from the store (`rations.rs`).
    pub rationed: bool,
    /// A guest (`visitors.rs`): the tick their stay ends (0: one of the camp), and what they are.
    pub guest_until: u64,
    pub visitor: Option<String>,
}

/// Whether a persona's liked creature is this game animal ("deer" and "red deer", "boar" and
/// "wild boar").
pub fn fond_of(p: &crate::persona::Persona, game: &str) -> bool {
    let liked = p.likes.creature.0.trim_end_matches('s');
    !liked.is_empty() && game.contains(liked)
}

/// The camp's names for the best at each kind of work.
/// Where everyday warmth stops raising an opinion (`Colony::warm`).
pub const FAMILIAR: i32 = 24;

pub const ROLES: [&str; 5] = ["forager", "fisher", "woodcutter", "carrier", "builder"];

/// Which skill a job uses.
pub fn skill_of(job: Job) -> Option<usize> {
    match job { Job::Forage(_) | Job::Hunt(_) => Some(0), Job::Fish(_) => Some(1), Job::Fell(_) | Job::Quarry(_) | Job::Dig(..) => Some(2), Job::Haul(_) => Some(3), Job::Build => Some(4), _ => None }
}

/// Skills a past brings: the veteran's axe and back, the wall-fighter's hands at building, the
/// child's eye for berries, a notable's kin's fishing.
pub fn skills_from_past(past: Option<&crate::history::settlers::Past>, name: &str) -> [f32; 5] {
    let h = |k: u64| { let mut x = name.bytes().fold(0xcbf2_9ce4_8422_2325u64, |a, b| (a ^ b as u64).wrapping_mul(0x100_0000_01b3)) ^ k.wrapping_mul(0x9E37_79B9_7F4A_7C15); x ^= x >> 29; (x % 1000) as f32 / 1000.0 };
    let mut s = [0; 5].map(|_| 0.0f32);
    for (k, v) in s.iter_mut().enumerate() { *v = 0.2 * h(k as u64); }
    if let Some(p) = past {
        let lines = p.lines.iter().map(|l| l.0.as_str()).collect::<Vec<_>>().join(" ");
        if p.calling.starts_with("a veteran") { s[2] += 0.3; s[3] += 0.2; }
        if lines.contains("fought on the walls") { s[4] += 0.35; }
        if lines.contains("was a child") || lines.contains("lost family") { s[0] += 0.3; }
        if p.calling.starts_with("kin of") { s[1] += 0.3; }
        if p.calling.starts_with("a refugee") { s[0] += 0.2; s[3] += 0.2; }
        if p.age >= 40 { s[4] += 0.1; }
    }
    s.map(|v| v.min(0.6))
}

/// A moment the window stops for: a card with what happened and why, at a place.
#[derive(Clone, Debug)]
pub struct Moment { pub tick: u64, pub title: String, pub text: String, pub because: String, pub at: Pos, /** The patron must answer (Y/N) before the clock runs on. */ pub choice: bool }

impl Moment {
    /// Whether the window should stop the clock for it: deaths, raids, troubles foretold,
    /// births, moods, artifacts, choices and the like. The camp's routine (a building finished, a
    /// role named, the season's festival, a wedding, a dream) goes to the status line: thirty
    /// moments a month had stopped the clock more than once a day.
    pub fn major(&self) -> bool {
        if self.choice { return true; }
        let t = self.title.as_str();
        let minor = t.ends_with(" finished") || t.contains(" festival") || t.starts_with("The camp's ") || t.ends_with(" speaks for the camp")
            || t == "The tithe refused" || t == "Rations" || t == "Judgement" || t.ends_with(" is founded") || t.starts_with("A hall for")
            || t.starts_with("The wedding") || t.ends_with("'s dream") || t.ends_with(" keeps the temple") || t.ends_with(" is hired")
            || t.starts_with("The friendship of") || t == "Migrants" || t == "The farm under the rock" || t.ends_with(" stays") || t.ends_with(" comes")
            || t == "Iron from the caravan" || t == "Water in the rock" || t.ends_with(" breaks") || t == "Old enemies at the fire"
            || t == "The wolves' den" || t.starts_with("The burning of") || t == "The old grave burned" || t.starts_with("Peace with")
            // The industries' digs and firsts (`industry.rs`).
            || t == "The first bars" || t.starts_with("Tools of ") || (t.ends_with(" below dug") && ["mason", "carpenter", "smelter", "forge", "kiln"].iter().any(|w| t.contains(w)));
        !minor
    }
}

/// The hut the settlers build together.
#[derive(Clone, Debug)]
pub struct Hut {
    /// Top-left corner of the footprint.
    pub at: Pos,
    pub logs_used: u32,
    pub done: bool,
}

/// What a dream asks of a settler, for a day.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dream {
    /// Of the hut finished: building and felling pull harder.
    Hut,
    /// Of plenty: foraging and fishing pull harder.
    Plenty,
    /// Of stillness: they rest by day.
    Rest,
    /// Of a fire burning in the dark: they keep watch tonight.
    Watch,
}

impl Dream {
    pub fn word(self) -> &'static str { match self { Dream::Hut => "the hut standing finished", Dream::Plenty => "baskets full of berries and fish", Dream::Rest => "a long quiet sleep", Dream::Watch => "a fire burning in the dark, and someone awake beside it" } }
}

/// A place the patron marked.
#[derive(Clone, Copy, Debug)]
pub struct PlaceMark { pub at: Pos, pub radius: u16, pub forbidden: bool }

/// The patron's indirect hand: no orders, only favour. Favour (3 at most, one more each dawn)
/// is spent on marking a place favoured or forbidden, favouring a settler, or sending a dream.
#[derive(Clone, Debug)]
pub struct Patron {
    pub favour: u32,
    pub marks: Vec<PlaceMark>,
    pub favourite: Option<usize>,
    /// (settler, dream, until tick).
    pub dreams: Vec<(usize, Dream, u64)>,
    last_refill_day: u64,
}

pub const FAVOUR_MAX: u32 = 3;

/// Exposure at which a night outside makes a settler ill (a full night by the fire is 0.77,
/// 0.96 for a child or an elder; a stocked woodpile halves it).
pub const ILL_AT: f32 = 0.85;
/// Settlers the first hut sleeps; the rest sleep by the fire until a second hut is built.
pub const HUT_BEDS: usize = 6;
/// Days stored food keeps, on average, without and with a drying rack.
pub const FOOD_KEEPS_DAYS: u32 = 6;
pub const FOOD_KEEPS_DAYS_RACK: u32 = 20;
/// Days a fishing spot takes to recover.
pub const FISH_RECOVER_DAYS: u64 = 3;

/// A founding stone: the patron's hand in the colony's shape, placed without designations.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StoneKind {
    /// The hall (the hut) is raised beside it.
    Hall,
    /// Keep this grove: its trees are never felled.
    Grove,
    /// A shrine: a standing stone the settlers rest by.
    Shrine,
}

impl StoneKind {
    pub fn word(self) -> &'static str { match self { StoneKind::Hall => "hall stone", StoneKind::Grove => "grove stone", StoneKind::Shrine => "shrine" } }
}

/// A permanent mark a moment leaves on the colony: a grave, a raised stone. Drawn on the map and
/// opened by a click (the inspector shows its words and the day).
#[derive(Clone, Debug)]
pub struct ColonyMark {
    pub at: Pos,
    pub kind: MarkKind,
    pub title: String,
    /// The words on it.
    pub text: String,
    pub day: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkKind { Grave, Stone, Scorch, Cage,
    /// A settler's own places (`haunts.rs`): a cairn, a bench or seat, a carved post.
    Cairn, Bench, Carving }

/// How far a grove stone keeps the axe away.
pub const GROVE_RADIUS: i32 = 7;

pub struct Colony {
    pub map: LocalMap,
    pub patron: Patron,
    /// What its patron called it, and the places named.
    pub name: Option<String>,
    pub place_names: Vec<(Pos, String)>,
    pub stones: Vec<(StoneKind, Pos)>,
    /// What happened here, left on the ground.
    pub marks: Vec<ColonyMark>,
    /// Every patron's act, as a replayable line "tick verb args" (`apply_intervention`): the
    /// same world code and the same interventions tell the same story on any machine.
    pub interventions: Vec<String>,
    /// Script lines up to this tick have been applied (`run_days_scripted`).
    script_at: u64,
    /// The first arc (`arc.rs`), planned from the history at founding.
    pub arc: Option<arc::Arc>,
    /// A banner for a great moment: (text, tick shown).
    pub banner: Option<(String, u64)>,
    /// Moments the window stops for (arc beats, deaths, finished buildings), in order.
    pub moments: Vec<Moment>,
    /// What settlers think of each other (pair, lower index first): meals shared, rescues, and the
    /// grudges their pasts carry.
    pub opinions: crate::history::det::FastMap<(usize, usize), i32>, // (never iterated)
    grudges: crate::history::det::HashSet<(usize, usize)>,
    /// Settler-nights that ended chilled (exposure 0.5 or more at dawn).
    pub chilled_nights: u32,
    /// Creatures on the map (the raid's attackers, wolves at night).
    pub creatures: Vec<creatures::Creature>,
    /// Where the last raid was fought (for its moment).
    pub(crate) clash_at: Option<Pos>,
    /// The side the last raid came from ("the north-east").
    pub raid_side: String,
    /// When the attackers were first on the map and when they clashed (ticks).
    pub raid_watch: Vec<(u64, u64)>,
    /// The global cell the camp was made at, when made where the walker stood (for its code).
    pub cell: Option<(u64, u64)>,
    /// Milestones reached, each with a saga (the first raid, the first winter, a year, the end).
    pub milestones_hit: Vec<String>,
    /// How many of them have had their saga written (by the window).
    pub sagas_written: usize,
    /// Every cell a settler can fish from (dry, passable, beside water), found once at founding.
    fishing_spots: Vec<Pos>,
    /// How these settlers' people build (`projects::BuildWay`).
    pub way: Option<projects::BuildWay>,
    /// Footsteps per cell: where feet go most a track is worn, then a lane.
    pub steps: Vec<u16>,
    pub(crate) next_creature: u32,
    /// Game no one could find a way to (across water): not stalked again.
    pub(crate) game_unreachable: crate::history::det::HashSet<u32>,
    /// Trees still within reach of the camp (reckoned each dawn).
    pub(crate) wood_in_reach: bool,
    /// Game brought down.
    pub hunted: u32,
    /// The dig under way: cells and the level each is dug to.
    pub dig_plan: Option<Vec<dig::DigCell>>,
    /// Seams of ore struck, and stone loads dug out of the rock.
    pub ore_found: u32,
    pub stone_dug: u32,
    /// A dug hall's cells (beds under rock), once it stands.
    pub hall_cells: Vec<Pos>,
    /// The level of the hall's floor.
    pub hall_z: i32,
    /// Rooms dug in the rock, on their levels (`delve.rs`).
    pub rooms: Vec<delve::Room>,
    /// The stair spine the delve goes down (`delve.rs`).
    pub spine: Option<delve::Spine>,
    /// Where the delve opens on the surface: dug stone is carried up to here.
    pub delve_mouth: Option<Pos>,
    /// Failed ways to the dig under way since a cut was made; digs given up (their first cut).
    pub(crate) dig_fails: u32,
    pub(crate) digs_given_up: Vec<Pos>,
    /// The deep shaft has reached the warm rock over the magma sea: metal is forged at the magma.
    pub magma_forge: bool,
    /// Places to fish from on the floor of the caverns the stair reaches (`delve.rs`).
    pub(crate) cave_fish: Vec<(Pos, i32)>,
    /// The lookout tower's stair cell and the level of its platform (`delve.rs::raise_tower`).
    pub tower: Option<(Pos, i32)>,
    /// Rooms the dig under way will make.
    pub dig_rooms: Vec<delve::Room>,
    /// A breach the raid made in the palisade, and its day, until mended.
    pub(crate) breach: Option<(Pos, u64)>,
    /// The jetty's fishing place: it never runs out.
    pub(crate) jetty: Option<Pos>,
    /// Minutes walked fetching water, and how far the nearest water lies (cells).
    pub water_walked: u64,
    pub water_distance: u32,
    /// Loads laid since a builder was named: (by the builder, by anyone).
    pub builder_share: (u32, u32),
    /// The camp's plan, said aloud ("Next: a smokehouse. Winter is 40 days off...").
    pub plan_line: String,
    quarrelled: bool,
    /// The day the settlers gave the land up and left (the colony ends).
    pub departed: Option<u64>,
    /// The day the camp last moved (they move at most once in five days).
    last_move: u64,
    /// Who keeps watch tonight (the arc).
    pub(crate) watcher: Option<usize>,
    /// Cavern layers the mine broke into (`mine.rs`), and what hunts up the mine at night.
    pub breached: Vec<u8>,
    /// Works made at the workshop (`craft.rs`).
    pub works: Vec<craft::Work>,
    /// The town that sends caravans (`trade.rs`), the day of the next, how many came, whether
    /// iron tools were bought, works sold so far.
    pub trade: Option<trade::Partner>,
    pub(crate) next_caravan: u64,
    pub caravans: u32,
    pub tools_bought: bool,
    pub(crate) traded_before: u32,
    /// Who may come later as migrants (`trade.rs`: word of the camp goes home with the
    /// caravans), and the day the next wave arrives.
    pub migrants: Vec<(String, crate::history::settlers::Past)>,
    pub(crate) migrant_day: Option<u64>,
    /// Who speaks for the camp, and their mandate for the season (`society.rs`).
    pub speaker: Option<usize>,
    pub mandate: Option<society::Mandate>,
    pub(crate) mandate_day: u64,
    /// The Shadow's corruption on this tile and its name (`dead.rs`).
    pub darkness: f32,
    pub shadow_name: Option<String>,
    /// A strange mood (`mood.rs`), and whether the camp has had its one.
    pub mood: Option<mood::Mood>,
    pub(crate) mood_done: bool,
    /// A werebeast near (`curse.rs`): its name; the cursed; how often each changed one has bitten.
    pub were: Option<String>,
    pub cursed: Vec<usize>,
    /// The last clash's telling blows: harm done to the foe and who struck hardest (`fight.rs`).
    pub(crate) blows: (f32, Option<usize>),
    /// Beasts the camp has slain, by name.
    pub slain: Vec<String>,
    /// A slain beast's hoard on its way home: (day, beast, things).
    pub(crate) hoard_due: Option<(u64, String, Vec<String>)>,
    /// Things of the old world the camp holds ("The Staff of X, a superior staff, from the hoard of Y").
    pub treasures: Vec<String>,
    /// The camp's spears (`militia.rs`).
    pub arms: Vec<militia::Arm>,
    /// The hall's carved walls (`engrave.rs`).
    pub engravings: Vec<engrave::Engraving>,
    /// Figures of the world who may visit (`visitors.rs`), and the day the last came.
    pub visitors: Vec<visitors::Visitor>,
    pub(crate) last_visit: u64,
    /// The night a refused seeker means to steal the relic (`relic.rs`).
    pub(crate) seeker_night: Option<u64>,
    /// A vampire among them (`night.rs`): who, and the day they came; the drained; the found
    /// dead at dawn; whether the sharp-eyed have noticed.
    pub vampire: Option<(usize, u64)>,
    pub(crate) drained: crate::history::det::HashMap<usize, u32>,
    pub(crate) drained_dead: Vec<usize>,
    pub(crate) vampire_noticed: bool,
    /// The watch post could not be reached: kept at the fire until this tick.
    pub(crate) watch_blocked_until: u64,
    /// Animals kept by settlers (`pets.rs`).
    pub pets: Vec<pets::Pet>,
    /// Who tends the wounded (`heal.rs`).
    pub healer: Option<usize>,
    /// Families (`family.rs`): (mother, father, due day); (mother, day born); (child, mother, father).
    pub(crate) expecting: Vec<(usize, usize, u64)>,
    pub(crate) born: Vec<(usize, u64)>,
    pub children: Vec<(usize, usize, usize)>,
    /// The aquifer (`dig.rs`): the day it was struck, the dig paused for lining, lined.
    pub aquifer_struck: Option<u64>,
    pub(crate) dig_paused: bool,
    pub aquifer_lined: bool,
    /// Gems prised from the rock, by kind (`dig.rs`; set in works, sold, wanted by moods).
    pub gems: Vec<(String, u32)>,
    /// The dead who died badly (`ghosts.rs`).
    pub restless: Vec<ghosts::Restless>,
    /// The militia away hunting (`expedition.rs`).
    pub expedition: Option<expedition::Expedition>,
    /// The world's width in tiles (for distances on foot; set at founding).
    pub world_width: usize,
    /// Cups of berry wine in the store (`drink.rs`).
    pub drink: u32,
    /// Beasts taken alive in the cage traps (`traps.rs`).
    pub caged: Vec<String>,
    pub(crate) food_warned_day: u64,
    /// The sellsword hired for the raid foretold (`tavern.rs`).
    pub(crate) sellsword_hired: Option<usize>,
    /// The beasts in the pen: their kind and head (`livestock.rs`).
    pub pen: Option<(String, u32)>,
    /// The kinds of ore struck ("iron", "copper", "gold").
    pub ores: Vec<String>,
    /// The industries' stock: ore, bars, charcoal, blocks, barrels, clay and sand (`industry.rs`).
    pub industry: industry::Industry,
    /// The day the miners following the adamantine break into the hollow (`deep.rs`).
    pub(crate) hollow_day: Option<u64>,
    /// Whose raiders the camp is fighting tonight (`fight.rs`: their own stand aside).
    pub(crate) fighting_people: Option<crate::history::FactionId>,
    /// The embark's places found (`explore.rs`), and a robbed tomb's dead to rise tonight.
    pub places_found: Vec<usize>,
    pub(crate) tomb_risen: Option<(String, u64)>,
    /// A raider held (`prisoners.rs`).
    pub prisoner: Option<prisoners::Prisoner>,
    /// What the world's peoples think of the camp, and why (`regard.rs`).
    pub regards: Vec<regard::Regard>,
    /// Armour made for the militia (`armour.rs`), and hides of the hunt used for it.
    pub armour: Vec<armour::Armour>,
    pub hides_used: u32,
    /// Peoples among the troubles who steal children, and the children taken (`snatch.rs`).
    pub snatchers: Vec<crate::history::FactionId>,
    pub snatched: Vec<snatch::Snatched>,
    /// A war band camped outside the walls (`siege.rs`).
    pub siege: Option<siege::Siege>,
    /// Guilds of the trades (`guild.rs`).
    pub guilds: Vec<guild::Guild>,
    /// Grievances against the ruling lord, per settler, and whether the camp has risen (`rising.rs`).
    pub grievances: crate::history::det::HashMap<usize, u32>,
    pub lord_risen: bool,
    /// What the camp asked of its trading town, and what came of it (`liaison.rs`).
    pub request: Option<liaison::Want>,
    pub salt_until: u64,
    pub seed_grain: bool,
    pub herbs: u32,
    /// Pairs who found they shared a battle (`recognize.rs`).
    pub(crate) recognized: crate::history::det::HashSet<(usize, usize)>,
    /// What is left of beasts the camp slew (`fight.rs`): (name, short name, bones, hide pieces,
    /// what its skin is), worked at the workshop into famous works and armour.
    pub remains: Vec<(String, String, u32, u32, String)>,
    /// What keeps coming back, and what the camp did about it (`respond.rs`).
    pub wolf_bites: u32,
    pub dens_cleared: Vec<Pos>,
    pub risings: crate::history::det::HashMap<Pos, u32>,
    pub burned: Vec<Pos>,
    /// Dawns running the camp has gone hungry (`projects.rs`).
    pub(crate) hungry_days: u32,
    /// Artifacts stolen in the night: (title, the thieves' people, day) (`thieves.rs`).
    pub stolen: Vec<(String, crate::history::FactionId, u64)>,
    pub(crate) thief_day: u64,
    /// The camp's graves consecrated by its priest (`priest.rs`).
    pub consecrated: bool,
    /// The war the camp's people fight, and who went (`warcall.rs`).
    pub war_call: Option<warcall::WarCall>,
    /// Trees felled, to grow back in a year (`regrow.rs`).
    pub(crate) felled: Vec<(Pos, crate::local::TreeKind, u64)>,
    /// Former marriages ended by death: (survivor, the dead, day) (`family.rs`).
    pub widowed: Vec<(usize, usize, u64)>,
    /// Vows of vengeance (`vow.rs`).
    pub vows: Vec<vow::Vow>,
    /// Those a strange mood has taken (`mood.rs`).
    pub moods_had: Vec<usize>,
    /// The last beast slaughtered from the pen (`livestock.rs`).
    pub(crate) slaughter_day: u64,
    /// Tonight's supper (day, dish, fine) and how many the cook has made (`kitchen.rs`).
    pub supper: Option<(u64, String, bool)>,
    pub suppers: u32,
    /// When each settler's clothes were made, and the caravans' cloth (`clothes.rs`).
    pub clothes: crate::history::det::HashMap<usize, u64>,
    pub cloth: u32,
    pub(crate) cloth_used: u32,
    /// Settlers whose dream came true (`dreams.rs`).
    pub dreamt: Vec<usize>,
    /// Children born here who have come of age (`childhood.rs`).
    pub come_of_age: Vec<usize>,
    /// Half rations ordered (`rations.rs`).
    pub rations: bool,
    /// The water frozen over (`frozen`).
    pub(crate) ice: bool,
    /// The herds gone to their winter grounds (`reckon_herds`).
    pub(crate) herds_away: bool,
    /// The patron's bell holds everyone indoors until this tick (`ring_bell`).
    pub bell_until: u64,
    /// The lord the camp's people will send (`nobles.rs`).
    pub lord: Option<nobles::Lord>,
    /// Where the map's shrubs grow (listed once), and whether any is ripe today (cached per
    /// day): the ring search for berries scanned the whole reach every time none were ripe.
    pub(crate) shrubs: Vec<Pos>,
    /// The same shrubs in `SHRUB_BUCKET`-cell squares (row-major), for `nearest_ripe_shrub`.
    shrub_buckets: Vec<Vec<Pos>>,
    /// Every column whose ground held a tree at founding, in the same squares, for the tree
    /// searches (`nearest_tree_by`). No other column can ever hold one: trees grow back only
    /// where one was felled (`regrow.rs`), and cutting the ground down never bares a planted
    /// cell (`dig_cell` clears the new floor's plant). A felled tree stays listed and fails
    /// `is_felling_tree` until it grows back.
    tree_buckets: Vec<Vec<Pos>>,
    pub(crate) ripe_today: std::cell::Cell<(u64, bool)>,
    /// The day the map was last looked over for a tree to fell, and whether it held none.
    pub(crate) treeless_day: std::cell::Cell<u64>,
    /// A lost artifact of the history lying near (`relic.rs`).
    pub relic: Option<relic::Relic>,
    pub(crate) were_bites: std::collections::BTreeMap<usize, u32>,
    pub(crate) changed: Vec<usize>,
    /// Crimes and their judgement (`justice.rs`); who is in the stocks, until when.
    pub crimes: Vec<justice::Crime>,
    pub(crate) stocks: Option<(usize, u64)>,
    pub(crate) cave_hunter: Option<String>,
    /// Bites by the cavern's hunters come up the stair (`delve.rs::seal_caverns`).
    pub(crate) cave_bites: u32,
    /// Where the stair stands on each breached cavern's floor: (layer, place) (`cavelife.rs`).
    pub(crate) cavern_feet: Vec<(u8, nav::P3)>,
    /// Artifacts set in the delve's rooms: (title, room index, cell) (`delve.rs::place_artifacts`).
    pub placed: Vec<(String, usize, Pos)>,
    /// Settlers' own places for their needs (`haunts.rs`).
    pub haunts: Vec<haunts::Haunt>,
    /// Talks said aloud (pair, day), at most one a pair in twenty days (`talk.rs`).
    pub talks_said: Vec<(usize, usize, u64)>,
    /// The gates' compass directions, chosen when the palisade is begun (`traps.rs`).
    pub gate_dirs: Vec<(i32, i32)>,
    /// News the camp heard from the world, as each teller told it (`news.rs`).
    pub heard: Vec<news::Heard>,
    /// What the migrants' people know (`history::knowledge`), told one item a wave.
    pub migrant_news: Vec<crate::history::knowledge::Told>,
    /// The drawbridges over the ditch's crossings: (cell, deck level), and whether they are up.
    pub bridges: Vec<(Pos, i32)>,
    pub bridges_up: bool,
    /// The hatch sealing the stair below the first cavern: its column and level.
    pub hatch: Option<(Pos, i32)>,
    /// Work the colony set itself after the hut (`projects.rs`).
    pub projects: Vec<projects::Project>,
    /// What the hut is built of: timber, or stone where no tree is in reach.
    pub hut_material: ItemKind,
    /// Everyone who laid a log in the hut, in order.
    builders: Vec<usize>,
    pub clock: Clock,
    pub settlers: Vec<Settler>,
    pub items: Vec<Item>,
    /// Where the stockpile and fire are.
    pub camp: Pos,
    pub hut: Option<Hut>,
    /// Day each foraged shrub bears again.
    shrub_ready: crate::history::det::FastMap<Pos, u64>, // (never iterated)
    /// Targets someone already went for (trees, shrubs, fishing spots).
    claimed: crate::history::det::FastSet<Pos>, // (never iterated)
    /// Unreachable targets, so nobody keeps trying them.
    unreachable: crate::history::det::FastSet<Pos>, // (never iterated)
    pub log: Vec<String>,
    /// Every choice a settler made, with its reason.
    pub decisions: Vec<String>,
    rng: ChaCha8Rng,
    /// The founding seed (for hashed draws that must not disturb `rng`, such as each night's cold).
    seed: u64,
    /// Moments already logged once (first meal, first log...).
    milestones: crate::history::det::HashSet<&'static str>,
    /// Items carried along with a hauled one (a forager's basket): stored when it arrives.
    basket: crate::history::det::HashMap<usize, Vec<usize>>,
}

impl Colony {
    /// Found a colony of `names` on `map`; the camp goes on the dry, open, flat ground nearest
    /// the centre.
    pub fn found(mut map: LocalMap, names: &[String], seed: u64) -> Self {
        let n = map.width;
        let centre = (n / 2) as i32;
        let mut camp = ((n / 2) as u16, (n / 2) as u16);
        let mut best = i32::MAX;
        // The camp goes on dry ground joined to enough land to live from: a camp on an island
        // or a spit in a lake (dev 30,30, a crater lake) was cut off and everyone starved.
        let (patch, sizes) = dry_patches(&map);
        let any_big = sizes.iter().any(|&c| c >= ENOUGH_LAND);
        for y in 8..n - 8 {
            for x in 8..n - 8 {
                if !nav::passable(&map, (x as u16, y as u16)) { continue; }
                if any_big && patch[y * n + x].checked_sub(0).and_then(|id| sizes.get(id as usize)).map_or(true, |&c| c < ENOUGH_LAND) { continue; }
                let flat = (-2i32..=2).all(|d| {
                    let (xx, yy) = ((x as i32 + d) as usize, y);
                    map.surface_z[yy * n + xx] == map.surface_z[y * n + x] && nav::passable(&map, (xx as u16, yy as u16))
                });
                if !flat { continue; }
                let trees = (-2i32..=2).flat_map(|dy| (-2i32..=2).map(move |dx| (dx, dy)))
                    .filter(|&(dx, dy)| matches!(map.cell((x as i32 + dx) as usize, (y as i32 + dy) as usize, map.surface_z[((y as i32 + dy) as usize) * n + (x as i32 + dx) as usize] as usize).plant, Plant::Tree(_)))
                    .count() as i32;
                let score = (x as i32 - centre).abs() + (y as i32 - centre).abs() + trees * 6;
                if score < best { best = score; camp = (x as u16, y as u16); }
            }
        }
        // Clear the camp ground.
        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                let (x, y) = ((camp.0 as i32 + dx) as usize, (camp.1 as i32 + dy) as usize);
                let k = map.idx(x, y, map.surface_z[y * n + x] as usize);
                map.cells[k].plant = Plant::None;
                map.cells[k].boulder = false;
            }
        }
        let mut rng = ChaCha8Rng::seed_from_u64(seed ^ 0xC010_4E57);
        let settlers = names.iter().enumerate().map(|(i, name)| {
            let pos = (camp.0 + (i as u16 % 3), camp.1 + (i as u16 / 3) % 3);
            Settler {
                name: name.clone(), pos: if nav::passable(&map, pos) { pos } else { camp }, z: 0, path: Vec::new(),
                hunger: 0.2 + 0.2 * rng.gen::<f32>(), fatigue: 0.1 * rng.gen::<f32>(), exposure: 0.0,
                job: Job::Idle, work_left: 0, carrying: None, why: "Just arrived".into(),
                alive: true, stuck: 0, retry_at: 0, starving: 0, stride: 0,
                taste: [0; 5].map(|_| 0.75 + 0.55 * rng.gen::<f32>()),
                past: None, ill_until: 0, skill: [0.0; 5], role: None, loads_laid: 0,
                persona: crate::persona::Persona::roll("human", None, crate::persona::seed_of(name, seed)), stride_frac: 0.0, mind: Default::default(), wounds: Vec::new(), office: None, made: Vec::new(), deeds: Vec::new(), drill: 0.0, bed_blocked_until: 0, spouse: None, away_until: 0, need_act: None, last_drink: 0, last_supper: 0, rationed: false, guest_until: 0, visitor: None,
            }
        }).collect::<Vec<_>>();
        // They arrive with two days of food.
        let items = (0..names.len() * 2).map(|_| Item { kind: ItemKind::Food, at: camp, stored: true, reserved: false }).collect();
        let mut c = Colony {
            map, clock: Clock { tick: 6 * 60 }, settlers, items, camp, hut: None,
            shrub_ready: Default::default(), claimed: Default::default(), unreachable: Default::default(),
            log: Vec::new(), decisions: Vec::new(), rng, seed, milestones: Default::default(), basket: Default::default(),
            patron: Patron { favour: FAVOUR_MAX, marks: Vec::new(), favourite: None, dreams: Vec::new(), last_refill_day: 1 },
            name: None, place_names: Vec::new(), stones: Vec::new(), marks: Vec::new(), builders: Vec::new(), interventions: Vec::new(), script_at: 0, arc: None, banner: None, moments: Vec::new(), departed: None, last_move: 0, opinions: Default::default(), grudges: Default::default(), quarrelled: false, chilled_nights: 0, plan_line: String::new(), builder_share: (0, 0), way: None, steps: Vec::new(), next_creature: 0, game_unreachable: Default::default(), wood_in_reach: true, hunted: 0, dig_plan: None, ore_found: 0, stone_dug: 0, hall_cells: Vec::new(), hall_z: 0, rooms: Vec::new(), spine: None, delve_mouth: None, dig_fails: 0, digs_given_up: Vec::new(), cave_fish: Vec::new(), magma_forge: false, tower: None, dig_rooms: Vec::new(), breach: None, jetty: None, water_walked: 0, water_distance: 0, fishing_spots: Vec::new(), creatures: Vec::new(), clash_at: None, raid_side: String::new(), raid_watch: Vec::new(), cell: None, milestones_hit: Vec::new(), sagas_written: 0, watcher: None, breached: Vec::new(), cave_hunter: None, cave_bites: 0, cavern_feet: Vec::new(), placed: Vec::new(), haunts: Vec::new(), talks_said: Vec::new(), gate_dirs: Vec::new(), bridges: Vec::new(), bridges_up: false, hatch: None, works: Vec::new(), trade: None, next_caravan: 0, caravans: 0, tools_bought: false, traded_before: 0, migrants: Vec::new(), migrant_day: None, speaker: None, mandate: None, mandate_day: 0, darkness: 0.0, shadow_name: None, mood: None, mood_done: false, were: None, cursed: Vec::new(), blows: (0.0, None), slain: Vec::new(), hoard_due: None, treasures: Vec::new(), arms: Vec::new(), engravings: Vec::new(), visitors: Vec::new(), last_visit: 0, seeker_night: None, vampire: None, drained: Default::default(), drained_dead: Vec::new(), vampire_noticed: false, watch_blocked_until: 0, pets: Vec::new(), healer: None, expecting: Vec::new(), born: Vec::new(), children: Vec::new(), aquifer_struck: None, dig_paused: false, aquifer_lined: false, gems: Vec::new(), restless: Vec::new(), expedition: None, world_width: 512, drink: 0, caged: Vec::new(), food_warned_day: 0, sellsword_hired: None, pen: None, ores: Vec::new(), hollow_day: None, fighting_people: None, places_found: Vec::new(), tomb_risen: None, prisoner: None, regards: Vec::new(), armour: Vec::new(), hides_used: 0, snatchers: Vec::new(), snatched: Vec::new(), siege: None, guilds: Vec::new(), grievances: Default::default(), lord_risen: false, request: None, salt_until: 0, seed_grain: false, herbs: 0, recognized: Default::default(), remains: Vec::new(), wolf_bites: 0, dens_cleared: Vec::new(), risings: Default::default(), burned: Vec::new(), hungry_days: 0, stolen: Vec::new(), thief_day: 0, consecrated: false, war_call: None, felled: Vec::new(), widowed: Vec::new(), vows: Vec::new(), moods_had: Vec::new(), slaughter_day: 0, supper: None, suppers: 0, clothes: Default::default(), cloth: 0, cloth_used: 0, dreamt: Vec::new(), come_of_age: Vec::new(), rations: false, ice: false, herds_away: false, bell_until: 0, lord: None, shrubs: Vec::new(), ripe_today: std::cell::Cell::new((u64::MAX, true)), treeless_day: std::cell::Cell::new(u64::MAX), relic: None, were_bites: Default::default(), changed: Vec::new(), crimes: Vec::new(), stocks: None, projects: Vec::new(), hut_material: ItemKind::Log, heard: Vec::new(), migrant_news: Vec::new(), industry: Default::default(), shrub_buckets: Vec::new(), tree_buckets: Vec::new(),
        };
        c.shrubs = (1..c.map.height - 1).flat_map(|y| (1..c.map.width - 1).map(move |x| (x as u16, y as u16))).filter(|&p| c.floor_plant(p) == Plant::Shrub).collect();
        let bw = c.map.width.div_ceil(SHRUB_BUCKET);
        c.shrub_buckets = vec![Vec::new(); bw * c.map.height.div_ceil(SHRUB_BUCKET)];
        for &p in &c.shrubs { c.shrub_buckets[(p.1 as usize / SHRUB_BUCKET) * bw + p.0 as usize / SHRUB_BUCKET].push(p); }
        c.tree_buckets = vec![Vec::new(); c.shrub_buckets.len()];
        for y in 0..c.map.height {
            for x in 0..c.map.width {
                if matches!(c.floor_plant((x as u16, y as u16)), Plant::Tree(_)) { c.tree_buckets[(y / SHRUB_BUCKET) * bw + x / SHRUB_BUCKET].push((x as u16, y as u16)); }
            }
        }
        c.fishing_spots = (1..c.map.height - 1).flat_map(|y| (1..c.map.width - 1).map(move |x| (x as u16, y as u16)))
            .filter(|&p| c.fishing_ground(p)).collect();
        c.spawn_game();
        c.hut = c.find_hut_site().map(|at| Hut { at, logs_used: 0, done: false });
        // No tree within reach: the hut goes up in stone.
        if !c.timber_near() { c.hut_material = ItemKind::Stone; }
        let site = c.hut.as_ref().map(|h| format!(" and a hut site at {},{}", h.at.0, h.at.1)).unwrap_or_default();
        c.note(format!("{} settlers make camp at {},{}{}. They carry two days of food.", names.len(), camp.0, camp.1, site));
        c
    }

    pub(crate) fn moment(&mut self, title: String, text: String, because: String, at: Pos) {
        self.moments.push(Moment { tick: self.clock.tick, title, text, because, at, choice: false });
    }

    fn note(&mut self, line: String) { self.log.push(format!("{}  {}", self.clock.stamp(), line)); }

    fn spend(&mut self) -> Result<(), String> {
        if self.patron.favour == 0 { return Err("no favour left today".into()); }
        self.patron.favour -= 1;
        Ok(())
    }

    /// Mark the ground within `radius` of `at` favoured (work there pulls harder) or forbidden
    /// (nobody works or wanders there).
    pub fn mark_place(&mut self, at: Pos, radius: u16, forbidden: bool) -> Result<String, String> {
        // The same mark again on marked ground lifts it (the favour spent is not returned).
        let covers = |m: &PlaceMark| m.forbidden == forbidden && (m.at.0 as i32 - at.0 as i32).abs().max((m.at.1 as i32 - at.1 as i32).abs()) <= m.radius as i32;
        if let Some(k) = self.patron.marks.iter().position(covers) {
            let m = self.patron.marks.remove(k);
            self.interventions.push(format!("{} {} {} {} {}", self.clock.tick, if forbidden { "forbid" } else { "bless" }, at.0, at.1, radius));
            let line = format!("The patron lifted the {} from the ground about {},{}. (your doing)", if forbidden { "ban" } else { "blessing" }, m.at.0, m.at.1);
            self.note(line.clone());
            return Ok(line);
        }
        self.spend()?;
        self.patron.marks.retain(|m| m.at != at);
        self.patron.marks.push(PlaceMark { at, radius, forbidden });
        // Work already under way on forbidden ground stops.
        if forbidden {
            for k in 0..self.settlers.len() {
                if let Job::Forage(t) | Job::Fish(t) | Job::Fell(t) | Job::Quarry(t) = self.settlers[k].job {
                    if self.marked(t, true) {
                        self.claimed.remove(&t);
                        self.settlers[k].job = Job::Idle;
                        self.settlers[k].path.clear();
                        self.settlers[k].work_left = 0;
                    }
                }
            }
        }
        self.interventions.push(format!("{} {} {} {} {}", self.clock.tick, if forbidden { "forbid" } else { "bless" }, at.0, at.1, radius));
        let line = format!("The patron {} the ground about {},{}. (your doing)", if forbidden { "forbade" } else { "blessed" }, at.0, at.1);
        self.note(line.clone());
        Ok(line)
    }

    /// Favour one settler: they work with more heart, and the others notice.
    pub fn favour_settler(&mut self, i: usize) -> Result<String, String> {
        if !self.settlers.get(i).map_or(false, |s| s.alive) { return Err("no one there".into()); }
        self.spend()?;
        self.patron.favourite = Some(i);
        self.interventions.push(format!("{} favour {}", self.clock.tick, i));
        let name = self.settlers[i].name.clone();
        let line = format!("The patron favours {}; the others notice. (your doing)", name);
        self.note(line.clone());
        self.feel(i, mind::Feel::Favoured);
        // The envious take it badly.
        for j in 0..self.settlers.len() {
            if j != i && self.settlers[j].alive && self.settlers[j].persona.facet(crate::persona::Facet::Envy) >= 70 {
                self.feel(j, mind::Feel::Envy { of: name.clone() });
            }
        }
        Ok(line)
    }

    /// Send a settler a dream: for a day it pulls them towards what they dreamt of.
    pub fn send_dream(&mut self, i: usize, dream: Dream) -> Result<String, String> {
        if !self.settlers.get(i).map_or(false, |s| s.alive) { return Err("no one there".into()); }
        self.spend()?;
        let until = self.clock.tick + TICKS_PER_DAY;
        self.patron.dreams.retain(|d| d.0 != i);
        self.patron.dreams.push((i, dream, until));
        self.interventions.push(format!("{} dream {} {:?}", self.clock.tick, i, dream));
        let name = self.settlers[i].name.clone();
        let line = format!("{} dreamt of {}. (your doing)", name, dream.word());
        self.note(line.clone());
        // Ask again now: a dream changes the day.
        self.decide(i, false);
        Ok(line)
    }

    /// The patron's bell (DF's civilian alert): one favour, and until the next dawn everyone keeps
    /// under a roof. The camp is warned (+0.1 readiness tonight), the raiders find no child out of
    /// doors (`snatch.rs`), and an evil sky by day finds everyone sheltered (`weather.rs`).
    pub fn ring_bell(&mut self) -> Result<String, String> {
        if self.bell_until > self.clock.tick { return Err("the bell has rung already; all are under a roof".into()); }
        self.spend()?;
        // Until the next 06:00.
        let into = self.clock.tick % TICKS_PER_DAY;
        let until = self.clock.tick - into + TICKS_PER_DAY + if into < 6 * 60 { 0 } else { 6 * 60 };
        self.bell_until = until.min(self.clock.tick + TICKS_PER_DAY);
        self.interventions.push(format!("{} bell", self.clock.tick));
        let line = "The bell rings over the camp: all are to keep under a roof until dawn. (your doing)".to_string();
        self.note(line.clone());
        for i in 0..self.settlers.len() { if self.settlers[i].alive { self.decide(i, false); } }
        Ok(line)
    }

    /// Whether the patron's bell holds the camp indoors now.
    pub fn bell_rung(&self) -> bool { self.bell_until > self.clock.tick }

    /// Place a founding stone (five at most). A hall stone moves the hut site beside it while no
    /// log is laid; a grove stone keeps the axe from the trees around it; a shrine is a standing
    /// stone the settlers rest by.
    pub fn place_stone(&mut self, kind: StoneKind, at: Pos) -> Result<String, String> {
        if self.stones.len() >= 5 { return Err("five founding stones are set already".into()); }
        if kind == StoneKind::Hall {
            if let Some(h) = self.hut.as_ref().filter(|h| h.logs_used > 0) {
                return Err(format!("the hall's first {} is laid at {},{}; it can't move now", if self.hut_material == ItemKind::Stone { "stone" } else { "log" }, h.at.0, h.at.1));
            }
        }
        if !nav::passable(&self.map, at) { return Err("no footing for a stone there".into()); }
        self.stones.retain(|s| !(s.0 == kind && kind == StoneKind::Hall));
        self.stones.push((kind, at));
        self.interventions.push(format!("{} stone {:?} {} {}", self.clock.tick, kind, at.0, at.1));
        if kind == StoneKind::Shrine {
            let k = at.1 as usize * self.map.width + at.0 as usize;
            self.map.features[k] = crate::local::wildlife::Feature::Stone;
        }
        let mut line = format!("The patron set a {} at {},{}.", kind.word(), at.0, at.1);
        if kind == StoneKind::Hall && self.hut.as_ref().map_or(true, |h| h.logs_used == 0) {
            self.hut = self.find_hut_site().map(|a| Hut { at: a, logs_used: 0, done: false });
            if let Some(h) = &self.hut { line.push_str(&format!(" The hall will stand at {},{}.", h.at.0, h.at.1)); }
        }
        self.note(format!("{} (your doing)", line));
        Ok(line)
    }

    /// A settler dies of `cause`: the others bury them at the edge of the camp, under words from
    /// their life.
    pub fn bury(&mut self, i: usize, cause: &str) {
        self.settlers[i].alive = false;
        // A marriage ends, but the link is kept (DF's former-spouse links): widowed (`family.rs`).
        if let Some(w) = self.settlers[i].spouse.take() {
            if self.settlers[w].spouse == Some(i) { self.settlers[w].spouse = None; }
            self.widowed.push((w, i, self.clock.day()));
        }
        // A bad death leaves the dead restless until a slab is carved (`ghosts.rs`).
        self.mourn(i, cause);
        // A niche in the tombs below, if one is free (`delve.rs`): no grave on the surface.
        let tomb = if self.alive() > 0 { self.rooms.iter().position(|r| r.kind == delve::RoomKind::Tomb && r.owner.is_none()) } else { None };
        if let Some(k) = tomb { self.entomb(i, k, cause); return; }
        let s = &self.settlers[i];
        let n = self.marks.iter().filter(|m| m.kind == MarkKind::Grave).count() as i32;
        let at = self.spot_from_camp(-8 + 2 * n, 6);
        let past = s.past.as_ref().map(|p| format!(" Aged {}, {}.", p.age, p.calling)).unwrap_or_default();
        let feeling = s.past.as_ref().and_then(|p| p.feeling.as_ref()).map(|f| format!(" Remembered as one who {}.", f.0)).unwrap_or_default();
        let text = format!("Here lies {}.{} Died {} on day {}.{}", s.name, past, cause, self.clock.day(), feeling);
        let title = format!("The grave of {}", s.name);
        // The ground is cleared for it.
        let w = self.map.width;
        for dy in -1i32..=1 { for dx in -1i32..=1 {
            let (x, y) = ((at.0 as i32 + dx).max(0) as usize, (at.1 as i32 + dy).max(0) as usize);
            if x < w && y < self.map.height {
                let k = self.map.idx(x, y, self.map.surface_z[y * w + x].max(0) as usize);
                self.map.cells[k].plant = Plant::None;
            }
        } }
        self.marks.push(ColonyMark { at, kind: MarkKind::Grave, title, text, day: self.clock.day() });
        let name = s.name.clone();
        if self.alive() == 0 {
            // The last of them: no one is left to bury them, and the camp falls.
            self.marks.pop();
            let to = if let Some(who) = cause.strip_prefix("in the raid of ") { format!("to {}", who) }
                else if let Some(what) = cause.strip_prefix("of ") { format!("to {}", what) } else { cause.to_string() };
            let line = format!("{} was the last. The camp fell on day {}, {}.", name, self.clock.day(), to);
            self.note(line.clone());
            self.moment("The camp falls".into(), line, format!("because {} died {}, and no one was left", name, cause), self.camp);
            self.milestones_hit.push("the end".into());
            return;
        }
        self.note(format!("They bury {} at {},{}.", name, at.0, at.1));
        self.mourn_death(i, cause, "They bury them at the camp's edge.", at);
    }

    /// The camp grieves a death, and it has its card (unless what killed them made one).
    pub(crate) fn mourn_death(&mut self, i: usize, cause: &str, laid: &str, at: Pos) {
        let name = self.settlers[i].name.clone();
        for j in 0..self.settlers.len() {
            if j != i && self.settlers[j].alive {
                let close = self.opinion(i, j) >= 6;
                self.feel(j, mind::Feel::Death { whom: name.clone(), close });
            }
        }
        // A death in the raid has the raid's own card; others get theirs, unless what killed
        // them has just made one (a fell mood, the night, a festering wound, old age).
        let told = self.moments.last().map_or(false, |m| m.tick == self.clock.tick && (m.text.contains(&name) || m.title.contains(&name)));
        if !cause.starts_with("in the raid") && !told {
            let because = if cause == "of hunger" {
                format!("because the camp had {} meals stored for {} mouths, and no one brought them food", self.food_stored(), self.alive() + 1)
            } else { format!("because they died {}", cause) };
            self.moment(format!("The death of {}", name), format!("{} died {} on day {}. {}", name, cause, self.clock.day(), laid), because, at);
        }
    }

    /// The graves the history left on this ground (`LocalMap::graves`) become marks: drawn as the
    /// colony's own, named on hover, opened by a click.
    pub fn adopt_graves(&mut self) {
        let graves = std::mem::take(&mut self.map.graves);
        for (x, y, text) in graves {
            self.map.features[y * self.map.width + x] = crate::local::wildlife::Feature::None;
            let who = text.split(['.', ',']).next().unwrap_or("").to_string();
            let title = if who.starts_with("A soldier") { "An old grave".to_string() } else { format!("The grave of {}", who) };
            self.marks.push(ColonyMark { at: (x as u16, y as u16), kind: MarkKind::Grave, title, text: format!("Here lies {}", text.replacen("A soldier", "a soldier", 1)), day: 0 });
        }
    }

    /// Name the colony.
    pub fn name_colony(&mut self, name: &str) {
        self.name = Some(name.to_string());
        self.interventions.push(format!("{} name {}", self.clock.tick, name));
        self.note(format!("The settlement is called {} by its patron. (your doing)", name));
    }

    /// Name a place: "called the Long Field by its patron".
    pub fn name_place(&mut self, at: Pos, name: &str) {
        self.place_names.push((at, name.to_string()));
        self.note(format!("The ground about {},{} is called {} by its patron. (your doing)", at.0, at.1, name));
    }

    fn marked(&self, p: Pos, forbidden: bool) -> bool {
        // Besieged, the ground beyond the palisade is as good as forbidden (`siege.rs`).
        if forbidden && self.under_siege_out(p) { return true; }
        self.patron.marks.iter().any(|m| m.forbidden == forbidden
            && (m.at.0 as i32 - p.0 as i32).abs().max((m.at.1 as i32 - p.1 as i32).abs()) <= m.radius as i32)
    }

    pub(crate) fn marked_at(&self, p: Pos, forbidden: bool) -> bool { self.marked(p, forbidden) }

    pub(crate) fn dream_of(&self, i: usize) -> Option<Dream> {
        self.patron.dreams.iter().find(|d| d.0 == i && d.2 > self.clock.tick).map(|d| d.1)
    }
    fn once(&mut self, key: &'static str, line: String) { if self.milestones.insert(key) { self.note(line); } }

    pub fn food_stored(&self) -> u32 { self.items.iter().filter(|i| i.kind == ItemKind::Food && i.stored).count() as u32 }
    pub fn logs_stored(&self) -> u32 { self.items.iter().filter(|i| i.kind == ItemKind::Log && i.stored).count() as u32 }
    pub fn alive(&self) -> usize { self.settlers.iter().filter(|s| s.alive).count() }
    /// Everyone who was of the camp: guests who moved on (`visitors.rs`) are not counted.
    pub fn company(&self) -> usize { self.settlers.iter().filter(|s| !(s.mind.left && s.guest_until > 0)).count() }

    /// A flat, dry 6x5 patch near the camp for the hut.
    fn find_hut_site(&self) -> Option<Pos> {
        let n = self.map.width as i32;
        let (cx, cy) = (self.camp.0 as i32, self.camp.1 as i32);
        // Beside the hall stone if the patron placed one, else near the camp.
        let (sx, sy) = self.stones.iter().find(|s| s.0 == StoneKind::Hall).map(|s| (s.1 .0 as i32, s.1 .1 as i32)).unwrap_or((cx, cy));
        let mut best: Option<(i32, Pos)> = None;
        for y in (sy - 14).max(2)..(sy + 14).min(n - HUT_H as i32 - 2) {
            for x in (sx - 14).max(2)..(sx + 14).min(n - HUT_W as i32 - 2) {
                // Not on the camp itself.
                if (x..x + HUT_W as i32).contains(&cx) && (y..y + HUT_H as i32).contains(&cy) { continue; }
                let z0 = self.map.surface_z[(y * n + x) as usize];
                let ok = (0..HUT_H as i32).all(|dy| (0..HUT_W as i32).all(|dx| {
                    let (xx, yy) = (x + dx, y + dy);
                    self.map.surface_z[(yy * n + xx) as usize] == z0 && nav::passable(&self.map, (xx as u16, yy as u16))
                }));
                if !ok { continue; }
                // Not on a stone.
                if self.stones.iter().any(|(_, at)| (x..x + HUT_W as i32).contains(&(at.0 as i32)) && (y..y + HUT_H as i32).contains(&(at.1 as i32))) { continue; }
                let d = (x - sx).abs() + (y - sy).abs();
                if best.map_or(true, |b| d < b.0) { best = Some((d, (x as u16, y as u16))); }
            }
        }
        best.map(|b| b.1)
    }

    /// Whether `p` lies on the hut's 6x5 site.
    fn in_hut_site(&self, p: Pos) -> bool {
        self.hut.as_ref().map_or(false, |h| (p.0 as u32).wrapping_sub(h.at.0 as u32) < HUT_W as u32 && (p.1 as u32).wrapping_sub(h.at.1 as u32) < HUT_H as u32)
    }

    /// The roof over `p`, if under one: the hut's or the second hut's centre cell.
    pub fn roof_over(&self, p: Pos) -> Option<Pos> {
        if let Some(h) = self.hut.as_ref().filter(|h| h.done && p.0 > h.at.0 && p.0 < h.at.0 + HUT_W as u16 - 1 && p.1 > h.at.1 && p.1 < h.at.1 + HUT_H as u16 - 1) {
            return Some((h.at.0 + HUT_W as u16 / 2, h.at.1 + HUT_H as u16 / 2));
        }
        self.projects.iter().find(|q| q.kind == projects::ProjectKind::SecondHut && q.done
            && p.0 > q.at.0 && p.0 < q.at.0 + HUT_W as u16 - 1 && p.1 > q.at.1 && p.1 < q.at.1 + HUT_H as u16 - 1)
            .map(|q| (q.at.0 + HUT_W as u16 / 2, q.at.1 + HUT_H as u16 / 2))
    }

    /// The season of the colony's year (30 days each, from spring).
    pub fn season(&self) -> crate::seasons::Season {
        use crate::seasons::Season::*;
        match ((self.clock.day().max(1) - 1) / SEASON_DAYS) % 4 { 0 => Spring, 1 => Summer, 2 => Autumn, _ => Winter }
    }

    /// The day's mean temperature (°C): the world tile's for the season.
    pub fn temperature(&self) -> f32 { self.map.season_temps[self.season() as usize] }

    /// A winter cold enough that nothing grows (under 4 °C): the bushes are bare.
    /// A deep freeze: a hard winter below -3 °C, when the rivers and lakes are ice.
    pub fn frozen(&self) -> bool { self.hard_winter() && self.temperature() < -3.0 }

    /// Dawn: in a hard winter the herds leave for their winter grounds, and come back in spring
    /// (the TODO's seasonal migration): no game to hunt meanwhile (`game_returns` waits too).
    pub(crate) fn reckon_herds(&mut self) {
        let away = self.hard_winter();
        if away == self.herds_away { return; }
        self.herds_away = away;
        let herds: Vec<String> = self.map.game.iter().map(|g| g.0.clone()).collect();
        if herds.is_empty() { return; }
        let names = crate::persona::list(&herds);
        if away {
            self.creatures.retain(|c| c.kind != creatures::CreatureKind::Game);
            self.note(format!("The {} have gone down to their winter grounds; there is nothing to hunt until spring.", names));
        } else {
            self.spawn_game();
            self.note(format!("The {} are back on the hills with the spring.", names));
        }
    }

    /// Dawn: the water freezes over, or thaws (a line each).
    pub(crate) fn reckon_ice(&mut self) {
        let f = self.frozen();
        if f == self.ice { return; }
        self.ice = f;
        if self.fishing_spots.is_empty() { return; }
        if f {
            let line = if self.jetty.is_some() { "The water freezes over; they fish now only through holes cut at the jetty." } else { "The water freezes over: there is no more fishing until the thaw." };
            self.note(line.into());
        } else {
            self.note("The ice breaks up on the water; the fishing places are open again.".into());
        }
    }

    pub fn hard_winter(&self) -> bool { self.season() == crate::seasons::Season::Winter && self.temperature() < 4.0 }

    /// Days until the next hard winter begins (0 while in it, None if winters here are mild).
    pub fn days_to_winter(&self) -> Option<u64> {
        if self.map.season_temps[3] >= 4.0 { return None; }
        if self.hard_winter() { return Some(0); }
        let day = self.clock.day().max(1) - 1;
        let year = 4 * SEASON_DAYS;
        let start = 3 * SEASON_DAYS;
        let into = day % year;
        Some(if into < start { start - into } else { year - into + start })
    }

    /// How dark it is, 0 by day to 0.55 deep in the night (dusk 19-21, dawn 5-7).
    pub fn darkness(&self) -> f32 {
        let hm = self.clock.hour() as f32 + self.clock.minute() as f32 / 60.0;
        let f = if hm >= 21.0 || hm < 5.0 { 1.0 } else if hm >= 19.0 { (hm - 19.0) / 2.0 } else if hm < 7.0 { (7.0 - hm) / 2.0 } else { 0.0 };
        0.55 * f
    }

    fn in_hut(&self, p: Pos) -> bool {
        self.in_second_hut(p) || self.hut.as_ref().map_or(false, |h| h.done && p.0 > h.at.0 && p.0 < h.at.0 + HUT_W as u16 - 1 && p.1 > h.at.1 && p.1 < h.at.1 + HUT_H as u16 - 1)
    }

    /// Each morning, if the camp cannot feed itself: move to the place on its own dry ground
    /// that can (at most once in five days), or, if none can, give the land up and leave.
    fn reckon_food(&mut self) {
        let alive = self.alive();
        // In a hard winter nothing grows anywhere: no use moving for bare bushes.
        if alive == 0 || self.hard_winter() { return; }
        let need = MEALS_NEEDED * alive as f32 / 7.0;
        let (patch, _) = dry_patches(&self.map);
        let here = self.survey_at(self.camp, &patch);
        // What the camp makes for itself counts too: the farm under the rock, the caravans' meals,
        // the store spread over a month.
        let farm = if self.projects.iter().any(|p| p.done && p.kind == projects::ProjectKind::CaveFarm) { if self.breached.is_empty() { 1.5 } else { 2.5 } } else { 0.0 };
        let caravans = if self.trade.is_some() && self.caravans > 0 { 1.6 } else { 0.0 };
        let own = farm + caravans + self.food_stored() as f32 / 30.0;
        if here.meals_a_day + own >= 0.6 * need { return; }
        // A camp with five works standing does not walk away from them for want of berries: only
        // when people starve with nothing in the store.
        let established = self.projects.iter().filter(|p| p.done).count() >= 5;
        if established && (self.food_stored() > 0 || !self.settlers.iter().any(|s| s.alive && s.starving > 0)) { return; }
        let day = self.clock.day();
        if self.last_move > 0 && day < self.last_move + 5 { return; }
        // The best place on the camp's own ground, on a 12-cell grid.
        let n = self.map.width;
        let home = patch[self.camp.1 as usize * n + self.camp.0 as usize];
        let mut best: Option<(Pos, Survey)> = None;
        for y in (6..self.map.height - 6).step_by(12) {
            for x in (6..n - 6).step_by(12) {
                if patch[y * n + x] != home { continue; }
                let sv = self.survey_at((x as u16, y as u16), &patch);
                if best.as_ref().map_or(true, |b| sv.meals_a_day > b.1.meals_a_day) { best = Some(((x as u16, y as u16), sv)); }
            }
        }
        match best {
            Some((p, sv)) if sv.meals_a_day >= need && sv.meals_a_day > 2.0 * here.meals_a_day => self.move_camp(p, &here, &sv),
            // Nowhere feeds them, or nowhere fully and they already starve (a place giving 0.4-1x
            // what they need used to leave them sitting where nothing grew: dev 48,18 starved).
            Some((_, sv)) if sv.meals_a_day < 0.4 * need || (sv.meals_a_day < need && self.settlers.iter().any(|s| s.alive && s.starving > 0)) => self.depart(&sv),
            _ => {}
        }
    }

    /// Strike camp and make it again at `to`: the store is carried, an unfinished hut is left.
    fn move_camp(&mut self, to: Pos, here: &Survey, there: &Survey) {
        let day = self.clock.day();
        let old = self.camp;
        let from_day = if self.last_move > 0 { self.last_move } else { 1 };
        self.marks.push(ColonyMark { at: old, kind: MarkKind::Stone, title: "The old camp".into(),
            text: if from_day == day { format!("They camped here on day {} and moved on: too little grew within reach.", day) }
                else { format!("They camped here from day {} to day {}, until hunger drove them on.", from_day, day) }, day });
        self.camp = to;
        self.last_move = day;
        for it in self.items.iter_mut().filter(|it| it.stored || it.reserved) { it.at = to; it.stored = true; it.reserved = false; }
        self.basket.clear();
        self.stones.retain(|s| s.0 != StoneKind::Hall);
        self.hut = self.find_hut_site().map(|a| Hut { at: a, logs_used: 0, done: false });
        self.projects.clear();
        for k in 0..self.settlers.len() {
            let st = &mut self.settlers[k];
            st.job = Job::Idle; st.path.clear(); st.carrying = None; st.work_left = 0; st.stride = 0;
        }
        self.claimed.clear();
        let line = format!("They strike camp and walk to {},{}, where the land gives {:.0} meals a day within reach.", to.0, to.1, there.meals_a_day);
        self.note(line.clone());
        self.moment("They move the camp".into(), line,
            format!("because within reach of the old camp there were {} berry bushes and {} fishing spots: {:.0} meals a day for {} mouths", here.shrubs, here.fishing, here.meals_a_day, self.alive()), to);
    }

    /// The land cannot feed them anywhere they can walk: they leave, and the colony ends.
    fn depart(&mut self, best: &Survey) {
        let day = self.clock.day();
        self.departed = Some(day);
        let line = format!("Nowhere they can walk to would feed them (at best {:.0} meals a day for {} mouths). They shoulder what they have and leave on day {}.", best.meals_a_day, self.alive(), day);
        self.note(line.clone());
        self.moment("They give the land up".into(), line, "because the land within reach holds too few berry bushes and no fishing water".into(), self.camp);
        self.milestones_hit.push("the end".into());
    }

    /// Each dawn, unless a well stands, someone fetches the camp's water: there and back to the
    /// nearest water is walking time (counted; a well ends it).
    fn draw_water(&mut self) {
        if self.projects.iter().any(|p| p.done && p.kind == projects::ProjectKind::Well) { return; }
        // Water stands in the shaft that struck the aquifer: no more walking for it.
        if self.aquifer_struck.is_some() { return; }
        let n = self.map.width as i32;
        let mut best: Option<i32> = None;
        for r in 0..60i32 {
            for dy in -r..=r { for dx in -r..=r {
                if dx.abs().max(dy.abs()) != r { continue; }
                let (x, y) = (self.camp.0 as i32 + dx, self.camp.1 as i32 + dy);
                if x < 0 || y < 0 || x >= n || y >= self.map.height as i32 { continue; }
                if self.water_at((x as u16, y as u16)) { best = Some(r); break; }
            } if best.is_some() { break; } }
            if best.is_some() { break; }
        }
        let d = best.unwrap_or(60) as u32;
        self.water_distance = d;
        // Close water costs nothing worth a word; far water costs a walk there and back.
        if d > 8 { self.water_walked += (2 * d * 5) as u64; }
    }

    /// A load laid by settler `i` (counted for them, and for the builder's share).
    pub(crate) fn count_load(&mut self, i: usize) {
        self.settlers[i].loads_laid += 1;
        if self.settlers.iter().any(|s| s.alive && s.role == Some(4)) {
            self.builder_share.1 += 1;
            if self.settlers[i].role == Some(4) { self.builder_share.0 += 1; }
        }
    }

    /// Each dawn the best at each trade, past a mark (0.45), is known for it: a moment the first
    /// time, and when a holder dies the next best takes it up (logged).
    fn reckon_roles(&mut self) {
        for k in 0..ROLES.len() {
            let holder = self.settlers.iter().position(|s| s.role == Some(k));
            let pace = |s: &Settler| (1.3 - 0.6 * s.skill[k]) * s.persona.work_time(k);
            if let Some(hd) = holder {
                if self.settlers[hd].alive {
                    // A clearly surer hand (15% quicker) takes the trade over.
                    let better = (0..self.settlers.len()).filter(|&i| i != hd && self.settlers[i].alive && self.settlers[i].guest_until == 0 && self.settlers[i].role.is_none() && pace(&self.settlers[i]) < 0.85 * pace(&self.settlers[hd]))
                        .min_by(|&a, &b| pace(&self.settlers[a]).total_cmp(&pace(&self.settlers[b])).then(a.cmp(&b)));
                    if let Some(b) = better {
                        self.settlers[hd].role = None;
                        self.settlers[b].role = Some(k);
                        let (old, new) = (self.settlers[hd].name.clone(), self.settlers[b].name.clone());
                        self.note(format!("{} has become surer at the work than {}: the camp's {} now.", new, old, ROLES[k]));
                    }
                    continue;
                }
                self.settlers[hd].role = None;
            }
            // The surest hand in practice: skill and body together (the minutes a job takes,
            // `Persona::work_time`), among those past the green.
            let best = (0..self.settlers.len()).filter(|&i| self.settlers[i].alive && self.settlers[i].guest_until == 0 && self.settlers[i].role.is_none() && self.settlers[i].skill[k] >= 0.45)
                .min_by(|&a, &b| pace(&self.settlers[a]).total_cmp(&pace(&self.settlers[b])).then(a.cmp(&b)));
            let Some(b) = best else { continue };
            self.settlers[b].role = Some(k);
            let name = self.settlers[b].name.clone();
            match holder {
                Some(hd) => {
                    let dead = self.settlers[hd].name.clone();
                    let tool = ["the basket", "the rod", "the axe", "the carrying frame", "the hammer"][k];
                    self.note(format!("With {} gone, {} takes up {}: the camp's {} now.", dead, name, tool, ROLES[k]));
                }
                None => {
                    let line = format!("{} is the camp's {} now.", name, ROLES[k]);
                    self.note(line.clone());
                    let why = if k == 4 && self.settlers[b].loads_laid > 0 { format!("because {} has laid {} loads, more than anyone", name, self.settlers[b].loads_laid) } else { format!("because no one is surer at it ({:.0}% of a master's hand)", self.settlers[b].skill[k] * 100.0) };
                    let at = self.settlers[b].pos;
                    self.moment(format!("The camp's {}", ROLES[k]), line, why, at);
                }
            }
        }
    }

    pub(crate) fn like(&mut self, a: usize, b: usize, by: i32) {
        if a == b { return; }
        *self.opinions.entry((a.min(b), a.max(b))).or_insert(0) += by;
    }

    /// Everyday warmth (a meal shared, a song heard, a festival, a cup at the tavern): +1, but
    /// only up to `FAMILIAR` (DF's relationships climb by familiarity only so far; what lifts them
    /// higher is what people do for one another). Without the cap, a year of shared meals had
    /// everyone at +100 for everyone, and no grudge or punishment counted for anything.
    pub(crate) fn warm(&mut self, a: usize, b: usize) {
        if a == b || self.opinion(a, b) >= FAMILIAR { return; }
        self.like(a, b, 1);
    }

    /// What `a` and `b` think of each other.
    pub fn opinion(&self, a: usize, b: usize) -> i32 { self.opinions.get(&(a.min(b), a.max(b))).copied().unwrap_or(0) }

    /// The closest friend of `i` (opinion 6 or more), if any.
    pub fn friend_of(&self, i: usize) -> Option<usize> {
        (0..self.settlers.len()).filter(|&j| j != i && self.settlers[j].alive).map(|j| (self.opinion(i, j), j)).filter(|&(o, _)| o >= 6)
            .max_by_key(|&(o, j)| (o, std::cmp::Reverse(j))).map(|(_, j)| j)
    }

    /// Grudges from the past: one who hates a people dislikes a settler of that people. Each dawn
    /// the worst pair may quarrel (once), and the closest pair is said to be close (once).
    fn reckon_company(&mut self) {
        let n = self.settlers.len();
        for a in 0..n {
            let Some((ref text, crate::history::EntityId::Faction(f))) = self.settlers[a].past.as_ref().and_then(|p| p.feeling.clone()) else { continue };
            if !(text.starts_with("hates") || text.starts_with("has not forgiven")) { continue; }
            for b in 0..n {
                if b == a || !self.settlers[b].alive { continue; }
                let key = (a.min(b), a.max(b));
                if self.settlers[b].past.as_ref().and_then(|p| p.people) == Some(f) && !self.grudges.contains(&key) {
                    self.grudges.insert(key);
                    self.like(a, b, -8);
                }
            }
        }
        let alive: Vec<usize> = (0..n).filter(|&k| self.settlers[k].alive).collect();
        let pairs: Vec<(usize, usize)> = alive.iter().flat_map(|&a| alive.iter().filter(move |&&b| b > a).map(move |&b| (a, b))).collect();
        if let Some(&(a, b)) = pairs.iter().filter(|p| self.opinion(p.0, p.1) <= -5).min_by_key(|p| (self.opinion(p.0, p.1), p.0, p.1)) {
            if !self.quarrelled {
                self.quarrelled = true;
                let (na, nb) = (self.settlers[a].name.clone(), self.settlers[b].name.clone());
                let hater = if self.settlers[a].past.as_ref().and_then(|p| p.feeling.as_ref()).map_or(false, |f| f.0.starts_with("hates") || f.0.starts_with("has not forgiven")) { a } else { b };
                let other = if hater == a { b } else { a };
                let why = self.settlers[hater].past.as_ref().and_then(|p| p.feeling.as_ref()).map(|f| f.0.clone()).unwrap_or_default();
                let _ = (na, nb);
                self.note(format!("{} and {} quarrel by the fire: {} {}, and {} is of that people.", self.settlers[hater].name, self.settlers[other].name, self.settlers[hater].name, why.replacen("hates ", "hates ", 1), self.settlers[other].name));
                let (hn, on) = (self.settlers[hater].name.clone(), self.settlers[other].name.clone());
                self.feel(hater, mind::Feel::Quarrel { with: on });
                self.feel(other, mind::Feel::Quarrel { with: hn });
                self.make_peace(hater, other);
            }
        }
        if let Some(&(a, b)) = pairs.iter().filter(|p| self.opinion(p.0, p.1) >= 6).max_by_key(|p| (self.opinion(p.0, p.1), std::cmp::Reverse(p.0), std::cmp::Reverse(p.1))) {
            let key: &'static str = "close pair";
            let (na, nb) = (self.settlers[a].name.clone(), self.settlers[b].name.clone());
            self.once(key, format!("{} and {} have grown close: they eat together, and work side by side when they can.", na, nb));
        }
    }

    /// Meals the camp wants stored: six a head, or, with a hard winter within 45 days and a
    /// place for food to keep (a smokehouse), most of a winter's eating.
    pub fn food_goal(&self) -> u32 {
        let base = FOOD_PER_SETTLER * self.alive() as u32;
        let smoke = self.projects.iter().any(|p| p.done && p.kind == projects::ProjectKind::Smokehouse);
        match self.days_to_winter() {
            // Enough to last the winter with what spoils meanwhile: cK(e^(W/K) - 1).
            Some(d) if d > 0 && d <= 45 && smoke => {
                let c = self.alive() as f32 * MEALS_NEEDED / 7.0;
                let k = self.keeps_days().max(1) as f32;
                base.max((c * k * ((SEASON_DAYS as f32 / k).exp() - 1.0) * 1.05) as u32)
            }
            _ => base,
        }
    }

    /// A new season: said in the log, with what it means for the store.
    fn season_turns(&mut self) {
        // The pen grows each season (`livestock.rs`).
        self.pen_breeds();
        use crate::seasons::Season::*;
        let t = self.temperature();
        let alive = self.alive().max(1) as f32;
        let _ = alive;
        let days_of_food = self.days_of_food();
        let days_word = if days_of_food.round() as i64 == 1 { "day" } else { "days" };
        let line = match self.season() {
            Spring => {
                if self.map.season_temps[3] < 4.0 && !self.milestones_hit.iter().any(|m| m == "the first winter") { self.milestones_hit.push("the first winter".into()); }
                if self.clock.day() >= 4 * SEASON_DAYS && !self.milestones_hit.iter().any(|m| m == "a year") { self.milestones_hit.push("a year".into()); }
                format!("Spring comes ({:.0} °C): the bushes bud again.", t)
            }
            Summer => format!("Summer comes ({:.0} °C).", t),
            Autumn => match self.days_to_winter() {
                Some(d) => format!("Autumn comes ({:.0} °C): the berries now are the last of the year. Winter is {} days off, and the store holds {:.0} {} of food.", t, d, days_of_food, days_word),
                None => format!("Autumn comes ({:.0} °C); the winters here are mild.", t),
            },
            Winter => if self.hard_winter() { format!("Winter comes ({:.0} °C): the bushes are bare, and what is stored must last. The store holds {:.0} {} of food.", t, days_of_food, days_word) }
                else { format!("Winter comes ({:.0} °C), mild enough that the bushes still bear.", t) },
        };
        self.note(line);
    }

    /// Each dawn some of the stored food spoils: it keeps `FOOD_KEEPS_DAYS` on average, or
    /// `FOOD_KEEPS_DAYS_RACK` once a drying rack stands.
    /// How many days food keeps in the store (a day's loss is the store over this).
    pub fn keeps_days(&self) -> u32 {
        let rack = self.projects.iter().any(|p| p.done && p.kind == projects::ProjectKind::DryingRack);
        let smoke = self.projects.iter().any(|p| p.done && p.kind == projects::ProjectKind::Smokehouse);
        let store = self.projects.iter().any(|p| p.done && p.kind == projects::ProjectKind::Storehouse);
        let keeps = if smoke { 90 } else if rack { FOOD_KEEPS_DAYS_RACK } else { FOOD_KEEPS_DAYS };
        // A storehouse keeps the rain off: food keeps half as long again.
        let keeps = if store { keeps * 3 / 2 } else { keeps };
        // A cellar in the rock keeps food three times as long.
        let keeps = if self.projects.iter().any(|p| p.done && p.kind == projects::ProjectKind::Cellar) { keeps * 3 } else { keeps };
        // Salt from the caravan, twice as long (`liaison.rs`).
        if self.salt_until > self.clock.day() { keeps * 2 } else { keeps }
    }

    /// Days the store lasts: eating and spoiling together (a store S, eating c a day, keeping K
    /// days, lasts K ln(1 + S / cK)).
    pub fn days_of_food(&self) -> f32 {
        let c = (self.alive().max(1) as f32 * MEALS_NEEDED / 7.0).max(0.1);
        let k = self.keeps_days().max(1) as f32;
        k * (1.0 + self.food_stored() as f32 / (c * k)).ln()
    }

    fn spoil(&mut self) {
        let rack = self.projects.iter().any(|p| p.done && p.kind == projects::ProjectKind::DryingRack);
        let smoke = self.projects.iter().any(|p| p.done && p.kind == projects::ProjectKind::Smokehouse);
        let keeps = self.keeps_days();
        let n = self.food_stored() / keeps;
        if n == 0 { return; }
        for _ in 0..n {
            if let Some(k) = self.items.iter().rposition(|it| it.kind == ItemKind::Food && it.stored && !it.reserved) {
                self.items.remove(k);
                self.fix_item_refs(k);
            }
        }
        // Only said while there is no rack: with one, the loss is small and daily.
        // (Said the first time and every fifth day: the loss is daily, the line need not be.)
        if !rack && !smoke && (self.milestones.insert("spoilage") || self.clock.day() % 5 == 0) {
            self.note(format!("{} in the store; nothing keeps without a drying rack.", if n == 1 { "A meal spoils".to_string() } else { format!("{} meals spoil", n) }));
        }
    }

    /// How cold tonight is, 0.75 (mild) to 1.2 (bitter), hashed from the seed and the day; the
    /// night after 21:00 belongs to the day it began.
    pub fn night_cold(&self) -> f32 {
        let day = if self.clock.hour() < 6 { self.clock.day().saturating_sub(1) } else { self.clock.day() };
        let mut x = self.seed ^ day.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ 0xC01D;
        x ^= x >> 31; x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9); x ^= x >> 29;
        0.75 + 0.45 * (x % 1000) as f32 / 1000.0
    }

    /// Advance the colony by one game minute.
    pub fn tick(&mut self) {
        self.clock.tick += 1;
        if self.departed.is_some() { return; }
        if self.clock.hour() == 7 && self.clock.minute() == 0 { self.reckon_food(); }
        // Favour returns with the dawn.
        let day = self.clock.day();
        if day > self.patron.last_refill_day && self.clock.hour() >= 6 {
            self.patron.last_refill_day = day;
            self.patron.favour = (self.patron.favour + 1).min(FAVOUR_MAX);
        }
        self.step_creatures();
        self.caravan_tick();
        if self.clock.tick % 10 == 0 { self.caravan_arrives(); }
        if self.clock.hour() == 10 && self.clock.minute() == 0 { self.migrants_arrive(); }
        if self.arc.is_some() { self.arc_tick(); }
        if self.relic.is_some() { self.relic_tick(); }
        if self.hoard_due.is_some() { self.hoard_home(); }
        if !self.map.places.is_empty() { self.explore_tick(); }
        // Needs fall an hour at a time (`needs.rs`).
        if self.clock.minute() == 0 { self.needs_hour(); }
        if self.clock.hour() == 6 && self.clock.minute() == 0 && self.clock.day() > 1 && (self.clock.day() - 1) % SEASON_DAYS == 0 {
            self.season_turns();
        }
        if self.clock.hour() == 6 && self.clock.minute() == 0 {
            // (Those who spent the night at the camp: a fisher out at a far river is cold with
            // or without a fire at home.)
            let camp = self.camp;
            self.chilled_nights += self.settlers.iter().filter(|s| s.alive && s.exposure >= 0.5
                && (s.pos.0 as i32 - camp.0 as i32).abs().max((s.pos.1 as i32 - camp.1 as i32).abs()) <= 12).count() as u32;
            self.spoil(); self.reckon_hunger_days(); self.field_season(); self.draw_water(); self.plan_projects(); self.reckon_company(); self.reckon_roles(); self.arm_militia(); self.reckon_wounds(); self.reckon_temper(); self.reckon_needs(); self.reckon_minds(); self.reckon_society(); self.reckon_hollow(); self.lord_arrives(); self.lord_displeased(); self.lord_demands(); self.sellsword_comes(); self.reckon_expedition(); self.vampire_dawn(); self.reckon_prisoner(); self.reckon_regard(); self.reckon_snatched(); self.reckon_siege(); self.reckon_guilds(); self.reckon_rising(); self.reckon_old_fields(); self.reckon_responses(); self.reckon_priest(); self.reckon_war_call(); self.reckon_cook(); self.reckon_clothes(); self.reckon_dreams(); self.reckon_childhood(); self.reckon_rations(); self.reckon_ice(); self.reckon_herds(); self.reckon_rooms(); self.lord_quarters(); self.place_artifacts(); self.regrow(); self.reckon_tithe(); self.reckon_justice(); self.reckon_mood(); self.reckon_pets(); self.reckon_years(); self.pen_slaughter(); self.reckon_family(); self.reckon_thirst(); self.moon_sets();
        }
        if self.clock.minute() == 0 { self.evil_weather(); }
        if self.clock.hour() == 16 && self.clock.minute() == 0 { self.cook_supper(); }
        if self.clock.hour() == 23 && self.clock.minute() == 0 { self.night_theft(); self.artifact_thief(); if self.prisoner.is_some() { self.prisoner_escapes(); } }
        if self.vampire.is_some() {
            if self.clock.minute() == 0 { self.vampire_hour(); }
            if self.clock.hour() == 2 && self.clock.minute() == 0 { self.vampire_feeds(); }
        }
        if self.clock.hour() == 19 && self.clock.minute() == 0 { self.drill_done(); }
        if self.clock.hour() == 7 && self.clock.minute() == 0 { self.cave_harvest(); }
        if self.clock.hour() == 23 && self.clock.minute() == 30 && !self.restless.is_empty() { self.ghosts_walk(); }
        if !self.visitors.is_empty() || self.settlers.iter().any(|s| s.guest_until > 0) {
            if self.clock.hour() == 15 && self.clock.minute() == 0 { self.visitors_arrive(); }
            if self.clock.hour() == 20 && self.clock.minute() == 30 { self.guests_perform(); }
            if self.clock.hour() == 9 && self.clock.minute() == 0 { self.guests_leave(); }
        }
        if self.clock.hour() == 20 && self.clock.minute() == 0 { self.tavern_evening(); }
        if self.clock.hour() == 20 && self.clock.minute() == 0 {
            // At the turn of a season, a festival; else, perhaps, a song.
            if self.clock.day() > 1 && (self.clock.day() - 1) % SEASON_DAYS == 0 { self.festival(); } else { self.evening_arts(); }
        }
        if self.clock.hour() == 21 && self.clock.minute() == 0 {
            self.burn_wood();
            if self.night_cold() > 1.1 { self.note("The night comes on bitter cold.".into()); }
        }
        let night = self.clock.is_night();
        for i in 0..self.settlers.len() {
            if !self.settlers[i].alive { continue; }
            self.update_needs(i, night);
            if !self.settlers[i].alive { continue; }
            // Choose: when the job is done, or every hour if something matters much more now.
            let idle = self.settlers[i].job == Job::Idle;
            if (idle && self.clock.tick >= self.settlers[i].retry_at) || (!idle && self.clock.tick % 60 == (i as u64 * 7) % 60) {
                self.decide(i, idle);
            }
            self.act(i);
        }
    }

    fn update_needs(&mut self, i: usize, night: bool) {
        // A hut, or a room under rock (warm whatever the fire).
        let under_rock = self.below(i);
        let sheltered = self.in_hut(self.settlers[i].pos) || under_rock;
        // The cold outside at night: the fire takes the edge off (and a stocked woodpile keeps it
        // burning), the young and the old feel it more.
        let pos = self.settlers[i].pos;
        let by_fire = (pos.0 as i32 - self.camp.0 as i32).abs().max((pos.1 as i32 - self.camp.1 as i32).abs()) <= 7;
        let woodpile = self.fire_kept();
        let frail = self.settlers[i].past.as_ref().map_or(false, |p| p.age < 14 || p.age >= 55);
        let cold = if by_fire { 0.8 * if woodpile { 0.5 } else { 1.0 } } else { 1.0 } * if frail { 1.25 } else { 1.0 } * self.night_cold()
            // The season: colder nights the colder the day (1x at 8 °C and above, 2x at -8 °C).
            * (1.0 + ((8.0 - self.temperature()) / 16.0).clamp(0.0, 1.0));
        let s = &mut self.settlers[i];
        let asleep = s.job == Job::Sleep && s.work_left > 0 && s.path.is_empty();
        s.hunger = (s.hunger + 1.0 / (18.0 * 60.0)).min(1.0);
        // The tireless tire slower (endurance, `Persona::tiring`).
        let tiring = s.persona.tiring();
        s.fatigue = if asleep { (s.fatigue - 1.0 / (7.0 * 60.0)).max(0.0) } else { (s.fatigue + tiring / (17.0 * 60.0)).min(1.0) };
        // A roof keeps the wind off, but in a hard winter a hut with no fire is cold too.
        let cold_hut = sheltered && !under_rock && night && self.hard_winter() && !woodpile;
        let s = &mut self.settlers[i];
        // The tough feel the cold less (toughness and resistance to sickness).
        let hardy = s.persona.hardiness();
        let cold = cold / hardy.powf(0.3);
        s.exposure = if cold_hut { (s.exposure + 0.4 * cold / (10.0 * 60.0)).min(1.0) }
            else if sheltered { (s.exposure - 1.0 / 120.0).max(0.0) }
            else if night { (s.exposure + cold / (10.0 * 60.0)).min(1.0) }
            else { (s.exposure - 1.0 / (6.0 * 60.0)).max(0.0) };
        if s.hunger >= 1.0 { s.starving += 1; } else { s.starving = 0; }
        // A night chilled through makes them ill for two days.
        // Ill for a day; once well, hardened against it for three more.
        let ill_at = (ILL_AT * hardy.powf(0.25)).min(0.98);
        let falls_ill = s.exposure >= ill_at && (s.ill_until == 0 || s.ill_until + 3 * TICKS_PER_DAY <= self.clock.tick);
        // Those who heal quickly are up sooner (recuperation).
        if falls_ill { s.ill_until = self.clock.tick + (TICKS_PER_DAY as f32 / s.persona.healing()) as u64; }
        let starving_days = s.starving as u64 / TICKS_PER_DAY;
        let name = s.name.clone();
        if s.starving == TICKS_PER_DAY as u32 / 4 {
            self.note(format!("{} is starving.", name));
        }
        if falls_ill { self.note(format!("{} spent the night chilled to the bone and falls ill.", name)); self.feel(i, mind::Feel::Ill); }
        // Company: time spent within a few cells of someone (the sociable need it).
        if self.clock.tick % 10 == 0 {
            let me = self.settlers[i].pos;
            let near = self.settlers.iter().enumerate().any(|(j, o)| j != i && o.alive && (o.pos.0 as i32 - me.0 as i32).abs().max((o.pos.1 as i32 - me.1 as i32).abs()) <= 4);
            if near { self.settlers[i].mind.company += 10; }
            for sh in self.stones.iter().filter(|s| s.0 == StoneKind::Shrine).map(|s| s.1).chain(self.temple_at()).collect::<Vec<_>>() {
                if matches!(self.settlers[i].job, Job::Wander(_)) && (sh.0 as i32 - me.0 as i32).abs().max((sh.1 as i32 - me.1 as i32).abs()) <= 3 { self.settlers[i].mind.prayed = true; }
            }
        }
        if starving_days >= 4 {
            self.settlers[i].alive = false;
            self.note(format!("{} died of hunger.", name));
            self.bury(i, "of hunger");
        }
        // Starving with nothing to gather in reach: say why, once.
        if self.settlers.get(i).map_or(false, |s| s.alive && s.starving == TICKS_PER_DAY as u32 / 4) && self.food_stored() == 0 {
            let sv = self.survey();
            if sv.meals_a_day < MEALS_NEEDED {
                self.once("no food in reach", format!("There is nothing to gather within reach of the camp: {} berry bushes, {} fishing spots.", sv.shrubs, sv.fishing));
            }
        }
    }

    /// Pick the best thing to do now. `free`: the settler has nothing in hand.
    fn decide(&mut self, i: usize, free: bool) {
        // A broken mind does what its break makes it do (`mind.rs`).
        if let Some((job, why)) = self.broken_choice(i) {
            if free || std::mem::discriminant(&job) != std::mem::discriminant(&self.settlers[i].job) { self.release(i); self.start(i, job, why); }
            return;
        }
        // An infant stays by its mother (`family.rs`).
        if let Some((job, why)) = self.infant_choice(i) {
            if free || std::mem::discriminant(&job) != std::mem::discriminant(&self.settlers[i].job) { self.release(i); self.start(i, job, why); }
            return;
        }
        // A child plays or tags along after a parent (`childhood.rs`).
        if let Some((job, why)) = self.child_choice(i) {
            if free || std::mem::discriminant(&job) != std::mem::discriminant(&self.settlers[i].job) { self.release(i); self.start(i, job, why); }
            return;
        }
        // Under the full moon the cursed are out in the dark (`curse.rs`).
        let d = self.clock.day();
        if self.cursed.contains(&i) && ((d % 28 == 14 && self.clock.hour() >= 21) || (d % 28 == 15 && self.clock.hour() < 6)) && !free { return; }
        // At the patron's bell, everyone keeps under a roof until dawn (`ring_bell`): they eat,
        // sleep, or wait by their beds.
        if self.bell_rung() && self.settlers[i].guest_until == 0 {
            let s = &self.settlers[i];
            let (job, why) = if s.hunger >= 0.6 && self.food_stored() > 0 { (Job::Eat, "Eating quickly, then indoors at the bell".to_string()) }
                else if self.clock.is_night() || s.fatigue > 0.3 { (Job::Sleep, "Abed early at the patron's bell".to_string()) }
                else { (Job::Wander(self.sleep_spot(i)), "Keeping indoors at the patron's bell".to_string()) };
            if free || std::mem::discriminant(&job) != std::mem::discriminant(&self.settlers[i].job) { self.release(i); self.start(i, job, why); }
            return;
        }
        // One in the stocks stays by the fire (`justice.rs`).
        if let Some((job, why)) = self.stocks_choice(i) {
            if free || std::mem::discriminant(&job) != std::mem::discriminant(&self.settlers[i].job) { self.release(i); self.start(i, job, why); }
            return;
        }
        // A strange mood holds its settler at the workshop (`mood.rs`).
        if let Some((job, why)) = self.mood_choice(i) {
            if free || std::mem::discriminant(&job) != std::mem::discriminant(&self.settlers[i].job) { self.release(i); self.start(i, job, why); }
            return;
        }
        let s = &self.settlers[i];
        let food = self.food_stored();
        let food_goal = self.food_goal();
        let night = self.clock.is_night();
        // The work under way (the hut, then the colony's own projects) and what it still needs.
        let material = self.building_material();
        let hut_pending = material.is_some();
        let (logs_needed, stones_needed) = self.material_needed();
        // What is on hand: stored, on its way, or lying where someone can fetch it (a log or a
        // stone left out of reach must not stop the work).
        let on_hand = |m: ItemKind| self.items.iter().filter(|it| it.kind == m && (it.stored || it.reserved || self.reachable_from(self.camp, it.at))).count() as u32;
        let logs_about = on_hand(ItemKind::Log);
        let stones_about = on_hand(ItemKind::Stone);
        let work = match (self.hut.as_ref().filter(|h| !h.done), self.active_project()) {
            (Some(_), _) => "the hut".to_string(),
            (None, Some(k)) => self.projects[k].kind.word().to_string(),
            _ => String::new(),
        };
        let loose = self.items.iter().position(|it| !it.stored && !it.reserved && self.reachable_from(s.pos, it.at));

        // Utilities, 0..~3. Each option says why.
        let mut options: Vec<(f32, Job, String)> = Vec::new();
        // They eat when a meal fills them (hunger 0.6: a meal takes 0.8), not before: eating
        // early wasted half of every meal and emptied the winter store.
        // (One at the workshop finishes the piece before eating unless truly hungry: a craft
        // takes six hours, and meals at 0.6 broke off nearly every one.)
        let at_bench = s.job == Job::Craft && s.path.is_empty() && s.work_left > 0;
        if food > 0 && s.hunger > if at_bench { 0.85 } else { 0.6 } {
            // Woken by hunger they eat before lying down again (a tired settler at 95% had woken,
            // chosen sleep over food and woken again every minute).
            options.push((if s.hunger >= 0.95 { 4.0 } else { s.hunger * s.hunger * 3.0 }, Job::Eat, format!("Hungry ({:.0}%) and there is food at the camp", s.hunger * 100.0)));
        }
        // Night is for sleeping, rested or not; by day only the tired lie down.
        let sleepy = s.fatigue * s.fatigue * 2.5 + if night { 0.3 + if s.fatigue > 0.25 { 0.5 } else { 0.0 } } else { 0.0 }
            + if night && s.exposure > 0.3 { 0.3 } else { 0.0 };
        // (At the bench by day, only the worn out lie down: see the meal above.)
        if (s.fatigue > 0.2 && !(at_bench && !night && s.fatigue < 0.85)) || night {
            let place = if self.bedroom_of(i).is_some() { "in a bedroom of their own under the rock" }
                else if !self.hall_cells.is_empty() { "in the hall under the hill" }
                else if self.second_hut_bed(i).is_some() { "in the second hut" }
                else if self.hut.as_ref().map_or(false, |h| h.done) { if i < HUT_BEDS { "in the hut" } else { "by the fire, the hut being full" } }
                else { "by the fire" };
            options.push((sleepy, Job::Sleep, format!("Tired ({:.0}%){}; sleeping {}", s.fatigue * 100.0, if night { " and it is night" } else { "" }, place)));
        }
        // Hunger first: with under two meals a head stored and someone starving, the whole camp
        // looks for food (across the whole map if need be) and nobody builds.
        let hungry_camp = food < 2 * self.alive() as u32 && self.settlers.iter().any(|o| o.alive && o.hunger > 0.85);
        if food < food_goal {
            let short = (food_goal - food) as f32 / food_goal as f32 + if hungry_camp { 1.0 } else { 0.0 };
            let reach = |c: &Colony, ok: &dyn Fn(&Colony, Pos) -> bool| c.nearest(s.pos, |c, p| ok(c, p))
                .or_else(|| if hungry_camp { c.nearest_within(s.pos, c.map.width as i32, |c, p| ok(c, p)) } else { None });
            let why = |what: &str, t: Pos| if hungry_camp { format!("The camp is going hungry ({} meals for {}); {} at {},{}", food, self.alive(), what, t.0, t.1) }
                else { format!("The camp has {} of the {} meals it needs; {} at {},{}", food, food_goal, what, t.0, t.1) };
            // Far from the fire is worth less: the walk home eats the day (dev 76 fished 46 cells
            // out while 140 bushes ripened by the fire). 1 at the camp, 0.6 at 40 cells.
            let camp = self.camp;
            let far = |t: Pos| 1.0 - 0.4 * ((t.0 as i32 - camp.0 as i32).abs().max((t.1 as i32 - camp.1 as i32).abs()) as f32 / 40.0).min(1.0);
            // In a hard winter the bushes are bare: no use looking.
            if let Some(t) = if !self.any_ripe_shrub() { None } else {
                self.nearest_ripe_shrub(s.pos, WORK_RADIUS).or_else(|| if hungry_camp { self.nearest_ripe_shrub(s.pos, self.map.width as i32) } else { None })
            } {
                options.push(((0.4 + short) * s.taste[0] * far(t), Job::Forage(t), why("picking berries", t)));
            }
            // Game: the nearest within reach (the whole map when the camp is going hungry).
            // (Never across the map: a hunt 70 cells off is a day's walk for six meals.)
            // (Below berries, and only near the camp: a herd 60 cells off drew the whole camp
            // after it while the bushes by the fire went unpicked, and the hut never rose.)
            if let Some((id, at)) = self.nearest_game(self.camp, FORAGE_RADIUS) {
                let name = self.creatures.iter().find(|c| c.id == id).map(|c| c.name.clone()).unwrap_or_default();
                // One who loves the creature hunts it only when hunger presses (`persona` likes).
                let fond = fond_of(&s.persona, &name);
                let w = (0.3 + short) * s.taste[0] * far(at) * if fond { 0.35 } else { 1.0 };
                let mut text = why(&format!("hunting the {}", name), at);
                if fond { text.push_str(&format!(", though {} is fond of {}", if s.persona.female { "she" } else { "he" }, s.persona.likes.creature.0)); }
                options.push((w, Job::Hunt(id), text));
            }
            let fish = self.nearest_fishing(s.pos, WORK_RADIUS).or_else(|| if hungry_camp { self.nearest_fishing(s.pos, self.map.width as i32) } else { None })
                // (Debug: PLANET_FORCE_CAVERN=1 sends the fishers below too.)
                .filter(|_| self.cave_fish.is_empty() || std::env::var("PLANET_FORCE_CAVERN").is_err());
            if let Some(t) = fish {
                options.push(((0.35 + short) * s.taste[1] * far(t), Job::Fish(t), why("fishing", t)));
            } else if let Some((t, down, name)) = self.cave_fishing_spot() {
                // No water to fish near the camp (or it is ice): the black water of the cavern
                // the stair reaches (`delve.rs`).
                options.push(((0.3 + short) * s.taste[1] * 0.8, Job::Fish(t), format!("The camp has {} of the {} meals it needs; fishing the still water of {}, {} levels down", food, food_goal, name, down)));
            }
        }
        let fellers = self.settlers.iter().filter(|o| o.alive && matches!(o.job, Job::Fell(_))).count() as u32;
        if logs_needed > 0 && logs_about + 2 * fellers < logs_needed {
            // (Debug: PLANET_FORCE_CAVERN=1 sends the fellers below once the cavern is reached.)
            let force_below = self.cavern_level().is_some() && std::env::var("PLANET_FORCE_CAVERN").is_ok();
            // (One search: a whole-map ring search twice a decision had cost a treeless camp most
            // of its run.)
            let any_tree = if force_below { None } else { self.nearest_tree(s.pos) };
            let near = any_tree.filter(|t| (t.0 as i32 - self.camp.0 as i32).abs().max((t.1 as i32 - self.camp.1 as i32).abs()) <= 30);
            if let Some(t) = near {
                options.push((0.75 * s.taste[2], Job::Fell(t), format!("{} needs {} more logs; felling the tree at {},{}", capital(&work), logs_needed - logs_about.min(logs_needed), t.0, t.1)));
            } else if let Some(t) = self.cavern_tree() {
                // No timber near the camp: the fungus trees of the cavern the stair reaches.
                options.push((0.7 * s.taste[2], Job::Fell(t), format!("{} needs {} more logs and no tree stands near the camp; felling a fungus tree in the cavern below", capital(&work), logs_needed - logs_about.min(logs_needed))));
            } else if let Some(t) = any_tree {
                options.push((0.75 * s.taste[2], Job::Fell(t), format!("{} needs {} more logs; felling the tree at {},{}", capital(&work), logs_needed - logs_about.min(logs_needed), t.0, t.1)));
            }
        }
        let quarriers = self.settlers.iter().filter(|o| o.alive && matches!(o.job, Job::Quarry(_))).count() as u32;
        if stones_needed > 0 && stones_about + 2 * quarriers < stones_needed {
            if let Some(t) = self.nearest(s.pos, |c, p| c.is_quarry_stone(p)) {
                options.push((0.75 * s.taste[2], Job::Quarry(t), format!("{} needs {} more stones, and there is no timber in reach; breaking stone at {},{}", capital(&work), stones_needed - stones_about.min(stones_needed), t.0, t.1)));
            }
        }
        if let Some(k) = loose {
            let is_food = self.items[k].kind == ItemKind::Food;
            let what = match self.items[k].kind { ItemKind::Food => "food", ItemKind::Log => "a log", ItemKind::Stone => "a stone" };
            let short = if is_food && food < food_goal { 1.0 } else { 0.0 };
            options.push(((0.8 + short) * s.taste[3], Job::Haul(k), format!("{} lies at {},{}; carrying it to the camp", capital(what), self.items[k].at.0, self.items[k].at.1)));
        }
        // No more builders than there are logs at the camp to set.
        let builders = self.settlers.iter().filter(|o| o.alive && o.job == Job::Build).count() as u32;
        let stored = |m: ItemKind| self.items.iter().filter(|it| it.kind == m && it.stored).count() as u32;
        if let Some(m) = material.filter(|m| stored(*m) > builders && !night) {
            let (used, needed) = match (self.hut.as_ref().filter(|h| !h.done), self.active_project()) {
                (Some(h), _) => (h.logs_used, HUT_LOGS),
                (None, Some(k)) => (self.projects[k].used, self.projects[k].needed),
                _ => (0, 0),
            };
            let stuff = if m == ItemKind::Stone { "stones" } else { "logs" };
            let why = self.active_project().filter(|_| self.hut.as_ref().map_or(false, |h| h.done)).map(|k| format!(", {}", self.projects[k].why)).unwrap_or_default();
            // The camp's builder lays most of it; the others bring the loads.
            let builder = self.settlers.iter().position(|o| o.alive && o.role == Some(4));
            let (bias, note) = match builder { Some(b) if b == i => (1.5, " (the camp's builder)".to_string()), Some(b) => (0.12, format!(", leaving the laying to {}", self.settlers[b].name)), None => (1.0, String::new()) };
            options.push((0.85 * s.taste[4] * bias, Job::Build, format!("There are {} at the camp; raising {} ({} of {} in){}{}", stuff, work, used, needed, why, note)));
        }
        // Beside a friend: the same kind of work a close friend is doing pulls a little harder.
        if let Some(f) = self.friend_of(i) {
            let fj = self.settlers[f].job;
            let fname = self.settlers[f].name.clone();
            for o in options.iter_mut() {
                let same = matches!((o.1, fj), (Job::Forage(_), Job::Forage(_)) | (Job::Fish(_), Job::Fish(_)) | (Job::Fell(_), Job::Fell(_)) | (Job::Build, Job::Build) | (Job::Quarry(_), Job::Quarry(_)));
                if same { o.0 += 0.2; o.2.push_str(&format!(", beside {}", fname)); }
            }
        }
        // (A work that feeds the camp goes on: `projects::feeds`.)
        let feeding = self.active_project().map_or(false, |k| !self.projects[k].done && projects::feeds(self.projects[k].kind));
        if hungry_camp && !feeding {
            options.retain(|o| !matches!(o.1, Job::Fell(_) | Job::Quarry(_) | Job::Build | Job::Dig(..))
                && !matches!(o.1, Job::Haul(k) if self.items[k].kind != ItemKind::Food));
        }
        // Digging: the next cell of the hall or cellar under way.
        // (As many diggers as the camp has picks: `picks`.)
        let digging = self.settlers.iter().enumerate().filter(|(j, o)| *j != i && o.alive && matches!(o.job, Job::Dig(..))).count();
        if !night && digging < self.picks() {
            if let Some((p, z, _)) = self.dig_target() {
                let kind = self.projects.iter().find(|q| !q.done && projects::is_dig(q.kind)).map(|q| (q.kind.word(), q.used, q.needed, q.why.clone()));
                if let Some((word, used, needed, why)) = kind {
                    options.push((0.9 * s.taste[2], Job::Dig(p, z), format!("Cutting {} into the rock at {},{} ({} of {} cells) with one of the camp's {} picks, {}", word, p.0, p.1, used, needed, self.picks(), why)));
                }
            }
        }
        // The ill rest, whatever else needs doing.
        if s.ill_until > self.clock.tick {
            options.retain(|o| matches!(o.1, Job::Eat | Job::Sleep));
            options.push((1.5, Job::Sleep, "Ill from the cold; resting by the fire".into()));
        }
        // Idle hours are spent by the fire, or by the shrine if the patron set one.
        let shrine = self.stones.iter().find(|s| s.0 == StoneKind::Shrine).map(|s| s.1);
        let (rest_at, rest_why) = match shrine { Some(p) if i % 2 == 0 => (p, "resting by the shrine"), _ => (self.camp, "resting near the fire") };
        let spots: Vec<Pos> = (0..8).map(|_| (rest_at.0.saturating_add_signed(self.rng.gen_range(-4..=4)), rest_at.1.saturating_add_signed(self.rng.gen_range(-4..=4)))).collect();
        let wander_to = spots.into_iter().find(|&p| nav::passable(&self.map, p) && !self.in_hut(p) && p != rest_at).unwrap_or(self.camp);
        if self.mandate != Some(society::Mandate::NoIdleHands) || night {
            options.push((0.05, Job::Wander(wander_to), format!("Nothing needs doing; {}", rest_why)));
        } else {
            // The mandate forbids idling, but with nothing to do they go looking for work.
            options.push((0.01, Job::Wander(wander_to), "No hand is to be idle: looking for work about the camp".to_string()));
        }
        // The others gather what a strange mood demands (`mood.rs`).
        if let Some(m) = self.mood.as_ref().filter(|m| !m.done && m.who != i).cloned() {
            let who = self.settlers[m.who].name.clone();
            let stored = |k: ItemKind| self.items.iter().filter(|it| it.stored && it.kind == k).count();
            if stored(ItemKind::Log) < 3 {
                if let Some(t) = self.nearest_tree(s.pos) { options.push((0.95, Job::Fell(t), format!("{} wants wood for the work the mood demands; felling the tree at {},{}", who, t.0, t.1))); }
            }
            if stored(ItemKind::Stone) < 3 {
                if let Some(t) = self.nearest(s.pos, |c, p| c.is_quarry_stone(p)) { options.push((0.95, Job::Quarry(t), format!("{} wants stone for the work the mood demands; breaking stone at {},{}", who, t.0, t.1))); }
            }
        }
        // Those who know the tale of a lost thing near here search for it (`relic.rs`).
        if let Some(o) = self.relic_option(i) { options.push(o); }
        // The devout pray at the temple in the evening (`society.rs`).
        // (The hour and the settler first: the temple's god is read from its reason.)
        if (18..21).contains(&self.clock.hour()) && s.persona.facet(crate::persona::Facet::Piety) >= 50 && !s.mind.prayed {
            if let Some((at, god)) = self.temple() {
                options.push((0.3 + 0.5 * s.persona.facet(crate::persona::Facet::Piety) as f32 / 100.0, Job::Wander(at), format!("Praying to {} at the temple", god)));
            }
        }
        // Spare hours at the workshop for those given to art or craft (`craft.rs`).
        let wish = self.craft_wish(i);
        // No stone or wood laid by: one who wants to make something gathers it.
        let wish_any = self.craft_wish_any(i);
        // The lord wants the land's stone and none is laid by: break some for it (`nobles.rs`).
        let want_stone = self.lord.as_ref().and_then(|l| l.demand.as_ref()).map_or(false, |d| !d.2 && d.0 == self.land_stone())
            && !self.items.iter().any(|it| it.stored && it.kind == ItemKind::Stone);
        if want_stone && wish_any > 0.3 && !night {
            if let Some(t) = self.nearest(s.pos, |c, p| c.is_quarry_stone(p)) {
                options.push((wish_any * 0.9, Job::Quarry(t), format!("Breaking stone at {},{}: the lord wants a work of {}", t.0, t.1, self.land_stone())));
            }
        }
        if wish == 0.0 && wish_any > 0.3 && !night {
            if let Some(t) = self.nearest_tree(s.pos) {
                options.push((wish_any * 0.8, Job::Fell(t), format!("Felling the tree at {},{} for wood to work at the workshop", t.0, t.1)));
            } else if let Some(t) = self.nearest(s.pos, |c, p| c.is_quarry_stone(p)) {
                options.push((wish_any * 0.8, Job::Quarry(t), format!("Breaking stone at {},{} to work at the workshop", t.0, t.1)));
            }
        }
        // Trouble foretold: the best hand at the workshop makes spears, and the militia drills
        // in the evening (`militia.rs`).
        let armour = self.armour_wanted();
        let spears = !armour && self.arms_wanted() && self.items.iter().any(|it| it.stored && it.kind == ItemKind::Log);
        if (spears || armour) && !night && self.workshop_spot().is_some()
            && !self.settlers.iter().any(|x| x.alive && x.job == Job::Craft && (x.why.starts_with("Making spears") || x.why.starts_with("Making armour"))) {
            // (With metal to forge, the best smith: `industry.rs`.)
            let (place, smith) = match self.arms_spot() { Some((_, w)) => (w, w == "the forge"), None => ("the workshop", false) };
            let hand = |j: usize| if smith { self.smith_skill(j) } else { self.settlers[j].skill[4] };
            let best = (0..self.settlers.len()).filter(|&j| self.settlers[j].alive && self.settlers[j].ill_until <= self.clock.tick)
                .max_by(|&a, &b| hand(a).total_cmp(&hand(b)).then(b.cmp(&a)));
            if best == Some(i) {
                let threat = self.trouble_foretold().unwrap_or_default();
                let fear = if threat.is_empty() { "against the next trouble".to_string() } else { format!("for fear of {}", threat) };
                if spears { options.push((if threat.is_empty() { 0.5 } else { 1.1 }, Job::Craft, format!("Making spears at {}, {} ({} of {} made)", place, fear, self.arms.len(), self.arms.len() + 1))); }
                else { options.push((if threat.is_empty() { 0.6 } else { 1.0 }, Job::Craft, format!("Making armour at {}, {} ({} of {} made)", place, fear, self.armour.len(), self.armour.len() + 1))); }
            }
        }
        if let Some(o) = self.drill_option(i) { options.push(o); }
        if let Some(o) = self.engrave_option(i) { options.push(o); }
        if let Some(o) = self.sew_option(i) { options.push(o); }
        if let Some(o) = self.furnish_option(i) { options.push(o); }
        if let Some(o) = self.tend_option(i) { options.push(o); }
        if let Some(o) = self.slab_option(i) { options.push(o); }
        if let Some(o) = self.brew_option(i) { options.push(o); }
        if let Some(o) = self.write_option(i) { options.push(o); }
        if let Some(o) = self.explore_option(i) { options.push(o); }
        if let Some(o) = self.industry_option(i) { options.push(o); }
        if wish > 0.0 && !night {
            let p = &self.settlers[i].persona;
            let why = if p.facet(crate::persona::Facet::ArtInclined) >= 60 { p.facet_phrase(crate::persona::Facet::ArtInclined as usize).unwrap_or_else(|| "loves beautiful things".into()) } else { "values fine work".to_string() };
            options.push((wish, Job::Craft, format!("Making something at the workshop: {} {}", if p.female { "she" } else { "he" }, why)));
        }

        // Spare hours of their own: a need long unmet pulls them away for a while (`needs.rs`).
        // (Not while the camp goes hungry.)
        // (Only between jobs: a need never breaks off work in hand; a six-hour craft broken off
        // for a walk had left the workshop making one work in ninety days.)
        // (Debug: PLANET_NO_NEEDS=1 turns the spare-hours acts off, to compare.)
        let need = if hungry_camp || !free || std::env::var("PLANET_NO_NEEDS").is_ok() { None } else { self.need_option(i).or_else(|| self.idle_talk(i)) };
        let need_why = need.as_ref().map(|n| n.0 .2.clone());
        let mut need_act = None;
        if let Some((o, a)) = need { options.push(o); need_act = Some(a); }
        // The night's watch (the first arc): the watcher stays up at the camp's edge.
        if self.watcher == Some(i) && night {
            let threat = self.arc.as_ref().map(|a| a.threat.name.clone()).unwrap_or_default();
            let edge = if self.watch_blocked_until > self.clock.tick { self.camp } else { self.watch_post() };
            let hate = self.arc.as_ref().and_then(|a| a.threat.faction).and_then(|f| s.past.as_ref().and_then(|p| p.feeling.as_ref())
                .filter(|x| (x.0.starts_with("hates") || x.0.starts_with("has not forgiven")) && x.1 == crate::history::EntityId::Faction(f)).map(|x| x.0.clone()));
            let place = if self.tower.map_or(false, |(t, _)| t == edge) { "on the lookout's platform" } else { "at the camp's edge" };
            let why = match (hate, self.battle_of(i)) {
                (Some(h), _) => format!("Keeping watch {}: they {}", place, arc::they_form(&h)),
                (None, Some(b)) => format!("Keeping watch {}, as at {}, for fear of {}", place, b, threat),
                _ => format!("Keeping watch {}, for fear of {}", place, threat),
            };
            options.push((2.5, Job::Wander(edge), why));
        }
        // The patron's hand: favoured ground pulls, dreams pull, the favourite works with heart.
        // Work in a favoured place: look there too, and prefer it.
        if self.patron.marks.iter().any(|m| !m.forbidden) {
            let fav = |c: &Colony, p: Pos| c.marked(p, false);
            if food < food_goal {
                if let Some(t) = if !self.any_ripe_shrub() { None } else { self.nearest(s.pos, |c, p| c.is_ripe_shrub(p) && fav(c, p)) } {
                    options.push(((0.4 + 1.0) * s.taste[0] * 1.4, Job::Forage(t), format!("Picking berries at {},{}, on the ground the patron blessed", t.0, t.1)));
                }
            }
            if hut_pending && logs_about + 2 * fellers < logs_needed {
                if let Some(t) = self.nearest_tree_by(s.pos, |c, p| fav(c, p)) {
                    options.push((0.75 * s.taste[2] * 1.4, Job::Fell(t), format!("Felling the tree at {},{}, on the ground the patron blessed", t.0, t.1)));
                }
            }
        }
        let forbidden_near = self.patron.marks.iter().any(|m| m.forbidden && (m.at.0 as i32 - s.pos.0 as i32).abs().max((m.at.1 as i32 - s.pos.1 as i32).abs()) <= WORK_RADIUS);
        let dream = self.dream_of(i);
        let favourite = self.patron.favourite == Some(i);
        for o in options.iter_mut() {
            let pull = match (dream, &o.1) {
                (Some(Dream::Hut), Job::Build | Job::Fell(_)) => 1.0,
                (Some(Dream::Plenty), Job::Forage(_) | Job::Fish(_)) => 0.8,
                (Some(Dream::Rest), Job::Sleep) => 1.2,
                _ => 0.0,
            };
            if pull > 0.0 {
                o.0 += pull;
                o.2 = format!("Dreamt of {}; {}", dream.unwrap().word(), lower_first(&o.2));
            }
            if favourite && !matches!(o.1, Job::Eat | Job::Sleep | Job::Wander(_)) {
                o.0 *= 1.15;
                o.2.push_str(" (the patron's favourite)");
            }
            if forbidden_near && matches!(o.1, Job::Forage(_) | Job::Fish(_) | Job::Fell(_) | Job::Wander(_)) {
                o.2.push_str(", keeping off the forbidden ground");
            }
            // Character tilts the work: the orderly carry things home, the dutiful build.
            match o.1 {
                Job::Haul(_) => o.0 *= s.persona.lean(crate::persona::Facet::Orderliness, 0.15),
                Job::Build => o.0 *= s.persona.lean(crate::persona::Facet::Dutifulness, 0.12),
                _ => {}
            }
            // A liked material is sought out (Dwarf Fortress's preferences): "likes oak".
            if let Job::Fell(p) | Job::Quarry(p) | Job::Dig(p, _) = o.1 {
                if let Some(m) = self.material_at(p, if let Job::Dig(_, z) = o.1 { Some(z + 1) } else { None }) {
                    if m == s.persona.likes.material {
                        o.0 *= 1.2;
                        o.2.push_str(&format!("; {} likes {}", if s.persona.female { "she" } else { "he" }, m));
                    }
                }
            }
        }
        // Wandering into forbidden ground is no rest.
        options.retain(|o| !matches!(o.1, Job::Wander(p) if self.marked(p, true)));
        // Nothing at all to choose: think again in half an hour (not every minute).
        let Some((best_u, best, why)) = options.into_iter().max_by(|a, b| a.0.total_cmp(&b.0)) else { self.settlers[i].retry_at = self.clock.tick + 30; return };
        if !free {
            // Keep the current job unless something matters much more (an empty belly, sleep).
            let current_u = match s.job { Job::Eat | Job::Sleep => 2.0, Job::Haul(_) if s.carrying.is_some() => 1.5, _ => 0.6 }
                // The stubborn stick at what they started.
                * s.persona.lean(crate::persona::Facet::Perseverance, 0.25);
            // The same kind of work elsewhere is no reason to drop this one.
            let same_kind = std::mem::discriminant(&best) == std::mem::discriminant(&s.job);
            if same_kind || best_u < current_u * 1.6 { return; }
            self.release(i);
        }
        // (The why may have gained a dream's or a mark's words on the way.)
        let chose_need = need_why.as_deref().map_or(false, |w| why.contains(w.split_once(": ").map_or(w, |x| x.1)) && matches!(best, Job::Wander(_)));
        self.start(i, best, why);
        // The act begun: what it meets, and how long they stay at it.
        if let Some(a) = need_act.filter(|_| chose_need && self.settlers[i].job == best) {
            self.settlers[i].work_left = a.minutes;
            self.settlers[i].need_act = Some(a);
        }
    }

    /// What the work at `p` is made of, as a liked thing is named ("oak", "granite"): the tree
    /// there, or the rock at level `z` (the surface's when None).
    pub fn material_at(&self, p: Pos, z: Option<i32>) -> Option<&'static str> {
        let (x, y) = (p.0 as usize, p.1 as usize);
        if x >= self.map.width || y >= self.map.height { return None; }
        let z = z.unwrap_or(self.map.surface_z[y * self.map.width + x]).clamp(0, self.map.depth as i32 - 1) as usize;
        let c = self.map.cell(x, y, z);
        match (c.plant, c.material) {
            (Plant::Tree(k), _) => crate::persona::material_of_the_land(&format!("{:?}", k)),
            (_, crate::local::Material::Rock(r)) => crate::persona::material_of_the_land(&format!("{:?}", r)),
            _ => None,
        }
    }

    pub(crate) fn reachable_pub(&self, from: Pos, to: Pos) -> bool { self.reachable_from(from, to) }

    fn reachable_from(&self, from: Pos, to: Pos) -> bool {
        let d = (from.0 as i32 - to.0 as i32).abs().max((from.1 as i32 - to.1 as i32).abs());
        d <= WORK_RADIUS && !self.unreachable.contains(&to)
    }

    fn nearest(&self, from: Pos, ok: impl Fn(&Colony, Pos) -> bool) -> Option<Pos> {
        self.nearest_within(from, WORK_RADIUS, ok)
    }

    fn nearest_within(&self, from: Pos, radius: i32, ok: impl Fn(&Colony, Pos) -> bool) -> Option<Pos> {
        let n = self.map.width as i32;
        let (fx, fy) = (from.0 as i32, from.1 as i32);
        // (The test itself first: it looks at the map, while the claims are hashed.)
        let pass = |x: i32, y: i32| {
            if x < 1 || y < 1 || x >= n - 1 || y >= n - 1 { return None; }
            let p = (x as u16, y as u16);
            (ok(self, p) && !self.claimed.contains(&p) && !self.unreachable.contains(&p) && !self.marked(p, true)).then_some(p)
        };
        // Search rings outward from the settler; in a ring the first by row, then column.
        // (Walked in that order, the first cell that passes is the ring's choice.)
        for r in 1..=radius {
            // A ring wholly off the map: so is every one beyond it.
            if fx - r < 1 && fy - r < 1 && fx + r >= n - 1 && fy + r >= n - 1 { break; }
            let (x0, x1) = ((fx - r).max(1), (fx + r).min(n - 2));
            if let Some(p) = (x0..=x1).find_map(|x| pass(x, fy - r)) { return Some(p); }
            for y in (fy - r + 1).max(1)..=(fy + r - 1).min(n - 2) {
                if let Some(p) = pass(fx - r, y).or_else(|| pass(fx + r, y)) { return Some(p); }
            }
            if let Some(p) = (x0..=x1).find_map(|x| pass(x, fy + r)) { return Some(p); }
        }
        None
    }

    pub(crate) fn floor_plant_pub(&self, p: Pos) -> Plant { self.floor_plant(p) }
    /// The nearest tree that may be felled, from `from`; a day that found none skips the
    /// whole-map search until the next dawn (a treeless camp had searched every decision).
    pub(crate) fn nearest_tree(&self, from: Pos) -> Option<Pos> {
        // Once a day the whole map is looked over: with no tree that may be felled anywhere, no
        // search is made that day. (`treeless_day`: (day checked, none anywhere) packed.)
        let day = self.clock.day();
        let (checked, none) = (self.treeless_day.get() >> 1, self.treeless_day.get() & 1 == 1);
        let none = if checked == day { none } else {
            let none = !self.tree_buckets.iter().flatten().any(|&p| self.is_felling_tree(p));
            self.treeless_day.set(day << 1 | none as u64);
            none
        };
        if none { return None; }
        self.nearest_tree_by(from, |_, _| true)
    }

    fn floor_plant(&self, p: Pos) -> Plant {
        let (x, y) = (p.0 as usize, p.1 as usize);
        self.map.cell(x, y, self.map.surface_z[y * self.map.width + x] as usize).plant
    }
    /// The nearest ripe shrub within `radius` (Chebyshev, 1 or more), ties by row then column:
    /// the same as a ring search (`nearest_within`) but over the listed shrubs only.
    fn nearest_ripe_shrub(&self, from: Pos, radius: i32) -> Option<Pos> {
        // Close by, rings are quickest; beyond, the list.
        const NEAR: i32 = 12;
        if let Some(p) = self.nearest_within(from, radius.min(NEAR), |c, p| c.is_ripe_shrub(p)) { return Some(p); }
        if radius <= NEAR { return None; }
        self.nearest_listed(&self.shrub_buckets, from, NEAR, radius, |c, p| c.is_ripe_shrub(p))
    }

    /// The nearest tree that may be felled and passes `ok` within the work radius: what
    /// `nearest(from, ..)` finds (rings out from `from`, ties by row then column), looked for
    /// only among the columns that can hold a tree (`tree_buckets`). A far tree had cost a camp
    /// with no wood nearby (dev 50,20) a ring search of thousands of cells every decision.
    pub(crate) fn nearest_tree_by(&self, from: Pos, ok: impl Fn(&Colony, Pos) -> bool) -> Option<Pos> {
        self.nearest_listed(&self.tree_buckets, from, 0, WORK_RADIUS, |c, p| c.is_felling_tree(p) && ok(c, p))
    }

    /// Among the listed cells (`buckets`: `SHRUB_BUCKET` squares, row-major) at a distance over
    /// `beyond` and up to `radius` (Chebyshev) from `from`, inside the map's rim, unclaimed,
    /// reachable as far as known, not forbidden and passing `ok`: the one with the least
    /// (distance, row, column), as a ring search (`nearest_within`) or a scan of the list would
    /// find. Square by square, nearest squares first, until no square left can hold one as near.
    fn nearest_listed(&self, buckets: &[Vec<Pos>], from: Pos, beyond: i32, radius: i32, ok: impl Fn(&Colony, Pos) -> bool) -> Option<Pos> {
        let n = self.map.width as i32;
        let (fx, fy) = (from.0 as i32, from.1 as i32);
        let fits = |p: Pos| {
            let (x, y) = (p.0 as i32, p.1 as i32);
            if x < 1 || y < 1 || x >= n - 1 || y >= n - 1 { return None; }
            let d = (x - fx).abs().max((y - fy).abs());
            if d <= beyond || d > radius { return None; }
            if !ok(self, p) || self.claimed.contains(&p) || self.unreachable.contains(&p) || self.marked(p, true) { return None; }
            Some((d, p.1, p.0))
        };
        let b = SHRUB_BUCKET as i32;
        let bw = (self.map.width as i32 + b - 1) / b;
        let mut squares: Vec<(i32, usize)> = (0..buckets.len()).filter(|&k| !buckets[k].is_empty()).filter_map(|k| {
            let (x0, y0) = ((k as i32 % bw) * b, (k as i32 / bw) * b);
            let gap = |v: i32, lo: i32| (lo - v).max(v - (lo + b - 1)).max(0);
            let d = gap(fx, x0).max(gap(fy, y0));
            (d <= radius).then_some((d, k))
        }).collect();
        squares.sort_unstable();
        let mut best: Option<(i32, u16, u16)> = None;
        for (d, k) in squares {
            if best.map_or(false, |t| d > t.0) { break; }
            for &p in &buckets[k] {
                if let Some(t) = fits(p) { if best.map_or(true, |b| t < b) { best = Some(t); } }
            }
        }
        best.map(|t| (t.2, t.1))
    }

    /// Whether any shrub of the map is ripe today (cached per day).
    fn any_ripe_shrub(&self) -> bool {
        let day = self.clock.day();
        let (d, any) = self.ripe_today.get();
        if d == day { return any; }
        let any = !self.hard_winter() && (self.shrubs.is_empty() || self.shrubs.iter().any(|&p| self.is_ripe_shrub(p)));
        self.ripe_today.set((day, any));
        any
    }
    fn is_ripe_shrub(&self, p: Pos) -> bool {
        !self.hard_winter() && self.floor_plant(p) == Plant::Shrub && self.shrub_ready.get(&p).map_or(true, |&d| d <= self.clock.day())
    }
    /// A boulder, or bare rock, a settler can break stone from.
    fn is_quarry_stone(&self, p: Pos) -> bool {
        let (x, y) = (p.0 as usize, p.1 as usize);
        if self.in_hut_site(p) { return false; }
        let c = self.map.cell(x, y, self.map.surface_z[y * self.map.width + x].max(0) as usize);
        (c.boulder || matches!(c.material, Material::Rock(_))) && c.water == 0
    }
    fn is_felling_tree(&self, p: Pos) -> bool {
        // Not trees inside the hut site.
        matches!(self.floor_plant(p), Plant::Tree(_)) && !self.in_hut_site(p)
            // A people who keep the wood fell nothing inside the wall's ring.
            && !(self.way.as_ref().map_or(false, |w| w.keep_trees) && (p.0 as i32 - self.camp.0 as i32).pow(2) + (p.1 as i32 - self.camp.1 as i32).pow(2) <= 13 * 13)
            && !self.stones.iter().any(|(k, at)| *k == StoneKind::Grove && (at.0 as i32 - p.0 as i32).abs().max((at.1 as i32 - p.1 as i32).abs()) <= GROVE_RADIUS)
            // The speaker's mandate may spare the trees near the fire.
            && !(self.mandate == Some(society::Mandate::SpareTrees) && (p.0 as i32 - self.camp.0 as i32).abs().max((p.1 as i32 - self.camp.1 as i32).abs()) <= 20)
    }
    /// Dry ground next to water a settler can stand on.
    fn is_fishing_spot(&self, p: Pos) -> bool {
        if self.shrub_ready.get(&p).map_or(false, |&d| d > self.clock.day()) { return false; }
        self.fishing_ground(p)
    }

    /// The nearest free fishing spot within `radius` of `from` (from the list found at founding;
    /// a ring search over the map was most of a year's run time).
    pub(crate) fn fishing_near(&self, from: Pos, radius: i32) -> Option<Pos> { self.nearest_fishing(from, radius) }

    fn nearest_fishing(&self, from: Pos, radius: i32) -> Option<Pos> {
        let day = self.clock.day();
        // In a deep freeze the water is ice: only holes cut at the jetty (`frozen`).
        let frozen = self.frozen();
        let jetty = self.jetty;
        self.fishing_spots.iter().copied()
            .filter(|p| !frozen || jetty.map_or(false, |j| (j.0 as i32 - p.0 as i32).abs().max((j.1 as i32 - p.1 as i32).abs()) <= 2))
            .filter(|p| { let d = (p.0 as i32 - from.0 as i32).abs().max((p.1 as i32 - from.1 as i32).abs()); d >= 1 && d <= radius })
            .filter(|p| !self.claimed.contains(p) && !self.unreachable.contains(p) && !self.marked(*p, true) && self.shrub_ready.get(p).map_or(true, |&d| d <= day))
            .min_by_key(|p| ((p.0 as i32 - from.0 as i32).abs().max((p.1 as i32 - from.1 as i32).abs()), p.1, p.0))
    }

    /// Dry, passable ground beside water: somewhere to fish from.
    fn fishing_ground(&self, p: Pos) -> bool {
        if !nav::passable(&self.map, p) || self.water_at(p) { return false; }
        [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)].iter().any(|&(dx, dy)| self.water_at(((p.0 as i32 + dx) as u16, (p.1 as i32 + dy) as u16)))
    }
    fn water_at(&self, p: Pos) -> bool {
        let (x, y) = (p.0 as usize, p.1 as usize);
        if x >= self.map.width || y >= self.map.height { return false; }
        let z = self.map.surface_z[y * self.map.width + x] + 1;
        (z as usize) < self.map.depth && self.map.cell(x, y, z as usize).water > 0
    }

    fn start(&mut self, i: usize, job: Job, why: String) {
        self.settlers[i].need_act = None;
        let target = match job {
            Job::Eat => self.eat_spot(i),
            Job::Build => self.camp,
            Job::Sleep => self.sleep_spot(i),
            Job::Forage(p) | Job::Fish(p) | Job::Wander(p) => p,
            Job::Craft if why.starts_with("Engraving") => match self.engrave_spot() { Some(p) => p, None => return },
            Job::Craft if why.starts_with("Brewing") => self.still().unwrap_or(self.camp),
            Job::Craft if why.starts_with("Writing") => self.temple_at().or_else(|| self.tavern()).or_else(|| self.hall_cells.first().copied()).unwrap_or(self.camp),
            // The workshops below (`industry.rs`): the job's own bench.
            Job::Craft if industry::is_industry(&why) => match self.industry_spot(&why) { Some(p) => p, None => return },
            Job::Craft if why.starts_with("Making spears") || why.starts_with("Making armour") => self.arms_spot().map(|s| s.0).unwrap_or(self.camp),
            Job::Craft if why.starts_with("Making furniture") => self.furniture_shop().map(|s| s.0).or_else(|| self.workshop_spot()).unwrap_or(self.camp),
            Job::Craft => self.workshop_spot().unwrap_or(self.camp),
            Job::Fell(p) => match (self.floor_plant(p), self.cavern_tree_level(p)) {
                (Plant::None, Some(f)) => match self.cavern_stand(p, f) { Some(q) => q, None => return },
                _ => self.stand_next_to(p).unwrap_or(p),
            },
            Job::Quarry(p) => if nav::passable(&self.map, p) { p } else { self.stand_next_to(p).unwrap_or(p) },
            Job::Dig(p, z) => match self.dig_stand3(p, z) { Some(q) => (q.0, q.1), None => return },
            Job::Hunt(id) => match self.creatures.iter().find(|c| c.id == id) { Some(c) => c.pos, None => return },
            Job::Haul(k) => self.items[k].at,
            Job::Idle => return,
        };
        let target = if job == Job::Build { self.hut_door_side() } else { target };
        let from = self.settlers[i].pos;
        // The level of the place they go: a room under the rock, a stair, else the surface.
        let to3 = self.spot_level(i, job, &why, target);
        let from3 = self.here3(i);
        // Building is done from the hut's door, or from the camp when a later work walls the
        // door in (seed 3: two settlers were stuck 22,000 times beside a blocked door).
        let workshop = self.projects.iter().any(|p| p.done && p.kind == projects::ProjectKind::Workshop);
        let iron = self.iron_worked();
        let first = nav::path3(&self.map, Some(&self.steps), from3, to3, PATH_BUDGET);
        // A bed or watch post that cannot be reached is remembered for a day (each try searched
        // the whole map: seed 3's cut-off second-hut beds cost most of its year).
        if first.is_none() {
            if job == Job::Sleep { self.settlers[i].bed_blocked_until = self.clock.tick + TICKS_PER_DAY; }
            if why.starts_with("Keeping watch") { self.watch_blocked_until = self.clock.tick + TICKS_PER_DAY; }
        }
        // (A meal in a hall below, from far out on the land, can be past the search's budget: they
        // walk back to the fire and eat there. Seed 11 had starved two foragers 60 cells out with
        // 90 meals in the store, failing that search 400 times.)
        let found = first.or_else(|| if matches!(job, Job::Build | Job::Sleep | Job::Eat) { nav::path3(&self.map, None, from3, nav::surface3(&self.map, self.camp), PATH_BUDGET) } else { None });
        match found {
            Some(p) => {
                match job {
                    Job::Forage(t) | Job::Fish(t) | Job::Fell(t) | Job::Quarry(t) | Job::Dig(t, _) => { self.claimed.insert(t); }
                    Job::Haul(k) => self.items[k].reserved = true,
                    _ => {}
                }
                if !matches!(job, Job::Wander(_)) || self.settlers[i].why != why {
                    self.decisions.push(format!("{}  {:<9} {:<16} {}", self.clock.stamp(), self.settlers[i].name, job.verb(), why));
                }
                let mood_pace = self.mood_pace(i) * self.focus_pace(i) * self.thirst_pace(i) * self.wound_work(i, matches!(job, Job::Fell(_) | Job::Quarry(_) | Job::Dig(..) | Job::Build | Job::Haul(_)));
                let craft_minutes = industry::minutes(&why).unwrap_or(360);
                let s = &mut self.settlers[i];
                s.path = p.into_iter().skip(1).collect();
                s.job = job;
                s.why = why;
                let base: u32 = match job {
                    Job::Eat => 20, Job::Sleep => 7 * 60, Job::Forage(_) => 60, Job::Fish(_) => 90,
                    Job::Fell(_) => 150, Job::Quarry(_) => 150, Job::Build => 60, Job::Haul(_) => 2, Job::Wander(_) => 30, Job::Idle => 0,
                    Job::Dig(p, z) => dig::dig_minutes(self.map.cell(p.0 as usize, p.1 as usize, (z + 1).max(0) as usize).material),
                    Job::Hunt(_) => 25,
                    Job::Craft => craft_minutes,
                };
                // Skill: a green hand takes 1.3x the time, a master 0.7x.
                // Body and mind: the strong fell faster, the patient fish better (`Persona::work_time`).
                let factor = skill_of(job).map_or(1.0, |k| (1.3 - 0.6 * s.skill[k]) * s.persona.work_time(k) * mood_pace)
                    // Tools from the workshop: felling, quarrying and building go faster.
                    // Iron tools, from ore they dug, faster still.
                    * if workshop && matches!(job, Job::Fell(_) | Job::Quarry(_) | Job::Build | Job::Dig(..)) { if iron { 0.65 } else { 0.8 } } else { 1.0 };
                s.work_left = ((base as f32 * factor).round() as u32).max(if base > 0 { 1 } else { 0 });
            }
            None => {
                if let Job::Forage(t) | Job::Fish(t) | Job::Fell(t) | Job::Quarry(t) = job { self.unreachable.insert(t); }
                if let Job::Haul(k) = job { let at = self.items[k].at; self.unreachable.insert(at); }
                // A cut no one can reach: after forty tries the dig is given up (`delve.rs`).
                if let Job::Dig(..) = job {
                    self.dig_fails += 1; if self.dig_fails >= 40 { self.give_up_dig(); }
                }
                if let Job::Hunt(id) = job { self.game_unreachable.insert(id); }
                if !matches!(job, Job::Wander(_)) { self.settlers[i].stuck += 1; }
                // Every failed way waits a quarter hour before the next try (a wander to an
                // unreachable spot had searched the map every minute).
                self.settlers[i].retry_at = self.clock.tick + 15;
                if std::env::var("PLANET_DEBUG_PATH").is_ok() && self.settlers[i].stuck % 500 == 1 { eprintln!("PATHFAIL day {} {} at {:?} {:?} -> {:?} stuck {}", self.clock.day(), self.settlers[i].name, from, job, target, self.settlers[i].stuck); }
                self.settlers[i].job = Job::Idle;
            }
        }
    }

    fn release(&mut self, i: usize) {
        match self.settlers[i].job {
            Job::Forage(t) | Job::Fish(t) | Job::Fell(t) | Job::Quarry(t) | Job::Dig(t, _) => { self.claimed.remove(&t); }
            Job::Haul(k) => {
                if let Some(kind) = self.settlers[i].carrying.take() {
                    // Put it (and the rest of the basket) down where they stand.
                    let at = self.settlers[i].pos;
                    self.items[k] = Item { kind, at, stored: at == self.camp, reserved: false };
                    for e in self.basket.remove(&k).unwrap_or_default() {
                        if e < self.items.len() { self.items[e] = Item { kind: self.items[e].kind, at, stored: at == self.camp, reserved: false }; }
                    }
                } else if k < self.items.len() { self.items[k].reserved = false; }
            }
            _ => {}
        }
        self.settlers[i].job = Job::Idle;
        self.settlers[i].path.clear();
    }

    fn sleep_spot(&self, i: usize) -> Pos {
        // A bed they could not reach: by the fire tonight.
        if self.settlers[i].bed_blocked_until > self.clock.tick { return self.camp; }
        // A bedroom of their own (`delve.rs`).
        if let Some(b) = self.bedroom_of(i).and_then(|r| r.bed) { return b; }
        // A hall dug into the rock sleeps everyone, warm.
        if !self.hall_cells.is_empty() { return self.hall_cells[i % self.hall_cells.len()]; }
        if let Some(b) = self.second_hut_bed(i) { return b; }
        match &self.hut {
            Some(h) if h.done && i < HUT_BEDS => (h.at.0 + 1 + (i as u16 % (HUT_W as u16 - 2)), h.at.1 + 1 + (i as u16 / (HUT_W as u16 - 2)) % (HUT_H as u16 - 2)),
            _ => {
                let p = (self.camp.0.saturating_add_signed((i as i16 % 3) - 1), self.camp.1.saturating_add_signed((i as i16 / 3) % 3 - 1));
                if nav::passable(&self.map, p) { p } else { self.camp }
            }
        }
    }

    /// Where a meal is eaten: by the fire, or in the hall's larder when the hall is nearer (a
    /// share of the store is kept there; a hall 20 cells off had its sleepers walking two hours
    /// to every meal).
    fn eat_spot(&self, i: usize) -> Pos {
        let p = self.settlers[i].pos;
        let d = |q: Pos| (q.0 as i32 - p.0 as i32).abs().max((q.1 as i32 - p.1 as i32).abs());
        // The great hall below, where the camp eats together (`delve.rs`), unless it lies much
        // farther than the fire (a reclaimed hall out in the hills).
        if let Some((t, _)) = self.great_hall() { if d(t) <= d(self.camp) + 10 { return t; } }
        match self.hall_cells.first() { Some(&h) if d(h) < d(self.camp) => h, _ => self.camp }
    }

    fn hut_door_side(&self) -> Pos {
        match &self.hut {
            Some(h) => (h.at.0 + HUT_W as u16 / 2, h.at.1 + HUT_H as u16),
            None => self.camp,
        }
    }

    fn stand_next_to(&self, p: Pos) -> Option<Pos> {
        [(0i32, 1i32), (1, 0), (-1, 0), (0, -1)].iter()
            .map(|&(dx, dy)| ((p.0 as i32 + dx) as u16, (p.1 as i32 + dy) as u16))
            .find(|&q| nav::passable(&self.map, q) && !matches!(self.floor_plant(q), Plant::Tree(_)))
    }

    /// Walk, then work, then finish.
    /// What a step from `from` to the neighbouring `to` costs (10 a plain cell, x1.4 diagonally).
    fn step_cost(&self, from: Pos, to: nav::P3) -> Option<i32> {
        let c = nav::cost3(&self.map, to.0 as usize, to.1 as usize, to.2)? as i32;
        Some(if from.0 != to.0 && from.1 != to.1 { c * 14 / 10 } else if from == (to.0, to.1) { c + 6 } else { c })
    }

    /// Where settler `i` stands, with its level (the surface's when their level is not one to
    /// stand on: they were set down somewhere new).
    pub fn here3(&self, i: usize) -> nav::P3 {
        let s = &self.settlers[i];
        if nav::standable(&self.map, s.pos.0 as usize, s.pos.1 as usize, s.z) { (s.pos.0, s.pos.1, s.z) } else { nav::surface3(&self.map, s.pos) }
    }

    /// Whether settler `i` is below the ground (in a hall, a stair, a cavern): out of the
    /// weather and out of the reach of anything on the surface.
    pub fn below(&self, i: usize) -> bool {
        let p = self.here3(i);
        p.2 < self.map.surface_z[p.1 as usize * self.map.width + p.0 as usize]
    }

    /// Where to draw settler `i`: between its cell and the next one on its path.
    pub fn draw_pos(&self, i: usize) -> (f32, f32) {
        let s = &self.settlers[i];
        let here = (s.pos.0 as f32, s.pos.1 as f32);
        match s.path.first() {
            Some(&next) => {
                let t = self.step_cost(s.pos, next).map_or(0.0, |c| (s.stride as f32 / c as f32).clamp(0.0, 1.0));
                let next = (next.0, next.1);
                (here.0 + (next.0 as f32 - here.0) * t, here.1 + (next.1 as f32 - here.1) * t)
            }
            None => here,
        }
    }

    fn act(&mut self, i: usize) {
        if !self.settlers[i].path.is_empty() {
            // A cell every `WALK_PER_TICK`-th of a plain cell's cost: about 5 minutes a cell (as
            // a dwarf takes ten ticks a tile), slower diagonally, through water and trees.
            self.settlers[i].stride += WALK_PER_TICK;
            // Their own pace: the agile and tireless a little faster.
            let s = &mut self.settlers[i];
            let limp = self.wound_walk(i);
            let s = &mut self.settlers[i];
            s.stride_frac += WALK_PER_TICK as f32 * (s.persona.walk() * limp - 1.0);
            if s.stride_frac >= 1.0 { s.stride += 1; s.stride_frac -= 1.0; } else if s.stride_frac <= -1.0 { s.stride -= 1; s.stride_frac += 1.0; }
            loop {
                let Some(&next) = self.settlers[i].path.first() else { self.settlers[i].stride = 0; break };
                let Some(c) = self.step_cost(self.settlers[i].pos, next) else {
                    self.settlers[i].stride = 0;
                    // The last cell itself is blocked (a bed under a wall, a target in rock):
                    // do the job from here, a step short.
                    if self.settlers[i].path.len() == 1 { self.settlers[i].path.clear(); return; }
                    // Blocked since the path was found (a wall went up): find another way after a
                    // quarter hour (re-planning every minute had cost seed 3 most of its time).
                    // (Not counted as stuck: with slow walking, walls often rise across a path.)
                    self.release(i);
                    self.settlers[i].retry_at = self.clock.tick + 15;
                    return;
                };
                if self.settlers[i].stride < c { break; }
                self.settlers[i].stride -= c;
                self.settlers[i].pos = (next.0, next.1);
                self.settlers[i].z = next.2;
                // Feet wear the ground (the surface's; halls and stairs are cut, not worn).
                if self.steps.len() != self.map.width * self.map.height { self.steps = vec![0; self.map.width * self.map.height]; }
                let k = next.1 as usize * self.map.width + next.0 as usize;
                if self.map.surface_z[k] == next.2 { self.steps[k] = self.steps[k].saturating_add(1); }
                self.settlers[i].path.remove(0);
            }
            return;
        }
        let job = self.settlers[i].job;
        if job == Job::Idle { return; }
        if self.settlers[i].work_left > 0 {
            // Chilled hands work at half pace.
            if job != Job::Sleep && self.settlers[i].exposure > 0.7 && self.clock.tick % 2 == 0 { return; }
            self.settlers[i].work_left -= 1;
            // Sleep ends when rested.
            // Rested by day: up again, unless ill (they keep to their bed until well).
            let ill = self.settlers[i].ill_until > self.clock.tick;
            // Ill: they rise only to eat (when eating outranks resting, and there is food).
            let ill_but_hungry = ill && self.settlers[i].hunger > 0.75 && self.food_stored() > 0;
            // Nobody sleeps through starving while there is food at the camp.
            if job == Job::Sleep && self.settlers[i].hunger >= 0.95 && self.food_stored() > 0 { self.settlers[i].work_left = 0; }
            // (Despair keeps them abed rested or not: ending it at once re-chose it every tick.)
            let despair = matches!(self.settlers[i].mind.broken, Some((mind::Break::Despair, _)));
            if job == Job::Sleep && self.settlers[i].fatigue <= 0.02 && !self.clock.is_night() && (!ill || ill_but_hungry) && !despair { self.settlers[i].work_left = 0; }
            if self.settlers[i].work_left > 0 { return; }
        }
        self.finish(i, job);
    }

    fn finish(&mut self, i: usize, job: Job) {
        // Practice: each job done well teaches a little (more beside a better hand).
        if let Some(k) = skill_of(job) {
            let me = self.settlers[i].pos;
            let mine = self.settlers[i].skill[k];
            let teacher = self.settlers.iter().enumerate().any(|(j, o)| j != i && o.alive && o.skill[k] > mine + 0.15
                && skill_of(o.job) == Some(k) && (o.pos.0 as i32 - me.0 as i32).abs().max((o.pos.1 as i32 - me.1 as i32).abs()) <= 4);
            let gain = 0.025 * (1.0 - mine) * if teacher { 2.0 } else { 1.0 } * self.settlers[i].persona.learning() * self.guild_learning(i, k);
            self.settlers[i].mind.made += 1;
            self.meet(i, needs::Need::StayOccupied, 120);
            // Feelings about the work itself: a liked material, a tree felled by one who loves the wild.
            if let Job::Fell(p) | Job::Quarry(p) | Job::Dig(p, _) = job {
                let z = if let Job::Dig(_, z) = job { Some(z + 1) } else { None };
                if let Some(m) = self.material_at(p, z) {
                    if m == self.settlers[i].persona.likes.material { self.feel(i, mind::Feel::LikedWork { material: m.to_string() }); }
                }
            }
            if matches!(job, Job::Fell(_)) && self.settlers[i].persona.value(crate::persona::Val::Nature) >= 26 { self.feel(i, mind::Feel::FelledTree); }
            self.settlers[i].skill[k] = (mine + gain).min(1.0);
            // Use builds the body (toward each attribute's cap).
            use crate::persona::Attr;
            let p = &mut self.settlers[i].persona;
            match k {
                2 | 3 => { p.train(Attr::Strength, 6.0); p.train(Attr::Endurance, 4.0); }
                4 => { p.train(Attr::SpatialSense, 4.0); p.train(Attr::KinestheticSense, 4.0); }
                0 => { p.train(Attr::Intuition, 3.0); p.train(Attr::Agility, 2.0); }
                _ => { p.train(Attr::Patience, 4.0); p.train(Attr::Focus, 2.0); }
            }
        }
        let name = self.settlers[i].name.clone();
        let day = self.clock.day();
        match job {
            Job::Eat => {
                if let Some(k) = self.items.iter().position(|it| it.kind == ItemKind::Food && it.stored) {
                    // On rations every second meal takes nothing from the store (`rations.rs`).
                    if self.ration_takes(i) {
                        self.items.remove(k);
                        self.fix_item_refs(k);
                    }
                    self.settlers[i].hunger = (self.settlers[i].hunger - MEAL).max(0.0);
                    // A cup with the meal (`drink.rs`), and the cook's supper (`kitchen.rs`).
                    self.drink_with_meal(i);
                    self.ate_supper(i);
                }
                // A meal by the fire with someone else eating: a little closer.
                let others: Vec<usize> = (0..self.settlers.len()).filter(|&j| j != i && self.settlers[j].alive && self.settlers[j].job == Job::Eat).collect();
                for j in others { self.warm(i, j); }
            }
            Job::Sleep => {}
            Job::Wander(_) if self.settlers[i].need_act.is_some() => { if let Some(a) = self.settlers[i].need_act.take() { self.complete_need(i, a); } }
            Job::Wander(_) if self.settlers[i].why.starts_with("Tending") => self.tend(i),
            Job::Dig(p, z) => { self.finish_dig(i, p, z); }
            Job::Hunt(id) => { if self.finish_hunt(i, id) { let n = self.eat_catch(i, 6); if self.carry_home(i, n) { return; } } }
            Job::Craft => if industry::is_industry(&self.settlers[i].why) { self.finish_industry(i) } else if self.settlers[i].why.starts_with("Making spears") { self.finish_arm(i) } else if self.settlers[i].why.starts_with("Making armour") { self.finish_armour(i) } else if self.settlers[i].why.starts_with("Sewing") { self.finish_sew(i) } else if self.settlers[i].why.starts_with("Making furniture") { self.finish_furniture(i) } else if self.settlers[i].why.starts_with("Engraving") { self.finish_engraving(i) } else if self.settlers[i].why.starts_with("Carving a slab") { self.finish_slab(i) } else if self.settlers[i].why.starts_with("Brewing") { self.finish_brew(i) } else if self.settlers[i].why.starts_with("Writing") { self.finish_book(i) } else { self.finish_craft(i) },
            Job::Forage(t) => {
                self.claimed.remove(&t);
                self.shrub_ready.insert(t, day + SHRUB_REGROW_DAYS);
                let n = 3 + self.rng.gen_range(0..3);
                for _ in 0..n { self.items.push(Item { kind: ItemKind::Food, at: self.settlers[i].pos, stored: false, reserved: false }); }
                self.once("forage", format!("{} brings in the first berries from the shrubs at {},{}.", name, t.0, t.1));
                // Carry the basket home (what doesn't fit is left for others to fetch).
                let n = self.eat_catch(i, n);
                if self.carry_home(i, n) { return; }
            }
            Job::Fish(t) => {
                self.claimed.remove(&t);
                let caught = self.rng.gen_range(1..4);
                // (Carried up to the mouth from below.)
                let at = if self.below(i) { self.delve_mouth.unwrap_or(self.camp) } else { self.settlers[i].pos };
                for _ in 0..caught { self.items.push(Item { kind: ItemKind::Food, at, stored: false, reserved: false }); }
                if self.below(i) { self.once("cave fish", format!("{} brings up the first blind white fish from the still water of the cavern: they taste of nothing, and they keep the camp.", name)); }
                else { self.once("fish", format!("{} catches the first fish from the river at {},{}.", name, t.0, t.1)); }
                // A spot fished out for a while (not the jetty's, which reaches deep water).
                if self.jetty.map_or(true, |j| (j.0 as i32 - t.0 as i32).abs().max((j.1 as i32 - t.1 as i32).abs()) > 2) {
                    self.shrub_ready.insert(t, self.clock.day() + FISH_RECOVER_DAYS);
                }
                let caught = self.eat_catch(i, caught);
                if self.carry_home(i, caught) { return; }
            }
            Job::Fell(t) => {
                self.claimed.remove(&t);
                // A fungus tree in the cavern (`delve.rs`).
                if let (Plant::None, Some(f)) = (self.floor_plant(t), self.cavern_tree_level(t)) {
                    let felled = self.fell_cavern_tree(i, t, f);
                    let carried = felled && self.carry_home(i, 2);
                    if carried { return; }
                    self.settlers[i].job = Job::Idle;
                    return;
                }
                let (x, y) = (t.0 as usize, t.1 as usize);
                let k = self.map.idx(x, y, self.map.surface_z[y * self.map.width + x] as usize);
                if let Plant::Tree(kind) = self.map.cells[k].plant {
                    self.map.cells[k].plant = Plant::None;
                    self.felled.push((t, kind, self.clock.day()));
                    // A stump marks where it stood.
                    self.map.features[y * self.map.width + x] = crate::local::wildlife::Feature::Stump;
                    for _ in 0..2 { self.items.push(Item { kind: ItemKind::Log, at: t, stored: false, reserved: false }); }
                    let for_hut = if self.hut.as_ref().map_or(false, |h| !h.done && self.hut_material == ItemKind::Log) { " for the hut" } else { "" };
                    self.once("fell", format!("{} fells the first tree, at {},{}: two logs{}.", name, t.0, t.1, for_hut));
                    // They drag both logs home themselves (walking costs time now).
                    if self.carry_home(i, 2) { return; }
                }
            }
            Job::Quarry(t) => {
                self.claimed.remove(&t);
                let (x, y) = (t.0 as usize, t.1 as usize);
                let k = self.map.idx(x, y, self.map.surface_z[y * self.map.width + x] as usize);
                if self.map.cells[k].boulder || matches!(self.map.cells[k].material, Material::Rock(_)) {
                    // A boulder is carried off; bare rock is broken down to gravel.
                    if self.map.cells[k].boulder { self.map.cells[k].boulder = false; } else { self.map.cells[k].material = Material::Gravel; }
                    for _ in 0..2 { self.items.push(Item { kind: ItemKind::Stone, at: t, stored: false, reserved: false }); }
                    self.once("quarry", format!("{} breaks the first stone, at {},{}: there is no timber to be had, so they will build in stone.", name, t.0, t.1));
                    if self.carry_home(i, 2) { return; }
                }
            }
            Job::Haul(k) => {
                if self.settlers[i].carrying.is_none() {
                    // Picked up: now carry it to the camp.
                    if k < self.items.len() && self.items[k].at == self.settlers[i].pos && !self.items[k].stored {
                        self.settlers[i].carrying = Some(self.items[k].kind);
                        let from = self.here3(i);
                        if let Some(p) = nav::path3(&self.map, None, from, nav::surface3(&self.map, self.camp), PATH_BUDGET) {
                            self.settlers[i].path = p.into_iter().skip(1).collect();
                            self.settlers[i].work_left = 2;
                            return;
                        }
                        self.settlers[i].carrying = None;
                    }
                    if k < self.items.len() { self.items[k].reserved = false; }
                } else {
                    let kind = self.settlers[i].carrying.take().unwrap();
                    if k < self.items.len() {
                        self.items[k] = Item { kind, at: self.camp, stored: true, reserved: false };
                    }
                    for e in self.basket.remove(&k).unwrap_or_default() {
                        if e < self.items.len() { self.items[e].stored = true; self.items[e].reserved = false; self.items[e].at = self.camp; }
                    }
                }
            }
            Job::Build => self.build_step(i),
            Job::Wander(_) | Job::Idle => {}
        }
        self.settlers[i].job = Job::Idle;
    }

    /// Settler `i` just made the last `n` items where they stand: carry one home now (a basket
    /// holds what a forager picks; the rest is left for others). Returns true if they set off.
    /// A hungry gatherer eats one of what they just gathered, there and then (a fisher 46 cells
    /// out walked four hours home for every meal and back). Returns what is left to carry.
    fn eat_catch(&mut self, i: usize, n: usize) -> usize {
        if n == 0 || self.settlers[i].hunger <= 0.8 { return n; }
        self.items.pop();
        self.settlers[i].hunger = (self.settlers[i].hunger - MEAL).max(0.0);
        n - 1
    }

    fn carry_home(&mut self, i: usize, n: usize) -> bool {
        if n == 0 { return false; }
        let k = self.items.len() - 1;
        // Everything picked rides in one basket: the extra items are carried along.
        let extra: Vec<usize> = (self.items.len() - n..self.items.len() - 1).collect();
        let from = self.here3(i);
        let Some(p) = nav::path3(&self.map, None, from, nav::surface3(&self.map, self.camp), PATH_BUDGET) else { return false };
        for &e in &extra { self.items[e].reserved = true; self.items[e].at = self.camp; }
        self.items[k].reserved = true;
        let s = &mut self.settlers[i];
        s.carrying = Some(self.items[k].kind);
        s.job = Job::Haul(k);
        s.path = p.into_iter().skip(1).collect();
        s.work_left = 2;
        s.why = format!("Carrying {} back to the camp", if n == 1 { "it".to_string() } else { format!("all {}", n) });
        // The rest of the basket is stored when it arrives.
        self.basket.insert(k, extra);
        true
    }

    pub(crate) fn fix_refs_pub(&mut self, removed: usize) { self.fix_item_refs(removed); }

    /// An item vector index was removed: fix haul jobs pointing past it.
    fn fix_item_refs(&mut self, removed: usize) {
        for s in &mut self.settlers {
            if let Job::Haul(k) = s.job {
                if k == removed { s.job = Job::Idle; s.path.clear(); s.carrying = None; }
                else if k > removed { s.job = Job::Haul(k - 1); }
            }
        }
        let shift = |k: usize| if k > removed { k - 1 } else { k };
        self.basket = std::mem::take(&mut self.basket).into_iter()
            .filter(|(k, _)| *k != removed)
            .map(|(k, v)| (shift(k), v.into_iter().filter(|&e| e != removed).map(shift).collect()))
            .collect();
    }

    /// Put one log into the hut; the last one finishes it.
    fn build_step(&mut self, i: usize) {
        if self.hut.as_ref().map_or(true, |h| h.done) { self.project_step(i); return; }
        let material = self.hut_material;
        let Some(k) = self.items.iter().position(|it| it.kind == material && it.stored) else { return };
        self.items.remove(k);
        self.fix_item_refs(k);
        let name = self.settlers[i].name.clone();
        let Some(hut) = self.hut.as_mut() else { return };
        hut.logs_used += 1;
        let used = hut.logs_used;
        self.count_load(i);
        if !self.builders.contains(&i) { self.builders.push(i); }
        if used == 1 { self.note(format!("{} sets the first {} of the hut.", name, if self.hut_material == ItemKind::Stone { "stone" } else { "log" })); }
        if used >= HUT_LOGS {
            self.raise_hut();
            self.note(format!("{} finishes the hut. Tonight they sleep under a roof.", name));
            for b in self.builders.clone() { self.feel(b, mind::Feel::Built { what: "the hut".into() }); }
            let at = self.hut.as_ref().map(|h| h.at).unwrap_or(self.camp);
            self.moment("The hut stands".into(), format!("{} lays the last {} of the hut. Tonight they sleep under a roof.", name, if self.hut_material == ItemKind::Stone { "stone" } else { "log" }),
                format!("because {} loads were carried in and laid since day 1, against the cold nights", HUT_LOGS), at);
            // The builders raise a stone by the door with their names on it.
            if let Some(h) = &self.hut {
                let at = (h.at.0 + HUT_W as u16 / 2 + 1, h.at.1 + HUT_H as u16);
                let names: Vec<String> = self.builders.iter().map(|&b| self.settlers[b].name.clone()).collect();
                let hall = self.name.as_ref().map(|n| format!("{}'s hall", n)).unwrap_or_else(|| "This hall".into());
                let text = format!("{} was raised on day {} by {}.", hall, self.clock.day(), join_names(&names));
                self.marks.push(ColonyMark { at, kind: MarkKind::Stone, title: "The builders' stone".into(), text, day: self.clock.day() });
                self.note("The builders raise a stone by the door with their names on it.".into());
            }
        }
    }

    /// Stamp the finished hut into the map: timber walls on the ring with a door on the south
    /// side, a timber floor, and a roof the ink renderer draws.
    fn raise_hut(&mut self) {
        let Some(h) = self.hut.as_mut() else { return };
        h.done = true;
        let at = h.at;
        let stone = self.hut_material == ItemKind::Stone;
        self.raise_hut_at(at, stone);
    }

    /// Stamp a hut at `at`: walls on the ring with a south door, a floor and a roof; of
    /// timber, or of stone.
    pub(crate) fn raise_hut_at(&mut self, at: Pos, stone: bool) { self.raise_building_at(at, HUT_W, HUT_H, stone, true); }

    /// Stamp a roofed building `w` x `h` at `at`: walls on the ring (a south door, or the whole
    /// south side open when `walled` is false: a workshop's front), a timber floor, a roof.
    pub(crate) fn raise_building_at(&mut self, at: Pos, bw: usize, bh: usize, stone: bool, walled: bool) {
        let (hw, hh) = (bw, bh);
        let n = self.map.width;
        let mat = if stone { Material::Rock(crate::erosion::materials::RockType::Granite) } else { Material::Wood };
        let mut cells = Vec::new();
        for dy in 0..hh {
            for dx in 0..hw {
                let (x, y) = (at.0 as usize + dx, at.1 as usize + dy);
                let sz = self.map.surface_z[y * n + x] as usize;
                let k = self.map.idx(x, y, sz);
                self.map.cells[k].plant = Plant::None;
                self.map.cells[k].boulder = false;
                self.map.cells[k].material = Material::Wood;
                let ring = dx == 0 || dy == 0 || dx == hw - 1 || (dy == hh - 1 && walled);
                let _ = mat;
                let door = dy == hh - 1 && dx == hw / 2;
                if ring && !door && sz + 1 < self.map.depth {
                    let kw = self.map.idx(x, y, sz + 1);
                    self.map.cells[kw].shape = Shape::Wall;
                    self.map.cells[kw].material = mat;
                }
                cells.push((x, y));
            }
        }
        let (cx, cy) = (at.0 as f32 + hw as f32 / 2.0, at.1 as f32 + hh as f32 / 2.0);
        self.map.houses.push(RoofPlan { cx, cy, axis: (1.0, 0.0), half_width: hh as f32 / 2.0, stone, flat: false });
        let id = self.map.houses.len() as u32;
        for (x, y) in cells { self.map.roofs[y * n + x] = id; }
        // Anyone standing in a wall steps out of it.
        for s in &mut self.settlers {
            if nav::cost(&self.map, s.pos.0 as usize, s.pos.1 as usize).is_none() { s.pos = (at.0 + hw as u16 / 2, at.1 + hh as u16); }
        }
        // A door onto blocked ground seals it (seed 3's second hut): open another side's middle
        // where the ground outside is clear, until the inside can be reached from the fire.
        if walled && hw >= 3 && hh >= 3 {
            let inside = (at.0 + 1, at.1 + 1);
            let sides = [((hw / 2, 0usize), (0i32, -1i32)), ((hw - 1, hh / 2), (1, 0)), ((0, hh / 2), (-1, 0)), ((hw / 2, hh - 1), (0, 1))];
            for ((dx, dy), (ox, oy)) in sides {
                if nav::path(&self.map, self.camp, inside, PATH_BUDGET).is_some() { break; }
                let (x, y) = (at.0 as usize + dx, at.1 as usize + dy);
                let (qx, qy) = (x as i32 + ox, y as i32 + oy);
                if qx < 0 || qy < 0 || qx as usize >= n || qy as usize >= self.map.height || !nav::passable(&self.map, (qx as u16, qy as u16)) { continue; }
                let sz = self.map.surface_z[y * n + x] as usize;
                if sz + 1 < self.map.depth {
                    let kw = self.map.idx(x, y, sz + 1);
                    self.map.cells[kw].shape = Shape::Empty;
                    self.map.cells[kw].material = Material::Air;
                }
            }
        }
    }

    /// Run `days` game days.
    /// Try each of the patron's verbs on a fresh colony and check it changes what the settlers do
    /// within a game day (`--sim-patron`). Returns one line per verb, "ok" or why not.
    pub fn patron_trial(&mut self) -> Vec<String> {
        let mut out = Vec::new();
        self.run_days(1);
        let inside = |m: &PlaceMark, p: Pos| (m.at.0 as i32 - p.0 as i32).abs().max((m.at.1 as i32 - p.1 as i32).abs()) <= m.radius as i32;
        // Forbid the trees nearest the camp.
        let tree = self.nearest_tree(self.camp);
        let forbid = tree.map(|t| PlaceMark { at: t, radius: 8, forbidden: true });
        if let Some(m) = forbid { let _ = self.mark_place(m.at, m.radius, true); }
        // Bless a berry patch away from the camp.
        let patch = (12..40).find_map(|r| {
            let p = (self.camp.0.saturating_add(r), self.camp.1);
            self.nearest(p, |c, q| c.is_ripe_shrub(q) && (q.0 as i32 - c.camp.0 as i32).abs() + (q.1 as i32 - c.camp.1 as i32).abs() >= 12)
        });
        let bless = patch.map(|t| PlaceMark { at: t, radius: 6, forbidden: false });
        if let Some(m) = bless { let _ = self.mark_place(m.at, m.radius, false); }
        let fav = 0usize;
        let _ = self.favour_settler(fav);
        let (mut felled_inside, mut kept_off, mut forage_inside, mut fav_seen) = (0, 0, 0, false);
        let start = self.decisions.len();
        for _ in 0..TICKS_PER_DAY {
            self.tick();
            for st in &self.settlers {
                if let (Some(m), Job::Fell(t)) = (forbid, st.job) { if inside(&m, t) { felled_inside += 1; } }
                if let (Some(m), Job::Forage(t)) = (bless, st.job) { if inside(&m, t) { forage_inside += 1; } }
            }
        }
        for d in &self.decisions[start..] {
            if d.contains("keeping off the forbidden ground") { kept_off += 1; }
            if d.contains(&self.settlers[fav].name) && d.contains("(the patron's favourite)") { fav_seen = true; }
        }
        out.push(match forbid { Some(_) if felled_inside == 0 && kept_off > 0 => format!("forbid: ok ({} choices kept off it, no tree felled inside)", kept_off),
            Some(_) => format!("forbid: FAILED ({} felling ticks inside, {} choices kept off)", felled_inside, kept_off), None => "forbid: no tree to forbid".into() });
        out.push(match bless { Some(_) if forage_inside > 0 => format!("bless: ok ({} settler-minutes foraging on the blessed ground)", forage_inside),
            Some(_) => "bless: FAILED (nobody foraged there)".into(), None => "bless: no berry patch".into() });
        out.push(if fav_seen { "favourite: ok".into() } else { "favourite: FAILED".into() });
        // A dream, after the dawn gives favour back.
        // A well settler (an ill one only eats and rests).
        let dreamer = (1..self.settlers.len()).find(|&i| self.settlers[i].alive && self.settlers[i].ill_until <= self.clock.tick).unwrap_or(1);
        let _ = self.send_dream(dreamer, Dream::Plenty);
        let start = self.decisions.len();
        let mut dreamt = false;
        for _ in 0..TICKS_PER_DAY {
            self.tick();
            if matches!(self.settlers[dreamer].job, Job::Forage(_) | Job::Fish(_)) && self.settlers[dreamer].why.starts_with("Dreamt of") { dreamt = true; }
        }
        let _ = start;
        out.push(if dreamt { "dream: ok".into() } else { "dream: FAILED".into() });
        // The same mark again lifts it, at no cost; a hall stone after the first log says why not.
        if let Some(m) = forbid {
            let (marks, favour) = (self.patron.marks.len(), self.patron.favour);
            let r = self.mark_place(m.at, m.radius, true);
            let lifted = self.patron.marks.len() + 1 == marks && self.patron.favour == favour;
            out.push(if lifted { format!("lift: ok ({})", r.unwrap_or_default()) } else { "lift: FAILED".into() });
        }
        out.push(match self.place_stone(StoneKind::Hall, self.camp) {
            Err(e) if self.hut.as_ref().map_or(false, |h| h.logs_used > 0) => format!("late hall: ok ({})", e),
            Err(e) => format!("late hall: FAILED ({})", e),
            Ok(_) => "late hall: FAILED (accepted after the first log)".into(),
        });
        out
    }

    /// Where the hut stands, and how many trees are left within `r` of a point (for comparing
    /// layouts).
    pub fn trees_near(&self, at: Pos, r: i32) -> usize {
        let mut n = 0;
        for y in (at.1 as i32 - r).max(1)..(at.1 as i32 + r).min(self.map.height as i32 - 1) {
            for x in (at.0 as i32 - r).max(1)..(at.0 as i32 + r).min(self.map.width as i32 - 1) {
                if matches!(self.floor_plant((x as u16, y as u16)), Plant::Tree(_)) { n += 1; }
            }
        }
        n
    }

    /// The camp and a free spot `dx`, `dy` from it (the nearest passable cell), for trials.
    /// Where the night's watch stands: the camp's north edge.
    /// The watch post: open ground at the camp's north edge, never under a roof or inside a
    /// building (seed 3's second hut was raised over it, and a lookout inside the hut sealed it).
    pub fn watch_post(&self) -> Pos {
        // On the lookout tower's platform, once it stands.
        if let Some((t, _)) = self.tower { return t; }
        let (x, y) = (self.camp.0 as i32, self.camp.1 as i32 - 6);
        for r in 0..10i32 {
            for (ox, oy) in [(0, 0), (r, 0), (-r, 0), (0, -r), (0, r), (r, -r), (-r, -r), (r, r), (-r, r)] {
                let p = ((x + ox).clamp(2, self.map.width as i32 - 3) as u16, (y + oy).clamp(2, self.map.height as i32 - 3) as u16);
                if nav::passable(&self.map, p) && self.map.roofs[p.1 as usize * self.map.width + p.0 as usize] == 0 && !self.built_near(p) { return p; }
            }
        }
        self.spot_from_camp(0, -6)
    }

    pub fn spot_from_camp(&self, dx: i32, dy: i32) -> Pos {
        let (x, y) = (self.camp.0 as i32 + dx, self.camp.1 as i32 + dy);
        for r in 0..10 {
            for (ox, oy) in [(0, 0), (r, 0), (-r, 0), (0, r), (0, -r), (r, r), (-r, -r)] {
                let p = ((x + ox).clamp(2, self.map.width as i32 - 3) as u16, (y + oy).clamp(2, self.map.height as i32 - 3) as u16);
                if nav::passable(&self.map, p) { return p; }
            }
        }
        self.camp
    }

    /// The nearest tree to the camp that would be felled.
    /// Strip every berry bush within `r` of the camp (a trial: a camp whose food ran out).
    pub fn strip_berries(&mut self, r: i32) {
        let n = self.map.width;
        for y in 0..self.map.height {
            for x in 0..n {
                if (x as i32 - self.camp.0 as i32).abs().max((y as i32 - self.camp.1 as i32).abs()) > r { continue; }
                let k = self.map.idx(x, y, self.map.surface_z[y * n + x].max(0) as usize);
                if self.map.cells[k].plant == Plant::Shrub { self.map.cells[k].plant = Plant::Grass; }
            }
        }
    }

    /// Food within reach of the camp on its own dry ground (see `Survey`).
    pub fn survey(&self) -> Survey {
        let (patch, _) = dry_patches(&self.map);
        self.survey_at(self.camp, &patch)
    }

    /// Food within reach of `center` on its dry patch (`patch` from `dry_patches`).
    pub fn survey_at(&self, center: Pos, patch: &[u32]) -> Survey {
        let n = self.map.width;
        let home = patch[center.1 as usize * n + center.0 as usize];
        let (mut shrubs, mut fishing) = (0, 0);
        let (x0, x1) = ((center.0 as i32 - FORAGE_RADIUS).max(1) as usize, ((center.0 as i32 + FORAGE_RADIUS) as usize).min(n - 2));
        let (y0, y1) = ((center.1 as i32 - FORAGE_RADIUS).max(1) as usize, ((center.1 as i32 + FORAGE_RADIUS) as usize).min(self.map.height - 2));
        for y in y0..=y1 {
            for x in x0..=x1 {
                let d = (x as i32 - center.0 as i32).abs().max((y as i32 - center.1 as i32).abs());
                if d > FORAGE_RADIUS || patch[y * n + x] != home { continue; }
                let p = (x as u16, y as u16);
                if self.floor_plant(p) == Plant::Shrub { shrubs += 1; }
                if self.is_fishing_spot(p) { fishing += 1; }
            }
        }
        // A bush gives ~2.5 meals every 12 days; a spot ~2 every 3 days (and a fisher's 90
        // minutes: at most ~25 spots' worth a day for seven).
        // Game: six meals a head, and each herd sends back a head every four days (counting a
        // head as a day's 1.2 meals made a herd of ten look like a living on a stripped site).
        let herds = self.map.game.iter().filter(|g| g.1 > 0).count();
        let meals_a_day = herds as f32 * 1.5 + shrubs as f32 * 4.0 / SHRUB_REGROW_DAYS as f32 + (fishing.min(40) as f32) * 2.0 / FISH_RECOVER_DAYS as f32;
        Survey { shrubs, fishing, meals_a_day }
    }

    pub fn nearest_tree_to_camp(&self) -> Option<Pos> { self.nearest_tree(self.camp) }

    /// Replay one recorded act ("tick verb args", as `interventions` writes them); the tick is
    /// the caller's business. Returns what happened, or why not.
    pub fn apply_intervention(&mut self, line: &str) -> Result<String, String> {
        let w: Vec<&str> = line.split_whitespace().collect();
        let num = |k: usize| w.get(k).and_then(|x| x.parse::<u16>().ok()).ok_or_else(|| format!("bad intervention: {line}"));
        match w.get(1).copied() {
            Some("bless") | Some("forbid") => self.mark_place((num(2)?, num(3)?), num(4)?, w[1] == "forbid"),
            Some("favour") => self.favour_settler(num(2)? as usize),
            Some("dream") => {
                let d = match w.get(3).copied() { Some("Hut") => Dream::Hut, Some("Plenty") => Dream::Plenty, Some("Watch") => Dream::Watch, _ => Dream::Rest };
                self.send_dream(num(2)? as usize, d)
            }
            Some("stone") => {
                let k = match w.get(2).copied() { Some("Hall") => StoneKind::Hall, Some("Grove") => StoneKind::Grove, _ => StoneKind::Shrine };
                self.place_stone(k, (num(3)?, num(4)?))
            }
            Some("name") => { let n = w[2..].join(" "); self.name_colony(&n); Ok(n) }
            Some("refugees") => self.answer_refugees(w.get(2).copied() == Some("take")),
            Some("bell") => self.ring_bell(),
            _ => Err(format!("unknown intervention: {line}")),
        }
    }

    /// Live `days` days, applying the scripted interventions ("tick verb args") at their ticks.
    pub fn run_days_scripted(&mut self, days: u64, script: &[String]) {
        let end = self.clock.tick + days * TICKS_PER_DAY;
        while self.clock.tick < end {
            // Every line whose tick has come and not yet run (lines timed before the colony's
            // first morning run at once).
            let now = self.clock.tick;
            for line in script {
                let t = line.split_whitespace().next().and_then(|t| t.parse::<u64>().ok()).unwrap_or(u64::MAX);
                if t >= self.script_at && t <= now {
                    if let Err(e) = self.apply_intervention(line) { self.note(format!("(intervention failed: {e})")); }
                }
            }
            self.script_at = now + 1;
            self.tick();
        }
    }

    pub fn run_days(&mut self, days: u64) {
        let end = self.clock.tick + days * TICKS_PER_DAY;
        let mut warned_food = false;
        while self.clock.tick < end {
            self.tick();
            if self.clock.tick % TICKS_PER_DAY == 6 * 60 {
                let hut = match &self.hut { Some(h) if h.done => "the hut stands".to_string(), Some(h) => format!("the hut has {} of {} logs", h.logs_used, HUT_LOGS), None => "no hut site".into() };
                let hungry = self.settlers.iter().filter(|s| s.alive && s.hunger > 0.7).count();
                let chilled = self.settlers.iter().filter(|s| s.alive && s.exposure > 0.6).count();
                self.note(format!("Dawn: {} of {} alive, {} meals at the camp, {} logs stored, {}{}{}.",
                    self.alive(), self.company(), self.food_stored(), self.logs_stored(), hut,
                    if hungry > 0 { format!(", {} hungry", hungry) } else { String::new() },
                    if chilled > 0 { format!(", {} chilled from the night", chilled) } else { String::new() }));
            }
            if self.clock.tick % 60 == 0 {
                // Once a day at most (a berry or two coming in had it said five times a day).
                let low = self.food_stored() == 0;
                if low && !warned_food && self.food_warned_day != self.clock.day() { self.note("The camp has no food left.".into()); self.food_warned_day = self.clock.day(); }
                warned_food = low;
            }
        }
    }

    /// One line per settler: who, what, why.
    pub fn roll_call(&self) -> Vec<String> {
        self.settlers.iter().map(|s| if s.alive {
            format!("{:<9} {:<16} hunger {:>3.0}% rest {:>3.0}% chill {:>3.0}%  {}", s.name, s.job.verb(), s.hunger * 100.0, (1.0 - s.fatigue) * 100.0, s.exposure * 100.0, s.why)
        } else { format!("{:<9} dead", s.name) }).collect()
    }
}

/// Dry, walkable ground joined in one piece this large (cells of 2 m: a tenth of a km2) is
/// enough to live from.
pub const ENOUGH_LAND: usize = 2500;

/// Connected patches of dry, walkable ground: each cell's patch id (u32::MAX if not dry) and
/// each patch's size.
pub fn dry_patches(map: &LocalMap) -> (Vec<u32>, Vec<usize>) {
    let n = map.width;
    let dry = |x: usize, y: usize| {
        let sz = map.surface_z[y * n + x].max(0) as usize;
        nav::passable(map, (x as u16, y as u16)) && (sz + 1 >= map.depth || map.cell(x, y, sz + 1).water == 0)
    };
    let mut patch = vec![u32::MAX; n * map.height];
    let mut sizes: Vec<usize> = Vec::new();
    for start in 0..n * map.height {
        if patch[start] != u32::MAX || !dry(start % n, start / n) { continue; }
        let id = sizes.len() as u32;
        let mut stack = vec![start];
        patch[start] = id;
        let mut count = 0;
        while let Some(i) = stack.pop() {
            count += 1;
            let (x, y) = (i % n, i / n);
            for (dx, dy) in [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)] {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx < 0 || ny < 0 || nx >= n as i32 || ny >= map.height as i32 { continue; }
                let j = ny as usize * n + nx as usize;
                if patch[j] == u32::MAX && dry(nx as usize, ny as usize) { patch[j] = id; stack.push(j); }
            }
        }
        sizes.push(count);
    }
    (patch, sizes)
}

/// Whether a colony can live here: some dry ground joined in one piece of `ENOUGH_LAND` (an
/// embark in a lake is not).
/// "3 hours 20 minutes", "1 day 4 hours".
pub fn span_words(ticks: u64) -> String {
    let (d, h, m) = (ticks / TICKS_PER_DAY, ticks % TICKS_PER_DAY / 60, ticks % 60);
    let part = |n: u64, w: &str| format!("{} {}{}", n, w, if n == 1 { "" } else { "s" });
    if d > 0 { format!("{} {}", part(d, "day"), part(h, "hour")) } else if h > 0 { format!("{} {}", part(h, "hour"), part(m, "minute")) } else { part(m, "minute") }
}

pub fn habitable(map: &LocalMap) -> bool { dry_patches(map).1.iter().any(|&c| c >= ENOUGH_LAND) }

/// What a camp could live on, counted before anyone settles: berry bushes and fishing spots on
/// the camp's own dry ground within working reach, and meals a day they would give.
#[derive(Clone, Debug)]
pub struct Survey { pub shrubs: usize, pub fishing: usize, pub meals_a_day: f32 }

/// Meals a day seven settlers eat (one meal per 18 hours each).
pub const MEALS_NEEDED: f32 = 7.0 * 24.0 / 18.0 / MEAL;

impl Survey {
    /// Err: no camp can live here (why); Ok(Some): hard (why); Ok(None): enough.
    pub fn verdict(&self) -> Result<Option<String>, String> {
        let what = format!("{} berry bush{}, {}", self.shrubs, if self.shrubs == 1 { "" } else { "es" },
            if self.fishing == 0 { "no fishing water".to_string() } else { format!("{} fishing spots", self.fishing) });
        if self.meals_a_day < 0.4 * MEALS_NEEDED {
            Err(format!("too little to eat within reach to feed seven ({}): they would starve", what))
        } else if self.meals_a_day < 1.2 * MEALS_NEEDED {
            Ok(Some(format!("hard: little to eat within reach ({})", what)))
        } else {
            Ok(None)
        }
    }
}

/// Survey an embark for a camp of seven (founds one on a copy, so it sees what they would).
pub fn survey(map: &LocalMap) -> Survey {
    let c = Colony::found(map.clone(), &(0..7).map(|i| format!("s{i}")).collect::<Vec<_>>(), 0);
    c.survey()
}

fn join_names(v: &[String]) -> String {
    match v {
        [] => "no one".into(),
        [a] => a.clone(),
        [rest @ .., last] => format!("{} and {}", rest.join(", "), last),
    }
}

fn lower_first(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_lowercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

fn capital(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}
