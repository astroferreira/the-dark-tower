//! Coverage: every kind the simulation defines, mapped to the sprite routine that draws it.
//!
//! One exhaustive `match` per simulated enum, with no wildcard arm: a variant added to the
//! simulation without a drawing does not compile until it is given one here (and drawn). The
//! tables are printed by `--coverage` and checked by `tests/sprites.rs`; every routine named is
//! one that exists in `src/tiles` and is exercised by `--inventory` or `--sprite-sheet`.

use crate::colony::creatures::CreatureKind;
use crate::colony::delve::RoomKind;
use crate::colony::dreams::LifeDream;
use crate::colony::liaison::Want;
use crate::colony::mind::{Break, Feel};
use crate::colony::mood::MoodKind;
use crate::colony::needs::Need;
use crate::colony::projects::ProjectKind;
use crate::colony::society::Mandate;
use crate::colony::talk::Topic;
use crate::colony::visitors::VisitKind;
use crate::colony::{Dream, ItemKind, Job, MarkKind, StoneKind, Stuff};
use crate::history::objects::monuments::MonumentType;
use crate::local::places::PlaceKind;
use crate::local::{Plant, Shape, TreeKind};
use super::status_ink::Emblem;

pub fn creature(k: CreatureKind) -> &'static str {
    match k {
        CreatureKind::Beast => "beasts::of_monster (its generated body) via local_ink::draw_creature",
        CreatureKind::Raider => "folk::raider (race, band, helm, arms, shield)",
        CreatureKind::Besieger => "folk::raider + fx_ink::draw_siege tents",
        CreatureKind::Wolf => "beasts::of_name (wolf, the risen dead, werebeast)",
        CreatureKind::Game => "beasts::of_name (species body plan)",
        CreatureKind::Trader => "folk::trader + a laden mule",
        CreatureKind::Pet => "beasts::of_name + collar at beasts::neck_point",
        CreatureKind::CaveHunter => "beasts::of_name (cave hunter)",
        CreatureKind::CaveLife => "beasts::of_name (cave life)",
    }
}

pub fn project(k: ProjectKind) -> &'static str {
    use ProjectKind as P;
    match k {
        P::Woodpile => "camp_ink::draw_works woodpile (log ends that run down)",
        P::DryingRack => "camp_ink::draw_works drying rack",
        P::SecondHut => "roof (local_ink) + camp_ink chimney",
        P::Palisade | P::Mending => "camp_ink::draw_palisade stakes / stone courses",
        P::Windbreak => "camp_ink::draw_works dry-stone wall",
        P::Smokehouse => "camp_ink::draw_works smokehouse with smoke and fish",
        P::Woodshed => "camp_ink::draw_works lean-to roof over the woodpile",
        P::Lookout => "tower (local_ink) + camp_ink pennant",
        P::Fence => "camp_ink fence",
        P::Storehouse => "roof + signboard (sack) + casks",
        P::Workshop => "roof + signboard (hammer) + anvil and bench",
        P::Field => "camp_ink fence + camp_ink::crops by season",
        P::Jetty => "camp_ink::draw_works jetty",
        P::Well => "camp_ink::draw_works well",
        P::DugHall => "furniture via camp_ink::draw_hill_hall (hearth, trestle)",
        P::Cellar => "furniture::cellar_stores",
        P::Mine | P::DeepShaft => "camp_ink::draw_headframe + the delve's stair",
        P::Temple => "roof + bell-cote + signboard (sun)",
        P::Lining => "camp_ink::draw_lining",
        P::Still => "camp_ink::draw_works still",
        P::Traps => "camp_ink::draw_cage at each gate",
        P::LordsHall => "roof + banners in the lord's people's colours + signboard (crown)",
        P::CaveFarm => "furniture::fungus_bed",
        P::Tavern => "roof + chimney + signboard (tankard)",
        P::Pen => "camp_ink fence + the penned beasts (beasts::draw)",
        P::GuildHall => "roof + guild banner + signboard (crossed tools)",
        P::Kitchen => "roof + two chimneys + signboard (pot)",
        P::Library => "roof + signboard (book)",
        P::Bedrooms => "furniture::bed / pallet, cradle, kept works",
        P::GreatHall => "furniture::long_table",
        P::Tombs => "furniture::coffin",
        P::Moat | P::Drawbridges => "the ditch (local_ink) + planked drawbridges",
        P::Workshops => "furniture::bench",
        P::Hatch => "furniture::hatch",
        P::MasonShop => "furniture::mason",
        P::CarpenterShop => "furniture::carpenter",
        P::Smelter => "furniture::smelter",
        P::Forge => "furniture::forge",
        P::Kiln => "furniture::kiln",
        P::StoneCut => "the cut gallery (render_level_ink dug rock)",
    }
}

pub fn room(k: RoomKind) -> &'static str {
    match k {
        RoomKind::Hall => "camp_ink::draw_hill_hall", RoomKind::Cellar => "furniture::cellar_stores", RoomKind::Bedroom => "furniture::bed / pallet",
        RoomKind::GreatHall => "furniture::long_table", RoomKind::Corridor => "the passage itself (render_level_ink)", RoomKind::Tomb => "furniture::coffin",
        RoomKind::Workshop => "furniture::bench", RoomKind::Farm => "furniture::fungus_bed", RoomKind::Mason => "furniture::mason",
        RoomKind::Carpenter => "furniture::carpenter", RoomKind::Smelter => "furniture::smelter", RoomKind::Forge => "furniture::forge", RoomKind::Kiln => "furniture::kiln",
    }
}

pub fn mark(k: MarkKind) -> &'static str {
    match k {
        MarkKind::Grave => "camp_ink::draw_grave", MarkKind::Stone => "camp_ink::draw_stone_mark (by title)", MarkKind::Scorch => "camp_ink::draw_scorch",
        MarkKind::Cage => "camp_ink::draw_cage (with its catch)", MarkKind::Cairn | MarkKind::Bench | MarkKind::Carving => "camp_ink::draw_haunt",
    }
}

pub fn stone(k: StoneKind) -> &'static str {
    match k { StoneKind::Hall => "camp_ink::draw_hall_stone", StoneKind::Grove => "camp_ink::draw_standing_stone + ring", StoneKind::Shrine => "ring round the standing stone" }
}

pub fn stuff(k: Stuff) -> &'static str {
    match k {
        Stuff::Berries | Stuff::Fish | Stuff::Meat | Stuff::Grain | Stuff::Fungus | Stuff::Provisions | Stuff::Timber | Stuff::Stone => "glyphs::Glyph::of_stuff (heaps, carried, on the ground)",
    }
}

pub fn item(k: ItemKind) -> &'static str { match k { ItemKind::Log | ItemKind::Food | ItemKind::Stone => "glyphs (via the load's Stuff)" } }

pub fn job(j: Job) -> &'static str {
    match j {
        Job::Idle => "the figure alone", Job::Eat => "status_ink Meal bubble", Job::Sleep => "a z by the head",
        Job::Forage(_) => "glyph Berries in hand", Job::Fish(_) => "a rod with a fish", Job::Fell(_) => "glyph Axe in hand", Job::Haul(_) => "the load's glyph carried",
        Job::Build | Job::Craft => "glyph Mace (hammer) in hand", Job::Wander(_) => "the need's bubble (status_ink)", Job::Quarry(_) | Job::Dig(..) => "glyph Tool (pick) in hand",
        Job::Hunt(_) => "glyph Spear in hand",
    }
}

pub fn need(n: Need) -> Emblem {
    match n {
        Need::Socialize | Need::Friends | Need::Family => Emblem::Talk, Need::Pray => Emblem::Pray, Need::TakeItEasy => Emblem::Rest, Need::SeeAnimal => Emblem::Watch,
        Need::AdmireArt => Emblem::Admire, Need::Wander => Emblem::Walk, Need::Excitement => Emblem::Thrill, Need::HelpSomebody => Emblem::Help, Need::Learn => Emblem::Learn,
        Need::ThinkAbstractly => Emblem::Think, Need::MakeMerry => Emblem::Merry, Need::Tradition => Emblem::Tale, Need::Martial => Emblem::Martial,
        Need::Craft | Need::BeCreative => Emblem::Whittle, Need::StayOccupied => Emblem::Busy, Need::Drink => Emblem::Drink, Need::GoodMeal => Emblem::Meal,
        Need::Romance => Emblem::Love, Need::Remember => Emblem::Grief, Need::Acquire => Emblem::Acquire,
    }
}

pub fn brk(b: Break) -> Emblem { match b { Break::Tantrum => Emblem::Tantrum, Break::Despair => Emblem::Despair, Break::Wandering => Emblem::Lost } }

pub fn mood(m: MoodKind) -> Emblem {
    match m { MoodKind::Fey => Emblem::Fey, MoodKind::Secretive => Emblem::Secretive, MoodKind::Possessed => Emblem::Possessed, MoodKind::Macabre => Emblem::Macabre, MoodKind::Fell => Emblem::Fell }
}

pub fn dream(d: Dream) -> Emblem { match d { Dream::Hut | Dream::Plenty | Dream::Rest | Dream::Watch => Emblem::Dreaming } }

pub fn life(d: LifeDream) -> Emblem { super::status_ink::life_emblem(d) }

pub fn mandate(m: Mandate) -> &'static str {
    match m {
        Mandate::Works => "camp_ink::draw_mandate + glyph Mace", Mandate::SpareTrees => "camp_ink::draw_mandate + glyph Herbs", Mandate::Watch => "camp_ink::draw_mandate + glyph Spear",
        Mandate::Songs => "camp_ink::draw_mandate + glyph Pipes", Mandate::NoIdleHands => "camp_ink::draw_mandate + glyph Tool", Mandate::Feasts => "camp_ink::draw_mandate + glyph Meat",
    }
}

pub fn visit(v: &VisitKind) -> &'static str {
    match v {
        VisitKind::Hunter { .. } => "status_ink::guest_marks: feathered cap and bow", VisitKind::Bard => "status_ink::guest_marks: wide hat and lute or book",
        VisitKind::Seeker { .. } => "status_ink::guest_marks: hood and lantern", VisitKind::Sellsword => "status_ink::guest_marks: helm and sword",
    }
}

pub fn threat(t: crate::colony::arc::ThreatKind) -> &'static str {
    use crate::colony::arc::ThreatKind as T;
    match t {
        T::Beast | T::Deep => "beasts::of_monster", T::Shadow | T::Warband | T::Outlaws => "folk::raider by kind", T::Envoy => "vignette (envoy roundel) + the tribute stone",
    }
}

pub fn place(p: PlaceKind) -> &'static str {
    match p {
        PlaceKind::Cave => "furniture::cave_end", PlaceKind::Lair => "furniture::lair", PlaceKind::Tomb => "furniture::coffin", PlaceKind::OldMine => "furniture::ore_cart",
        PlaceKind::Cavern => "the cavern (render_level_ink) + its life", PlaceKind::Halls => "furniture::halls_end",
    }
}

pub fn topic(t: &Topic) -> Emblem {
    match t { Topic::Memory { grief: true, .. } => Emblem::Grief, Topic::Memory { .. } => Emblem::TalkMemory, Topic::Home { .. } => Emblem::TalkHome, Topic::Agree { .. } => Emblem::TalkAgree, Topic::Argue { .. } => Emblem::Argue, Topic::Small => Emblem::Talk }
}

pub fn want(w: Want) -> super::glyphs::Glyph {
    use super::glyphs::Glyph as G;
    match w { Want::Herbs => G::Herbs, Want::Salt => G::Barrel, Want::SeedGrain => G::Grain, Want::IronTools => G::Tool, Want::Cloth => G::Cloth, Want::Ore => G::Ore, Want::Charcoal => G::Charcoal }
}

/// The sign of every feeling a settler can have (shown by the thought's line on their sheet).
pub fn feel(f: &Feel) -> Emblem {
    match f {
        Feel::Death { .. } | Feel::Remembered { .. } => Emblem::Grief,
        Feel::ColdNight => Emblem::Cold,
        Feel::SleptWarm | Feel::Idle => Emblem::Rest,
        Feel::Hungry => Emblem::Hunger,
        Feel::Rationed => Emblem::HalfRation,
        Feel::Ill => Emblem::Sick,
        Feel::Wounded { .. } | Feel::Struck => Emblem::Hurt,
        Feel::Saved { .. } | Feel::SavedBy { .. } | Feel::Reconciled { .. } => Emblem::Help,
        Feel::RaidNight | Feel::Breach { .. } | Feel::TheDeep { .. } | Feel::SawFall { .. } => Emblem::Fear,
        Feel::Quarrel { .. } | Feel::Envy { .. } | Feel::Punished { .. } | Feel::Torn { .. } => Emblem::Anger,
        Feel::Friend { .. } => Emblem::Talk,
        Feel::Built { .. } | Feel::Made { .. } | Feel::Found { .. } | Feel::Slew { .. } | Feel::FelledTree | Feel::LikedWork { .. } => Emblem::Pride,
        Feel::Favoured | Feel::Festival { .. } | Feel::OwnRoom { .. } => Emblem::Joy,
        Feel::Mandate { .. } => Emblem::Grievance,
        Feel::Lonely => Emblem::Lonely,
        Feel::NowhereToPray => Emblem::NoShrine,
        Feel::Ragged => Emblem::Rags,
        Feel::NeedUnmet { .. } => Emblem::Unmet,
        Feel::Prayed => Emblem::Pray,
        Feel::Busy => Emblem::Busy,
        Feel::KilledLiked { .. } => Emblem::Despair,
        Feel::SawLiked { .. } | Feel::Pet { .. } => Emblem::Watch,
        // Ill news is heard as gloom, a hated people's caravan with anger.
        Feel::News { good: false, .. } => Emblem::BadNews,
        Feel::Caravan { hated: true, .. } => Emblem::Anger,
        Feel::News { .. } | Feel::Caravan { .. } => Emblem::TalkMemory,
        Feel::Admired { .. } => Emblem::Admire,
        Feel::Performed { .. } | Feel::Heard { .. } => Emblem::Merry,
        Feel::Drank => Emblem::Drink,
        Feel::Thirsty => Emblem::Thirst,
        Feel::AteWell { .. } => Emblem::Meal,
        Feel::Dreamt { .. } => Emblem::Dreaming,
        Feel::Acquired { .. } => Emblem::Acquire,
    }
}

pub fn monument(m: &MonumentType) -> &'static str {
    use MonumentType as M;
    match m {
        M::Statue | M::Obelisk | M::Tomb | M::Pyramid | M::Temple | M::Castle | M::Wall | M::Tower | M::Bridge | M::Fountain | M::Memorial | M::Trophy | M::Altar => "world_ink::monument (by type)",
    }
}

pub fn shape(s: Shape) -> &'static str {
    match s { Shape::Empty => "open air / water (local_ink)", Shape::Floor => "ground wash (local_ink)", Shape::Ramp => "hachures (local_ink)", Shape::Wall => "walls and rock (local_ink, cut_rock)", Shape::Stair => "local_ink::stair_mark / profile" }
}

pub fn plant(p: Plant) -> &'static str {
    match p { Plant::None => "bare ground", Plant::Grass => "tufts", Plant::Shrub => "shrub", Plant::Tree(t) => tree(t), Plant::Crop(_) => "furrows + camp_ink::crops" }
}

pub fn tree(t: TreeKind) -> &'static str {
    match t { TreeKind::Broadleaf | TreeKind::Jungle => "scalloped crown", TreeKind::Conifer => "pointed fir", TreeKind::Palm | TreeKind::Acacia => "crown (local_ink::crown)", TreeKind::Dead => "bare dead tree", TreeKind::Fungus => "fungus stalk and cap" }
}

/// The table: (simulated type, kinds, drawn by). Lists every value of each type.
pub fn table() -> Vec<(&'static str, String, String)> {
    let mut t: Vec<(&'static str, String, String)> = Vec::new();
    let mut add = |ty: &'static str, k: String, d: String| t.push((ty, k, d));
    for k in [CreatureKind::Beast, CreatureKind::Raider, CreatureKind::Wolf, CreatureKind::Game, CreatureKind::Trader, CreatureKind::Pet, CreatureKind::Besieger, CreatureKind::CaveHunter, CreatureKind::CaveLife] { add("CreatureKind", format!("{:?}", k), creature(k).into()); }
    use ProjectKind as P;
    for k in [P::Woodpile, P::DryingRack, P::SecondHut, P::Palisade, P::Windbreak, P::Smokehouse, P::Woodshed, P::Lookout, P::Fence, P::Mending, P::Storehouse, P::Workshop, P::Field, P::Jetty, P::Well, P::DugHall, P::Cellar, P::Mine, P::Temple, P::Lining, P::Still, P::Traps, P::LordsHall, P::CaveFarm, P::Tavern, P::Pen, P::DeepShaft, P::GuildHall, P::Kitchen, P::Library, P::Bedrooms, P::GreatHall, P::Tombs, P::Moat, P::Workshops, P::Hatch, P::Drawbridges, P::MasonShop, P::CarpenterShop, P::Smelter, P::Forge, P::Kiln, P::StoneCut] { add("ProjectKind", format!("{:?}", k), project(k).into()); }
    for k in [RoomKind::Hall, RoomKind::Cellar, RoomKind::Bedroom, RoomKind::GreatHall, RoomKind::Corridor, RoomKind::Tomb, RoomKind::Workshop, RoomKind::Farm, RoomKind::Mason, RoomKind::Carpenter, RoomKind::Smelter, RoomKind::Forge, RoomKind::Kiln] { add("RoomKind", format!("{:?}", k), room(k).into()); }
    for k in [MarkKind::Grave, MarkKind::Stone, MarkKind::Scorch, MarkKind::Cage, MarkKind::Cairn, MarkKind::Bench, MarkKind::Carving] { add("MarkKind", format!("{:?}", k), mark(k).into()); }
    for k in [StoneKind::Hall, StoneKind::Grove, StoneKind::Shrine] { add("StoneKind", format!("{:?}", k), stone(k).into()); }
    for k in Stuff::ALL { add("Stuff", format!("{:?}", k), stuff(k).into()); }
    for k in [ItemKind::Log, ItemKind::Food, ItemKind::Stone] { add("ItemKind", format!("{:?}", k), item(k).into()); }
    for k in [Job::Idle, Job::Eat, Job::Sleep, Job::Forage((0, 0)), Job::Fish((0, 0)), Job::Fell((0, 0)), Job::Haul(0), Job::Build, Job::Wander((0, 0)), Job::Quarry((0, 0)), Job::Dig((0, 0), 0), Job::Hunt(0), Job::Craft] { add("Job", format!("{:?}", k).split('(').next().unwrap_or("").to_string(), job(k).into()); }
    for k in crate::colony::needs::ALL.iter().copied().chain([Need::Remember]) { add("Need", format!("{:?}", k), format!("status_ink::{:?} bubble", need(k))); }
    for k in [Break::Tantrum, Break::Despair, Break::Wandering] { add("Break", format!("{:?}", k), format!("status_ink::{:?} bubble", brk(k))); }
    for k in [MoodKind::Fey, MoodKind::Secretive, MoodKind::Possessed, MoodKind::Macabre, MoodKind::Fell] { add("MoodKind", format!("{:?}", k), format!("status_ink::{:?} bubble", mood(k))); }
    for k in [Dream::Hut, Dream::Plenty, Dream::Rest, Dream::Watch] { add("Dream", format!("{:?}", k), format!("status_ink::{:?} bubble; dream card button", dream(k))); }
    for k in [LifeDream::Child, LifeDream::Masterwork, LifeDream::Slay, LifeDream::Book, LifeDream::Rule, LifeDream::Discover, LifeDream::Peace] { add("LifeDream", format!("{:?}", k), format!("status_ink::{:?} icon on the sheet", life(k))); }
    for k in [Mandate::Works, Mandate::SpareTrees, Mandate::Watch, Mandate::Songs, Mandate::NoIdleHands, Mandate::Feasts] { add("Mandate", format!("{:?}", k), mandate(k).into()); }
    for k in [VisitKind::Hunter { beast: String::new() }, VisitKind::Bard, VisitKind::Seeker { relic: String::new() }, VisitKind::Sellsword] { add("VisitKind", format!("{:?}", k).split([' ', '{']).next().unwrap_or("").to_string(), visit(&k).into()); }
    use crate::colony::arc::ThreatKind as T;
    for k in [T::Beast, T::Shadow, T::Warband, T::Outlaws, T::Envoy, T::Deep] { add("ThreatKind", format!("{:?}", k), threat(k).into()); }
    for k in [PlaceKind::Cave, PlaceKind::Lair, PlaceKind::Tomb, PlaceKind::OldMine, PlaceKind::Cavern, PlaceKind::Halls] { add("PlaceKind", format!("{:?}", k), place(k).into()); }
    for k in [Topic::Memory { what: String::new(), grief: true }, Topic::Memory { what: String::new(), grief: false }, Topic::Home { town: String::new() }, Topic::Agree { value: 0 }, Topic::Argue { value: 0 }, Topic::Small] { add("Topic", format!("{:?}", k).split([' ', '{']).next().unwrap_or("").to_string(), format!("status_ink::{:?} bubble", topic(&k))); }
    for k in [Want::Herbs, Want::Salt, Want::SeedGrain, Want::IronTools, Want::Cloth, Want::Ore, Want::Charcoal] { add("Want", format!("{:?}", k), format!("glyph {:?} on the Camp leaf", want(k))); }
    use crate::history::objects::monuments::MonumentType as M;
    for k in [M::Statue, M::Obelisk, M::Tomb, M::Pyramid, M::Temple, M::Castle, M::Wall, M::Tower, M::Bridge, M::Fountain, M::Memorial, M::Trophy, M::Altar] { add("MonumentType", format!("{:?}", k), monument(&k).into()); }
    for k in [Shape::Empty, Shape::Floor, Shape::Ramp, Shape::Wall, Shape::Stair] { add("Shape", format!("{:?}", k), shape(k).into()); }
    for k in [TreeKind::Broadleaf, TreeKind::Conifer, TreeKind::Jungle, TreeKind::Palm, TreeKind::Acacia, TreeKind::Dead, TreeKind::Fungus] { add("TreeKind", format!("{:?}", k), tree(k).into()); }
    for k in [Plant::None, Plant::Grass, Plant::Shrub, Plant::Crop(0)] { add("Plant", format!("{:?}", k), plant(k).into()); }
    for f in feels() { add("Feel", format!("{:?}", f).split([' ', '{']).next().unwrap_or("").to_string(), format!("status_ink::{:?} icon in the ledger", feel(&f))); }
    for g in super::glyphs::Glyph::ALL { add("Glyph", format!("{:?}", g), "glyphs::draw".into()); }
    t
}

/// One of every thought (`Feel`), with stand-in names, for the table and the thoughts page.
/// `tests/sprites.rs` checks there are as many as the enum has variants.
pub fn feels() -> Vec<Feel> {
    let n = || "Gaunauth".to_string();
    vec![
        Feel::Death { whom: n(), close: true }, Feel::ColdNight, Feel::SleptWarm, Feel::Hungry, Feel::Ill, Feel::Struck,
        Feel::Saved { whom: n() }, Feel::SavedBy { whom: n() }, Feel::RaidNight, Feel::Quarrel { with: n() }, Feel::Friend { with: n() },
        Feel::Built { what: "the palisade".into() }, Feel::LikedWork { material: "oak".into() }, Feel::FelledTree, Feel::Favoured,
        Feel::Envy { of: n() }, Feel::Lonely, Feel::Prayed, Feel::NowhereToPray, Feel::Idle, Feel::Busy,
        Feel::Breach { what: "the first cavern".into() }, Feel::Wounded { what: "a broken arm".into() }, Feel::Festival { what: "spring".into() },
        Feel::Punished { by: n() }, Feel::KilledLiked { what: "deer".into() }, Feel::SawLiked { what: "deer".into() },
        Feel::News { what: "a town fell".into(), good: false }, Feel::Mandate { what: "the watch".into() }, Feel::Reconciled { by: n() },
        Feel::Caravan { town: "Brolmdustoor".into(), hated: false, sold: true }, Feel::Made { what: "a bowl".into(), quality: 3 },
        Feel::Admired { what: "a statue".into() }, Feel::Performed { what: "a dance".into() }, Feel::Heard { what: "a song".into(), own: true },
        Feel::TheDeep { what: "the dead walk".into() }, Feel::Found { what: "the staff".into() }, Feel::Slew { what: "a beast".into() },
        Feel::SawFall { what: "a beast".into() }, Feel::Pet { name: "Tha".into() }, Feel::Drank, Feel::Thirsty,
        Feel::Torn { people: "the Git Clans".into() }, Feel::AteWell { dish: "a stew".into(), fine: true }, Feel::Ragged,
        Feel::Dreamt { what: "a child".into() }, Feel::Rationed, Feel::OwnRoom { value: 6 },
        Feel::NeedUnmet { what: "prayer".into(), level: 6, days: 9 }, Feel::Remembered { whom: n() }, Feel::Acquired { what: "a bowl".into() },
    ]
}

/// The variant's name, for counting distinct kinds.
pub fn feel_name(f: &Feel) -> String { format!("{:?}", f).split([' ', '{']).next().unwrap_or("").to_string() }
