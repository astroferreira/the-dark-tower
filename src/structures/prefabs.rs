//! Prefab building templates
//!
//! Pre-defined small structure templates for villages, buildings, and features.

use crate::zlevel::ZTile;
use super::types::Prefab;

/// Create a small house prefab (5x5)
pub fn small_house() -> Prefab {
    use ZTile::*;
    Prefab::new(
        "small_house",
        vec![
            vec![StoneWall, StoneWall, StoneWall, StoneWall, StoneWall],
            vec![StoneWall, WoodFloor, WoodFloor, WoodFloor, StoneWall],
            vec![StoneWall, WoodFloor, WoodFloor, WoodFloor, Door],
            vec![StoneWall, WoodFloor, WoodFloor, WoodFloor, StoneWall],
            vec![StoneWall, StoneWall, StoneWall, StoneWall, StoneWall],
        ],
        vec!["house", "village", "small"],
    )
}

/// Create a medium house prefab (7x6)
pub fn medium_house() -> Prefab {
    use ZTile::*;
    Prefab::new(
        "medium_house",
        vec![
            vec![StoneWall, StoneWall, StoneWall, StoneWall, StoneWall, StoneWall, StoneWall],
            vec![StoneWall, WoodFloor, WoodFloor, WoodFloor, WoodFloor, WoodFloor, StoneWall],
            vec![StoneWall, WoodFloor, WoodFloor, WoodFloor, WoodFloor, WoodFloor, StoneWall],
            vec![Door, WoodFloor, WoodFloor, WoodFloor, WoodFloor, WoodFloor, StoneWall],
            vec![StoneWall, WoodFloor, WoodFloor, WoodFloor, WoodFloor, WoodFloor, StoneWall],
            vec![StoneWall, StoneWall, StoneWall, StoneWall, StoneWall, StoneWall, StoneWall],
        ],
        vec!["house", "village", "medium"],
    )
}

/// Create a tavern/inn prefab (9x8)
pub fn tavern() -> Prefab {
    use ZTile::*;
    Prefab::new(
        "tavern",
        vec![
            vec![StoneWall, StoneWall, StoneWall, StoneWall, StoneWall, StoneWall, StoneWall, StoneWall, StoneWall],
            vec![StoneWall, WoodFloor, WoodFloor, WoodFloor, WoodFloor, WoodFloor, WoodFloor, WoodFloor, StoneWall],
            vec![StoneWall, WoodFloor, WoodFloor, WoodFloor, WoodFloor, WoodFloor, WoodFloor, WoodFloor, StoneWall],
            vec![StoneWall, WoodFloor, WoodFloor, WoodFloor, WoodFloor, WoodFloor, WoodFloor, WoodFloor, StoneWall],
            vec![StoneWall, WoodFloor, WoodFloor, WoodFloor, WoodFloor, WoodFloor, WoodFloor, WoodFloor, Door],
            vec![StoneWall, WoodFloor, WoodFloor, Column, WoodFloor, Column, WoodFloor, WoodFloor, StoneWall],
            vec![StoneWall, WoodFloor, WoodFloor, WoodFloor, WoodFloor, WoodFloor, WoodFloor, WoodFloor, StoneWall],
            vec![StoneWall, StoneWall, StoneWall, StoneWall, StoneWall, StoneWall, StoneWall, StoneWall, StoneWall],
        ],
        vec!["tavern", "city", "village", "large"],
    )
}

/// Create a small temple prefab (7x7)
pub fn small_temple() -> Prefab {
    use ZTile::*;
    Prefab::new(
        "small_temple",
        vec![
            vec![StoneWall, StoneWall, Column, StoneWall, Column, StoneWall, StoneWall],
            vec![StoneWall, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneWall],
            vec![Column, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneFloor, Column],
            vec![StoneWall, StoneFloor, StoneFloor, Altar, StoneFloor, StoneFloor, StoneWall],
            vec![Column, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneFloor, Column],
            vec![StoneWall, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneWall],
            vec![StoneWall, StoneWall, Column, Door, Column, StoneWall, StoneWall],
        ],
        vec!["temple", "city", "sacred"],
    )
}

/// Create a watchtower base prefab (5x5)
pub fn watchtower() -> Prefab {
    use ZTile::*;
    Prefab::new(
        "watchtower",
        vec![
            vec![StoneWall, StoneWall, StoneWall, StoneWall, StoneWall],
            vec![StoneWall, StoneFloor, StoneFloor, StoneFloor, StoneWall],
            vec![StoneWall, StoneFloor, StairsUp, StoneFloor, StoneWall],
            vec![StoneWall, StoneFloor, StoneFloor, StoneFloor, Door],
            vec![StoneWall, StoneWall, StoneWall, StoneWall, StoneWall],
        ],
        vec!["tower", "castle", "military"],
    )
}

/// Create a well/plaza center (3x3)
pub fn well() -> Prefab {
    use ZTile::*;
    Prefab::new(
        "well",
        vec![
            vec![CobblestoneFloor, CobblestoneFloor, CobblestoneFloor],
            vec![CobblestoneFloor, StoneWall, CobblestoneFloor],
            vec![CobblestoneFloor, CobblestoneFloor, CobblestoneFloor],
        ],
        vec!["well", "village", "plaza", "small"],
    )
}

/// Create a storage shed (4x4)
pub fn storage_shed() -> Prefab {
    use ZTile::*;
    Prefab::new(
        "storage_shed",
        vec![
            vec![WoodWall, WoodWall, WoodWall, WoodWall],
            vec![WoodWall, DirtFloor, Chest, WoodWall],
            vec![WoodWall, DirtFloor, DirtFloor, WoodWall],
            vec![WoodWall, Door, WoodWall, WoodWall],
        ],
        vec!["storage", "village", "small"],
    )
}

/// Create a market stall (3x4)
pub fn market_stall() -> Prefab {
    use ZTile::*;
    Prefab::new(
        "market_stall",
        vec![
            vec![WoodWall, WoodWall, WoodWall],
            vec![WoodFloor, WoodFloor, WoodFloor],
            vec![WoodFloor, Chest, WoodFloor],
            vec![CobblestoneFloor, CobblestoneFloor, CobblestoneFloor],
        ],
        vec!["market", "city", "small"],
    )
}

/// Create a guard house (6x5)
pub fn guard_house() -> Prefab {
    use ZTile::*;
    Prefab::new(
        "guard_house",
        vec![
            vec![StoneWall, StoneWall, StoneWall, StoneWall, StoneWall, StoneWall],
            vec![StoneWall, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneWall],
            vec![StoneWall, StoneFloor, StoneFloor, StoneFloor, StoneFloor, Window],
            vec![StoneWall, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneWall],
            vec![StoneWall, StoneWall, Door, StoneWall, StoneWall, StoneWall],
        ],
        vec!["guard", "city", "castle", "military"],
    )
}

/// Create a blacksmith forge (6x6)
pub fn blacksmith() -> Prefab {
    use ZTile::*;
    Prefab::new(
        "blacksmith",
        vec![
            vec![BrickWall, BrickWall, BrickWall, BrickWall, BrickWall, BrickWall],
            vec![BrickWall, StoneFloor, StoneFloor, StoneFloor, StoneFloor, BrickWall],
            vec![BrickWall, StoneFloor, StoneFloor, StoneFloor, StoneFloor, BrickWall],
            vec![BrickWall, StoneFloor, StoneFloor, StoneFloor, StoneFloor, Window],
            vec![BrickWall, StoneFloor, StoneFloor, StoneFloor, StoneFloor, BrickWall],
            vec![BrickWall, BrickWall, Door, BrickWall, BrickWall, BrickWall],
        ],
        vec!["blacksmith", "city", "craft"],
    )
}

/// Create a dungeon room (8x8)
pub fn dungeon_room() -> Prefab {
    use ZTile::*;
    Prefab::new(
        "dungeon_room",
        vec![
            vec![StoneWall, StoneWall, StoneWall, Door, Door, StoneWall, StoneWall, StoneWall],
            vec![StoneWall, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneWall],
            vec![StoneWall, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneWall],
            vec![Door, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneFloor, Door],
            vec![Door, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneFloor, Door],
            vec![StoneWall, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneWall],
            vec![StoneWall, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneFloor, StoneWall],
            vec![StoneWall, StoneWall, StoneWall, Door, Door, StoneWall, StoneWall, StoneWall],
        ],
        vec!["dungeon", "underground"],
    )
}

/// Create a treasure room (5x5)
pub fn treasure_room() -> Prefab {
    use ZTile::*;
    Prefab::new(
        "treasure_room",
        vec![
            vec![StoneWall, StoneWall, Door, StoneWall, StoneWall],
            vec![StoneWall, Chest, StoneFloor, Chest, StoneWall],
            vec![StoneWall, StoneFloor, Chest, StoneFloor, StoneWall],
            vec![StoneWall, Chest, StoneFloor, Chest, StoneWall],
            vec![StoneWall, StoneWall, StoneWall, StoneWall, StoneWall],
        ],
        vec!["dungeon", "treasure", "underground"],
    )
}

/// Create a mine entrance (6x5)
pub fn mine_entrance() -> Prefab {
    use ZTile::*;
    Prefab::new(
        "mine_entrance",
        vec![
            vec![StoneWall, MineSupport, MinedTunnel, MineSupport, StoneWall, StoneWall],
            vec![MineSupport, MinedTunnel, MinedTunnel, MinedTunnel, MineSupport, StoneWall],
            vec![MinedTunnel, MinedTunnel, StairsDown, MinedTunnel, MinedTunnel, MineSupport],
            vec![MineSupport, MinedTunnel, MinedTunnel, MinedTunnel, MineSupport, StoneWall],
            vec![StoneWall, MineSupport, MinedTunnel, MineSupport, StoneWall, StoneWall],
        ],
        vec!["mine", "cave", "entrance"],
    )
}

/// Create a mine chamber (7x7)
pub fn mine_chamber() -> Prefab {
    use ZTile::*;
    Prefab::new(
        "mine_chamber",
        vec![
            vec![StoneWall, MineSupport, MinedTunnel, MinedTunnel, MinedTunnel, MineSupport, StoneWall],
            vec![MineSupport, MinedRoom, MinedRoom, MinedRoom, MinedRoom, MinedRoom, MineSupport],
            vec![MinedTunnel, MinedRoom, MinedRoom, Torch, MinedRoom, MinedRoom, MinedTunnel],
            vec![MinedTunnel, MinedRoom, Torch, MinedRoom, Torch, MinedRoom, MinedTunnel],
            vec![MinedTunnel, MinedRoom, MinedRoom, Torch, MinedRoom, MinedRoom, MinedTunnel],
            vec![MineSupport, MinedRoom, MinedRoom, MinedRoom, MinedRoom, MinedRoom, MineSupport],
            vec![StoneWall, MineSupport, MinedTunnel, MinedTunnel, MinedTunnel, MineSupport, StoneWall],
        ],
        vec!["mine", "cave", "chamber"],
    )
}

/// Get all available prefabs
pub fn all_prefabs() -> Vec<Prefab> {
    vec![
        small_house(),
        medium_house(),
        tavern(),
        small_temple(),
        watchtower(),
        well(),
        storage_shed(),
        market_stall(),
        guard_house(),
        blacksmith(),
        dungeon_room(),
        treasure_room(),
        mine_entrance(),
        mine_chamber(),
    ]
}

/// Get prefabs filtered by tag
pub fn prefabs_by_tag(tag: &str) -> Vec<Prefab> {
    all_prefabs()
        .into_iter()
        .filter(|p| p.has_tag(tag))
        .collect()
}

/// Get a random prefab from a list
pub fn random_prefab(prefabs: &[Prefab], rng_value: usize) -> Option<&Prefab> {
    if prefabs.is_empty() {
        None
    } else {
        Some(&prefabs[rng_value % prefabs.len()])
    }
}
