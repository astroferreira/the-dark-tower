//! Digging too deep: the mine, the breach, and what comes up.
//!
//! Once the camp has a hall or cellar and wants ore (a workshop to work it, or a people who dig),
//! it sinks a mine: the delve's stair spine going on down, a level a cut, as far as 35 levels
//! (`delve.rs`). Ore met on the way is struck as in any dig. Where the mine reaches one of the caverns under the embark
//! (`local/caverns.rs`) it breaks in: the breach is a moment, each settler feels it by their
//! character (the curious wonder, the anxious dread), the mine stops, and from then on the
//! cavern's hunters may come up the mine at night, as wolves come out of a den. Worse, the
//! digging wakes the forgotten beast that sleeps in the deep (a generated monster): it becomes
//! the next trouble of the camp's arc, foretold by the miners ("something vast moves below"), and
//! comes up the mine, not over the hills. The patron's brake is the one they already have:
//! ground forbidden at the mine's mouth stops the digging.

use super::*;
use super::arc::{Threat, ThreatKind};
use super::projects::ProjectKind;

use super::cavelife::hunter;

impl Colony {
    /// Where the mine opens (the cell before its first row), once there is a mine.
    pub fn mine_mouth(&self) -> Option<Pos> {
        self.projects.iter().find(|p| p.kind == ProjectKind::Mine)?;
        self.delve_mouth
    }

    /// The mine reaches a cavern: the breach.
    pub(crate) fn breach_cavern(&mut self, i: usize, layer: usize, p: Pos, z: i32) {
        if self.breached.contains(&(layer as u8)) { return; }
        self.breached.push(layer as u8);
        let Some(c) = self.map.caverns.iter().find(|c| c.layer as usize == layer).cloned() else { return };
        let name = self.settlers[i].name.clone();
        let down = self.dig_depth(p, z);
        // A stair let down through the open dark to the cavern's floor: the dark can be walked.
        let floor = self.let_down_stair(p, z);
        let wet = c.water_cells > c.floor_cells / 20;
        let sight = format!("{}{}", if c.fungus > 0 { "pale fungus trees taller than a hut, " } else { "" }, if wet { "black water lying still" } else { "dust that has never been walked" });
        let text = format!("{}'s pick breaks through into darkness: {}, {} levels down. A cold wind comes up the mine; in the lamplight, {}, and {} scatter from the light.{}", name, c.name, down, sight, c.life.first().cloned().unwrap_or_else(|| "something".into()),
            if floor > 0 { format!(" They let a stair of timber down {} levels to its floor.", floor) } else { String::new() });
        let why = self.projects.iter().find(|q| q.kind == ProjectKind::Mine).map(|q| q.why.clone()).unwrap_or_else(|| "they dug down after ore".into());
        self.note(text.clone());
        self.moment(format!("They break into {}", c.name), text, format!("because they sank the mine {} levels, as {}", down, why), p);
        // Each feels it by their character: wonder for the curious, dread for the anxious.
        for j in 0..self.settlers.len() {
            if self.settlers[j].alive { let n = c.name.clone(); self.feel(j, mind::Feel::Breach { what: n }); }
        }
        // A lost thing lying in the dark (`relic.rs`).
        self.relic_below(i);
        // Its still water to fish (`delve.rs`).
        self.find_cave_fishing();
        // The mine stops here.
        // (The deep shaft goes on through; only the mine stops here.)
        if let Some(k) = self.projects.iter().position(|q| q.kind == ProjectKind::Mine && !q.done) {
            self.projects[k].done = true;
            self.projects[k].used = self.projects[k].needed;
            self.dig_plan = None;
        }
        // Its own creatures, roaming its floor (`cavelife.rs`).
        self.populate_cavern(layer, p, z);
        self.debug_cave_ways("breach");
        // What hunts there comes up at night from now on.
        // (Below a hatch nothing comes up: `delve.rs::seal_caverns`.)
        if self.hatch.is_some() { }
        else if let Some(h) = c.life.iter().find(|l| hunter(l)) { self.cave_hunter = Some(h.clone()); }
        else { self.note(format!("At dusk {} come up out of the mine and scatter into the night; nothing that hunts.", c.life.first().cloned().unwrap_or_else(|| "bats".into()))); }
        // The digging wakes what sleeps in the deep.
        self.wake_the_deep(&c.name);
    }

    /// From a cut at `(p, z)` that opened into a cavern, a stair built down through the open air
    /// to the cavern floor (as DF's built stairs): returns how many levels it spans.
    pub(crate) fn let_down_stair_pub(&mut self, p: Pos, z: i32) -> i32 { self.let_down_stair(p, z) }

    fn let_down_stair(&mut self, p: Pos, z: i32) -> i32 {
        let (x, y) = (p.0 as usize, p.1 as usize);
        let mut zz = z;
        let mut n = 0;
        while zz >= 1 && self.map.cell(x, y, zz as usize).shape == crate::local::Shape::Empty && self.map.cell(x, y, zz as usize).water == 0 {
            let k = self.map.idx(x, y, zz as usize);
            self.map.cells[k].shape = crate::local::Shape::Stair;
            self.map.cells[k].material = crate::local::Material::Wood;
            zz -= 1;
            n += 1;
        }
        if n > 0 { if let Some(sp) = self.spine.as_mut().filter(|s| s.at == p) { sp.bottom = sp.bottom.min(zz); } }
        n
    }

    /// The forgotten beast that sleeps in the embark's deepest cavern hears the breach: it is the
    /// camp's next trouble, foretold soon, and it comes up the mine.
    fn wake_the_deep(&mut self, breached: &str) {
        let Some((bname, m)) = self.map.caverns.iter().find_map(|c| c.beast.clone()) else { return };
        let Some(arc) = self.arc.as_mut() else { return };
        if arc.later.iter().chain(std::iter::once(&arc.threat)).any(|t| t.kind == ThreatKind::Deep) { return; }
        let size = m.size;
        let t = Threat {
            kind: ThreatKind::Deep, name: format!("{} the {}", bname, m.kind_word),
            why: format!("the mine broke into {}, and the noise went down into the deep", breached),
            cause: None, cause_text: Some(format!("the mine broke into {} on day {}", breached, self.clock.day())),
            faction: None, from: None, size: size.clamp(1.0, 3.0), monster: Some(m),
        };
        arc.later.insert(0, t);
        // It does not wait out the quiet: it comes within days.
        let day = self.clock.day();
        if arc.stage == 3 { arc.quiet_until = Some(day + 3); }
    }
}
