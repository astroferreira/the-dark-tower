//! The site layer: what a region holds beyond heights and water (ROADMAP Stages 2-6).
//!
//! Planned once from the world's fields at region resolution and read by the region generator,
//! the region ink and the embark, so the three never disagree: the cliff the region draws is the
//! cliff the embark carves, the cave mouth is at the same cell in both, and two embarks that
//! overlap carve the same passages. Every value here is a pure function of world position
//! (position-keyed noise, bounded neighbourhoods inside the region's margin).
//!
//! Positions are **absolute region cells**: `x = world tile x * cells_per_tile + i` (fractional
//! for a point), the frame `local::generate_local` and `ZoomRegion` already share. A region
//! converts with `x - world_x0 * s`; metres are cells times `ZoomRegion::cell_m`.

use crate::erosion::materials::RockType;

/// A class of ground at region resolution, from the process that made it. `None` is plain
/// ground: soil over rock, shaped only by the rivers.
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Hash)]
pub enum Landform {
    #[default]
    None = 0,
    // Rock: where the surface rock and its hardness shape the ground.
    Cliff,
    Scree,
    Outcrop,
    Tor,
    Mesa,
    SlotCanyon,
    Badland,
    Hoodoo,
    // Karst: carbonate rock drained through the ground.
    Pavement,
    Doline,
    DryValley,
    Sink,
    Resurgence,
    TowerKarst,
    Cenote,
    KarstGorge,
    // Ice: what the ice left, and where it still lies.
    UValley,
    Cirque,
    Tarn,
    Arete,
    Moraine,
    Drumlin,
    Erratic,
    Outwash,
    Kettle,
    Glacier,
    Snowfield,
    PatternedGround,
    // Fire: volcanic ground.
    Crater,
    Caldera,
    CinderCone,
    LavaField,
    ColumnarBasalt,
    AshPlain,
    Fumarole,
    ThermalGround,
    // Wind: bare dry ground moved by the climate's winds.
    Dune,
    SandSea,
    Yardang,
    DesertPavement,
    Playa,
    // River: the valley floor and the channel's work.
    Floodplain,
    Levee,
    Backswamp,
    Oxbow,
    Terrace,
    Braided,
    Fan,
    Delta,
    Estuary,
    Waterfall,
    Rapids,
    Gorge,
    // Coast.
    SeaCliff,
    Stack,
    Beach,
    Spit,
    Bar,
    BarrierIsland,
    Lagoon,
    CoastalDune,
    TidalFlat,
    SaltMarsh,
    Mangrove,
    RaisedBeach,
    // Wet ground.
    Bog,
    Fen,
    Marsh,
    ReedBed,
    // Upheaval (`crate::upheaval`, ROADMAP Stage 7): the sky, the faults and the slopes in a
    // hurry. Appended so the classes before keep their numbers.
    ImpactCrater,
    ImpactRim,
    Ejecta,
    CentralPeak,
    FaultScarp,
    FaultTrough,
    SagPond,
    PressureRidge,
    LandslideScar,
    LandslideDebris,
    RockAvalanche,
    BurnScar,
    // Fire, more of it.
    Maar,
    FissureVents,
    CollapseScar,
    Lahar,
    Traps,
    // Ground that gives way.
    SaltSink,
    ThawLake,
    // Ice and snow that kill.
    Crevasses,
    AvalanchePath,
    // Floods.
    Scabland,
    AbandonedChannel,
}

/// The process a landform belongs to (one colour family, one legend group).
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum Family { Plain, Rock, Karst, Ice, Fire, Wind, River, Coast, Wet, Upheaval }

impl Landform {
    pub const ALL: &'static [Landform] = &[
        Landform::None, Landform::Cliff, Landform::Scree, Landform::Outcrop, Landform::Tor, Landform::Mesa,
        Landform::SlotCanyon, Landform::Badland, Landform::Hoodoo, Landform::Pavement, Landform::Doline,
        Landform::DryValley, Landform::Sink, Landform::Resurgence, Landform::TowerKarst, Landform::Cenote,
        Landform::KarstGorge, Landform::UValley, Landform::Cirque, Landform::Tarn, Landform::Arete,
        Landform::Moraine, Landform::Drumlin, Landform::Erratic, Landform::Outwash, Landform::Kettle,
        Landform::Glacier, Landform::Snowfield, Landform::PatternedGround, Landform::Crater, Landform::Caldera,
        Landform::CinderCone, Landform::LavaField, Landform::ColumnarBasalt, Landform::AshPlain,
        Landform::Fumarole, Landform::ThermalGround, Landform::Dune, Landform::SandSea, Landform::Yardang,
        Landform::DesertPavement, Landform::Playa, Landform::Floodplain, Landform::Levee, Landform::Backswamp,
        Landform::Oxbow, Landform::Terrace, Landform::Braided, Landform::Fan, Landform::Delta, Landform::Estuary,
        Landform::Waterfall, Landform::Rapids, Landform::Gorge, Landform::SeaCliff, Landform::Stack,
        Landform::Beach, Landform::Spit, Landform::Bar, Landform::BarrierIsland, Landform::Lagoon,
        Landform::CoastalDune, Landform::TidalFlat, Landform::SaltMarsh, Landform::Mangrove,
        Landform::RaisedBeach, Landform::Bog, Landform::Fen, Landform::Marsh, Landform::ReedBed,
        Landform::ImpactCrater, Landform::ImpactRim, Landform::Ejecta, Landform::CentralPeak, Landform::FaultScarp,
        Landform::FaultTrough, Landform::SagPond, Landform::PressureRidge, Landform::LandslideScar,
        Landform::LandslideDebris, Landform::RockAvalanche, Landform::BurnScar, Landform::Maar, Landform::FissureVents,
        Landform::CollapseScar, Landform::Lahar, Landform::Traps, Landform::SaltSink, Landform::ThawLake,
        Landform::Crevasses, Landform::AvalanchePath, Landform::Scabland, Landform::AbandonedChannel,
    ];

    pub fn family(self) -> Family {
        use Landform::*;
        match self {
            None => Family::Plain,
            Cliff | Scree | Outcrop | Tor | Mesa | SlotCanyon | Badland | Hoodoo => Family::Rock,
            Pavement | Doline | DryValley | Sink | Resurgence | TowerKarst | Cenote | KarstGorge | SaltSink => Family::Karst,
            UValley | Cirque | Tarn | Arete | Moraine | Drumlin | Erratic | Outwash | Kettle | Glacier | Snowfield | PatternedGround
            | ThawLake | Crevasses | AvalanchePath => Family::Ice,
            Crater | Caldera | CinderCone | LavaField | ColumnarBasalt | AshPlain | Fumarole | ThermalGround
            | Maar | FissureVents | CollapseScar | Lahar | Traps => Family::Fire,
            Dune | SandSea | Yardang | DesertPavement | Playa => Family::Wind,
            Floodplain | Levee | Backswamp | Oxbow | Terrace | Braided | Fan | Delta | Estuary | Waterfall | Rapids | Gorge
            | Scabland | AbandonedChannel => Family::River,
            ImpactCrater | ImpactRim | Ejecta | CentralPeak | FaultScarp | FaultTrough | SagPond | PressureRidge
            | LandslideScar | LandslideDebris | RockAvalanche | BurnScar => Family::Upheaval,
            SeaCliff | Stack | Beach | Spit | Bar | BarrierIsland | Lagoon | CoastalDune | TidalFlat | SaltMarsh | Mangrove | RaisedBeach => Family::Coast,
            Bog | Fen | Marsh | ReedBed => Family::Wet,
        }
    }

    /// The word on the hover and in the survey.
    pub fn name(self) -> &'static str {
        use Landform::*;
        match self {
            None => "ground",
            Cliff => "cliff", Scree => "scree", Outcrop => "outcrop", Tor => "tor", Mesa => "mesa",
            SlotCanyon => "slot canyon", Badland => "badlands", Hoodoo => "hoodoos",
            Pavement => "limestone pavement", Doline => "doline", DryValley => "dry valley", Sink => "sink",
            Resurgence => "resurgence", TowerKarst => "tower karst", Cenote => "cenote", KarstGorge => "karst gorge",
            UValley => "glacial valley", Cirque => "cirque", Tarn => "tarn", Arete => "arete", Moraine => "moraine",
            Drumlin => "drumlin", Erratic => "erratic", Outwash => "outwash plain", Kettle => "kettle hole",
            Glacier => "glacier", Snowfield => "snowfield", PatternedGround => "patterned ground",
            Crater => "crater", Caldera => "caldera", CinderCone => "cinder cone", LavaField => "lava field",
            ColumnarBasalt => "columnar basalt", AshPlain => "ash plain", Fumarole => "fumaroles", ThermalGround => "thermal ground",
            Dune => "dunes", SandSea => "sand sea", Yardang => "yardangs", DesertPavement => "desert pavement", Playa => "playa",
            Floodplain => "floodplain", Levee => "levee", Backswamp => "backswamp", Oxbow => "oxbow", Terrace => "river terrace",
            Braided => "braided channel", Fan => "alluvial fan", Delta => "delta", Estuary => "estuary",
            Waterfall => "waterfall", Rapids => "rapids", Gorge => "gorge",
            SeaCliff => "sea cliff", Stack => "sea stack", Beach => "beach", Spit => "spit", Bar => "bar",
            BarrierIsland => "barrier island", Lagoon => "lagoon", CoastalDune => "coastal dunes", TidalFlat => "tidal flat",
            SaltMarsh => "salt marsh", Mangrove => "mangrove", RaisedBeach => "raised beach",
            Bog => "bog", Fen => "fen", Marsh => "marsh", ReedBed => "reed bed",
            ImpactCrater => "impact crater", ImpactRim => "crater rim", Ejecta => "ejecta blanket", CentralPeak => "central peak",
            FaultScarp => "fault scarp", FaultTrough => "fault trough", SagPond => "sag pond", PressureRidge => "pressure ridge",
            LandslideScar => "landslide scar", LandslideDebris => "landslide debris", RockAvalanche => "rock avalanche debris",
            BurnScar => "burned woods", Maar => "maar", FissureVents => "fissure vents", CollapseScar => "collapse scar",
            Lahar => "lahar deposits", Traps => "basalt traps", SaltSink => "salt sinkholes", ThawLake => "thaw lakes",
            Crevasses => "crevasse field", AvalanchePath => "avalanche path", Scabland => "scabland",
            AbandonedChannel => "abandoned channel",
        }
    }

    /// Bare rock shows here: no soil, no trees.
    pub fn is_bare_rock(self) -> bool {
        use Landform::*;
        matches!(self, Cliff | Scree | Outcrop | Tor | SlotCanyon | Badland | Hoodoo | Pavement | TowerKarst | KarstGorge
            | Arete | Erratic | Crater | CinderCone | LavaField | ColumnarBasalt | SeaCliff | Stack | Gorge
            | ImpactRim | CentralPeak | FaultScarp | LandslideScar | RockAvalanche | CollapseScar | FissureVents | Scabland)
    }

    /// Standing or saturated water is usual here.
    pub fn is_wet(self) -> bool {
        use Landform::*;
        matches!(self, Tarn | Kettle | Cenote | Oxbow | Backswamp | Estuary | Lagoon | TidalFlat | SaltMarsh | Mangrove
            | Bog | Fen | Marsh | ReedBed | Playa | SagPond | ThawLake | Maar)
    }

    /// Ground that can kill the unwary: it gives way, slides, swallows or breaks.
    pub fn is_dangerous(self) -> bool {
        use Landform::*;
        matches!(self, LandslideScar | RockAvalanche | Crevasses | AvalanchePath | SaltSink | Fumarole | Scree
            | Cliff | SeaCliff | FaultScarp | CollapseScar | Lahar)
    }

    pub fn from_u8(v: u8) -> Landform { Landform::ALL.get(v as usize).copied().unwrap_or(Landform::None) }
}

/// The ages a place belongs to. The chronicle (the history simulation) is the last; everything
/// before it is "the ages before" (ROADMAP Stage 4), a prehistory the land keeps.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, PartialOrd, Ord)]
pub enum Age {
    /// Made by rock, water, ice, wind or fire alone.
    Natural,
    /// The first hunters: shelters, painted caves, kill sites, middens.
    Hunters,
    /// Herders and the first farmers: long barrows, enclosures, flint mines, lynchets.
    Farmers,
    /// The builders of stone: circles, standing stones, henges, dolmens, rock art.
    Builders,
    /// The first metal: round barrows, hill forts, crannogs, hoards, copper workings.
    Metal,
    /// The last age before the written history: towers, quarries, mills, drowned villages.
    LastAge,
    /// The chronicle's own: ruins, battlefields, bone fields, lairs, tombs, old mines.
    Chronicle,
}

/// What a place is. Natural wonders first, then the ages before in order, then the chronicle's
/// (which the embark already makes from the history: listed here so one layer holds them all).
#[repr(u8)]
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum PlaceKind {
    // Natural wonders.
    CaveMouth,
    Spring,
    Waterfall,
    HotSpring,
    Geyser,
    MudPot,
    Gorge,
    NaturalArch,
    SeaStack,
    Erratic,
    BoulderField,
    SaltSpring,
    TarSeep,
    PetrifiedForest,
    FossilBed,
    AmberShore,
    MeteorCrater,
    Oxbow,
    Tarn,
    Tor,
    Pavement,
    Blowhole,
    GiantTree,
    CraterLake,
    // The ages before.
    RockShelter,
    PaintedCave,
    KillSite,
    BoneBed,
    ShellMidden,
    FlintMine,
    LongBarrow,
    RoundBarrows,
    StoneCircle,
    StandingStone,
    Dolmen,
    RockArt,
    Enclosure,
    Henge,
    HillFort,
    Crannog,
    BogHoard,
    BogBody,
    CopperWorking,
    DesertKite,
    Ridgeway,
    Causeway,
    Ford,
    Lynchets,
    ClearanceCairns,
    Watchtower,
    Quarry,
    MillRuin,
    DrownedVillage,
    // The chronicle's.
    Ruin,
    Battlefield,
    BoneField,
    Lair,
    Tomb,
    OldMine,
    // Upheaval (ROADMAP Stage 7), appended: the natural places of a violent land.
    ImpactStructure,
    Landslide,
    LandslideLake,
    FaultScarp,
    SagPond,
    Maar,
    GasHollow,
    CollapsedFlank,
    Sinkhole,
    DryFalls,
    Quicksand,
    CrevasseField,
    TarPit,
    CrownHoles,
    /// Where something burst in the sky in the chronicle's years (`history::space`).
    Skyburn,
}

impl PlaceKind {
    pub fn age(self) -> Age {
        use PlaceKind::*;
        match self {
            CaveMouth | Spring | Waterfall | HotSpring | Geyser | MudPot | Gorge | NaturalArch | SeaStack | Erratic
            | BoulderField | SaltSpring | TarSeep | PetrifiedForest | FossilBed | AmberShore | MeteorCrater | Oxbow
            | Tarn | Tor | Pavement | Blowhole | GiantTree | CraterLake => Age::Natural,
            RockShelter | PaintedCave | KillSite | BoneBed | ShellMidden => Age::Hunters,
            FlintMine | LongBarrow | Enclosure | Lynchets | ClearanceCairns => Age::Farmers,
            StoneCircle | StandingStone | Dolmen | RockArt | Henge | Ridgeway | Causeway | Ford => Age::Builders,
            RoundBarrows | HillFort | Crannog | BogHoard | BogBody | CopperWorking | DesertKite => Age::Metal,
            Watchtower | Quarry | MillRuin | DrownedVillage => Age::LastAge,
            Ruin | Battlefield | BoneField | Lair | Tomb | OldMine => Age::Chronicle,
            ImpactStructure | Landslide | LandslideLake | FaultScarp | SagPond | Maar | GasHollow | CollapsedFlank
            | Sinkhole | DryFalls | Quicksand | CrevasseField | TarPit | CrownHoles => Age::Natural,
            Skyburn => Age::Chronicle,
        }
    }

    /// "a cave mouth", "a long barrow".
    pub fn word(self) -> &'static str {
        use PlaceKind::*;
        match self {
            CaveMouth => "a cave mouth", Spring => "a spring", Waterfall => "a waterfall", HotSpring => "a hot spring",
            Geyser => "a geyser", MudPot => "mud pots", Gorge => "a gorge", NaturalArch => "a natural arch",
            SeaStack => "a sea stack", Erratic => "an erratic boulder", BoulderField => "a boulder field",
            SaltSpring => "a salt spring", TarSeep => "a tar seep", PetrifiedForest => "a petrified forest",
            FossilBed => "a fossil bed", AmberShore => "an amber shore", MeteorCrater => "a meteorite crater",
            Oxbow => "an oxbow lake", Tarn => "a tarn", Tor => "a tor", Pavement => "a limestone pavement",
            Blowhole => "a blowhole", GiantTree => "a giant tree", CraterLake => "a crater lake",
            RockShelter => "a rock shelter", PaintedCave => "a painted cave", KillSite => "an old kill site",
            BoneBed => "a bone bed", ShellMidden => "a shell midden", FlintMine => "an old flint mine",
            LongBarrow => "a long barrow", RoundBarrows => "a barrow cemetery", StoneCircle => "a stone circle",
            StandingStone => "a standing stone", Dolmen => "a dolmen", RockArt => "carved rocks",
            Enclosure => "an ancient enclosure", Henge => "a henge", HillFort => "a hill fort", Crannog => "a crannog",
            BogHoard => "a bog hoard", BogBody => "a bog body", CopperWorking => "old copper workings",
            DesertKite => "a desert kite", Ridgeway => "an old ridgeway", Causeway => "an old causeway",
            Ford => "an old ford", Lynchets => "ancient field terraces", ClearanceCairns => "clearance cairns",
            Watchtower => "a ruined watchtower", Quarry => "an old quarry", MillRuin => "a ruined mill",
            DrownedVillage => "a drowned village",
            Ruin => "a ruin", Battlefield => "a battlefield", BoneField => "a bone field", Lair => "a lair",
            Tomb => "a tomb", OldMine => "an old mine",
            ImpactStructure => "an impact crater", Landslide => "a landslide", LandslideLake => "a landslide lake",
            FaultScarp => "a fault scarp", SagPond => "a sag pond", Maar => "a maar", GasHollow => "a gas hollow",
            CollapsedFlank => "a volcano's fallen flank", Sinkhole => "a sinkhole", DryFalls => "a dry falls",
            Quicksand => "quicksand", CrevasseField => "a crevasse field", TarPit => "a tar pit",
            CrownHoles => "crown holes over old workings", Skyburn => "a place burned from the sky",
        }
    }

    /// A place that can kill the unwary (the hover and the place's words say how).
    pub fn is_dangerous(self) -> bool {
        use PlaceKind::*;
        matches!(self, GasHollow | Sinkhole | Quicksand | CrevasseField | TarPit | CrownHoles | Landslide | MudPot | Geyser | Blowhole)
    }

    /// Opens into the rock: the embark carves a cave or a working for it (`Place::cave`).
    pub fn is_underground(self) -> bool {
        use PlaceKind::*;
        matches!(self, CaveMouth | PaintedCave | RockShelter | FlintMine | CopperWorking | Quarry | Lair | Tomb | OldMine)
    }
}

/// Somewhere worth walking to. Named, caused, described; drawn by the region, carved by the
/// embark, listed by the gazetteer.
#[derive(Clone, Debug)]
pub struct Place {
    pub kind: PlaceKind,
    /// "the Hollow Hill" (may be empty while unnamed).
    pub name: String,
    /// "where the first farmers laid their dead".
    pub epithet: String,
    /// One line: why it is here ("limestone drained by the Aln's valley").
    pub cause: String,
    /// One to three sentences.
    pub description: String,
    /// What it holds, or what digging finds.
    pub contents: Vec<String>,
    /// Position in absolute region cells (fractional).
    pub x: f64,
    pub y: f64,
    /// Footprint radius in metres (0 for a point).
    pub radius_m: f32,
    /// Cells of its footprint in absolute region cells (empty for a point).
    pub cells: Vec<(i64, i64)>,
    /// Its floor's height (m above the sea) where it is not the ground's: a cave's chamber, a
    /// falls' pool, a drowned village's street.
    pub z_m: Option<f32>,
    /// The cave it opens into (an index into `ZoomRegion::caves`), if any.
    pub cave: Option<u32>,
    /// For thinning labels: 0 the greatest of its kind in the world, 255 a local thing.
    pub rank: u8,
    /// The world tile that planned it.
    pub tile: (usize, usize),
}

impl Place {
    pub fn point(kind: PlaceKind, x: f64, y: f64, tile: (usize, usize)) -> Place {
        Place { kind, name: String::new(), epithet: String::new(), cause: String::new(), description: String::new(),
            contents: Vec::new(), x, y, radius_m: 0.0, cells: Vec::new(), z_m: None, cave: None, rank: 200, tile }
    }

    /// "the Hollow Hill, a long barrow" or "a long barrow" while unnamed.
    pub fn title(&self) -> String {
        if self.name.is_empty() { self.kind.word().to_string() } else { format!("{}, {}", self.name, self.kind.word()) }
    }
}

/// What kind of cave a graph describes.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum CaveKind { Karst, LavaTube, SeaCave, RockShelter, Talus, Ice, Mine }

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum CaveNodeKind { Mouth, Sink, Resurgence, Shaft, Passage, Chamber, Lake, Skylight }

/// A point of a cave in absolute region cells and metres above the sea.
#[derive(Clone, Debug)]
pub struct CaveNode {
    pub x: f64,
    pub y: f64,
    pub z_m: f32,
    pub kind: CaveNodeKind,
    pub width_m: f32,
    pub height_m: f32,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum PassageKind {
    /// A stream's canyon above the water table: tall and narrow.
    Vadose,
    /// Dissolved full of water, below the table: round.
    Phreatic,
    /// A lava tube.
    Tube,
    /// Roof fall: a jumble of blocks.
    Breakdown,
    /// Vertical, on a joint.
    Shaft,
    /// Cut by people.
    Adit,
}

#[derive(Clone, Debug)]
pub struct CavePassage {
    pub a: u32,
    pub b: u32,
    pub kind: PassageKind,
    pub width_m: f32,
    pub height_m: f32,
    /// Water runs along it.
    pub stream: bool,
}

/// A cave as a graph (ROADMAP Stage 5): planned per world tile, carved by every embark it
/// crosses, drawn by the region at its mouths.
#[derive(Clone, Debug)]
pub struct CaveGraph {
    pub kind: CaveKind,
    pub name: String,
    pub nodes: Vec<CaveNode>,
    pub passages: Vec<CavePassage>,
    pub tile: (usize, usize),
}

/// The rock kinds that water dissolves (karst).
pub fn is_carbonate(r: RockType) -> bool { matches!(r, RockType::Limestone) }
