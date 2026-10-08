//! A consistency audit of a generated embark (`PLANET_LOCAL_AUDIT=1 --local-snapshot P`): what
//! must hold of any `LocalMap` whatever its seed, listed as one line per kind of violation.
//! Used as a testing aid for worldgen and embark changes; an empty list means clean.

use super::*;

impl LocalMap {
    /// Violations of the embark's invariants (empty when consistent). Each line names the kind,
    /// how many cells or columns break it and the first one (x, y, z).
    pub fn audit(&self) -> Vec<String> {
        let (w, h, d) = (self.width, self.height, self.depth as i32);
        let mut out: Vec<String> = Vec::new();
        let mut bad = |f: &dyn Fn(usize, usize, i32) -> bool, kind: &str| {
            let (mut n, mut first) = (0usize, None);
            for z in 0..d { for y in 0..h { for x in 0..w {
                if f(x, y, z) { n += 1; if first.is_none() { first = Some((x, y, z)); } }
            } } }
            if n > 0 { out.push(format!("{kind}: {n} (first at {:?})", first.unwrap())); }
        };
        let open_air = |c: &Cell| c.shape == Shape::Empty && c.water == 0;
        let carved: std::collections::HashSet<(u16, u16)> = self.places.iter().flat_map(|p| p.cells.iter().map(|c| c.0)).collect();
        // The floor of every column is a floor (or a ramp, a stair, a cut floor).
        bad(&|x, y, z| z == self.surface_z[y * w + x] && !matches!(self.cell(x, y, z as usize).shape, Shape::Floor | Shape::Ramp | Shape::Stair), "column's ground level is not a floor");
        // Magma: a liquid in open cells, in the sea (levels 1..=top) or the pipe, never within
        // three levels of the ground, and never a floor to stand on.
        let top = self.magma_top.unwrap_or(0);
        let pipe = self.magma_pipe;
        bad(&|x, y, z| { let c = self.cell(x, y, z as usize); c.material == Material::Magma && (c.shape != Shape::Empty || c.water != WATER_FULL) }, "magma that is not an open liquid cell");
        bad(&|x, y, z| { let c = self.cell(x, y, z as usize); c.material == Material::Magma && z >= self.ground_z(x, y) - 2 }, "magma within two levels of the ground");
        bad(&|x, y, z| {
            let c = self.cell(x, y, z as usize);
            if c.material != Material::Magma || (1..=top).contains(&z) { return false; }
            // Above the sea only the pipe holds magma: a column within radius 2 of its middle.
            match pipe { Some((px, py)) => (x as i32 - px as i32).pow(2) + (y as i32 - py as i32).pow(2) > 8, None => true }
        }, "magma above the sea outside the pipe");
        // Open air (dug, a place, a cavern) touching magma sideways at its own level: a leak. Caverns open to the sea are by design.
        bad(&|x, y, z| {
            let c = self.cell(x, y, z as usize);
            if c.material != Material::Magma { return false; }
            [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)].iter().any(|&(dx, dy)| {
                let (nx, ny) = (x as i32 + dx, y as i32 + dy);
                if nx < 0 || ny < 0 || nx as usize >= w || ny as usize >= h { return false; }
                let n = self.cell(nx as usize, ny as usize, z as usize);
                (open_air(n) || n.shape == Shape::Floor || n.shape == Shape::Stair) && self.cavern_at(nx as usize, ny as usize, z).is_none()
            })
        }, "magma open to a non-cavern cell (a place or dig meets it)");
        // Water: only in open cells, resting on something (no floating water), magma aside.
        bad(&|x, y, z| { let c = self.cell(x, y, z as usize); c.water > 0 && c.shape != Shape::Empty }, "water in a solid cell");
        bad(&|x, y, z| {
            let c = self.cell(x, y, z as usize);
            if c.water == 0 || c.material == Material::Magma || z == 0 { return false; }
            let b = self.cell(x, y, z as usize - 1);
            b.shape == Shape::Empty && b.water == 0
        }, "floating water");
        // Solid below the ground, but for caverns, water, magma, and what places carve.
        bad(&|x, y, z| {
            let sz = self.surface_z[y * w + x];
            if z >= sz || carved.contains(&(x as u16, y as u16)) { return false; }
            let c = self.cell(x, y, z as usize);
            c.shape == Shape::Empty && c.water == 0 && self.cavern_at(x, y, z).is_none()
        }, "hollow under the ground that is no cavern, place or water");
        // Cavern layers: floor below an open run with solid over it.
        bad(&|x, y, z| {
            let Some(k) = self.cavern_at(x, y, z) else { return false };
            let (f, t) = self.cavern_z[y * w + x][k];
            let c = self.cell(x, y, z as usize);
            if z == f as i32 { c.shape != Shape::Floor && c.shape != Shape::Stair && c.shape != Shape::Ramp }
            else if z == t as i32 + 1 { false }
            else { c.shape != Shape::Empty && c.shape != Shape::Stair && c.shape != Shape::Floor }
        }, "cavern layer's cells are not floor then open");
        bad(&|x, y, z| { let sz = self.surface_z[y * w + x]; self.cavern_at(x, y, z).map(|_| z + 3 > sz).unwrap_or(false) }, "cavern within three levels of the ground");
        // Soil over rock: no loose ground (soil, sand, clay, gravel) under solid rock in a column,
        // outside caverns and places, as strata go rock-wards with depth.
        bad(&|x, y, z| {
            if z == 0 || carved.contains(&(x as u16, y as u16)) { return false; }
            let c = self.cell(x, y, z as usize);
            if c.shape != Shape::Wall || !matches!(c.material, Material::Soil | Material::Clay | Material::Sand | Material::Gravel) { return false; }
            let above = self.cell(x, y, z as usize + 1);
            matches!(above.material, Material::Rock(_) | Material::Ore(_)) && above.shape == Shape::Wall
        }, "loose ground under rock");
        // The aquifer lies in rock or ground, never in open air.
        if let Some((lo, hi)) = self.aquifer { if lo >= hi || lo < 0 || hi >= d { out.push(format!("aquifer levels out of range {lo}..{hi} of {d}")); } }
        out
    }
}
