//! The ages before the chronicle (ROADMAP Stage 4): a light prehistory per world tile, and the
//! sites it leaves, placed by terrain logic and never by roll.
//!
//! `Ages::of` reads a tile's climate and land and says who lived there: hunters wherever it was
//! habitable; herders and farmers where the soil is fertile and the year warm enough; builders
//! of stone where the farmers had hard rock to raise; the first metal where ore lies within three
//! tiles of fields; the last age before the written history (towers, quarries, mills, drowned
//! villages) where the land fed the most. Each age leaves its kinds of site at the spots that
//! fit them (the rules are the roadmap's: a long barrow on a false crest above the valley its
//! builders farmed, a hill fort on a spur with steep ground on most sides, a crannog on a
//! shallow lake margin, a hoard in the bog, a ridgeway along the skyline), one tile in several
//! by hash, the best spot of a tile by score, never two of a kind within eight cells.
//!
//! The scan (`natural::Scan`) and the facts of the tile (`natural::TileFacts`) are the natural
//! wonders'; the landform classes are preferred where the Stage 2 passes set them and every
//! rule also reads the raw fields, so sites exist on ground the passes have not labelled.

use crate::erosion::materials::RockType;
use crate::region::landforms::RegionFields;
use crate::region::plan::{Landform, PlaceKind};

use super::natural::{lattice, one_in, rock_word, Scan, ScanData, TileFacts};
use super::{Core, SitePlan};

/// The soil's fertility (`world.resources().fertility`) from which a tile was farmed, and from
/// which it carried the last age's works.
const FARMED_FERTILITY: f32 = 0.3;
const RICH_FERTILITY: f32 = 0.55;
/// The warmth a year needs for crops (mean C) and the moisture.
const FARMED_TEMP_C: f32 = 3.0;
const FARMED_MOIST: f32 = 0.18;
/// The share of a tile's cells on hard rock for its farmers to have raised stone.
const BUILDERS_HARD_SHARE: f32 = 0.15;
/// One farmed tile in this many is a farmed valley: the tile the ages' works cluster on (its
/// barrows on the skyline, its enclosure on the terrace, its fort on the spur, its track along
/// the ridge); the other farmed tiles carry a stray thing or two.
const VALLEY_ONE_IN: u64 = 5;

/// Who lived on a tile, by age.
#[derive(Clone, Copy, Debug, Default)]
pub struct Ages {
    pub hunters: bool,
    pub farmers: bool,
    /// A farmed valley: where the farmers', builders' and metal's works cluster.
    pub valley: bool,
    pub builders: bool,
    pub metal: bool,
    pub last: bool,
}

impl Ages {
    /// From the tile's facts and the share of its cells on hard rock. The first metal reached
    /// every farmed valley by trade; only its workings need the ore near.
    pub(super) fn of(t: &TileFacts, tile: (usize, usize), hard_share: f32) -> Ages {
        let hunters = !t.ice && t.temp > -5.0;
        let farmers = hunters && t.fertility >= FARMED_FERTILITY && t.temp > FARMED_TEMP_C && t.moist > FARMED_MOIST;
        let valley = farmers && one_in(tile, 39, VALLEY_ONE_IN);
        let builders = farmers && hard_share >= BUILDERS_HARD_SHARE;
        let metal = farmers;
        let last = farmers && t.fertility >= RICH_FERTILITY;
        Ages { hunters, farmers, valley, builders, metal, last }
    }
}

/// A rule's gate: on a farmed valley one tile in `valley`, elsewhere one in `stray` (0 never).
fn gate(ages: &Ages, tile: (usize, usize), salt: u64, valley: u64, stray: u64) -> bool {
    if ages.valley { one_in(tile, salt, valley) } else { stray > 0 && one_in(tile, salt, stray) }
}

pub fn plan(f: &RegionFields, land: &[Landform], rock: &[RockType], core: Core, out: &mut SitePlan) {
    let d = ScanData::take_or_new(f, land);
    let mut sc = Scan::new(f, land, rock, core, d.clone(), out);
    for tile in core.tiles() {
        let wt = sc.wrap_tile(tile);
        let facts = TileFacts::of(f, wt);
        let (i0, j0, i1, j1) = sc.tile_cells(tile);
        let mut hard = 0usize;
        let mut land_cells = 0usize;
        for j in (j0..j1).step_by(4) {
            for i in (i0..i1).step_by(4) {
                let k = sc.idx(i, j);
                if sc.land_cell(k) { land_cells += 1; if sc.hard(k) { hard += 1; } }
            }
        }
        if land_cells == 0 { continue; }
        let ages = Ages::of(&facts, wt, hard as f32 / land_cells as f32);
        if !ages.hunters { continue; }
        hunters(&mut sc, &facts, tile, wt, out);
        if ages.farmers { farmers(&mut sc, &ages, &facts, tile, wt, out); }
        if ages.builders { builders(&mut sc, &ages, tile, wt, out); }
        if ages.metal { metal(&mut sc, &ages, &facts, tile, wt, out); }
        if ages.last { last_age(&mut sc, &ages, tile, wt, out); }
    }
    ScanData::hand_over(d);
}

/// A crest: near the highest ground of its surroundings, standing well over the low ground
/// within the long radius, gentle on top.
fn crest(sc: &Scan, k: usize) -> bool {
    sc.dry(k) && sc.slope(k) < 0.1 && sc.prom_l(k) < 15.0 && sc.rel_l(k) >= 30.0 && sc.rel_l(k) <= 150.0
}

/// A cliff foot: gentle ground under a face that rises 25 m+ within the short radius, with a
/// steep neighbour.
fn cliff_foot(sc: &Scan, i: usize, j: usize, k: usize) -> bool {
    if !sc.dry(k) || sc.slope(k) > 0.12 || sc.prom_s(k) < 25.0 { return false; }
    if matches!(sc.land[k], Landform::Scree) { return true; }
    let (gw, gh) = (sc.f.gw as i64, sc.f.gh as i64);
    (0..8).any(|d| {
        let (dx, dy) = [(1, 0), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)][d];
        let (x, y) = (i as i64 + dx, j as i64 + dy);
        x >= 0 && y >= 0 && x < gw && y < gh && (sc.slope(sc.idx(x as usize, y as usize)) > 0.3 || matches!(sc.land[sc.idx(x as usize, y as usize)], Landform::Cliff))
    })
}

/// A spur: the ground falls away in most directions within a few cells, the top is gentle.
fn spur_score(sc: &Scan, i: usize, j: usize, k: usize) -> Option<f32> {
    if !sc.dry(k) || sc.slope(k) > 0.06 || sc.rel_l(k) < 40.0 { return None; }
    let r = sc.cells(1500.0).clamp(2, 4);
    let falls = sc.falls_around(i, j, r, 20.0);
    if falls < 5 { return None; }
    Some(falls as f32 * 20.0 + sc.rel_l(k).min(200.0))
}

/// A sheltered shore: land beside the sea with little open water about.
fn sheltered_shore(sc: &Scan, i: usize, j: usize, k: usize) -> bool {
    sc.dry(k) && sc.sea_beside(i, j) && sc.slope(k) < 0.05 && sc.sat_sea().share(i, j, 6) < 0.35
}

fn hunters(sc: &mut Scan, t: &TileFacts, tile: (i64, i64), wt: (usize, usize), out: &mut SitePlan) {
    // Rock shelters under a face, on sandstone and limestone first.
    if one_in(wt, 40, 5) {
        if let Some((i, j, _)) = sc.best(tile, |sc, i, j, k| {
            if !cliff_foot(sc, i, j, k) { return None; }
            Some(sc.prom_s(k) + if matches!(sc.rock[k], RockType::Sandstone | RockType::Limestone) { 30.0 } else { 0.0 })
        }) {
            let k = sc.idx(i, j);
            let cause = format!("an overhang in the {} face, dry at the back, with water below", rock_word(sc.rock[k]));
            sc.put(out, PlaceKind::RockShelter, i, j, cause);
        }
    }
    // A painted cave: one limestone cave mouth in eight, the dark zone marked by the first people.
    if one_in(wt, 41, 8) {
        let (i0, j0, i1, j1) = sc.tile_cells(tile);
        let s = sc.f.s as i64;
        let (ax0, ay0) = sc.f.abs_cell(i0, j0);
        let in_tile = |p: &crate::region::plan::Place| p.x >= ax0 as f64 && p.x < (ax0 + s) as f64 && p.y >= ay0 as f64 && p.y < (ay0 + s) as f64;
        let _ = (i1, j1);
        if let Some(p) = out.places.iter_mut().find(|p| p.kind == PlaceKind::CaveMouth && in_tile(p)) {
            p.kind = PlaceKind::PaintedCave;
            p.cause = format!("{}; the first hunters painted the beasts they followed on its walls, far from the light", p.cause);
            p.rank = p.rank.min(60);
        }
    }
    // Kill sites at the edges of bogs and lakes, where the herds were driven into the soft
    // ground; bone beds in the river gravels, where the cold age's beasts washed up.
    if t.once_cold && one_in(wt, 42, 8) {
        if let Some((i, j, _)) = sc.best(tile, |sc, i, j, k| {
            if !sc.dry(k) || sc.slope(k) > 0.03 { return None; }
            let edge = sc.lake_beside(i, j) || (sc.sat_bog().count(i, j, 1) > 0 && !sc.bog(k));
            if !edge { return None; }
            Some(10.0 - sc.slope(k) * 100.0)
        }) {
            sc.put(out, PlaceKind::KillSite, i, j, "the soft ground at the water's edge, where the first hunters drove the herds".into());
        }
    }
    if t.once_cold && one_in(wt, 43, 8) {
        if let Some((i, j, _)) = sc.best(tile, |sc, i, j, k| {
            if !sc.dry(k) || sc.slope(k) > 0.02 { return None; }
            let gravels = matches!(sc.land[k], Landform::Braided | Landform::Floodplain | Landform::Terrace | Landform::Outwash) || sc.sat_river().count(i, j, 2) > 0;
            if !gravels { return None; }
            Some(5.0 - sc.rel_s(k))
        }) {
            sc.put(out, PlaceKind::BoneBed, i, j, "a bank of old river gravel, the bones of the cold age's beasts in it".into());
        }
    }
    // Shell middens on sheltered coasts.
    if one_in(wt, 44, 4) {
        if let Some((i, j, _)) = sc.best(tile, |sc, i, j, k| if sheltered_shore(sc, i, j, k) { Some(1.0 - sc.sat_sea().share(i, j, 6)) } else { None }) {
            sc.put(out, PlaceKind::ShellMidden, i, j, "a sheltered shore where the first people gathered shellfish, season after season".into());
        }
    }
}

fn farmers(sc: &mut Scan, ages: &Ages, t: &TileFacts, tile: (i64, i64), wt: (usize, usize), out: &mut SitePlan) {
    // Flint mines in the limestone plateaus.
    if gate(ages, wt, 45, 3, 12) {
        if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| {
            if !sc.dry(k) || sc.rock[k] != RockType::Limestone || sc.slope(k) > 0.04 || sc.rel_l(k) < 20.0 { return None; }
            Some(sc.rel_l(k) + if sc.land[k] == Landform::Pavement { 50.0 } else { 0.0 })
        }) {
            sc.put(out, PlaceKind::FlintMine, i, j, "a seam of flint in the limestone, dug by shafts and galleries for the first farmers' axes".into());
        }
    }
    // A long barrow on a false crest above the fields.
    if gate(ages, wt, 46, 1, 10) {
        if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| if crest(sc, k) && sc.convexity(k % sc.f.gw, k / sc.f.gw) > 0.0 { Some(sc.rel_l(k)) } else { None }) {
            let k = sc.idx(i, j);
            let cause = format!("a crest seen from the valley its builders ploughed, of the local {}", rock_word(sc.rock[k]));
            if let Some(ix) = sc.put(out, PlaceKind::LongBarrow, i, j, cause) { out.places[ix].rank = 90; }
        }
    }
    // An enclosure on a river terrace.
    if gate(ages, wt, 47, 2, 0) {
        if let Some((i, j, _)) = sc.best(tile, |sc, i, j, k| {
            if !sc.dry(k) || sc.slope(k) > 0.03 { return None; }
            let terrace = sc.land[k] == Landform::Terrace || (sc.sat_river().count(i, j, sc.cells(2000.0)) > 0 && (8.0..40.0).contains(&sc.rel_s(k)));
            if !terrace { return None; }
            Some(40.0 - sc.rel_s(k) + if sc.land[k] == Landform::Terrace { 20.0 } else { 0.0 })
        }) {
            sc.put(out, PlaceKind::Enclosure, i, j, "a terrace over the river, ringed with banks and ditches for the gatherings of the first farmers".into());
        }
    }
    // Lynchets on the hillsides below the barrows, and cairns where the fields were cleared.
    if gate(ages, wt, 48, 2, 0) {
        let barrows: Vec<(i64, i64)> = sc.placed.iter().filter(|(_, k)| matches!(k, PlaceKind::LongBarrow | PlaceKind::RoundBarrows)).map(|(p, _)| *p).collect();
        if let Some((i, j, _)) = sc.best(tile, |sc, i, j, k| {
            if !sc.dry(k) || !(0.05..0.2).contains(&sc.slope(k)) { return None; }
            let key = sc.key(i, j);
            let near_barrow = barrows.iter().any(|p| (p.0 - key.0).abs().max((p.1 - key.1).abs()) <= 6);
            Some(sc.slope(k) + if near_barrow { 1.0 } else { 0.0 })
        }) {
            sc.put(out, PlaceKind::Lynchets, i, j, "a hillside stepped by ages of ploughing along the slope".into());
        }
    }
    if t.fertility > FARMED_FERTILITY && gate(ages, wt, 49, 3, 12) {
        if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| if sc.dry(k) && sc.hard(k) && sc.slope(k) < 0.08 { Some(0.1 - sc.slope(k)) } else { None }) {
            sc.put(out, PlaceKind::ClearanceCairns, i, j, "heaps of the stones picked off the fields, generation after generation".into());
        }
    }
}

fn builders(sc: &mut Scan, ages: &Ages, tile: (i64, i64), wt: (usize, usize), out: &mut SitePlan) {
    let plateau = |sc: &Scan, k: usize| sc.dry(k) && sc.hard(k) && sc.slope(k) < 0.03 && sc.rel_l(k) >= 20.0 && !sc.bog(k);
    // A stone circle, one in a 4x4 block of tiles.
    if lattice(wt, 50, 4) {
        if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| if plateau(sc, k) { Some(sc.rel_l(k) - sc.slope(k) * 100.0) } else { None }) {
            let k = sc.idx(i, j);
            let cause = format!("a flat of hard {} with the sky all round it, for the stones to stand in a ring", rock_word(sc.rock[k]));
            if let Some(ix) = sc.put(out, PlaceKind::StoneCircle, i, j, cause) { out.places[ix].rank = 30; }
        }
    }
    if gate(ages, wt, 51, 3, 12) {
        if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| if plateau(sc, k) || (sc.dry(k) && sc.hard(k) && crest(sc, k)) { Some(sc.rel_l(k)) } else { None }) {
            let k = sc.idx(i, j);
            let cause = format!("a single stone of {} raised where it is seen from far", rock_word(sc.rock[k]));
            sc.put(out, PlaceKind::StandingStone, i, j, cause);
        }
    }
    if gate(ages, wt, 52, 3, 0) || lattice(wt, 52, 6) {
        if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| if sc.dry(k) && sc.hard(k) && sc.slope(k) < 0.05 { Some(sc.rel_s(k)) } else { None }) {
            let k = sc.idx(i, j);
            let cause = format!("three uprights and a capstone of {}, the chamber of a tomb whose mound is gone", rock_word(sc.rock[k]));
            sc.put(out, PlaceKind::Dolmen, i, j, cause);
        }
    }
    // Rock art on an outcrop over the valley.
    if gate(ages, wt, 53, 2, 10) {
        if let Some((i, j, _)) = sc.best(tile, |sc, i, j, k| {
            if !sc.dry(k) || !sc.hard(k) { return None; }
            let bare = matches!(sc.land[k], Landform::Outcrop | Landform::Tor | Landform::Cliff) || (sc.convexity(i, j) > 2.0 && sc.slope(k) > 0.05);
            if !bare || sc.rel_l(k) < 20.0 { return None; }
            Some(sc.rel_l(k))
        }) {
            sc.put(out, PlaceKind::RockArt, i, j, "a bare slab of rock over the valley, cut with cups and rings".into());
        }
    }
    // A henge on the terrace.
    if lattice(wt, 54, 5) {
        if let Some((i, j, _)) = sc.best(tile, |sc, i, j, k| {
            if !sc.dry(k) || sc.slope(k) > 0.02 { return None; }
            let terrace = sc.land[k] == Landform::Terrace || (sc.sat_river().count(i, j, sc.cells(2500.0)) > 0 && (5.0..40.0).contains(&sc.rel_s(k)));
            if terrace { Some(40.0 - sc.rel_s(k)) } else { None }
        }) {
            if let Some(ix) = sc.put(out, PlaceKind::Henge, i, j, "a level terrace by the river, banked and ditched in a great ring".into()) { out.places[ix].rank = 50; }
        }
    }
    // The ridgeway: a track along the skyline.
    if gate(ages, wt, 55, 2, 12) {
        if let Some(cells) = ridge_walk(sc, tile) {
            let (i, j) = cells[cells.len() / 2];
            let abs: Vec<(i64, i64)> = cells.iter().map(|&(x, y)| sc.f.abs_cell(x, y)).collect();
            if let Some(ix) = sc.put(out, PlaceKind::Ridgeway, i, j, "the dry high way along the crest, walked before any road was laid in the valleys".into()) {
                out.places[ix].cells = abs;
            }
        }
    }
    // A causeway across the bog.
    if one_in(wt, 56, 8) {
        if let Some(cells) = causeway_walk(sc, tile) {
            let (i, j) = cells[cells.len() / 2];
            let abs: Vec<(i64, i64)> = cells.iter().map(|&(x, y)| sc.f.abs_cell(x, y)).collect();
            if let Some(ix) = sc.put(out, PlaceKind::Causeway, i, j, "a road of split logs laid across the bog between two dry shores".into()) {
                out.places[ix].cells = abs;
            }
        }
    }
    // A ford where the river runs wide and shallow.
    if gate(ages, wt, 57, 2, 10) {
        if let Some((i, j, _)) = sc.best(tile, |sc, i, j, k| {
            let w = sc.river_w(k);
            if !(15.0..150.0).contains(&w) || sc.slope(k) > 0.01 || sc.lake(k) { return None; }
            // Dry land on both banks within two cells.
            let gw = sc.f.gw;
            let land_both = [(2i64, 0i64), (-2, 0)].iter().all(|&(dx, dy)| { let (x, y) = (i as i64 + dx, j as i64 + dy); x >= 0 && y >= 0 && (x as usize) < gw && (y as usize) < sc.f.gh && sc.dry(sc.idx(x as usize, y as usize)) })
                || [(0i64, 2i64), (0, -2)].iter().all(|&(dx, dy)| { let (x, y) = (i as i64 + dx, j as i64 + dy); x >= 0 && y >= 0 && (x as usize) < gw && (y as usize) < sc.f.gh && sc.dry(sc.idx(x as usize, y as usize)) });
            if !land_both { return None; }
            Some(w)
        }) {
            sc.put(out, PlaceKind::Ford, i, j, "the river runs wide and shallow over gravel here; the old track crosses".into());
        }
    }
}

fn metal(sc: &mut Scan, ages: &Ages, t: &TileFacts, tile: (i64, i64), wt: (usize, usize), out: &mut SitePlan) {
    // Round barrows in a line along the ridgeway, else on a crest.
    if gate(ages, wt, 58, 2, 8) {
        let track: Option<Vec<(i64, i64)>> = out.places.iter().rev().find(|p| p.kind == PlaceKind::Ridgeway && p.tile == wt).map(|p| p.cells.clone());
        let spot = match &track {
            Some(cells) if cells.len() >= 4 => {
                let (ax, ay) = cells[cells.len() / 3];
                let (ax0, ay0) = sc.f.abs_cell(0, 0);
                Some(((ax - ax0) as usize, (ay - ay0) as usize, 1.0))
            }
            _ => sc.best(tile, |sc, _i, _j, k| if crest(sc, k) { Some(sc.rel_l(k)) } else { None }),
        };
        if let Some((i, j, _)) = spot {
            if let Some(ix) = sc.put(out, PlaceKind::RoundBarrows, i, j, "a line of mounds along the skyline, each over a chief of the first metal age".into()) {
                if let Some(cells) = track { out.places[ix].cells = cells.iter().copied().skip(1).step_by(2).take(6).collect(); }
            }
        }
    }
    // The hill fort on the spur.
    if gate(ages, wt, 59, 2, 8) {
        if let Some((i, j, _)) = sc.best(tile, spur_score) {
            let k = sc.idx(i, j);
            let cause = format!("a spur with steep ground on most sides, {:.0} m over the valley: banks and a ditch made it a stronghold", sc.rel_l(k));
            if let Some(ix) = sc.put(out, PlaceKind::HillFort, i, j, cause) { out.places[ix].rank = 40; }
        }
    }
    // A crannog on the shallow margin of a lake.
    if gate(ages, wt, 60, 3, 12) {
        if let Some((i, j, _)) = sc.best(tile, |sc, i, j, k| {
            if !sc.lake(k) { return None; }
            let depth = sc.f.lake_depth(k);
            if !(1.0..4.0).contains(&depth) || sc.sat_lake().count(i, j, 2) == 25 { return None; }
            Some(4.0 - depth)
        }) {
            sc.put(out, PlaceKind::Crannog, i, j, "a shallow lake margin, where a house was built on a made island of timber and stone".into());
        }
    }
    // A hoard, and a body, in the bog.
    if gate(ages, wt, 61, 3, 12) {
        if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| if sc.bog(k) { Some(1.0) } else { None }) {
            sc.put(out, PlaceKind::BogHoard, i, j, "the bog took what was given to it: bronze sunk in the wet ground as an offering".into());
        }
    }
    if one_in(wt, 62, 12) {
        if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| if sc.bog(k) { Some(1.0) } else { None }) {
            sc.put(out, PlaceKind::BogBody, i, j, "the peat keeps what it takes; someone was put into it, bound".into());
        }
    }
    // Copper workings on the ore.
    if t.copper_near && one_in(wt, 63, 2) {
        if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| if sc.dry(k) && sc.hard(k) && (0.08..0.45).contains(&sc.slope(k)) { Some(sc.slope(k)) } else { None }) {
            sc.put(out, PlaceKind::CopperWorking, i, j, "green-stained rock on the hillside, worked by fire-setting for the first metal".into());
        }
    }
    // Desert kites.
    if t.desert() && one_in(wt, 64, 3) {
        if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| if sc.dry(k) && sc.slope(k) < 0.03 { Some(0.03 - sc.slope(k)) } else { None }) {
            sc.put(out, PlaceKind::DesertKite, i, j, "two long walls of stone narrowing to a pen, into which the herds were driven".into());
        }
    }
}

fn last_age(sc: &mut Scan, ages: &Ages, tile: (i64, i64), wt: (usize, usize), out: &mut SitePlan) {
    // A watchtower on a height over the sea or the valley.
    if gate(ages, wt, 65, 2, 8) {
        if let Some((i, j, _)) = sc.best(tile, |sc, i, j, k| {
            if !sc.dry(k) || sc.prom_l(k) > 10.0 || sc.rel_l(k) < 60.0 || sc.slope(k) > 0.1 { return None; }
            let sea = sc.sat_sea().share(i, j, 12);
            Some(sc.rel_l(k) + sea * 200.0)
        }) {
            sc.put(out, PlaceKind::Watchtower, i, j, "a height that sees far: a tower stood here and fell".into());
        }
    }
    // A quarry in the hard rock near the fields.
    if gate(ages, wt, 66, 2, 10) {
        if let Some((i, j, _)) = sc.best(tile, |sc, i, j, k| {
            if !sc.dry(k) || !sc.hard(k) { return None; }
            let bare = matches!(sc.land[k], Landform::Outcrop | Landform::Cliff | Landform::Scree) || sc.slope(k) > 0.15;
            if !bare { return None; }
            Some(sc.slope(k) + if sc.convexity(i, j) > 0.0 { 0.2 } else { 0.0 })
        }) {
            let k = sc.idx(i, j);
            let cause = format!("good {} close under the turf, cut for the walls and mills of the valley", rock_word(sc.rock[k]));
            sc.put(out, PlaceKind::Quarry, i, j, cause);
        }
    }
    // A mill on a small quick river.
    if gate(ages, wt, 67, 2, 6) {
        if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| {
            let w = sc.river_w(k);
            if !(4.0..60.0).contains(&w) || sc.slope(k) < 0.008 || sc.lake(k) { return None; }
            Some(sc.slope(k) * 100.0)
        }) {
            sc.put(out, PlaceKind::MillRuin, i, j, "a quick stream with fall enough to turn a wheel; the mill's walls stand roofless".into());
        }
    }
    // A drowned village under a lake on the valley floor.
    if one_in(wt, 68, 10) {
        if let Some((i, j, _)) = sc.best(tile, |sc, i, j, k| {
            if !sc.lake(k) || sc.f.lake_depth(k) < 5.0 || sc.sat_lake().count(i, j, 1) < 9 { return None; }
            Some(sc.f.lake_depth(k))
        }) {
            sc.put(out, PlaceKind::DrownedVillage, i, j, "the valley was dammed and the water rose over the village; its walls show in a dry summer".into());
        }
    }
}

/// A walk along the crest from the tile's highest gentle ground: at each step the highest of
/// the neighbours not yet walked that is still near the top of its surroundings. Six to
/// fourteen cells, else none.
fn ridge_walk(sc: &Scan, tile: (i64, i64)) -> Option<Vec<(usize, usize)>> {
    let (i0, j0, i1, j1) = sc.tile_cells(tile);
    let mut start: Option<(usize, usize, f32)> = None;
    for j in (j0..j1).step_by(2) {
        for i in (i0..i1).step_by(2) {
            let k = sc.idx(i, j);
            if !crest(sc, k) { continue; }
            let score = sc.rel_l(k);
            if start.map_or(true, |s| score > s.2) { start = Some((i, j, score)); }
        }
    }
    let (si, sj, _) = start?;
    let mut cells = vec![(si, sj)];
    let (gw, gh) = (sc.f.gw as i64, sc.f.gh as i64);
    for _ in 0..13 {
        let &(x, y) = cells.last().unwrap();
        let mut next: Option<((usize, usize), f32)> = None;
        for (dx, dy) in [(1i64, 0i64), (1, 1), (0, 1), (-1, 1), (-1, 0), (-1, -1), (0, -1), (1, -1)] {
            let (nx, ny) = (x as i64 + dx, y as i64 + dy);
            if nx < i0 as i64 || ny < j0 as i64 || nx >= i1 as i64 || ny >= j1 as i64 || nx >= gw || ny >= gh { continue; }
            let (nx, ny) = (nx as usize, ny as usize);
            if cells.iter().any(|&c| c == (nx, ny)) { continue; }
            let k = sc.idx(nx, ny);
            if !sc.dry(k) || sc.prom_s(k) > 20.0 { continue; }
            let h = sc.h(k);
            if next.map_or(true, |n| h > n.1) { next = Some(((nx, ny), h)); }
        }
        match next { Some((c, _)) => cells.push(c), None => break }
    }
    if cells.len() >= 6 { Some(cells) } else { None }
}

/// A line across the bog from dry ground to dry ground, up to ten cells.
fn causeway_walk(sc: &Scan, tile: (i64, i64)) -> Option<Vec<(usize, usize)>> {
    let (i0, j0, i1, j1) = sc.tile_cells(tile);
    for j in (j0 + 1..j1 - 1).step_by(3) {
        for i in (i0 + 1..i1 - 1).step_by(3) {
            let k = sc.idx(i, j);
            if !sc.bog(k) { continue; }
            for (dx, dy) in [(1i64, 0i64), (0, 1), (1, 1), (1, -1)] {
                // Walk both ways until dry ground; the bog between must be 2-10 cells.
                let mut line = vec![(i, j)];
                let mut ends = 0;
                for sign in [1i64, -1] {
                    let (mut x, mut y) = (i as i64, j as i64);
                    for _ in 0..6 {
                        x += dx * sign; y += dy * sign;
                        if x < i0 as i64 || y < j0 as i64 || x >= i1 as i64 || y >= j1 as i64 { break; }
                        let kk = sc.idx(x as usize, y as usize);
                        if sign > 0 { line.push((x as usize, y as usize)); } else { line.insert(0, (x as usize, y as usize)); }
                        if sc.dry(kk) && !sc.bog(kk) { ends += 1; break; }
                        if sc.lake(kk) || sc.sea(kk) { break; }
                    }
                }
                if ends == 2 && line.len() >= 4 { return Some(line); }
            }
        }
    }
    None
}
