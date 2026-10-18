//! Names and words for places (ROADMAP Stage 4): a name in the old tongue with an epithet, a
//! description and what the place holds, for every place the planners left unnamed.
//!
//! The old tongue: the history's languages are not kept on `WorldData`, so each family of tiles
//! (a 6x6 block, the size of a valley or a coast) draws its own sounds (`Tongue`: a set of
//! onsets, vowels and codas picked by the block's hash), and a place's name is two or three
//! syllables of them with a cap on the harsh clusters, so the names of one valley sound alike
//! and the next valley's differ. The epithet is the kind's, in plain words ("where the first
//! farmers laid their dead"); the description weaves the cause in; the contents are what a
//! visitor, or a spade, finds. Ranks the planners did not set come from the kind's rarity.

use crate::region::landforms::RegionFields;
use crate::region::plan::{Age, Place, PlaceKind};
use crate::region::zoom::ZoomRegion;

use super::natural::hash;
use super::SitePlan;

/// Tiles to a side of the block that shares a tongue.
const FAMILY_TILES: i64 = 6;

/// The sounds of a block of tiles.
struct Tongue {
    onsets: Vec<&'static str>,
    vowels: Vec<&'static str>,
    codas: Vec<&'static str>,
    /// The share of names that take a third syllable.
    long: f32,
}

const ONSETS: [&str; 28] = ["b", "br", "c", "cr", "d", "dr", "f", "g", "gl", "h", "k", "l", "m", "n", "p", "r", "s", "sk", "t", "th", "tr", "v", "w", "y", "sh", "wr", "st", "fl"];
const VOWELS: [&str; 14] = ["a", "e", "i", "o", "u", "ae", "ai", "ea", "ei", "ou", "oe", "y", "io", "au"];
const CODAS: [&str; 20] = ["", "", "n", "r", "l", "s", "th", "nd", "rn", "ld", "st", "m", "ck", "rth", "ll", "nn", "dd", "gh", "w", "x"];

impl Tongue {
    fn of(tile: (usize, usize)) -> Tongue {
        let (bx, by) = (tile.0 as i64 / FAMILY_TILES, tile.1 as i64 / FAMILY_TILES);
        let pick = |pool: &[&'static str], n: usize, salt: u64| -> Vec<&'static str> {
            let mut out: Vec<&'static str> = Vec::new();
            let mut i = 0u64;
            while out.len() < n && i < 200 {
                let s = pool[(hash(bx, by, salt ^ i) % pool.len() as u64) as usize];
                if !out.contains(&s) { out.push(s); }
                i += 1;
            }
            out
        };
        Tongue {
            onsets: pick(&ONSETS, 9, 0x7A1),
            vowels: pick(&VOWELS, 5, 0x7A2),
            codas: pick(&CODAS, 7, 0x7A3),
            long: 0.25 + 0.5 * (hash(bx, by, 0x7A4) % 100) as f32 / 100.0,
        }
    }

    /// A name from the place's own hash: two or three syllables, capitalised.
    fn name(&self, key: u64) -> String {
        let syllables = if (hash(key as i64, 1, 0x7B0) % 100) as f32 / 100.0 < self.long { 3 } else { 2 };
        let mut s = String::new();
        for n in 0..syllables {
            let h = hash(key as i64, n as i64, 0x7B1);
            let onset = self.onsets[(h % self.onsets.len() as u64) as usize];
            let vowel = self.vowels[((h >> 8) % self.vowels.len() as u64) as usize];
            // A coda only on the last syllable, and on a middle one in three.
            let coda = if n == syllables - 1 || (h >> 16) % 3 == 0 { self.codas[((h >> 24) % self.codas.len() as u64) as usize] } else { "" };
            s.push_str(onset);
            s.push_str(vowel);
            s.push_str(coda);
        }
        let mut c = s.chars();
        match c.next() { Some(f) => f.to_uppercase().collect::<String>() + c.as_str(), None => s }
    }
}

/// The epithet of a kind: what the name is said with.
fn epithet(kind: PlaceKind, key: u64) -> &'static str {
    use PlaceKind::*;
    let pick = |opts: &[&'static str]| opts[(hash(key as i64, 2, 0x7C0) % opts.len() as u64) as usize];
    match kind {
        CaveMouth => pick(&["the mouth in the hill", "the dark door", "where the water goes under", "the cold breath"]),
        Spring => pick(&["the well that never fails", "the clear spring", "the water from the rock"]),
        Waterfall => pick(&["the white fall", "the leap", "the roaring water", "the grey mare's tail"]),
        HotSpring => pick(&["the warm pool", "the steaming water", "the bath of the old ones"]),
        Geyser => pick(&["the spout", "the water that leaps"]),
        MudPot => pick(&["the boiling clay", "the grey cauldron"]),
        Gorge => pick(&["the cleft", "the narrows", "the river's gate", "the deep cut"]),
        NaturalArch => pick(&["the stone bridge", "the eye in the rock", "the door of the giants"]),
        SeaStack => pick(&["the old man of the sea", "the needle", "the stack"]),
        Erratic => pick(&["the stone the ice left", "the wanderer", "the giant's throw"]),
        BoulderField => pick(&["the broken ground", "the field of stones"]),
        SaltSpring => pick(&["the salt lick", "the bitter spring"]),
        TarSeep => pick(&["the pitch pool", "the black seep"]),
        PetrifiedForest => pick(&["the stone wood", "the trees of stone"]),
        FossilBed => pick(&["the shell cliff", "the bones in the rock"]),
        AmberShore => pick(&["the gold strand", "the amber shore"]),
        MeteorCrater => pick(&["the ring of the fallen star", "the sky's wound"]),
        Oxbow => pick(&["the crooked water", "the horseshoe"]),
        Tarn => pick(&["the black tarn", "the eye of the mountain", "the high water"]),
        Tor => pick(&["the stacked stones", "the giant's chair"]),
        Pavement => pick(&["the bare stone field", "the grey pavement"]),
        Blowhole => pick(&["the breathing rock", "the sea's chimney"]),
        GiantTree => pick(&["the old one", "the tree older than any people"]),
        CraterLake => pick(&["the lake in the fire's bowl", "the round water"]),
        RockShelter => pick(&["the roof of the first people", "the hunters' overhang"]),
        PaintedCave => pick(&["the painted dark", "the cave of the beasts"]),
        KillSite => pick(&["the killing ground", "where the herds died"]),
        BoneBed => pick(&["the gravel of bones", "the bank of old bones"]),
        ShellMidden => pick(&["the shell hill", "the eaters' mound"]),
        FlintMine => pick(&["the flint pits", "the shafts of the axe-makers"]),
        LongBarrow => pick(&["the long mound", "where the first farmers laid their dead", "the house of the dead"]),
        RoundBarrows => pick(&["the row of mounds", "the chiefs' graves on the skyline"]),
        StoneCircle => pick(&["the ring of stones", "the dancers", "the moon's ring"]),
        StandingStone => pick(&["the lone stone", "the finger of stone"]),
        Dolmen => pick(&["the table of the giants", "the capped stones"]),
        RockArt => pick(&["the carved rock", "the cups and rings"]),
        Enclosure => pick(&["the banked ring", "the gathering ground"]),
        Henge => pick(&["the great ring", "the ditch of the sky"]),
        HillFort => pick(&["the ramparts", "the old stronghold", "the hill of the banks"]),
        Crannog => pick(&["the island of timber", "the lake house"]),
        BogHoard => pick(&["the bog's gift", "the sunk bronze"]),
        BogBody => pick(&["the bound one", "the peat's keeping"]),
        CopperWorking => pick(&["the green rock", "the fire-set hill"]),
        DesertKite => pick(&["the long walls", "the pen of the herds"]),
        Ridgeway => pick(&["the high way", "the old road on the crest"]),
        Causeway => pick(&["the log road", "the way over the bog"]),
        Ford => pick(&["the shallow crossing", "the gravel ford"]),
        Lynchets => pick(&["the stepped hillside", "the old fields"]),
        ClearanceCairns => pick(&["the heaps", "the fields' stones"]),
        Watchtower => pick(&["the broken tower", "the watch"]),
        Quarry => pick(&["the cut rock", "the old quarry"]),
        MillRuin => pick(&["the mill that fell", "the wheel's ruin"]),
        DrownedVillage => pick(&["the village under the water", "the drowned street"]),
        ImpactStructure => pick(&["the ring of the fallen star", "the sky's wound", "the great bowl", "the stone that fell"]),
        Landslide => pick(&["where the hill came down", "the fallen side", "the broken slope"]),
        LandslideLake => pick(&["the water the hill held back", "the dammed lake", "the lake behind the fall"]),
        FaultScarp => pick(&["the step in the land", "the earth's crack", "the long wall"]),
        SagPond => pick(&["the pond on the crack", "the sunk water"]),
        Maar => pick(&["the round water", "the eye of the ground", "the lake the fire blew"]),
        GasHollow => pick(&["the hollow of bad air", "where the birds fall", "the breathless dip"]),
        CollapsedFlank => pick(&["the mountain's missing side", "the open crater"]),
        Sinkhole => pick(&["the ground that fell", "the pit", "the swallow"]),
        DryFalls => pick(&["the falls without water", "the flood's cliff"]),
        Quicksand => pick(&["the sucking sand", "the false ground"]),
        CrevasseField => pick(&["the broken ice", "the ice's teeth"]),
        TarPit => pick(&["the black trap", "the pitch that holds"]),
        CrownHoles => pick(&["the fallen ground over the shafts", "the miners' pits"]),
        Skyburn => pick(&["where the sky burst", "the felled wood"]),
        Ruin | Tomb | Lair | Battlefield | BoneField | OldMine => "",
    }
}

/// What the place holds, or what a spade finds.
fn contents(kind: PlaceKind, key: u64) -> Vec<String> {
    use PlaceKind::*;
    let maybe = |n: u64, s: &str| if hash(key as i64, 3, 0x7D0 ^ n) % 2 == 0 { Some(s.to_string()) } else { None };
    let list: Vec<Option<String>> = match kind {
        CaveMouth => vec![Some("a cold draught from the dark".into()), maybe(1, "bats"), maybe(2, "the bones of a cave bear")],
        Spring | SaltSpring => vec![Some("fresh water".into())],
        Waterfall => vec![Some("a plunge pool".into()), maybe(1, "a fisherman's ledge")],
        HotSpring => vec![Some("warm water all winter".into()), maybe(1, "a crust of coloured salts")],
        Geyser | MudPot => vec![Some("scalding ground".into())],
        Gorge => vec![Some("a path along the ledge".into()), maybe(1, "a rope bridge's old posts")],
        NaturalArch | SeaStack | Blowhole => vec![Some("sea birds' nests".into())],
        Erratic | BoulderField | Tor => vec![Some("shelter among the stones".into())],
        TarSeep => vec![Some("pitch for boats and torches".into()), maybe(1, "the bones of beasts the pitch caught")],
        PetrifiedForest => vec![Some("logs of stone with their rings".into())],
        FossilBed => vec![Some("shells and the ribs of sea beasts in the rock".into())],
        AmberShore => vec![Some("amber after storms".into())],
        MeteorCrater => vec![Some("black iron in the broken ground".into())],
        Oxbow | Tarn | CraterLake => vec![Some("still water and fish".into())],
        Pavement => vec![Some("ferns in the cracks".into())],
        GiantTree => vec![Some("a trunk ten men cannot ring".into())],
        RockShelter => vec![Some("a hearth of the first people".into()), Some("flint flakes".into()), maybe(1, "a burial under the floor")],
        PaintedCave => vec![Some("beasts painted in ochre and soot".into()), Some("the hunters' handprints".into())],
        KillSite => vec![Some("the bones of aurochs and giant deer".into()), maybe(1, "a flint point among the ribs")],
        BoneBed => vec![Some("the bones of the cold age's beasts".into()), maybe(1, "a mammoth's tusk")],
        ShellMidden => vec![Some("shells by the thousand".into()), maybe(1, "bone hooks and a stone lamp")],
        FlintMine => vec![Some("shafts and galleries".into()), Some("antler picks".into()), maybe(1, "a miner's bones")],
        LongBarrow => vec![Some("a chamber of great slabs".into()), Some("the bones of the first farmers".into()), maybe(1, "a polished stone axe")],
        RoundBarrows => vec![Some("the chiefs' graves".into()), maybe(1, "a bronze dagger"), maybe(2, "a gold cup")],
        StoneCircle => vec![Some("standing stones in a ring".into()), maybe(1, "a stone fallen and cracked")],
        StandingStone => vec![Some("one stone taller than a man".into())],
        Dolmen => vec![Some("a capstone on three uprights".into()), maybe(1, "bones under the floor")],
        RockArt => vec![Some("cups and rings cut in the rock".into())],
        Enclosure | Henge => vec![Some("banks and ditches".into()), maybe(1, "antler and pottery in the ditch")],
        HillFort => vec![Some("ramparts and a gate".into()), Some("hut rings".into()), maybe(1, "sling stones in heaps")],
        Crannog => vec![Some("timber piles in the shallows".into()), maybe(1, "a dugout boat in the mud")],
        BogHoard => vec![Some("bronze sunk in the peat".into()), maybe(1, "a gold torc")],
        BogBody => vec![Some("a body the peat kept, bound".into())],
        CopperWorking => vec![Some("green ore and fire-cracked rock".into()), Some("stone hammers".into())],
        DesertKite => vec![Some("walls of stone in a V".into())],
        Ridgeway | Causeway | Ford => vec![Some("the old track".into())],
        Lynchets | ClearanceCairns => vec![Some("the shapes of old fields".into())],
        Watchtower => vec![Some("a fallen tower".into()), maybe(1, "a hearth of signal fires")],
        Quarry => vec![Some("cut faces and spoil".into()), maybe(1, "a block left half-cut")],
        MillRuin => vec![Some("the wheel pit and the race".into()), maybe(1, "a millstone")],
        DrownedVillage => vec![Some("walls under the water".into()), maybe(1, "a church tower above it in a dry year")],
        ImpactStructure => vec![Some("broken rock fused to glass".into()), maybe(1, "shatter cones in the ring's rock"), maybe(2, "lumps of black iron")],
        Landslide => vec![Some("raw ground and broken trees".into()), maybe(1, "stones still falling after rain")],
        LandslideLake => vec![Some("drowned trees standing in the water".into()), Some("a dam of rubble that may not hold".into())],
        FaultScarp => vec![Some("a fresh step of earth and rock".into()), maybe(1, "a stream bent along the crack")],
        SagPond => vec![Some("still water on the fault line".into())],
        Maar => vec![Some("deep still water in a ring of ash".into())],
        GasHollow => vec![Some("dead birds and a fox, lying where the air is bad".into()), Some("air that puts out a torch".into())],
        CollapsedFlank => vec![Some("a field of hummocks where the mountain's side came down".into())],
        Sinkhole => vec![Some("a shaft where the ground gave way".into()), maybe(1, "a sheep's bones at the bottom")],
        DryFalls => vec![Some("a cliff of bare rock with plunge pools and no river".into()), maybe(1, "boulders the size of houses")],
        Quicksand => vec![Some("sand that takes a beast to the belly".into())],
        CrevasseField => vec![Some("cracks in the ice too deep to see the bottom".into()), maybe(1, "snow bridges over them")],
        TarPit => vec![Some("pitch".into()), Some("the bones of beasts the pitch caught".into())],
        CrownHoles => vec![Some("round pits where old shafts fell in".into()), maybe(1, "a gallery open at the bottom of one")],
        Skyburn => vec![Some("dead trunks lying in rows, all pointing away from one place".into()), maybe(1, "young birch coming up through the charcoal")],
        Ruin | Tomb | Lair | Battlefield | BoneField | OldMine => vec![],
    };
    list.into_iter().flatten().collect()
}

/// The description: the cause woven into a sentence or two of the kind's.
fn description(p: &Place) -> String {
    use PlaceKind::*;
    let cause = if p.cause.is_empty() { String::new() } else { let mut c = p.cause.clone(); if let Some(f) = c.get_mut(0..1) { f.make_ascii_uppercase(); } format!("{c}. ") };
    let z = p.z_m.map(|z| format!(" It lies {:.0} m above the sea.", z)).unwrap_or_default();
    let body = match p.kind.age() {
        Age::Natural => match p.kind {
            CaveMouth => "A dark opening into the rock, cold air moving out of it; the passage goes in further than a torch shows.",
            Gorge => "The river runs between walls of rock; the path follows a ledge, with the water loud below.",
            Tarn => "A small lake in a high bowl, walled by rock on three sides, cold and clear.",
            HotSpring => "Steam stands over a pool whose water is warm in the coldest month.",
            ImpactStructure => "A round bowl in the land with a raised rim, where a stone from the sky struck long before any people: the rock in its walls is broken and melted.",
            Landslide => "The hillside gave way and came down in a tongue of rubble; the scar above is still bare.",
            LandslideLake => "A slide fell across the valley and the river backed up behind it into a lake; the dam is rubble, and one spring it may go.",
            FaultScarp => "The ground stands higher on one side of a line than the other: the earth moved here, and will again.",
            GasHollow => "A still hollow where the ground breathes out an air that does not burn; beasts that go down to drink do not come up.",
            Sinkhole => "The ground fell in where the rock under it had dissolved away; the edge is not to be trusted.",
            Quicksand => "Wet sand that looks like any shore; water welling up through it keeps it loose, and it holds what steps in it.",
            CrevasseField => "The ice cracks where it bends over a step in its bed; fresh snow hides the cracks.",
            TarPit => "Pitch seeps up and pools, a skin of water and leaves over it; beasts that wade in stay.",
            CrownHoles => "The ground has fallen into the old workings beneath, one pit after another; more will open.",
            _ => "A thing the land made, worth the walk.",
        },
        Age::Hunters => "The first people lived here before any field was ploughed; what they left is under the turf and in the dark.",
        Age::Farmers => "The first farmers of this valley made it, before the stones were raised; it is older than any tale.",
        Age::Builders => "Raised by the people who set up stones, in the age before metal; the valley's folk still keep away from it after dark.",
        Age::Metal => "The work of the first metal age, when chiefs were buried under mounds and hills were walled.",
        Age::LastAge => "From the last age before the written history: not so old that its purpose is forgotten, old enough that no one remembers its builders.",
        Age::Chronicle => "",
    };
    format!("{cause}{body}{z}")
}

/// A rank by rarity, for the kinds whose planners set none (255 is the default of
/// `Place::point`; 200 the natural wonders').
fn rank_of(kind: PlaceKind) -> u8 {
    use PlaceKind::*;
    match kind {
        ImpactStructure => 10,
        MeteorCrater | GiantTree | CraterLake => 20,
        Maar | CollapsedFlank | DryFalls | LandslideLake => 45,
        FaultScarp | Landslide | GasHollow | Sinkhole | TarPit | CrevasseField => 80,
        SagPond | Quicksand | CrownHoles => 120,
        StoneCircle | Henge | HillFort | PaintedCave | NaturalArch | Geyser => 45,
        Waterfall | Gorge | HotSpring | LongBarrow | RoundBarrows | Dolmen | DrownedVillage | Tarn | SeaStack | Blowhole | Crannog => 80,
        CaveMouth | Spring | RockShelter | FlintMine | Enclosure | StandingStone | RockArt | Watchtower | CopperWorking | BogHoard | BogBody | FossilBed | PetrifiedForest | AmberShore | TarSeep | SaltSpring => 120,
        _ => 170,
    }
}

/// Give every unnamed place its name and words.
pub fn name_all(_f: &RegionFields, plan: &mut SitePlan) {
    for (i, p) in plan.places.iter_mut().enumerate() {
        if p.kind.age() == Age::Chronicle { continue; }
        let key = hash(p.x.round() as i64, p.y.round() as i64, 0x7E0 ^ i as u64);
        if p.name.is_empty() {
            let tongue = Tongue::of(p.tile);
            p.name = tongue.name(key);
        }
        if p.epithet.is_empty() { p.epithet = epithet(p.kind, key).to_string(); }
        if p.description.is_empty() { p.description = description(p); }
        if p.contents.is_empty() { p.contents = contents(p.kind, key); }
        if p.rank >= 200 { p.rank = rank_of(p.kind); }
    }
}

/// The listing `--places` prints: by age, each place with its title, epithet, cause,
/// description and contents.
pub fn listing(z: &ZoomRegion, secs: f32) -> String {
    let mut s = format!("{} places in {}x{} cells ({:.2} s)\n", z.places.len(), z.width, z.height, secs);
    let sc = (z.params.cells_per_tile.max(8) & !1) as i64;
    for age in [Age::Natural, Age::Hunters, Age::Farmers, Age::Builders, Age::Metal, Age::LastAge, Age::Chronicle] {
        let mut of_age: Vec<&Place> = z.places.iter().filter(|p| p.kind.age() == age).collect();
        if of_age.is_empty() { continue; }
        of_age.sort_by_key(|p| (p.rank, p.kind as u8));
        s += &format!("\n== {}: {}\n", match age {
            Age::Natural => "The land's own wonders", Age::Hunters => "The first hunters", Age::Farmers => "The first farmers",
            Age::Builders => "The builders of stone", Age::Metal => "The first metal", Age::LastAge => "The last age before the chronicle",
            Age::Chronicle => "The chronicle's",
        }, of_age.len());
        for p in of_age {
            let (i, j) = (p.x - (z.world_x0 * sc) as f64, p.y - (z.world_y0 * sc) as f64);
            s += &format!("{} ({}; rank {}) at cell {:.0},{:.0}, tile {},{}\n", p.title(), if p.epithet.is_empty() { "-" } else { &p.epithet }, p.rank, i, j, p.tile.0, p.tile.1);
            if !p.cause.is_empty() { s += &format!("    why here: {}\n", p.cause); }
            if !p.description.is_empty() { s += &format!("    {}\n", p.description); }
            if !p.contents.is_empty() { s += &format!("    holds: {}\n", p.contents.join("; ")); }
        }
    }
    s
}
