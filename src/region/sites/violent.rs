//! The places of a violent land (ROADMAP Stage 7; only with upheaval, `crate::upheaval`): each
//! put where its landform is, with the cause the record or the ground gives.
//!
//! - **Impact craters** at the centre of every crater of the record whose centre is on a core
//!   tile and on land (named, ranked by size), a **crater lake** where its bowl holds water, and
//!   the small fresh **meteorite craters** of the embark's scale (`upheaval::small_craters`),
//!   which the embark carves (`local::sites`).
//! - **Landslides** and **fallen flanks** at each slide's scar (`landforms::mass`), and a
//!   **landslide lake** where the debris dammed a valley (a lake 2 m+ deep within reach of the
//!   toe, upstream).
//! - A **fault scarp** (the tile's highest face, one tile in two), **sag ponds** on a
//!   strike-slip trace's lakes, **maars** at their lakes.
//! - The dangerous: a **gas hollow** in a closed hollow near young volcanism (one tile in three
//!   with thermal ground), a **sinkhole** in salt sinks and one doline in four, a **dry falls**
//!   in the scablands, **quicksand** on tidal flats and braided bars by springs or channels, a
//!   **crevasse field** on the glacier.
//!
//! After the natural wonders and the ages before (`after`): one tar seep in two is a **tar pit**
//! with the bones of what it caught, and one old flint mine or copper working in two has **crown
//! holes** over its galleries.

use crate::erosion::materials::RockType;
use crate::region::landforms::{RegionFields, Slide, SlideKind};
use crate::region::plan::{Landform, Place, PlaceKind};
use crate::upheaval::{self, CraterKind};

use super::natural::{hash, one_in, Scan, ScanData};
use super::{Core, SitePlan};

fn ago(myr: f32) -> String {
    if myr >= 2.0 { format!("some {:.0} million years ago", myr) }
    else if myr >= 0.95 { "a million years ago".into() }
    else if myr >= 0.01 { format!("some {} thousand years ago", ((myr * 1000.0 / 10.0).round() * 10.0) as i64) }
    else { format!("some {} years ago", ((myr * 1e6 / 100.0).round() * 100.0).max(100.0) as i64) }
}

fn size_word(d_km: f32) -> String {
    if d_km >= 10.0 { format!("{:.0} km", d_km) } else if d_km >= 1.0 { format!("{:.1} km", d_km) } else { format!("{:.0} m", d_km * 1000.0) }
}

pub fn plan(f: &RegionFields, land: &[Landform], rock: &[RockType], core: Core, slides: &[Slide], out: &mut SitePlan) {
    let Some(up) = f.world.upheaval() else { return };
    let d = ScanData::take_or_new(f, land);
    let mut sc = Scan::new(f, land, rock, core, d.clone(), out);
    let s = f.s as i64;
    let ww = f.world.width as i64;
    let in_core = |ax: f64, ay: f64| -> Option<(usize, usize)> {
        let (i, j) = ((ax.floor() as i64) - f.gx0 * s, (ay.floor() as i64) - f.gy0 * s);
        let (ci0, cj0) = ((core.x0 - f.gx0) * s, (core.y0 - f.gy0) * s);
        if i < ci0 || j < cj0 || i >= ci0 + core.w as i64 * s || j >= cj0 + core.h as i64 * s { return None; }
        Some((i as usize, j as usize))
    };
    // Craters of the record.
    if let Some(rec) = f.world.impacts() {
        for tile in core.tiles() {
            let wt = sc.wrap_tile(tile);
            for c in rec.near(wt.0, wt.1) {
                if rec.tile_of(c) != wt { continue; }
                // The centre on this (unwrapped) core tile.
                let u = c.u + (tile.0 - wt.0 as i64) as f32;
                let (ax, ay) = (((u + 0.5) * s as f32) as f64, ((c.v + 0.5) * s as f32) as f64);
                let Some((i, j)) = in_core(ax, ay) else { continue };
                let k = sc.idx(i, j);
                if sc.sea(k) { continue; }
                let kind = if c.d_km < 2.0 * f.cell_m / 1000.0 { PlaceKind::MeteorCrater } else { PlaceKind::ImpactStructure };
                let shape = match c.kind { CraterKind::Simple => "a bowl", CraterKind::Complex => "a flat floor round a central peak", CraterKind::PeakRing => "a ring of peaks inside the rim" };
                let worn = if c.wear > 0.75 { ", worn nearly away" } else if c.wear > 0.4 { ", worn and half filled" } else { "" };
                let cause = format!("a stone from the sky struck here {}: {} across, {}{}", ago(c.age_myr), size_word(c.d_km), shape, worn);
                // Where the bowl holds a lake, the crater is marked on its rim (the highest dry
                // ground round it, where one stands and sees it) and the lake at the centre.
                let (pi, pj) = if sc.lake(k) || !sc.dry(k) {
                    let r_cells = (c.r_km() * 1000.0 / f.cell_m) as f32;
                    let lat = std::f32::consts::FRAC_PI_2 - (c.v + 0.5) / f.world.height as f32 * std::f32::consts::PI;
                    let mut best: Option<(usize, usize, f32)> = None;
                    for a in 0..48 {
                        let t = a as f32 / 48.0 * std::f32::consts::TAU;
                        for rr in [0.95f32, 1.05, 1.15] {
                            let x = i as f32 + rr * r_cells * t.cos() / lat.cos().abs().max(0.05);
                            let y = j as f32 + rr * r_cells * t.sin();
                            if x < 0.0 || y < 0.0 { continue; }
                            let (x, y) = (x as usize, y as usize);
                            if x >= f.gw || y >= f.gh { continue; }
                            let kk = sc.idx(x, y);
                            if sc.dry(kk) && in_core(((x as i64 + f.gx0 * s) as f64) + 0.5, ((y as i64 + f.gy0 * s) as f64) + 0.5).is_some() && best.map_or(true, |b| sc.h(kk) > b.2) {
                                best = Some((x, y, sc.h(kk)));
                            }
                        }
                    }
                    best.map_or((i, j), |b| (b.0, b.1))
                } else { (i, j) };
                if let Some(ix) = sc.put(out, kind, pi, pj, cause) {
                    out.places[ix].radius_m = if (pi, pj) == (i, j) { c.r_km() * 1000.0 } else { 0.0 };
                    out.places[ix].rank = if c.d_km > 50.0 { 0 } else if c.d_km > 15.0 { 5 } else { 15 };
                }
                // A lake in the bowl.
                if sc.lake(k) {
                    let _ = sc.put(out, PlaceKind::CraterLake, i, j, format!("rain filled the bowl a stone from the sky left {}", ago(c.age_myr)));
                }
            }
        }
    }
    for tile in core.tiles() {
        let wt = sc.wrap_tile(tile);
        // Small fresh craters (the embark carves them).
        for (cu, cv, dkm, wear) in upheaval::small_craters(f.world.width, f.world.height, f.world.seed(), &up, wt.0, wt.1) {
            let u = cu + (tile.0 - wt.0 as i64) as f32;
            let (ax, ay) = (((u + 0.5) * s as f32) as f64, ((cv + 0.5) * s as f32) as f64);
            let Some((i, j)) = in_core(ax, ay) else { continue };
            let k = sc.idx(i, j);
            if !sc.dry(k) || sc.slope(k) > 0.15 { continue; }
            // Kept about a million years a kilometre (`upheaval`): its wear says how far into that.
            let age = dkm.powf(1.2) * wear.max(0.02);
            let cause = format!("a stone from the sky struck here {}: a bowl {} across{}", ago(age), size_word(dkm), if wear > 0.6 { ", its rim worn low" } else { ", its rim still sharp" });
            if let Some(ix) = sc.put(out, PlaceKind::MeteorCrater, i, j, cause) {
                out.places[ix].radius_m = dkm * 500.0;
                out.places[ix].rank = 30;
            }
        }
        // The fault's face, the ponds on its trace, the maars' lakes, the crevasses.
        if one_in(wt, 0xFA, 2) {
            if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| (sc.land[k] == Landform::FaultScarp && sc.dry(k)).then(|| sc.slope(k))) {
                let ks = f.world.faults().tile(wt.0, wt.1).kind;
                let _ = sc.put(out, PlaceKind::FaultScarp, i, j, format!("{}: the ground stepped up or down here in earthquakes, a few metres at a time", ks.word()));
            }
        }
        // Groundwater rises where a fault cuts the beds: a spring at the foot of the scarp.
        if one_in(wt, 0xF5B, 2) {
            if let Some((i, j, _)) = sc.best(tile, |sc, i, j, k| {
                if !sc.dry(k) || sc.land[k] == Landform::FaultScarp || sc.slope(k) > 0.08 { return None; }
                let r = 2i64;
                let foot = (-r..=r).any(|dy| (-r..=r).any(|dx| {
                    let (x, y) = (i as i64 + dx, j as i64 + dy);
                    x >= 0 && y >= 0 && (x as usize) < sc.f.gw && (y as usize) < sc.f.gh && {
                        let kk = sc.idx(x as usize, y as usize);
                        sc.land[kk] == Landform::FaultScarp && sc.h(kk) > sc.h(k)
                    }
                }));
                foot.then(|| -sc.h(k))
            }) {
                let _ = sc.put(out, PlaceKind::Spring, i, j, "groundwater rising where the fault breaks the beds, at the foot of its scarp".into());
            }
        }
        if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| (sc.land[k] == Landform::SagPond).then_some(1.0)) {
            let _ = sc.put(out, PlaceKind::SagPond, i, j, "the fault's trace steps aside here and the ground between sank; water stands in it".into());
        }
        // On the maar's ring of ash beside its water, where one stands and sees it.
        for (i, j, _) in sc.several(tile, 2, |sc, i, j, k| (sc.land[k] == Landform::Maar && sc.dry(k) && sc.lake_beside(i, j)).then(|| sc.h(k))) {
            let _ = sc.put(out, PlaceKind::Maar, i, j, "rising magma met the groundwater and the steam blew a crater below the land; rain filled it".into());
        }
        if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| (sc.land[k] == Landform::Crevasses).then(|| sc.slope(k))) {
            let _ = sc.put(out, PlaceKind::CrevasseField, i, j, "the glacier bends over a step in its bed and cracks open; snow bridges hide the cracks".into());
        }
        // Gas in a hollow near young volcanism.
        let thermal = |sc: &Scan, k: usize| matches!(sc.land[k], Landform::ThermalGround | Landform::Fumarole | Landform::Maar | Landform::Crater | Landform::Caldera | Landform::FissureVents);
        if one_in(wt, 0x6A5, 3) {
            if let Some((i, j, _)) = sc.best(tile, |sc, i, j, k| {
                if !sc.dry(k) || sc.slope(k) > 0.05 || sc.rel_s(k) > 3.0 { return None; }
                let r = sc.cells(3000.0) as i64;
                let near = (-r..=r).step_by(2).any(|dy| (-r..=r).step_by(2).any(|dx| {
                    let (x, y) = (i as i64 + dx, j as i64 + dy);
                    x >= 0 && y >= 0 && (x as usize) < sc.f.gw && (y as usize) < sc.f.gh && thermal(sc, sc.idx(x as usize, y as usize))
                }));
                near.then(|| -sc.rel_s(k))
            }) {
                let _ = sc.put(out, PlaceKind::GasHollow, i, j, "the young volcanic ground breathes out carbon dioxide, heavier than air, which pools in the still hollow".into());
            }
        }
        // Ground that falls in.
        if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| (sc.land[k] == Landform::SaltSink).then_some(1.0)) {
            let _ = sc.put(out, PlaceKind::Sinkhole, i, j, "water from the dry lake's margin dissolved the salt under its crust, and the crust fell in".into());
        } else if one_in(wt, 0x51C, 4) {
            if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| (sc.land[k] == Landform::Doline && sc.dry(k)).then_some(1.0)) {
                let _ = sc.put(out, PlaceKind::Sinkhole, i, j, "the limestone under a thin cover of soil dissolved into a void, and the cover fell into it".into());
            }
        }
        if let Some((i, j, _)) = sc.best(tile, |sc, _i, _j, k| (sc.land[k] == Landform::Scabland && sc.dry(k) && sc.slope(k) > 0.06).then(|| sc.slope(k))) {
            let _ = sc.put(out, PlaceKind::DryFalls, i, j, "when the ice's dam broke, a lake emptied down this valley in days and cut a cataract the river cannot fill".into());
        }
        if one_in(wt, 0x0A5, 3) {
            if let Some((i, j, _)) = sc.best(tile, |sc, i, j, k| {
                if !matches!(sc.land[k], Landform::TidalFlat | Landform::Estuary | Landform::Braided | Landform::Backswamp) { return None; }
                // Not where the flats freeze.
                if sc.f.temp[k] < 3.0 { return None; }
                (sc.sat_river().count(i, j, 2) > 0 || sc.sea_beside(i, j)).then_some(1.0)
            }) {
                let _ = sc.put(out, PlaceKind::Quicksand, i, j, "fine sand kept loose by water welling up through it from the channel and the tide".into());
            }
        }
    }
    // The chronicle's slides and quakes on a core tile (`history::geology`): a slide the region
    // made on that tile takes the chronicle's words; else the slide is put on the tile's steepest
    // slope near the town, and a quake's last rupture names the fault's scarp.
    let mut told: Vec<(i64, i64, String)> = Vec::new();
    for tile in core.tiles() {
        let wt = sc.wrap_tile(tile);
        for ev in f.world.ground_events_at(wt.0, wt.1) {
            match ev.kind {
                crate::history::events::types::EventType::Skyfall if ev.crater_m > 0.0 => {
                    // The stone that fell in the chronicle: its crater, somewhere on the tile (by
                    // the event), carved by the embark.
                    let (i0, j0, i1, j1) = sc.tile_cells(tile);
                    let (ci, cj) = (i0 + (hash(ev.id.0 as i64, 1, 0x5F1) % (i1 - i0) as u64) as usize, j0 + (hash(ev.id.0 as i64, 2, 0x5F2) % (j1 - j0) as u64) as usize);
                    let spot = sc.best(tile, |sc, i, j, k| (sc.dry(k) && (i as i64 - ci as i64).abs() < 12 && (j as i64 - cj as i64).abs() < 12).then(|| -((i as f32 - ci as f32).hypot(j as f32 - cj as f32))));
                    if let Some((i, j, _)) = spot {
                        if let Some(ix) = sc.put(out, PlaceKind::MeteorCrater, i, j, format!("{} in the year {}: the stone's pit is still here, {:.0} m across", ev.title.replacen("A stone falls", "a stone fell", 1), ev.year, ev.crater_m)) {
                            out.places[ix].radius_m = ev.crater_m * 0.5;
                            out.places[ix].rank = 25;
                        }
                    }
                }
                crate::history::events::types::EventType::Skyfall if ev.airburst => {
                    let (i0, j0, i1, j1) = sc.tile_cells(tile);
                    let (ci, cj) = ((i0 + i1) / 2, (j0 + j1) / 2);
                    let (t, m) = (*f.world.temperature.get(wt.0, wt.1), *f.world.moisture.get(wt.0, wt.1));
                    let what = if t > -4.0 && m > 0.35 { "the forest lies flattened and burned, every trunk pointing away from here" } else { "the ground was scorched and stripped bare for a day's walk round, and the stones still lie blackened" };
                    let _ = sc.put(out, PlaceKind::Skyburn, ci.min(f.gw - 1), cj.min(f.gh - 1), format!("{} in the year {}: {}", ev.title.replacen("The sky bursts", "the sky burst", 1), ev.year, what));
                }
                crate::history::events::types::EventType::Landslide => {
                    let words = format!("{} in the year {}, shaken loose by the earthquake", ev.title.replacen("The hillside falls on", "the hillside fell on", 1), ev.year);
                    let (i0, j0, i1, j1) = sc.tile_cells(tile);
                    let mine = slides.iter().position(|sl| { let (i, j) = ((sl.scar.0.floor() as i64 - f.gx0 * s) as usize, (sl.scar.1.floor() as i64 - f.gy0 * s) as usize); i >= i0 && i < i1 && j >= j0 && j < j1 });
                    if let Some(ix) = mine { told.push((slides[ix].scar.0 as i64, slides[ix].scar.1 as i64, words)); continue; }
                    let mid = ((i0 + i1) / 2, (j0 + j1) / 2);
                    let r = sc.cells(4000.0);
                    if let Some((i, j, _)) = sc.best(tile, |sc, i, j, k| {
                        if !sc.dry(k) || (i as i64 - mid.0 as i64).abs() > r as i64 || (j as i64 - mid.1 as i64).abs() > r as i64 { return None; }
                        Some(sc.slope(k))
                    }) {
                        let _ = sc.put(out, PlaceKind::Landslide, i, j, words);
                    }
                }
                // Quakes name the fault's scarp below, once the scarps are put.
                _ => {}
            }
        }
    }
    // Slides.
    for sl in slides {
        // A fallen flank is marked on its field of hummocks (the amphitheatre often holds a
        // lake); a landslide at its scar.
        let at = if sl.kind == SlideKind::FallenFlank { ((sl.scar.0 + sl.toe.0) * 0.5, (sl.scar.1 + sl.toe.1) * 0.5) } else { sl.scar };
        let Some((i, j)) = in_core(at.0, at.1) else { continue };
        let (i, j) = if sl.kind == SlideKind::FallenFlank && !sc.dry(sc.idx(i, j)) { in_core(sl.toe.0, sl.toe.1).unwrap_or((i, j)) } else { (i, j) };
        let (kind, cause) = match sl.kind {
            SlideKind::FallenFlank => (PlaceKind::CollapsedFlank, format!("the volcano's side gave way and ran out {:.0} km in a field of hummocks", (sl.toe.0 - sl.scar.0).hypot(sl.toe.1 - sl.scar.1) as f32 * f.cell_m / 1000.0)),
            SlideKind::RockAvalanche => (PlaceKind::Landslide, format!("a high face of hard rock fell and ran {:.1} km down the valley as a sheet of blocks", (sl.toe.0 - sl.scar.0).hypot(sl.toe.1 - sl.scar.1) as f32 * f.cell_m / 1000.0)),
            SlideKind::Landslide => (PlaceKind::Landslide, format!("the slope was steeper than its rock could hold{}, and it came down", if f.world.faults().at((sl.scar.0 / s as f64) as f32, (sl.scar.1 / s as f64) as f32).intensity > 0.3 { " and the earthquakes shook it" } else { "" })),
        };
        let cause = told.iter().find(|t| t.0 == sl.scar.0 as i64 && t.1 == sl.scar.1 as i64).map(|t| t.2.clone()).unwrap_or(cause);
        let _ = sc.put(out, kind, i, j, cause);
        // A lake behind the debris.
        let Some((ti, tj)) = in_core(sl.toe.0, sl.toe.1) else { continue };
        let r = 8i64;
        let mut best: Option<(usize, usize, f32)> = None;
        for dy in -r..=r {
            for dx in -r..=r {
                let (x, y) = (ti as i64 + dx, tj as i64 + dy);
                if x < 0 || y < 0 || x as usize >= f.gw || y as usize >= f.gh { continue; }
                let k = f.idx(x as usize, y as usize);
                let depth = f.lake_depth(k);
                if depth > 2.0 && best.map_or(true, |b| depth > b.2) { best = Some((x as usize, y as usize, depth)); }
            }
        }
        if let Some((x, y, depth)) = best {
            let _ = sc.put(out, PlaceKind::LandslideLake, x, y, format!("the slide's debris fell across the valley and the water backed up behind it, {:.0} m deep", depth));
        }
    }
    // A fault scarp on a tile the chronicle's earthquake struck names it.
    for p in out.places.iter_mut().filter(|p| p.kind == PlaceKind::FaultScarp) {
        if let Some(ev) = f.world.ground_events_at(p.tile.0, p.tile.1).into_iter().filter(|e| e.kind == crate::history::events::types::EventType::Earthquake).max_by_key(|e| e.year) {
            p.cause = format!("{}; its last great rupture was the earthquake at {} in the year {}", p.cause, ev.title.trim_start_matches("Earthquake at "), ev.year);
        }
    }
    ScanData::hand_over(d);
}

/// After the natural wonders and the ages before: tar pits and crown holes.
pub fn after(f: &RegionFields, out: &mut SitePlan) {
    if f.world.upheaval().is_none() { return; }
    let mut extra: Vec<Place> = Vec::new();
    for p in out.places.iter_mut() {
        let key = (p.x.round() as i64, p.y.round() as i64);
        match p.kind {
            PlaceKind::TarSeep if hash(key.0, key.1, 0x7A2) % 2 == 0 => {
                p.kind = PlaceKind::TarPit;
                p.cause = format!("{}; the pitch pools under a skin of water and holds what wades in", p.cause);
            }
            PlaceKind::FlintMine | PlaceKind::CopperWorking if hash(key.0, key.1, 0xC20) % 2 == 0 => {
                let mut q = Place::point(PlaceKind::CrownHoles, p.x + 1.0, p.y, p.tile);
                q.cause = format!("the galleries of {} under them fell in, one after another", p.kind.word());
                q.z_m = p.z_m;
                extra.push(q);
            }
            _ => {}
        }
    }
    out.places.extend(extra);
}
