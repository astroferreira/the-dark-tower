//! Haunts: the places a settler makes their own. A need met out of doors (prayer, rest, time to
//! think, a walk alone, whittling, the old ways) is met again where it was first met, and on the
//! third visit they leave a mark there: a cairn on the rise where they pray, a bench by the water
//! where they rest, a carved post by the fire where they whittle, a standing stone of their
//! people. Others come to use it (the devout pray at another's cairn when they have none), so a
//! camp grows its own small shrines and benches out of the people who live in it, and two camps
//! never grow the same ones.

use super::*;
use super::needs::Need;

/// A settler's place for a need, and the mark they left there.
#[derive(Clone, Debug)]
pub struct Haunt {
    pub who: usize,
    pub need: Need,
    pub at: Pos,
    pub visits: u32,
    /// The mark's index in `Colony::marks`, once made.
    pub mark: Option<usize>,
}

/// The needs whose place a settler keeps.
pub(crate) fn kept(n: Need) -> bool {
    matches!(n, Need::Pray | Need::TakeItEasy | Need::ThinkAbstractly | Need::Wander | Need::Craft | Need::BeCreative | Need::Tradition)
}

impl Colony {
    /// Settler `i`'s place for `need`, if they have one that can still be used.
    pub(crate) fn haunt_of(&self, i: usize, need: Need) -> Option<&Haunt> {
        self.haunts.iter().find(|h| h.who == i && h.need == need && !self.marked(h.at, true))
    }

    /// Another's marked place for `need` within `r` cells of the camp that settler `i` might share
    /// (the devout pray at a cairn another raised): by a hash, one day in two.
    pub(crate) fn shared_haunt(&self, i: usize, need: Need, r: i32) -> Option<(&Haunt, String)> {
        let h = crate::history::settlers::hash_pub(self.seed ^ i as u64, self.clock.day() ^ 0x5EA7);
        if h % 2 == 1 { return None; }
        self.haunts.iter().filter(|x| x.who != i && x.need == need && x.mark.is_some() && !self.marked(x.at, true)
            && super::needs::cheb(x.at, self.camp) <= r).min_by_key(|x| (super::needs::cheb(x.at, self.settlers[i].pos), x.who))
            .map(|x| (x, x.mark.and_then(|m| self.marks.get(m)).map(|m| m.title.clone()).unwrap_or_default()))
    }

    /// A need met out of doors at `at`: count the visit to the settler's place, and on the third
    /// leave a mark there.
    pub(crate) fn visit_haunt(&mut self, i: usize, need: Need, at: Pos) {
        if !kept(need) || self.in_hut(at) || super::needs::cheb(at, self.camp) <= 1 && need != Need::Craft && need != Need::BeCreative { return; }
        let k = match self.haunts.iter().position(|h| h.who == i && h.need == need) {
            Some(k) => k,
            None => {
                // Another's mark of the same kind close by: it becomes theirs too (a second
                // settler adds a stone to the cairn on the rise, rather than raising another).
                let same = self.haunts.iter().find(|h| h.need == need && h.mark.is_some() && super::needs::cheb(h.at, at) <= 5).map(|h| (h.at, h.mark, h.who));
                // (Another's place not yet marked: theirs is shared from the start.)
                let near = self.haunts.iter().find(|h| h.need == need && super::needs::cheb(h.at, at) <= 5).map(|h| h.at);
                if same.is_none() {
                    if let Some(at2) = near { self.haunts.push(Haunt { who: i, need, at: at2, visits: 0, mark: None }); return; }
                }
                if let Some((at2, mark, owner)) = same {
                    self.haunts.push(Haunt { who: i, need, at: at2, visits: 3, mark });
                    if need == Need::Pray {
                        let (a, b) = (self.settlers[i].name.clone(), self.settlers[owner].name.clone());
                        self.note(format!("{} adds a stone to {}'s cairn, and prays there too.", a, b));
                        if let Some(m) = mark.and_then(|m| self.marks.get_mut(m)) { m.text.push_str(&format!(" {} added a stone on day {}.", a, self.clock.day())); }
                    }
                    self.like(i, owner, 1);
                    return;
                }
                self.haunts.push(Haunt { who: i, need, at, visits: 0, mark: None }); self.haunts.len() - 1
            }
        };
        // (A visit counts at the place itself: a few cells off is still it.)
        if super::needs::cheb(self.haunts[k].at, at) > 3 { return; }
        self.haunts[k].visits += 1;
        if self.haunts[k].visits != 3 || self.haunts[k].mark.is_some() { return; }
        // Not on a work, a grave or another's mark; not in water.
        if self.marks.iter().any(|m| super::needs::cheb(m.at, at) == 0) || self.built_near(at) || !nav::passable(&self.map, at) { return; }
        let s = &self.settlers[i];
        let (name, they, their) = (s.name.clone(), if s.persona.female { "she" } else { "he" }, if s.persona.female { "her" } else { "his" });
        let god = s.past.as_ref().and_then(|p| p.faith.as_ref()).map(|f| f.1.clone()).unwrap_or_else(|| "the gods".into());
        let day = self.clock.day();
        let (kind, title, text, line) = match need {
            Need::Pray => (MarkKind::Cairn, format!("{}'s cairn", name),
                format!("A cairn of fieldstones raised on day {} by {}, who prays to {} here.", day, name, god),
                format!("{} raises a cairn of fieldstones on the rise at {},{}, where {} prays to {}.", name, at.0, at.1, they, god)),
            Need::TakeItEasy => (MarkKind::Bench, format!("{}'s bench", name),
                format!("A bench of split logs set on day {} by {}, who comes here to rest.", day, name),
                format!("{} sets a bench of split logs {}, where {} likes to rest.", name, self.place_word(at), they)),
            Need::ThinkAbstractly => (MarkKind::Bench, format!("{}'s seat", name),
                format!("Flat stones laid as a seat on day {} by {}, who sits here alone with {} thoughts.", day, name, their),
                format!("{} lays flat stones as a seat on the rise at {},{}, where {} goes to think.", name, at.0, at.1, they)),
            Need::Wander => (MarkKind::Cairn, format!("{}'s waymark", name),
                format!("A small cairn left on day {} by {} at the far end of {} walks.", day, name, their),
                format!("{} leaves a small cairn at {},{}, at the far end of {} walks.", name, at.0, at.1, their)),
            Need::Craft | Need::BeCreative => (MarkKind::Carving, format!("{}'s carved post", name),
                format!("A post carved all over with little figures by {}, begun on day {} in the evenings by the fire.", name, day),
                format!("{} sets up a post by the fire and carves it all over with little figures.", name)),
            Need::Tradition => (MarkKind::Stone, format!("The stone of {}'s people", name),
                format!("A standing stone set on day {} by {}, where the old tales of {} people are told.", day, name, their),
                format!("{} sets up a standing stone {}, as {} people do, and tells the old tales there.", name, self.place_word(at), their)),
            _ => return,
        };
        self.marks.push(ColonyMark { at, kind, title, text, day });
        // (Everyone whose place it is shares the mark.)
        let m = self.marks.len() - 1;
        for h in self.haunts.iter_mut().filter(|h| h.need == need && super::needs::cheb(h.at, at) <= 1) { h.mark = Some(m); }
        self.note(line);
    }

    /// The marks the camp's people made for themselves.
    pub fn haunt_marks(&self) -> usize { self.haunts.iter().filter(|h| h.mark.is_some()).count() }
}
