//! The camp's annals: its whole story as one page, legends-style.
//!
//! Dwarf Fortress writes the fortress into the world's legends; here the colony keeps its own
//! book: every moment in order with why it happened (and the world event behind it where there
//! is one), the works of its hands (artifacts and masterworks first), and its people, each with
//! who they were, what they did and what became of them. Written by `--sim-snapshot` as
//! `<prefix>_annals.html`; the page is plain HTML in the journal's parchment style.

use super::*;
use crate::history::world_state::WorldHistory;

fn esc(s: &str) -> String { s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;") }

impl Colony {
    /// What became of settler `i`, in a few words.
    pub fn fate(&self, i: usize) -> String {
        let s = &self.settlers[i];
        if s.alive { return "lives".into(); }
        if s.away_until > 0 { return if s.why.starts_with("Away at ") { s.why.replacen("Away at ", "away at ", 1) } else { "away hunting".into() }; }
        if s.mind.left && self.war_call.as_ref().map_or(false, |c| self.log.iter().any(|l| l.contains(&format!("{} fell in {}", s.name, c.war)))) {
            return format!("fell in {}", self.war_call.as_ref().unwrap().war);
        }
        if let Some(t) = self.snatched.iter().find(|t| t.who == i && t.home.is_none()) { return format!("carried off by {} on day {}", t.people, t.day); }
        if s.mind.left && s.visitor.is_some() && s.guest_until > 0 { return "moved on".into(); }
        if s.mind.left {
            return self.log.iter().find(|l| l.contains(&format!("casts {} out", s.name)) || (l.contains(&s.name) && l.contains("leaves the camp for good")))
                .map(|l| if l.contains("casts") { "cast out".to_string() } else { "walked away".to_string() }).unwrap_or_else(|| "left".into());
        }
        self.marks.iter().find(|m| m.kind == MarkKind::Grave && m.title == format!("The grave of {}", s.name))
            .and_then(|m| m.text.split(" Died ").nth(1).map(|r| format!("died {}", r.split('.').next().unwrap_or("").trim())))
            .unwrap_or_else(|| "died".into())
    }

    /// The annals as an HTML page.
    pub fn annals(&self, history: Option<&WorldHistory>) -> String {
        let name = self.name.clone().unwrap_or_else(|| "the Camp".into());
        let age = history.and_then(|h| crate::history::ages::current(h)).map(|a| format!(", in {}", a.name.replacen("The ", "the ", 1))).unwrap_or_default();
        let mut o = format!("<!doctype html><meta charset=\"utf-8\"><title>The Annals of {0}</title>\
<style>body{{background:#efe6d0;color:#382a20;font:18px/1.6 'IM Fell English',Georgia,serif;max-width:46rem;margin:3rem auto;padding:0 1rem}}\
h1,h2{{color:#9a2a1e;font-variant:small-caps;letter-spacing:.05em}}b{{color:#9a2a1e}}.why{{color:#806a52;font-size:.92em}}li{{margin:.3em 0}}</style>\
<h1>The Annals of {0}</h1><p class=\"why\">{1} days{2}. {3} came; {4} live.</p>",
            esc(&capital(&name)), self.clock.day(), esc(&age), self.settlers.len(), self.alive());
        // The days: every moment, with why.
        o.push_str("<h2>The Days</h2>");
        for m in &self.moments {
            o.push_str(&format!("<p><b>Day {}. {}.</b> {} <span class=\"why\">({})</span></p>", m.tick / TICKS_PER_DAY + 1, esc(&m.title), esc(&m.text), esc(&m.because)));
        }
        // The works: artifacts and the best of the rest.
        if !self.works.is_empty() {
            o.push_str("<h2>Works of Their Hands</h2><ul>");
            let mut works: Vec<&craft::Work> = self.works.iter().collect();
            works.sort_by_key(|w| (std::cmp::Reverse(w.quality), w.day));
            for w in works.iter().take(8) {
                let what = w.describe();
                let maker = self.settlers.get(w.maker).map(|s| s.name.clone()).unwrap_or_default();
                let sold = if w.traded && w.quality < 5 { " (sold to the caravans)" } else { "" };
                o.push_str(&format!("<li>{} by {}, day {}{}</li>", esc(&capital(&what)), esc(&maker), w.day, sold));
            }
            let fine = self.works.iter().filter(|w| w.quality >= 2).count();
            o.push_str(&format!("</ul><p class=\"why\">{} things made in all, {} of them fine or better.</p>", self.works.len(), fine));
        }
        // The hall's walls.
        if !self.engravings.is_empty() {
            o.push_str("<h2>The Walls of the Hall</h2><ul>");
            let mut es: Vec<&engrave::Engraving> = self.engravings.iter().collect();
            es.sort_by_key(|e| (std::cmp::Reverse(e.quality), e.day));
            for e in es.iter().take(8) {
                let maker = self.settlers.get(e.maker).map(|s| s.name.clone()).unwrap_or_default();
                o.push_str(&format!("<li>{} {}image of {}, by {}, day {}</li>", if craft::QUALITY[e.quality as usize].starts_with(|c: char| "aeiou".contains(c)) { "An" } else { "A" }, craft::QUALITY[e.quality as usize], esc(&e.image), esc(&maker), e.day));
            }
            o.push_str(&format!("</ul><p class=\"why\">{} engravings in all.</p>", self.engravings.len()));
        }
        // A lost thing of the world, found here.
        let old: Vec<String> = self.relic_line().into_iter().chain(self.treasures.iter().cloned()).collect();
        if !old.is_empty() {
            o.push_str("<h2>Things of the Old World</h2><ul>");
            for l in &old { o.push_str(&format!("<li>{}.</li>", esc(&capital(l)))); }
            o.push_str("</ul>");
        }
        // The world's peoples, and why they think what they do (`regard.rs`).
        if !self.regards.is_empty() {
            o.push_str("<h2>The Camp and the World</h2><ul>");
            for r in &self.regards {
                let why: Vec<String> = r.causes.iter().map(|c| format!("{} ({:+})", c.text, c.delta)).collect();
                o.push_str(&format!("<li>{} {}: the camp {}.</li>", esc(&capital(&r.people)), r.word(), esc(&crate::persona::list(&why))));
            }
            o.push_str("</ul>");
        }
        // News from the world, as each teller told it, and how the others tell it (`news.rs`).
        if !self.heard.is_empty() {
            o.push_str("<h2>News from the World</h2><ul>");
            for hd in &self.heard {
                let t = &hd.told;
                let mut how: Vec<String> = Vec::new();
                if !t.as_told().is_empty() { how.push(t.as_told()); }
                for (f, name, g) in &t.others {
                    if Some(*f) != t.teller && t.gloss.as_deref() != Some(g.as_str()) { how.push(format!("{} {} it {}", name, if name.ends_with('s') { "call" } else { "calls" }, g)); }
                }
                let how = if how.is_empty() { String::new() } else { format!(" <span class=\"why\">({})</span>", esc(&how.join("; "))) };
                o.push_str(&format!("<li>Day {}, from {}: {}.{}</li>", hd.day, esc(&hd.from), esc(&capital(&t.line())), how));
            }
            o.push_str("</ul>");
        }
        // The people.
        o.push_str("<h2>The People</h2>");
        for (i, s) in self.settlers.iter().enumerate() {
            let past = s.past.as_ref().map(|p| format!("{}, {}", p.age, p.calling)).unwrap_or_else(|| "a wanderer".into());
            let mut bits: Vec<String> = Vec::new();
            if let Some(v) = &s.visitor { bits.push(format!("came as {}{}", v, if s.guest_until == 0 { " and stayed" } else { "" })); }
            if let Some(o) = &s.office { bits.push(o.to_lowercase()); }
            if let Some(r) = s.role { bits.push(format!("the camp's {}", ROLES[r])); }
            for d in &s.deeds { bits.push(d.clone()); }
            if let Some(p) = self.pet_of(i) { bits.push(p); }
            if let Some(f) = self.family_of(i) { bits.push(f); }
            if let Some(g) = self.guild_of(i) { bits.push(g.replacen("Sworn", "sworn", 1)); }
            if let Some(v) = self.vow_of(i) { bits.push(v.replacen("Sworn", "sworn", 1).replacen("Kept", "kept", 1)); }
            if let Some(d) = self.dream_line(i) { bits.push(d.replacen("Dreams", "dreamt", 1).replacen("Realized", "realized", 1)); }
            if let Some(a) = self.armour_of(i) { bits.push(format!("wore {}", a.kind)); }
            if !s.made.is_empty() { bits.push(format!("made {} {}", s.made.len(), if s.made.len() == 1 { "thing" } else { "things" })); }
            if !s.wounds.is_empty() { bits.push(format!("bore {}", crate::persona::list(&s.wounds.iter().map(|w| w.word()).collect::<Vec<_>>()))); }
            if self.cursed.contains(&i) { bits.push("carries the curse of the full moon".into()); }
            let who = s.persona.describe(&s.name, s.past.as_ref().map_or(30, |p| p.age));
            let character = who.get(2).cloned().unwrap_or_default();
            o.push_str(&format!("<p><b>{}</b>, {}. {} <span class=\"why\">{}{}.</span></p>",
                esc(&s.name), esc(&past), esc(&character), esc(&capital(&self.fate(i))), if bits.is_empty() { String::new() } else { format!("; {}", esc(&bits.join("; "))) }));
        }
        o
    }
}

fn capital(s: &str) -> String { let mut c = s.chars(); c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default() }
