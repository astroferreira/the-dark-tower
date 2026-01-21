//! Physical evidence generation
//!
//! Places physical evidence of historical events, monster territories,
//! and trade routes in the world.

use rand::Rng;
use rand_chacha::ChaCha8Rng;
use rand::SeedableRng;

use crate::tilemap::Tilemap;
use crate::zlevel::{Tilemap3D, ZTile};

use super::factions::FactionRegistry;
use super::timeline::{Timeline, HistoricalEvent, EventType};
use super::territories::TerritoryRegistry;
use super::monsters::MonsterRegistry;
use super::trade::TradeRegistry;
use super::types::*;

/// Whether to write historical evidence tiles to world zlevels.
/// Set to false to defer all evidence rendering to local map generation.
/// The metadata is still tracked in WorldHistory registries.
const WRITE_EVIDENCE_TILES: bool = false;

/// Generate all historical evidence in the world
pub fn generate_historical_evidence(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    factions: &FactionRegistry,
    timeline: &Timeline,
    territories: &TerritoryRegistry,
    monsters: &MonsterRegistry,
    trade: &TradeRegistry,
    seed: u64,
) {
    // Skip tile placement if disabled - metadata is still tracked in registries
    if !WRITE_EVIDENCE_TILES {
        println!("  Skipping historical evidence tiles (metadata only)...");
        return;
    }

    let mut rng = ChaCha8Rng::seed_from_u64(seed.wrapping_add(0xE01DE1CE));
    let width = surface_z.width;
    let height = surface_z.height;

    println!("  Placing historical evidence...");

    // Place battlefield evidence
    let battle_events: Vec<_> = timeline.events.values()
        .filter(|e| matches!(e.event_type,
            EventType::Battle | EventType::Siege | EventType::Massacre
        ))
        .collect();

    for event in battle_events {
        if let Some((x, y)) = event.location {
            place_battlefield_evidence(
                zlevels,
                surface_z,
                x,
                y,
                event,
                width,
                height,
                &mut rng,
            );
        }
    }

    // Place graveyards near old settlements
    for settlement in territories.settlements.values() {
        if settlement.state != SettlementState::Thriving {
            place_graveyard(
                zlevels,
                surface_z,
                settlement.x,
                settlement.y,
                settlement.peak_population,
                settlement.age(),
                width,
                height,
                &mut rng,
            );
        }
    }

    // Place monument evidence
    let monument_events: Vec<_> = timeline.events.values()
        .filter(|e| matches!(e.event_type, EventType::MonumentBuilt))
        .collect();

    for event in monument_events {
        if let Some((x, y)) = event.location {
            place_monument(
                zlevels,
                surface_z,
                x,
                y,
                event,
                width,
                height,
                &mut rng,
            );
        }
    }

    // Place monster territory evidence
    for lair in monsters.lairs.values() {
        place_monster_evidence(
            zlevels,
            surface_z,
            lair.x,
            lair.y,
            lair.species.territory_evidence(),
            lair.territory.len() / 10,
            width,
            height,
            &mut rng,
        );
    }

    // Place trade route evidence
    for route in trade.routes.values() {
        place_route_evidence(
            zlevels,
            surface_z,
            &route.path,
            &route.waypoints,
            route.active,
            width,
            height,
            &mut rng,
        );
    }

    // Place faction boundary markers
    place_boundary_markers(
        zlevels,
        surface_z,
        &territories.territory_map,
        factions,
        width,
        height,
        &mut rng,
    );

    // Place resource site markers
    for site in &trade.resources {
        if site.depleted {
            place_depleted_resource(
                zlevels,
                surface_z,
                site.x,
                site.y,
                &mut rng,
            );
        }
    }

    // Place evidence at various Z-levels (underground, caves, peaks)
    place_multi_level_evidence(
        zlevels,
        surface_z,
        timeline,
        monsters,
        width,
        height,
        &mut rng,
    );
}

/// Place battlefield evidence (bone fields, rusted weapons, memorials)
fn place_battlefield_evidence(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    cx: usize,
    cy: usize,
    event: &HistoricalEvent,
    width: usize,
    height: usize,
    rng: &mut ChaCha8Rng,
) {
    let age = event.year.age();
    let casualties = event.casualties;

    // Radius based on casualties
    let radius = ((casualties as f32).sqrt() / 5.0).clamp(3.0, 15.0) as i32;

    // Density decreases with age
    let density = (1.0 - (age as f32 / 1000.0).min(0.9)).max(0.1);

    for dy in -radius..=radius {
        for dx in -radius..=radius {
            let dist_sq = dx * dx + dy * dy;
            if dist_sq > radius * radius {
                continue;
            }

            let nx = (cx as i32 + dx).rem_euclid(width as i32) as usize;
            let ny = (cy as i32 + dy).clamp(0, height as i32 - 1) as usize;

            let z = *surface_z.get(nx, ny);
            let current = *zlevels.get(nx, ny, z);

            // Skip non-surface tiles
            if current != ZTile::Surface {
                continue;
            }

            // Probability decreases with distance from center
            let dist_factor = 1.0 - (dist_sq as f32).sqrt() / radius as f32;
            let prob = (density * dist_factor * 0.3).clamp(0.0, 1.0);

            if rng.gen_bool(prob as f64) {
                // Pick evidence type
                let tile = if dist_sq < 4 && rng.gen_bool(0.3) {
                    // War memorial at center
                    ZTile::WarMemorial
                } else if rng.gen_bool(0.6) {
                    ZTile::BoneField
                } else {
                    ZTile::RustedWeapons
                };

                zlevels.set(nx, ny, z, tile);
            }
        }
    }
}

/// Place a graveyard near a settlement
fn place_graveyard(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    cx: usize,
    cy: usize,
    population: u32,
    age: i32,
    width: usize,
    height: usize,
    rng: &mut ChaCha8Rng,
) {
    // Place graveyard offset from settlement center
    let offset_x = rng.gen_range(-10..10);
    let offset_y = rng.gen_range(-10..10);

    let gx = (cx as i32 + offset_x).rem_euclid(width as i32) as usize;
    let gy = (cy as i32 + offset_y).clamp(0, height as i32 - 1) as usize;

    // Graveyard size based on population
    let size = ((population as f32).sqrt() / 10.0).clamp(2.0, 8.0) as i32;

    for dy in -size..=size {
        for dx in -size..=size {
            let nx = (gx as i32 + dx).rem_euclid(width as i32) as usize;
            let ny = (gy as i32 + dy).clamp(0, height as i32 - 1) as usize;

            let z = *surface_z.get(nx, ny);
            let current = *zlevels.get(nx, ny, z);

            if current != ZTile::Surface {
                continue;
            }

            if rng.gen_bool(0.4) {
                let tile = if dx == 0 && dy == 0 && population > 1000 {
                    if age > 500 {
                        ZTile::Mausoleum
                    } else {
                        ZTile::Tomb
                    }
                } else if population > 5000 && rng.gen_bool(0.1) {
                    ZTile::MassGrave
                } else {
                    ZTile::Gravestone
                };

                zlevels.set(nx, ny, z, tile);
            }
        }
    }
}

/// Place a monument
fn place_monument(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    cx: usize,
    cy: usize,
    event: &HistoricalEvent,
    width: usize,
    height: usize,
    rng: &mut ChaCha8Rng,
) {
    let z = *surface_z.get(cx, cy);
    let current = *zlevels.get(cx, cy, z);

    if current != ZTile::Surface {
        return;
    }

    // Pick monument type
    let tile = if event.name.contains("Tower") {
        ZTile::Obelisk
    } else if event.name.contains("Temple") || event.name.contains("Sacred") {
        ZTile::Shrine
    } else if rng.gen_bool(0.3) {
        ZTile::Statue
    } else {
        ZTile::Obelisk
    };

    zlevels.set(cx, cy, z, tile);

    // Add surrounding area
    for dy in -1i32..=1 {
        for dx in -1i32..=1 {
            if dx == 0 && dy == 0 {
                continue;
            }

            let nx = (cx as i32 + dx).rem_euclid(width as i32) as usize;
            let ny = (cy as i32 + dy).clamp(0, height as i32 - 1) as usize;

            let nz = *surface_z.get(nx, ny);
            let ncurrent = *zlevels.get(nx, ny, nz);

            if ncurrent == ZTile::Surface && rng.gen_bool(0.5) {
                zlevels.set(nx, ny, nz, ZTile::CobblestoneFloor);
            }
        }
    }
}

/// Place monster territory evidence
fn place_monster_evidence(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    cx: usize,
    cy: usize,
    evidence_type: &str,
    count: usize,
    width: usize,
    height: usize,
    rng: &mut ChaCha8Rng,
) {
    let tile = match evidence_type {
        "WebCluster" => ZTile::WebCluster,
        "BoneNest" => ZTile::BoneNest,
        "CharredGround" => ZTile::CharredGround,
        "AntMound" => ZTile::AntMound,
        "BeeHive" => ZTile::BeeHive,
        "ClawMarks" => ZTile::ClawMarks,
        "CursedGround" => ZTile::CursedGround,
        _ => ZTile::TerritoryMarking,
    };

    // Scatter evidence around the lair
    for _ in 0..count.max(3) {
        let dx = rng.gen_range(-15..15);
        let dy = rng.gen_range(-15..15);

        let nx = (cx as i32 + dx).rem_euclid(width as i32) as usize;
        let ny = (cy as i32 + dy).clamp(0, height as i32 - 1) as usize;

        let z = *surface_z.get(nx, ny);
        let current = *zlevels.get(nx, ny, z);

        if current == ZTile::Surface {
            zlevels.set(nx, ny, z, tile);
        }
    }
}

/// Place trade route evidence
fn place_route_evidence(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    path: &[(usize, usize)],
    waypoints: &[(usize, usize, super::trade::WaypointType)],
    active: bool,
    width: usize,
    height: usize,
    rng: &mut ChaCha8Rng,
) {
    // Place mile markers along the route
    let marker_spacing = 20;

    for (i, &(x, y)) in path.iter().enumerate() {
        if i % marker_spacing != 0 || i == 0 {
            continue;
        }

        let z = *surface_z.get(x, y);
        let current = *zlevels.get(x, y, z);

        if current == ZTile::Surface {
            zlevels.set(x, y, z, ZTile::MileMarker);
        }
    }

    // Place waypoint structures
    for &(x, y, waypoint_type) in waypoints {
        let z = *surface_z.get(x, y);
        let current = *zlevels.get(x, y, z);

        if current != ZTile::Surface {
            continue;
        }

        let tile = match waypoint_type {
            super::trade::WaypointType::Inn => {
                if active { ZTile::WoodWall } else { ZTile::WaystationRuin }
            }
            super::trade::WaypointType::TradePost => {
                if active { ZTile::WoodWall } else { ZTile::WaystationRuin }
            }
            super::trade::WaypointType::Watchtower => ZTile::StoneWall,
            super::trade::WaypointType::Waystation => {
                if active { ZTile::WoodFloor } else { ZTile::WaystationRuin }
            }
            super::trade::WaypointType::Bridge => ZTile::Bridge,
        };

        zlevels.set(x, y, z, tile);
    }

    // Place abandoned carts along inactive routes
    if !active {
        for &(x, y) in path.iter() {
            if rng.gen_bool(0.01) {
                let z = *surface_z.get(x, y);
                let current = *zlevels.get(x, y, z);

                if current == ZTile::Surface {
                    zlevels.set(x, y, z, ZTile::AbandonedCart);
                }
            }
        }
    }
}

/// Place faction boundary markers
fn place_boundary_markers(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    territory_map: &Tilemap<Option<FactionId>>,
    factions: &FactionRegistry,
    width: usize,
    height: usize,
    rng: &mut ChaCha8Rng,
) {
    // Find boundary tiles (where territory changes)
    for y in 1..(height - 1) {
        for x in 1..(width - 1) {
            let faction = *territory_map.get(x, y);

            // Skip unclaimed tiles
            if faction.is_none() {
                continue;
            }

            // Check if this is a boundary (different faction adjacent)
            let mut is_boundary = false;
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] {
                let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;

                let neighbor = *territory_map.get(nx, ny);
                if neighbor != faction {
                    is_boundary = true;
                    break;
                }
            }

            if !is_boundary {
                continue;
            }

            // Low probability to place a marker
            if !rng.gen_bool(0.02) {
                continue;
            }

            let z = *surface_z.get(x, y);
            let current = *zlevels.get(x, y, z);

            if current == ZTile::Surface {
                zlevels.set(x, y, z, ZTile::BoundaryStone);
            }
        }
    }
}

/// Place depleted resource evidence
fn place_depleted_resource(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    x: usize,
    y: usize,
    rng: &mut ChaCha8Rng,
) {
    let z = *surface_z.get(x, y);
    let current = *zlevels.get(x, y, z);

    if current == ZTile::Surface {
        // Place depleted evidence
        let tile = if rng.gen_bool(0.5) {
            ZTile::DriedWell
        } else {
            ZTile::OvergrownGarden
        };

        zlevels.set(x, y, z, tile);
    }
}

/// Place evidence at various Z-levels (underground and high peaks)
/// Evidence gets rarer at extreme depths and heights
pub fn place_multi_level_evidence(
    zlevels: &mut Tilemap3D<ZTile>,
    surface_z: &Tilemap<i32>,
    timeline: &Timeline,
    monsters: &MonsterRegistry,
    width: usize,
    height: usize,
    rng: &mut ChaCha8Rng,
) {
    use crate::zlevel::{MIN_Z, MAX_Z};

    println!("  Placing multi-level evidence...");

    // Place underground historical evidence (tombs, ossuaries, cursed grounds)
    // These are remnants of ancient battles, buried dead, and forgotten shrines
    let underground_count = (width * height) / 500; // Scale with map size

    for _ in 0..underground_count {
        let x = rng.gen_range(0..width);
        let y = rng.gen_range(0..height);
        let surf_z = *surface_z.get(x, y);

        // Pick a random depth below surface (deeper = rarer)
        let max_depth = (surf_z - MIN_Z).min(12) as usize;
        if max_depth < 2 {
            continue;
        }

        // Exponential distribution favoring shallower depths
        let depth = (rng.gen::<f32>().powi(2) * max_depth as f32) as i32 + 1;
        let z = surf_z - depth;

        if z < MIN_Z {
            continue;
        }

        let current = *zlevels.get(x, y, z);

        // Only place on cave floors or solid rock (carving into it)
        let can_place = matches!(current,
            ZTile::CaveFloor | ZTile::Solid | ZTile::MinedTunnel | ZTile::MinedRoom
        );

        if !can_place {
            continue;
        }

        // Underground evidence types - more ancient feeling
        let tile = match rng.gen_range(0..10) {
            0 => ZTile::Ossuary,       // Ancient bone storage
            1 => ZTile::Tomb,          // Underground tomb
            2 => ZTile::Shrine,        // Forgotten shrine
            3 => ZTile::BoneField,     // Mass burial
            4 => ZTile::CursedGround,  // Cursed area
            5 => ZTile::RustedWeapons, // Ancient armory remains
            6 => ZTile::Gravestone,    // Buried marker
            7 => ZTile::Crater,        // Ancient collapse
            8 => ZTile::Statue,        // Underground statue
            _ => ZTile::Obelisk,       // Underground monument
        };

        zlevels.set(x, y, z, tile);

        // Sometimes place a small cluster
        if rng.gen_bool(0.3) {
            for _ in 0..rng.gen_range(1..4) {
                let dx = rng.gen_range(-2..=2);
                let dy = rng.gen_range(-2..=2);
                let nx = (x as i32 + dx).rem_euclid(width as i32) as usize;
                let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as usize;

                let nz = z + rng.gen_range(-1..=1);
                if nz < MIN_Z || nz > MAX_Z {
                    continue;
                }

                let ncurrent = *zlevels.get(nx, ny, nz);
                if matches!(ncurrent, ZTile::CaveFloor | ZTile::Solid | ZTile::MinedTunnel) {
                    let secondary = match rng.gen_range(0..4) {
                        0 => ZTile::BoneField,
                        1 => ZTile::RustedWeapons,
                        2 => ZTile::Gravestone,
                        _ => ZTile::CursedGround,
                    };
                    zlevels.set(nx, ny, nz, secondary);
                }
            }
        }
    }

    // Place high peak evidence (shrines, obelisks, statues at mountain tops)
    let peak_count = (width * height) / 1000;

    for _ in 0..peak_count {
        let x = rng.gen_range(0..width);
        let y = rng.gen_range(0..height);
        let surf_z = *surface_z.get(x, y);

        // Only place at high elevations (z >= 8 is mountains/peaks)
        if surf_z < 8 {
            continue;
        }

        // Place above surface sometimes (on peaks/plateaus)
        let elevation_bonus = ((surf_z - 8) as f32 / 8.0).min(1.0);
        let above_z = if rng.gen_bool((elevation_bonus * 0.5) as f64) {
            let above = rng.gen_range(1..=3);
            (surf_z + above).min(MAX_Z)
        } else {
            surf_z
        };

        let current = *zlevels.get(x, y, above_z);

        // Place on surface or air (mountain peaks can have floating elements)
        if current != ZTile::Surface && current != ZTile::Air {
            continue;
        }

        // High altitude evidence - sacred/ancient feeling
        let tile = match rng.gen_range(0..6) {
            0 => ZTile::Shrine,       // Mountain shrine
            1 => ZTile::Obelisk,      // Peak marker
            2 => ZTile::Statue,       // Guardian statue
            3 => ZTile::WarMemorial,  // Battle memorial
            4 => ZTile::BoundaryStone,// Territory marker
            _ => ZTile::Altar,        // Sacrificial altar
        };

        zlevels.set(x, y, above_z, tile);
    }

    // Place monster evidence in caves at various depths
    for lair in monsters.lairs.values() {
        // Underground monster lairs spread evidence through cave systems
        let surf_z = *surface_z.get(lair.x, lair.y);

        // Compute territory radius from territory size
        let territory_radius = ((lair.territory.len() as f32).sqrt() as i32).max(5);

        // Place evidence at multiple Z levels around the lair
        for _ in 0..territory_radius {
            let dx = rng.gen_range(-territory_radius..territory_radius);
            let dy = rng.gen_range(-territory_radius..territory_radius);
            let dz = rng.gen_range(-8..4); // Mostly underground, some above

            let nx = (lair.x as i32 + dx).rem_euclid(width as i32) as usize;
            let ny = (lair.y as i32 + dy).clamp(0, height as i32 - 1) as usize;
            let nz = (surf_z + dz).clamp(MIN_Z, MAX_Z);

            // Probability decreases with distance from lair center and depth
            let dist = ((dx * dx + dy * dy) as f32).sqrt();
            let depth_penalty = if dz < 0 { (-dz as f32 / 16.0).min(0.8) } else { 0.0 };
            let prob = ((1.0 - dist / territory_radius as f32) * (1.0 - depth_penalty) * 0.15).clamp(0.0, 1.0);

            if !rng.gen_bool(prob as f64) {
                continue;
            }

            let current = *zlevels.get(nx, ny, nz);

            // Can place on cave floors, solid rock, or surface
            let can_place = matches!(current,
                ZTile::CaveFloor | ZTile::Surface | ZTile::Solid
            );

            if !can_place {
                continue;
            }

            // Use species-appropriate evidence
            let tile = match lair.species.territory_evidence() {
                "WebCluster" => ZTile::WebCluster,
                "BoneNest" => ZTile::BoneNest,
                "CharredGround" => ZTile::CharredGround,
                "AntMound" => ZTile::AntMound,
                "BeeHive" => ZTile::BeeHive,
                "ClawMarks" => ZTile::ClawMarks,
                "CursedGround" => ZTile::CursedGround,
                _ => ZTile::TerritoryMarking,
            };

            zlevels.set(nx, ny, nz, tile);
        }
    }

    // Place ancient battle evidence in caves (old wars fought underground)
    let underground_battles: Vec<_> = timeline.events.values()
        .filter(|e| matches!(e.event_type, EventType::Battle | EventType::Siege) && e.year.age() > 500)
        .collect();

    for event in underground_battles.iter().take(10) {
        if let Some((cx, cy)) = event.location {
            let surf_z = *surface_z.get(cx, cy);

            // Some battles had underground components
            if !rng.gen_bool(0.3) {
                continue;
            }

            let depth = rng.gen_range(2..10);
            let battle_z = (surf_z - depth).max(MIN_Z);

            // Place bone fields and rusted weapons at this level
            let radius = ((event.casualties as f32).sqrt() / 10.0).clamp(2.0, 8.0) as i32;

            for _ in 0..(event.casualties / 100).max(5).min(30) {
                let dx = rng.gen_range(-radius..=radius);
                let dy = rng.gen_range(-radius..=radius);
                let dz = rng.gen_range(-2..=2);

                let nx = (cx as i32 + dx).rem_euclid(width as i32) as usize;
                let ny = (cy as i32 + dy).clamp(0, height as i32 - 1) as usize;
                let nz = (battle_z + dz).clamp(MIN_Z, MAX_Z);

                let current = *zlevels.get(nx, ny, nz);

                if matches!(current, ZTile::CaveFloor | ZTile::Solid) {
                    let tile = if rng.gen_bool(0.6) {
                        ZTile::BoneField
                    } else {
                        ZTile::RustedWeapons
                    };
                    zlevels.set(nx, ny, nz, tile);
                }
            }
        }
    }

    println!("    Underground and peak evidence placed");
}
