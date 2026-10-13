//! The floodplain and the spring flood (cards 'Rivers freeze and flood in game time' and 'Fields
//! on the floodplain, and spring floods').
//!
//! Bottomland is dry ground by the river or lake (within 12 cells of its water) no higher than a
//! level above it. A field laid there yields half again (the river's silt), and the camp seeks it
//! out for its field. But after a hard winter (ten frozen dawns or more) the thaw comes down the
//! river: at the first dawn the ice is gone the water rises a level over the bottomland for six
//! days, drowning what was sown there and holding back the sowing; as it goes down it leaves a
//! skin of silt, and that field gives a meal more a row this year. Rivers freezing over (and the
//! fishing stopping) was already in (`reckon_ice`).

use super::*;
use crate::local::Shape;
use super::projects::ProjectKind;

/// How far from the water bottomland reaches (cells), and how long the flood lasts (days).
const REACH: i32 = 12;
const FLOOD_DAYS: u64 = 6;
/// Frozen dawns that make a winter hard enough to flood.
const HARD_WINTER: u32 = 10;

/// The spring flood under way: when it goes down, and the cells it covered (map indices of the
/// water cells it set).
#[derive(Clone, Debug, Default)]
pub struct Flood {
    pub until: u64,
    pub cells: Vec<usize>,
}

impl Colony {
    /// Bottomland, per column: dry ground within `REACH` of water, no higher than a level above
    /// the water's own cell. Reckoned once (the land does not move).
    pub(crate) fn bottomland(&self) -> &Vec<bool> {
        self.bottom.get_or_init(|| {
            let (w, h) = (self.map.width, self.map.height);
            let water = |x: usize, y: usize| { let z = self.map.surface_z[y * w + x] + 1; (z as usize) < self.map.depth && self.map.cell(x, y, z as usize).water > 0 };
            // Only a river or a lake floods (a body of water of 50 cells or more), not a spring's
            // pool or a puddle.
            let mut body = vec![0usize; w * h];
            let mut sizes = vec![0usize];
            for y in 0..h { for x in 0..w {
                if body[y * w + x] != 0 || !water(x, y) { continue; }
                let id = sizes.len();
                let (mut stack, mut n) = (vec![(x, y)], 0usize);
                body[y * w + x] = id;
                while let Some((cx, cy)) = stack.pop() {
                    n += 1;
                    for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
                        let (nx, ny) = (cx as i32 + dx, cy as i32 + dy);
                        if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 { continue; }
                        let k = ny as usize * w + nx as usize;
                        if body[k] == 0 && water(nx as usize, ny as usize) { body[k] = id; stack.push((nx as usize, ny as usize)); }
                    }
                }
                sizes.push(n);
            } }
            // From each such water column outward (rounded distances: 2 a step, 3 a diagonal):
            // the nearest water's level (its cell's z).
            let mut level = vec![i32::MAX; w * h];
            let mut dist = vec![i32::MAX; w * h];
            let mut heap = std::collections::BinaryHeap::new();
            for k in 0..w * h {
                if body[k] != 0 && sizes[body[k]] >= 50 { level[k] = self.map.surface_z[k] + 1; dist[k] = 0; heap.push(std::cmp::Reverse((0i32, k))); }
            }
            let reach = 2 * REACH + 6;
            while let Some(std::cmp::Reverse((d, k))) = heap.pop() {
                if d > dist[k] || d >= reach { continue; }
                let (x, y) = ((k % w) as i32, (k / w) as i32);
                for (dx, dy, c) in [(1i32, 0i32, 2i32), (-1, 0, 2), (0, 1, 2), (0, -1, 2), (1, 1, 3), (1, -1, 3), (-1, 1, 3), (-1, -1, 3)] {
                    let (nx, ny) = (x + dx, y + dy);
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 { continue; }
                    let q = ny as usize * w + nx as usize;
                    if d + c < dist[q] { dist[q] = d + c; level[q] = level[k]; heap.push(std::cmp::Reverse((d + c, q))); }
                }
            }
            if std::env::var("PLANET_DEBUG_FLOOD").is_ok() {
                let mut big: Vec<usize> = sizes.iter().copied().filter(|&n| n >= 10).collect(); big.sort_unstable(); big.reverse();
                let near = (0..w * h).filter(|&k| dist[k] > 0 && dist[k] <= 2 * REACH).count();
                let low = (0..w * h).filter(|&k| dist[k] > 0 && dist[k] <= 2 * REACH && self.map.surface_z[k] <= level[k]).count();
                let low2 = (0..w * h).filter(|&k| dist[k] > 0 && dist[k] <= 2 * REACH && self.map.surface_z[k] <= level[k] + 1).count();
                let wet = (0..w * h).filter(|&k| water(k % w, k / w)).count();
                eprintln!("FLOOD debug day {}: wet columns {}, bodies {:?}; near {} low {} low+1 {}", self.clock.day(), wet, &big[..big.len().min(6)], near, low, low2);
            }
            // The edge wanders (7-16 cells out) with the lie of the land, not a line drawn round.
            let wobble = |x: usize, y: usize| -> f32 {
                let cell = |a: i64, b: i64| (crate::history::settlers::hash_pub(self.seed ^ 0xF100D, (a as u64) << 20 ^ b as u64) % 1000) as f32 / 1000.0;
                let (fx, fy) = (x as f32 / 9.0, y as f32 / 9.0);
                let (ix, iy) = (fx.floor() as i64, fy.floor() as i64);
                let (u, v) = (fx - ix as f32, fy - iy as f32);
                let (u, v) = (u * u * (3.0 - 2.0 * u), v * v * (3.0 - 2.0 * v));
                let top = cell(ix, iy) * (1.0 - u) + cell(ix + 1, iy) * u;
                let bot = cell(ix, iy + 1) * (1.0 - u) + cell(ix + 1, iy + 1) * u;
                top * (1.0 - v) + bot * v
            };
            (0..w * h).map(|k| {
                let (x, y) = (k % w, k / w);
                let limit = (2.0 * REACH as f32 * (0.6 + 0.75 * wobble(x, y))) as i32;
                dist[k] > 0 && dist[k] <= limit && !water(x, y) && self.map.surface_z[k] <= level[k]
                    && self.map.cell(x, y, self.map.surface_z[k].max(0) as usize).shape != Shape::Wall
            }).collect()
        })
    }

    /// The share of a lot (top-left `at`, w x h) that is bottomland.
    pub(crate) fn bottom_share(&self, at: (i32, i32), w: i32, h: i32) -> f32 {
        let b = self.bottomland();
        let n = self.map.width as i32;
        let mut k = 0;
        for y in at.1..at.1 + h { for x in at.0..at.0 + w { if x >= 0 && y >= 0 && x < n && y < self.map.height as i32 && b[(y * n + x) as usize] { k += 1; } } }
        k as f32 / (w * h).max(1) as f32
    }

    /// Whether the camp's field lies on the floodplain (half of it bottomland or more).
    pub(crate) fn field_on_bottomland(&self) -> bool {
        self.projects.iter().find(|p| p.kind == ProjectKind::Field).map_or(false, |p| self.bottom_share((p.at.0 as i32 + 1, p.at.1 as i32 + 1), 6, 4) >= 0.5)
    }

    /// Dawn: count the winter's frozen dawns; at the thaw after a hard one, the flood; when its
    /// days are done, the water goes down.
    pub(crate) fn reckon_flood(&mut self) {
        if self.frozen() { self.frozen_dawns += 1; }
        let day = self.clock.day();
        if let Some(f) = self.flood.as_ref() {
            if day >= f.until {
                let cells = std::mem::take(&mut self.flood.as_mut().unwrap().cells);
                for k in cells { self.map.cells[k].water = 0; }
                self.flood = None;
                self.silted = self.field_on_bottomland();
                self.note(if self.silted { "The water goes down, and leaves a skin of grey silt over the field.".to_string() } else { "The water goes down off the bottomland.".to_string() });
                // Sown again where the flood took the crop (the season is still young).
                if let Some(at) = self.projects.iter().find(|p| p.kind == ProjectKind::Field && p.done).map(|p| p.at) {
                    if self.silted && self.map.sky.into_year(day) < self.map.sky.season_start(1) { self.sow_field(at); self.note("They sow the field again in the silt.".into()); }
                }
            }
            return;
        }
        // The thaw after a hard winter.
        // (The winter over, not a warmer dawn in the middle of it.)
        let thaw = !self.hard_winter() && self.frozen_dawns >= HARD_WINTER;
        if !thaw { if !self.hard_winter() { self.frozen_dawns = 0; } return; }
        let hard = self.frozen_dawns;
        self.frozen_dawns = 0;
        let (w, n) = (self.map.width, self.map.width * self.map.height);
        let bottom = self.bottomland().clone();
        let mut cells = Vec::new();
        for k in 0..n {
            if !bottom[k] { continue; }
            let (x, y) = (k % w, k / w);
            let z = self.map.surface_z[k] + 1;
            if z as usize >= self.map.depth { continue; }
            let i = self.map.idx(x, y, z as usize);
            // (Under a roof the floor is raised and stays dry.)
            if self.map.roofs.get(k).map_or(false, |&r| r != 0) { continue; }
            if self.map.cells[i].shape == Shape::Empty && self.map.cells[i].water == 0 { self.map.cells[i].water = 2; cells.push(i); }
        }
        if cells.is_empty() { return; }
        // Water standing in the camp spoils what of the store lies in it (a third).
        let wet: std::collections::HashSet<(u16, u16)> = cells.iter().map(|&i| { let col = i % n; ((col % w) as u16, (col / w) as u16) }).collect();
        let soaked: Vec<usize> = (0..self.items.len()).filter(|&j| self.items[j].stored && self.items[j].kind == ItemKind::Food && wet.contains(&self.items[j].at)).collect();
        let spoil = soaked.len() / 3;
        for &j in soaked.iter().rev().take(spoil) { self.items.remove(j); self.fix_refs_pub(j); }
        let in_camp = wet.contains(&self.camp);
        // A crop sown on it is drowned.
        let drowned = self.field_on_bottomland();
        if drowned {
            if let Some(at) = self.projects.iter().find(|p| p.kind == ProjectKind::Field).map(|p| p.at) {
                for dy in 1..5u16 { for dx in 1..7u16 {
                    let (x, y) = ((at.0 + dx) as usize, (at.1 + dy) as usize);
                    if x >= w || y >= self.map.height { continue; }
                    let i = self.map.idx(x, y, self.map.surface_z[y * w + x].max(0) as usize);
                    if matches!(self.map.cells[i].plant, crate::local::Plant::Crop(_)) { self.map.cells[i].plant = crate::local::Plant::None; }
                } }
            }
        }
        self.flood = Some(Flood { until: day + FLOOD_DAYS, cells: cells.clone() });
        let line = format!("The thaw comes down the river after {} frozen dawns: the water rises over the bottomland ({} cells){}{}.",
            hard, cells.len(), if drowned { ", and over the field" } else { "" },
            if in_camp { format!(", and stands in the camp{}", if spoil > 0 { format!(": {} meals in the store are spoilt", spoil) } else { String::new() }) } else { String::new() });
        self.note(line.clone());
        if self.milestones.insert("the spring flood") {
            let at = cells.get(cells.len() / 2).map(|&i| { let col = i % n; ((col % w) as u16, (col / w) as u16) }).unwrap_or(self.camp);
            self.moment("The spring flood".into(), line, format!("because a hard winter's snow melts all at once{}", if drowned { ", and they laid their field on the river's bottomland" } else { "" }), at);
        }
    }

    /// Meals a crop row cell gives at the harvest: 2 (3 with seed grain from the caravan), half
    /// again on the floodplain, one more in a year the flood silted it.
    pub(crate) fn meals_a_row(&self) -> usize {
        let base = if self.seed_grain { 3 } else { 2 };
        let bottom = if self.field_on_bottomland() { base / 2 } else { 0 };
        base + bottom + if self.silted { 1 } else { 0 }
    }
}
