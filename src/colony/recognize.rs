//! Old comrades and old enemies: two who were at the same battle know each other.
//!
//! The idea is Dwarf Fortress's per-figure knowledge of events: what each person was part of is
//! remembered, and two who were in the same event share it, for good or ill. Here, at dawn
//! (`reckon_old_fields`), two settlers (guests too) whose pasts cite the same battle or siege
//! (the same chronicle event in `Past::lines`) recognise each other, once: of one people, they
//! were comrades ("X and Y find they both stood at the Battle of Z", +6, `Feel::Friend`); of two
//! peoples, they stood on opposite sides: if both are slow to hold a grudge (vengefulness under
//! 40) they share a cup and let it lie (+2); else it is a grudge (-8, `Feel::Quarrel`, a
//! moment).

use super::*;
use crate::persona::Facet;

impl Colony {
    /// Dawn: those who shared a battle find each other out.
    pub(crate) fn reckon_old_fields(&mut self) {
        let alive: Vec<usize> = (0..self.settlers.len()).filter(|&i| self.settlers[i].alive && self.settlers[i].past.is_some()).collect();
        let battle = |t: &str| t.contains("Battle") || t.contains("battle") || t.contains("siege") || t.contains("Siege");
        for a in 0..alive.len() {
            for b in a + 1..alive.len() {
                let (i, j) = (alive[a], alive[b]);
                if self.recognized.contains(&(i, j)) { continue; }
                let (pi, pj) = (self.settlers[i].past.as_ref().unwrap(), self.settlers[j].past.as_ref().unwrap());
                let shared = pi.lines.iter().filter(|(t, e)| e.is_some() && battle(t))
                    .find_map(|(t, e)| pj.lines.iter().find(|(_, f)| f == e).map(|_| t.clone()));
                let Some(line) = shared else { continue };
                self.recognized.insert((i, j));
                // "the Battle of Reveredkeep Field": the battle's name from the line.
                let name = battle_name(&line);
                let (ni, nj) = (self.settlers[i].name.clone(), self.settlers[j].name.clone());
                let same = pi.people.is_some() && pi.people == pj.people;
                if same {
                    self.note(format!("{} and {} find they both stood at {}, and talk of it late into the night.", ni, nj, name));
                    self.like(i, j, 6);
                    self.feel(i, mind::Feel::Friend { with: nj.clone() });
                    self.feel(j, mind::Feel::Friend { with: ni.clone() });
                } else {
                    let soft = self.settlers[i].persona.facet(Facet::Vengefulness) < 40 && self.settlers[j].persona.facet(Facet::Vengefulness) < 40;
                    if soft {
                        self.note(format!("{} and {} find they stood on opposite sides at {}; they share a cup and let it lie.", ni, nj, name));
                        self.like(i, j, 2);
                    } else {
                        let line = format!("{} and {} find they stood on opposite sides at {}, and neither has forgotten it.", ni, nj, name);
                        self.note(line.clone());
                        let at = self.settlers[i].pos;
                        self.moment(format!("Old enemies at the fire"), line, format!("because both fought at {}", name), at);
                        self.like(i, j, -8);
                        self.feel(i, mind::Feel::Quarrel { with: nj.clone() });
                        self.feel(j, mind::Feel::Quarrel { with: ni.clone() });
                    }
                }
            }
        }
    }
}

/// "Fought at the Battle of Reveredkeep Field in 449 under Krachoulg." -> "the Battle of
/// Reveredkeep Field"; "Saw it begin with the siege of Brolmdustoor." -> "the siege of Brolmdustoor".
fn battle_name(line: &str) -> String {
    let lower = line.to_string();
    let start = lower.find("the Battle").or_else(|| lower.find("Battle")).or_else(|| lower.find("the siege")).or_else(|| lower.find("siege")).unwrap_or(0);
    let rest = &lower[start..];
    let end = [" in ", " under ", " (", ".", ";", ","].iter().filter_map(|s| rest.find(s)).min().unwrap_or(rest.len());
    let n = rest[..end].trim().to_string();
    if n.starts_with("the ") || n.starts_with("The ") { n } else { format!("the {}", n) }
}

#[cfg(test)]
mod tests {
    use super::battle_name;

    #[test]
    fn battle_names_come_out_of_lines() {
        assert_eq!(battle_name("Fought at the Battle of Reveredkeep Field in 449 under Krachoulg."), "the Battle of Reveredkeep Field");
        assert_eq!(battle_name("Saw it begin with the siege of Brolmdustoor."), "the siege of Brolmdustoor");
    }
}
