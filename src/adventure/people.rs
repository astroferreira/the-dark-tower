//! Townsfolk are people, not roles (DF's rolled individuals, `persona.rs`): each one's persona
//! is rolled from their name and town (the same roller as the history's figures, so a priest is
//! as particular as a king), their temper read off its strongest facets, and they remember the
//! adventurer (`Met`: how often they met, what was done for them, what was done against them).
//! Their greeting, their terms (prices, what a timid priest asks of a famous hero) and their
//! refusals follow; a quest giver greets the hero by the deed they did.

use super::actor::{Npc, Role};
use crate::persona::{Facet, Persona};

/// What someone remembers of the adventurer.
#[derive(Clone, Debug, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct Met {
    pub times: u32,
    /// The game turn of the last meeting.
    pub last: u64,
    /// Deeds done for them (quests' titles), newest last.
    pub helped: Vec<String>,
    /// Wrongs done to them or theirs (regard).
    pub wronged: u32,
}

/// The temper a persona shows a stranger, by its strongest facet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Temper { Kind, Greedy, Gruff, Timid, Cheerful, Gloomy, Proud, Curious, Plain }

impl Temper {
    pub fn word(self) -> &'static str {
        match self { Temper::Kind => "kindly", Temper::Greedy => "grasping", Temper::Gruff => "gruff", Temper::Timid => "timid", Temper::Cheerful => "cheerful", Temper::Gloomy => "gloomy", Temper::Proud => "proud", Temper::Curious => "curious", Temper::Plain => "plain-spoken" }
    }
}

/// A townsperson's persona (pure: the same person every time).
pub fn persona(n: &Npc) -> Persona {
    let mut p = Persona::roll(&n.race, None, crate::persona::seed_of(&n.name, 0x9E09 ^ n.home as u64));
    p.female = n.female;
    p
}

/// Their temper: the facet furthest from ordinary among those a stranger meets.
pub fn temper(p: &Persona) -> Temper {
    let c = [
        (Temper::Kind, (p.facet(Facet::Friendliness) as i32 + p.facet(Facet::Altruism) as i32) / 2 - 50),
        (Temper::Greedy, p.facet(Facet::Greed) as i32 - 50),
        (Temper::Gruff, (p.facet(Facet::Anger) as i32 + 100 - p.facet(Facet::Politeness) as i32) / 2 - 50),
        (Temper::Timid, (p.facet(Facet::Anxiety) as i32 + p.facet(Facet::Bashfulness) as i32) / 2 - 50),
        (Temper::Cheerful, p.facet(Facet::Cheer) as i32 - 50),
        (Temper::Gloomy, p.facet(Facet::Gloom) as i32 - 50),
        (Temper::Proud, (p.facet(Facet::Pride) as i32 + p.facet(Facet::Vanity) as i32) / 2 - 50),
        (Temper::Curious, p.facet(Facet::Curiosity) as i32 - 50),
    ];
    let best = c.iter().max_by_key(|x| x.1).unwrap();
    if best.1 < 12 { Temper::Plain } else { best.0 }
}

/// What they charge over the plain price (and pay under it): by temper and what they remember.
pub fn price_factor(t: Temper, m: &Met) -> f32 {
    let base = match t { Temper::Greedy => 1.2, Temper::Kind => 0.9, Temper::Proud => 1.1, Temper::Timid => 0.95, _ => 1.0 };
    let friend = if m.helped.is_empty() { 1.0 } else { 0.9 };
    base * friend * (1.0 + 0.25 * m.wronged.min(4) as f32)
}

/// How they greet the adventurer: their role's welcome in their temper's words, and what they
/// remember of them.
pub fn greeting(n: &Npc, t: Temper, m: &Met, hero: &str, town: &str, fame: usize, turn: u64) -> String {
    let job = match n.role {
        Role::Priest => format!("the temple of {}", n.of),
        Role::Smith => "my forge".into(),
        Role::Trader => "my shop".into(),
        Role::Innkeeper => "my inn".into(),
        Role::Lord => format!("the hall of {}", town),
        Role::Guard => "the watch".into(),
        Role::Sage => "my books".into(),
        Role::Townsfolk => town.into(),
    };
    // What they remember first.
    if m.wronged > 0 {
        return match t {
            Temper::Timid => format!("{} backs away. \"You... what do you want? Take it and go.\"", n.name),
            Temper::Gruff | Temper::Proud => format!("\"You have some nerve showing your face at {}, {}. Say your business.\"", job, hero),
            _ => format!("\"I know what you did, {}. Say what you came for and be gone.\"", hero),
        };
    }
    if let Some(d) = m.helped.last() {
        let d = d.trim_start_matches("Slay ").trim_start_matches("Bring ");
        return match t {
            Temper::Cheerful | Temper::Kind => format!("\"{}! The one who saw to {}! Come in, come in, you are welcome here always.\"", hero, d.to_lowercase()),
            Temper::Gruff => format!("\"{}. You did right by us with {}. I don't forget that.\"", hero, d.to_lowercase()),
            Temper::Greedy => format!("\"Ah, {}, who handled {} so well. A friend's price for you, almost.\"", hero, d.to_lowercase()),
            _ => format!("\"Welcome back, {}. We still speak of {}.\"", hero, d.to_lowercase()),
        };
    }
    if m.times > 0 && turn.saturating_sub(m.last) > 30 * super::land::DAY {
        return format!("\"{}, is it? It has been a long while. Back to {} at last.\"", hero, job);
    }
    if m.times > 0 {
        return match t {
            Temper::Gruff => format!("\"You again, {}. What is it now?\"", hero),
            Temper::Cheerful => format!("\"Back again, {}! Good, good.\"", hero),
            Temper::Timid => format!("\"Oh. Hello again, {}.\"", hero),
            _ => format!("\"Back again, {}.\"", hero),
        };
    }
    // A stranger; a name others sing of is known already.
    if fame >= 3 {
        return match t {
            Temper::Timid => format!("{} goes pale. \"You are {}. The songs... how can I serve you?\"", n.name, hero),
            Temper::Proud => format!("\"So you are the {} they sing of. Smaller than I thought. Welcome to {}.\"", hero, job),
            Temper::Greedy => format!("\"{}, the famous! A famous purse too, I hope. Welcome to {}.\"", hero, job),
            _ => format!("\"{}? The one in the songs? Welcome to {}, and well met.\"", hero, job),
        };
    }
    match (t, n.role) {
        (Temper::Kind, _) => format!("\"Come in, traveller, you look worn. Welcome to {}. How can I help?\"", job),
        (Temper::Greedy, _) => format!("\"Coin first, talk after. Welcome to {}.\"", job),
        (Temper::Gruff, _) => format!("\"What do you want? This is {}, not a tavern.\"", job),
        (Temper::Timid, _) => format!("{} startles. \"Oh! A stranger. W-welcome to {}.\"", n.name, job),
        (Temper::Cheerful, _) => format!("\"A new face! Welcome, welcome to {}!\"", job),
        (Temper::Gloomy, _) => format!("\"Another one come to {}. They never last. What do you need?\"", job),
        (Temper::Proud, _) => format!("\"You stand in {}. Mind your manners, stranger.\"", job),
        (Temper::Curious, _) => format!("\"A traveller! Where from? What have you seen? Welcome to {}.\"", job),
        (Temper::Plain, Role::Priest) => format!("\"Welcome, {}, to the temple of {}. The god's light on you.\"", hero, n.of),
        (Temper::Plain, _) => format!("\"Good day, {}. Welcome to {}.\"", hero, job),
    }
}

/// How they say no to a want they cannot meet (no work, too poor): in their temper's words.
pub fn refusal(t: Temper) -> &'static str {
    match t {
        Temper::Gruff => "Nothing. Go bother someone else.",
        Temper::Kind => "I am sorry, I have nothing for you now. Come back later, and take care on the road.",
        Temper::Greedy => "Nothing that pays, not for you, not yet.",
        Temper::Timid => "I... no, nothing, I'm sorry.",
        Temper::Gloomy => "Nothing. There is never anything.",
        Temper::Proud => "When I have work worthy of you, you will hear of it. Not before.",
        Temper::Cheerful => "Nothing today, friend! Tomorrow, who knows?",
        Temper::Curious => "Nothing now. But tell me what you find out there, will you?",
        Temper::Plain => "I have no work for you now. Come back when you are stronger, or when the land is worse.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn npc(name: &str, home: u32, role: Role) -> Npc { Npc { name: name.into(), role, x: 0, y: 0, z: 0, post: (0, 0), race: "human".into(), female: false, of: "Balorn".into(), home, met: Met::default() } }

    /// People differ: of twelve priests not all greet alike in words or terms, and a priest
    /// remembers who did a deed for them.
    #[test]
    fn people_differ_and_remember() {
        let priests: Vec<Npc> = (0..12).map(|k| npc(&format!("Priest{}", k), k + 1, Role::Priest)).collect();
        let lines: Vec<String> = priests.iter().map(|n| greeting(n, temper(&persona(n)), &Met::default(), "Tess", "Greenburg", 0, 0)).collect();
        let terms: Vec<i32> = priests.iter().map(|n| (price_factor(temper(&persona(n)), &Met::default()) * 100.0) as i32).collect();
        let mut l2 = lines.clone(); l2.sort(); l2.dedup();
        let mut t2 = terms.clone(); t2.sort(); t2.dedup();
        assert!(l2.len() >= 3, "priests all alike: {:?}", lines);
        assert!(t2.len() >= 2, "priests' terms all alike: {:?}", terms);
        let n = &priests[0];
        let m = Met { times: 1, last: 0, helped: vec!["Slay the beast of the Bat Pit".into()], wronged: 0 };
        let g = greeting(n, temper(&persona(n)), &m, "Tess", "Greenburg", 0, 10);
        assert!(g.contains("Tess") && g.contains("beast of the bat pit"), "{}", g);
    }
}
