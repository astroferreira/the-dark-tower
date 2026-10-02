//! Turns world data into per-tile drawing instructions: a ground tile, an optional sprite,
//! a variant, relief shading, river connections and coastline edges.

use crate::biomes::ExtendedBiome;
use crate::map_export::{get_biome_family, BiomeFamily};
use crate::world::WorldData;

use super::atlas::{Atlas, TileKind, VARIANTS};

/// Smallest water body (tiles) drawn as a lake.
const MIN_LAKE_TILES: usize = 4;
/// Drainage area (world tiles) at which a river is drawn.
const RIVER_MIN_FLOW: f32 = 50.0;
/// One-tile-wide water deeper than this (m) is a real strait, not a river channel.
const CHANNEL_MAX_DEPTH_M: f32 = -300.0;

/// Neighbour offsets, indexed by bit: N, NE, E, SE, S, SW, W, NW.
pub const DIRS: [(i32, i32); 8] = [(0, -1), (1, -1), (1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1)];

pub struct TileWorld {
    pub width: usize,
    pub height: usize,
    pub ground: Vec<TileKind>,
    pub sprite: Vec<Option<TileKind>>,
    pub variant: Vec<u8>,
    /// Relief shading multiplier (1 = flat).
    pub shade: Vec<f32>,
    /// Bitmask over `DIRS`: which neighbours a river channel connects to.
    pub river: Vec<u8>,
    /// River channel half-width as a fraction of the tile (0 = none).
    pub river_width: Vec<f32>,
    /// Whether this tile or one of its 8 neighbours carries a river (strokes cross tile edges).
    pub river_near: Vec<bool>,
    /// For water tiles, bitmask over `DIRS` of neighbours that are land (for coastline foam).
    pub shore: Vec<u8>,
    /// Flat colour per tile (minimap, far zoom).
    pub color: Vec<[u8; 3]>,
    /// Bitmask over `DIRS`: road links (history overlay).
    pub road: Vec<u8>,
    /// Owning faction per tile (`u64::MAX` = unclaimed), for border lines.
    pub owner: Vec<u64>,
    /// Settlement (living, else most recent ruin) per tile.
    pub settlement: Vec<Option<crate::history::SettlementId>>,
    /// Whether this tile or a neighbour carries a road.
    pub road_near: Vec<bool>,
    /// Current season's snow cover 0..1, foliage tint (multiplier) and frozen-water flag.
    pub season_snow: Vec<f32>,
    pub season_tint: Vec<[f32; 3]>,
    pub season_frozen: Vec<bool>,
}

/// Distinct, muted colour for a faction's border.
pub fn faction_color(id: u64) -> u32 {
    let hue = (hash(id as usize, 7) % 360) as f32;
    let (r, g, b) = hsv(hue, 0.65, 0.95);
    ((r as u32) << 16) | ((g as u32) << 8) | b as u32
}

fn hsv(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;
    let (r, g, b) = match (h / 60.0) as u32 { 0 => (c, x, 0.0), 1 => (x, c, 0.0), 2 => (0.0, c, x), 3 => (0.0, x, c), 4 => (x, 0.0, c), _ => (c, 0.0, x) };
    (((r + m) * 255.0) as u8, ((g + m) * 255.0) as u8, ((b + m) * 255.0) as u8)
}

fn hash(x: usize, y: usize) -> u64 {
    let mut h = (x as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ (y as u64).wrapping_mul(0xC2B2_AE3D_27D4_EB4F);
    h ^= h >> 31;
    h = h.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    h ^ (h >> 29)
}

/// Ground tile and vegetation sprite for a land biome.
fn land_tiles(biome: ExtendedBiome, sparse: bool) -> (TileKind, Option<TileKind>) {
    use ExtendedBiome::*;
    use TileKind as T;
    let tree = |t: TileKind| if sparse { None } else { Some(t) };
    match biome {
        Ice => (T::Snow, None),
        SnowyPeaks => (T::Snow, Some(T::SnowPeak)),
        AlpineTundra => (T::Tundra, Some(T::Mountain)),
        Tundra | AuroraWastes => (T::Tundra, None),
        BorealForest | SubalpineForest => (T::Grass, Some(T::Conifer)),
        TemperateForest | MontaneForest => (T::Grass, Some(T::Deciduous)),
        TemperateRainforest | CloudForest => (T::JungleFloor, Some(T::Conifer)),
        TropicalForest | TropicalRainforest | AncientGrove => (T::JungleFloor, Some(T::Jungle)),
        TemperateGrassland | AlpineMeadow | Paramo => (T::Steppe, None),
        Foothills => (T::Grass, Some(T::Hills)),
        Savanna => (T::Savanna, tree(T::Acacia)),
        Desert | SingingDunes | GlassDesert => (T::Sand, None),
        Oasis => (T::Sand, Some(T::Palm)),
        SaltFlats => (T::Salt, None),
        Swamp | Marsh | Bog | MangroveSaltmarsh | Shadowfen => (T::Swamp, tree(T::DeadTree)),
        VolcanicWasteland | Ashlands | ObsidianFields | Geysers | TarPits => (T::Ash, None),
        DeadForest | PetrifiedForest => (T::Tundra, Some(T::DeadTree)),
        MushroomForest | BioluminescentForest => (T::JungleFloor, Some(T::Mushroom)),
        CrystalForest | CrystalWasteland => (T::Rock, Some(T::Crystal)),
        _ => match get_biome_family(biome).0 {
            BiomeFamily::Polar => (T::Snow, None),
            BiomeFamily::Boreal => (T::Grass, tree(T::Conifer)),
            BiomeFamily::TemperateDry => (T::Steppe, None),
            BiomeFamily::TemperateWet => (T::Grass, tree(T::Deciduous)),
            BiomeFamily::Tropical => (T::JungleFloor, tree(T::Jungle)),
            BiomeFamily::Arid => (T::Sand, None),
            BiomeFamily::Mountain => (T::Rock, Some(T::Mountain)),
            BiomeFamily::Wetland => (T::Swamp, None),
            BiomeFamily::Volcanic => (T::Ash, None),
            BiomeFamily::Ruins => (T::Grass, Some(T::Ruins)),
            BiomeFamily::Fantasy => (T::Grass, tree(T::Crystal)),
            _ => (T::Grass, None),
        },
    }
}

impl TileWorld {
    pub fn build(world: &WorldData, atlas: &Atlas) -> Self {
        let (w, h) = (world.width, world.height);
        let n = w * h;
        let hm = &world.heightmap;
        let at = |x: i64, y: i64| -> (usize, usize) {
            (x.rem_euclid(w as i64) as usize, y.clamp(0, h as i64 - 1) as usize)
        };
        // Lakes are real water bodies of a few tiles; one-tile hollows along a river path are
        // drawn as river instead of as a chain of square lake tiles.
        let body_size: std::collections::HashMap<u16, usize> =
            world.water_bodies.iter().map(|b| (b.id.0, b.tile_count)).collect();
        let is_lake = |x: usize, y: usize| {
            let id = *world.water_body_map.get(x, y);
            id.is_lake() && body_size.get(&id.0).copied().unwrap_or(0) >= MIN_LAKE_TILES
        };
        // Open water is part of some 2x2 all-water block. One-tile-wide strips of water are river
        // channels (often carved below sea level by erosion); they are drawn as rivers running
        // over land, not as chains of square sea tiles.
        let raw_water: Vec<bool> = (0..n).map(|i| *hm.get(i % w, i / w) <= 0.0 || is_lake(i % w, i / w)).collect();
        let wet = |x: i64, y: i64| {
            let (xx, yy) = at(x, y);
            raw_water[yy * w + xx]
        };
        let open: Vec<bool> = (0..n)
            .map(|i| {
                let (x, y) = ((i % w) as i64, (i / w) as i64);
                raw_water[i]
                    && [(0, 0), (-1, 0), (0, -1), (-1, -1)].iter().any(|&(ox, oy)| {
                        wet(x + ox, y + oy) && wet(x + ox + 1, y + oy) && wet(x + ox, y + oy + 1) && wet(x + ox + 1, y + oy + 1)
                    })
            })
            .collect();
        let is_channel = |x: usize, y: usize| {
            let i = y * w + x;
            raw_water[i] && !open[i] && *hm.get(x, y) > CHANNEL_MAX_DEPTH_M
        };
        let is_water = |x: usize, y: usize| raw_water[y * w + x] && !is_channel(x, y);
        // Rivers follow the D8 drainage (one tile wide) rather than the rasterised Bezier network,
        // which marks rivers several tiles wide and would sprout spokes around every lake.
        let is_river = |x: usize, y: usize| {
            is_channel(x, y)
                || (*hm.get(x, y) > 0.0
                    && !is_lake(x, y)
                    && match &world.flow_accumulation {
                        Some(f) => *f.get(x, y) >= RIVER_MIN_FLOW,
                        None => world.river_tile_cache.as_ref().map(|c| *c.get(x, y)).unwrap_or(false),
                    })
        };
        let volcano: std::collections::HashSet<(usize, usize)> = world.volcanoes.iter().map(|v| (v.x, v.y)).collect();

        let mut tw = TileWorld {
            width: w,
            height: h,
            ground: Vec::with_capacity(n),
            sprite: Vec::with_capacity(n),
            variant: Vec::with_capacity(n),
            shade: Vec::with_capacity(n),
            river: vec![0; n],
            river_width: vec![0.0; n],
            river_near: vec![false; n],
            shore: vec![0; n],
            color: Vec::with_capacity(n),
            road: vec![0; n],
            owner: vec![u64::MAX; n],
            settlement: vec![None; n],
            road_near: vec![false; n],
            season_snow: vec![0.0; n],
            season_tint: vec![[1.0; 3]; n],
            season_frozen: vec![false; n],
        };

        for y in 0..h {
            for x in 0..w {
                let e = *hm.get(x, y);
                let biome = *world.biomes.get(x, y);
                let hsh = hash(x, y);
                let (ground, sprite) = if is_channel(x, y) {
                    (TileKind::Grass, None) // real ground filled in from the neighbours below
                } else if e <= 0.0 {
                    let g = if matches!(biome, ExtendedBiome::Ice) || *world.temperature.get(x, y) < -12.0 {
                        TileKind::SeaIce
                    } else if e < -2000.0 {
                        TileKind::DeepOcean
                    } else if e < -150.0 {
                        TileKind::Ocean
                    } else {
                        TileKind::Shallows
                    };
                    (g, None)
                } else if is_lake(x, y) {
                    let g = match biome {
                        ExtendedBiome::LavaLake => TileKind::Lava,
                        ExtendedBiome::FrozenLake => TileKind::SeaIce,
                        _ if *world.temperature.get(x, y) < -8.0 => TileKind::SeaIce,
                        _ => TileKind::Lake,
                    };
                    (g, None)
                } else {
                    let sparse = hsh % 100 < 55;
                    let (mut g, mut s) = land_tiles(biome, sparse);
                    // Relief overrides vegetation where the land is high and rugged.
                    let mut relief = 0.0f32;
                    for (dx, dy) in DIRS {
                        let (nx, ny) = at(x as i64 + dx as i64, y as i64 + dy as i64);
                        relief = relief.max((e - *hm.get(nx, ny)).abs());
                    }
                    let cold = *world.temperature.get(x, y) < -2.0;
                    if volcano.contains(&(x, y)) {
                        s = Some(TileKind::Volcano);
                    } else if e > 2800.0 {
                        s = Some(if cold { TileKind::SnowPeak } else { TileKind::Mountain });
                        if g == TileKind::JungleFloor || g == TileKind::Grass { g = TileKind::Rock; }
                    } else if e > 1400.0 && relief > 350.0 && !matches!(s, Some(TileKind::Conifer | TileKind::Deciduous | TileKind::Jungle)) {
                        s = Some(TileKind::Hills);
                    }
                    // Beaches on low, gentle coasts in warm places.
                    let coastal = DIRS.iter().any(|&(dx, dy)| {
                        let (nx, ny) = at(x as i64 + dx as i64, y as i64 + dy as i64);
                        *hm.get(nx, ny) <= 0.0
                    });
                    if coastal && e < 60.0 && !cold && g != TileKind::Swamp && s.is_none() {
                        g = TileKind::Beach;
                    }
                    (g, s)
                };

                let variant = (hsh % VARIANTS as u64) as u8;
                let shade = if e > 0.0 && !is_lake(x, y) {
                    let (lx, ly) = at(x as i64 - 1, y as i64);
                    let (rx, ry) = at(x as i64 + 1, y as i64);
                    let (ux, uy) = at(x as i64, y as i64 - 1);
                    let (dx, dy) = at(x as i64, y as i64 + 1);
                    let gx = *hm.get(lx, ly) - *hm.get(rx, ry);
                    let gy = *hm.get(ux, uy) - *hm.get(dx, dy);
                    (1.0 + (gx + gy) / 3000.0).clamp(0.75, 1.25)
                } else {
                    1.0
                };
                let mut col = atlas.average(ground, variant as usize);
                if let Some(sp) = sprite {
                    let sc = atlas.average(sp, variant as usize);
                    col = [((col[0] as u16 + sc[0] as u16) / 2) as u8, ((col[1] as u16 + sc[1] as u16) / 2) as u8, ((col[2] as u16 + sc[2] as u16) / 2) as u8];
                }
                tw.ground.push(ground);
                tw.sprite.push(sprite);
                tw.variant.push(variant);
                tw.shade.push(shade);
                tw.color.push(col);
            }
        }

        // Channel tiles take the most common ground of their land neighbours.
        for y in 0..h {
            for x in 0..w {
                if !is_channel(x, y) { continue; }
                let mut counts: Vec<(TileKind, u32)> = Vec::new();
                for (dx, dy) in DIRS {
                    let (nx, ny) = at(x as i64 + dx as i64, y as i64 + dy as i64);
                    if raw_water[ny * w + nx] { continue; }
                    let g = tw.ground[ny * w + nx];
                    match counts.iter_mut().find(|c| c.0 == g) {
                        Some(c) => c.1 += 1,
                        None => counts.push((g, 1)),
                    }
                }
                let i = y * w + x;
                if let Some(&(g, _)) = counts.iter().max_by_key(|c| c.1) {
                    tw.ground[i] = g;
                }
                tw.color[i] = atlas.average(tw.ground[i], tw.variant[i] as usize);
            }
        }

        // Rivers: each river tile links to its steepest-descent neighbour (if that neighbour is
        // river or water); the reverse links come from upstream tiles, giving a clean tree.
        for y in 0..h {
            for x in 0..w {
                let i = y * w + x;
                if is_water(x, y) {
                    let mut m = 0u8;
                    for (b, (dx, dy)) in DIRS.iter().enumerate() {
                        let (nx, ny) = at(x as i64 + *dx as i64, y as i64 + *dy as i64);
                        if !is_water(nx, ny) { m |= 1 << b; }
                    }
                    tw.shore[i] = m;
                    continue;
                }
                if !is_river(x, y) { continue; }
                let acc = world.flow_accumulation.as_ref().map(|f| *f.get(x, y)).unwrap_or(60.0);
                tw.river_width[i] = (0.06 + 0.035 * (acc / 50.0).max(1.0).ln()).min(0.22);
                if is_channel(x, y) {
                    // Channels are drawn as continuous strips: link orthogonal neighbours, and
                    // diagonal ones only where no orthogonal step already joins them.
                    let joins = |nx: usize, ny: usize| is_river(nx, ny) || is_water(nx, ny);
                    for (b, (dx, dy)) in DIRS.iter().enumerate() {
                        let (nx, ny) = at(x as i64 + *dx as i64, y as i64 + *dy as i64);
                        if !joins(nx, ny) { continue; }
                        if dx.abs() + dy.abs() == 2 {
                            let (ax, ay) = at(x as i64 + *dx as i64, y as i64);
                            let (bx, by) = at(x as i64, y as i64 + *dy as i64);
                            if joins(ax, ay) || joins(bx, by) { continue; }
                        }
                        tw.river[i] |= 1 << b;
                        if is_river(nx, ny) {
                            tw.river[ny * w + nx] |= 1 << ((b + 4) % 8);
                        }
                    }
                    continue;
                }
                let e = *hm.get(x, y);
                let mut best: Option<(usize, usize, usize, f32)> = None;
                for (b, (dx, dy)) in DIRS.iter().enumerate() {
                    let (nx, ny) = at(x as i64 + *dx as i64, y as i64 + *dy as i64);
                    let ne = if is_water(nx, ny) { -1.0e6 + *hm.get(nx, ny) } else { *hm.get(nx, ny) };
                    let dist = if dx.abs() + dy.abs() == 2 { 1.414 } else { 1.0 };
                    let slope = (e - ne) / dist;
                    if slope > 0.0 && best.map(|bb| slope > bb.3).unwrap_or(true) {
                        best = Some((b, nx, ny, slope));
                    }
                }
                if let Some((b, nx, ny, _)) = best {
                    if is_river(nx, ny) || is_water(nx, ny) {
                        tw.river[i] |= 1 << b;
                        if is_river(nx, ny) {
                            tw.river[ny * w + nx] |= 1 << ((b + 4) % 8);
                        }
                    }
                }
            }
        }
        for y in 0..h {
            for x in 0..w {
                if tw.river[y * w + x] == 0 { continue; }
                for dy in -1i64..=1 {
                    for dx in -1i64..=1 {
                        let (nx, ny) = at(x as i64 + dx, y as i64 + dy);
                        tw.river_near[ny * w + nx] = true;
                    }
                }
            }
        }
        tw
    }

    /// Recompute snow cover, foliage colour and frozen water for a season.
    pub fn set_season(&mut self, world: &WorldData, season: crate::seasons::Season) {
        use crate::seasons::Season;
        let (w, h) = (self.width, self.height);
        let sc = world.seasonal_climate.as_ref();
        let sm = |a: f32, b: f32, x: f32| { let t = ((x - a) / (b - a)).clamp(0.0, 1.0); t * t * (3.0 - 2.0 * t) };
        for y in 0..h {
            let north = y < h / 2;
            for x in 0..w {
                let i = y * w + x;
                let t = sc.map(|c| c.get_temperature(x, y, season, north)).unwrap_or(*world.temperature.get(x, y));
                let m = sc.map(|c| c.get_moisture(x, y, season, north)).unwrap_or(*world.moisture.get(x, y));
                let ground = self.ground[i];
                let water = ground.is_water() || ground == TileKind::SeaIce;
                self.season_frozen[i] = t < -4.0;
                // Snow lies on land when it is cold and there is moisture to fall as snow.
                self.season_snow[i] = if water { 0.0 } else { sm(1.0, -7.0, t) * sm(0.04, 0.2, m).max(0.35) };
                // Foliage: spring flush, summer drought on dry grass, autumn colour, winter dullness.
                let sprite = self.sprite[i];
                let leafy = matches!(sprite, Some(TileKind::Deciduous | TileKind::Acacia));
                let grassy = matches!(ground, TileKind::Grass | TileKind::Steppe | TileKind::Savanna);
                let dry = (0.35 - m).max(0.0) * 2.0;
                let tint = match season {
                    Season::Spring => if grassy || leafy { [0.94, 1.10, 0.90] } else { [1.0; 3] },
                    Season::Summer => if grassy { [1.0 + 0.18 * dry, 1.0 + 0.04 * dry, 1.0 - 0.18 * dry] } else { [1.0; 3] },
                    Season::Autumn => if leafy { [1.35, 0.92, 0.62] } else if grassy { [1.12, 1.0, 0.78] } else { [1.0; 3] },
                    Season::Winter => if leafy || grassy { [0.93, 0.93, 0.96] } else { [0.97, 0.98, 1.0] },
                };
                self.season_tint[i] = tint;
            }
        }
    }

    /// Overlay what history left on the land: settlements and ruins as sprites, roads, and
    /// territory ownership.
    pub fn apply_history(&mut self, world: &WorldData, history: &crate::history::world_state::WorldHistory, atlas: &Atlas) {
        use crate::history::civilizations::settlement::SettlementType;
        let (w, h) = (self.width, self.height);
        for y in 0..h {
            for x in 0..w {
                let t = history.tile_history.get(x, y);
                if let Some(f) = t.current_owner { self.owner[y * w + x] = f.0; }
                if t.has_road { self.road[y * w + x] = 0x80; } // marker, links resolved below
            }
        }
        // Claims are scattered tile by tile; a few majority-filter passes turn them into
        // coherent territories so borders read as outlines, not a lattice.
        for _ in 0..3 {
            let prev = self.owner.clone();
            for y in 0..h {
                for x in 0..w {
                    let mut votes: [(u64, u8); 9] = [(u64::MAX, 0); 9];
                    let mut n = 0;
                    for dy in -1i64..=1 {
                        let ny = y as i64 + dy;
                        if ny < 0 || ny >= h as i64 { continue; }
                        for dx in -1i64..=1 {
                            let o = prev[ny as usize * w + (x as i64 + dx).rem_euclid(w as i64) as usize];
                            match votes[..n].iter_mut().find(|v| v.0 == o) {
                                Some(v) => v.1 += 1,
                                None => { votes[n] = (o, 1); n += 1; }
                            }
                        }
                    }
                    let best = votes[..n].iter().max_by_key(|v| v.1).unwrap();
                    if best.1 >= 5 { self.owner[y * w + x] = best.0; }
                }
            }
        }
        // Settlements: ruins first so a living settlement on the same tile wins.
        let mut list: Vec<_> = history.settlements.values().collect();
        list.sort_by_key(|s| (!s.is_destroyed(), s.id.0));
        for s in list {
            let (x, y) = s.location;
            if x >= w || y >= h { continue; }
            let i = y * w + x;
            let kind = if s.is_destroyed() {
                TileKind::Ruins
            } else {
                match s.settlement_type {
                    SettlementType::Capital | SettlementType::Fort => TileKind::Castle,
                    SettlementType::City | SettlementType::Port => TileKind::City,
                    SettlementType::Town | SettlementType::Temple | SettlementType::Mine => TileKind::Town,
                    _ => TileKind::Village,
                }
            };
            self.sprite[i] = Some(kind);
            self.settlement[i] = Some(s.id);
            self.color[i] = atlas.average(kind, 0);
            self.road[i] |= 0x80;
        }
        // Road links between neighbouring road/settlement tiles; diagonals only where no
        // orthogonal step already joins the pair (keeps lines one tile wide).
        let at = |x: i64, y: i64| -> Option<usize> {
            if y < 0 || y >= h as i64 { None } else { Some(y as usize * w + x.rem_euclid(w as i64) as usize) }
        };
        let marked: Vec<bool> = self.road.iter().map(|&r| r & 0x80 != 0).collect();
        for y in 0..h as i64 {
            for x in 0..w as i64 {
                let i = y as usize * w + x as usize;
                if !marked[i] { continue; }
                let mut mask = 0u8;
                for (b, (dx, dy)) in DIRS.iter().enumerate() {
                    let Some(j) = at(x + *dx as i64, y + *dy as i64) else { continue };
                    if !marked[j] { continue; }
                    if dx.abs() + dy.abs() == 2 {
                        let a = at(x + *dx as i64, y).map(|k| marked[k]).unwrap_or(false);
                        let c = at(x, y + *dy as i64).map(|k| marked[k]).unwrap_or(false);
                        if a || c { continue; }
                    }
                    mask |= 1 << b;
                }
                self.road[i] = mask;
            }
        }
        for y in 0..h as i64 {
            for x in 0..w as i64 {
                if self.road[y as usize * w + x as usize] == 0 { continue; }
                for dy in -1..=1 {
                    for dx in -1..=1 {
                        if let Some(j) = at(x + dx, y + dy) { self.road_near[j] = true; }
                    }
                }
            }
        }
        let _ = world;
    }
}
