//! A voice for the log (card adv-voice): arrival paragraphs for each new region and place, a
//! town's mood when one walks in (war, plague, a feast day, its regard, what travellers from the
//! next town say), and fight lines that vary. Repeated lines are collapsed by `Game::say` ("x3").
//!
//! Every sentence is picked by a hash of the place and the day, so the same walk reads the same
//! (deterministic), but two lands never read alike: the land's kind and season, the hour and the
//! weather (`weather.rs`), what the eye falls on (towers and wonders seen far off, a river, a road,
//! a town near), and what is wrong with it (danger, the Shadow, old battles).

use super::game::Game;
use super::site::{SiteKind, SiteSpec};
use super::surface::{hash, Kind};
use super::weather::Weather;

/// Pick one of `v` by a hash.
fn pick<'a>(v: &[&'a str], h: u64) -> &'a str { v[(h % v.len() as u64) as usize] }

fn land_line(kind: Kind, h: u64) -> &'static str {
    match kind {
        Kind::Plains => pick(&["Open grass runs to the edge of sight, rippling where the wind walks on it", "The land lies flat and wide, grass to the knee", "Meadows roll away in long low swells"], h),
        Kind::Forest => pick(&["The woods close in, old trees leaning together over the path", "Oak and beech stand thick here, their roots knotted over the stones", "The forest is quiet in the way of forests that are not empty"], h),
        Kind::Jungle => pick(&["The jungle steams, every leaf as big as a shield", "Vines hang from the canopy, and something calls and is answered", "Green on green on green: the jungle swallows the light"], h),
        Kind::Taiga => pick(&["Pines march up every slope, black against the sky", "The pine forest smells of resin and cold", "Needles hush every step under the dark firs"], h),
        Kind::Desert => pick(&["Sand, and more sand, shaped by a wind that never stops", "The desert shimmers; the horizon will not keep still", "Stones crack in the heat, and nothing grows that does not bite"], h),
        Kind::Snow => pick(&["Snowfields stretch white and blinding to the hills", "The cold here is a living thing, and it does not like you", "Ice creaks underfoot under a sky the colour of iron"], h),
        Kind::Swamp => pick(&["The marsh sucks at every step, and the pools stare back", "Reeds and black water, and the smell of things rotting slowly", "Mist rises off the bog, and the ground is never quite ground"], h),
        Kind::Mountain => pick(&["The mountains rise in walls of grey rock, the passes narrow between them", "Scree slides under your boots; above, the peaks hold the snow", "The air is thin and sharp up here among the crags"], h),
        Kind::Savanna => pick(&["Dry grass and lone flat-topped trees, and the bones of what the lions left", "The plains are tawny and wide, herds dark on the far rises", "Thorn and dry grass under a huge sky"], h),
        Kind::Waste => pick(&["A blasted waste: ash, black glass, and nothing alive that you would want to meet", "The land here is burned to its bones", "Grey dust and broken stone, as if the world were scraped raw"], h),
        Kind::Lake => pick(&["Lakes lie between the low hills like dropped mirrors", "Water everywhere: lakes, reeds, and wading birds", "The lake country is green and wet, and the fish jump"], h),
        Kind::Sea => pick(&["The sea is loud on the shore, gulls crying over it", "Salt wind off the water, and the long grey line of the sea", "Waves break white on the rocks"], h),
    }
}

fn season_line(s: u8, kind: Kind, h: u64) -> &'static str {
    let barren = matches!(kind, Kind::Waste | Kind::Desert | Kind::Snow | Kind::Mountain | Kind::Sea);
    match (s, barren) {
        (0, false) => pick(&["Everything is green with the spring", "Spring has come: new leaves, mud and birdsong", "The first flowers are out"], h),
        (1, false) => pick(&["Summer lies heavy on it, loud with insects", "It is high summer, and the air is thick and warm", "The summer sun has everything drowsing"], h),
        (2, false) => pick(&["The leaves are turning gold and red", "Autumn: long light, and the smell of leaves rotting", "The year is going over into autumn"], h),
        (_, false) => pick(&["Winter has stripped it bare and silent", "Frost holds everything in its fist", "It is the dead of winter"], h),
        (0, true) => pick(&["Spring makes no difference here", "Spring passes this land by"], h),
        (1, true) => pick(&["The summer heat beats down on it", "Summer makes it an anvil"], h),
        (2, true) => pick(&["An autumn wind scours it", "The autumn gales come through unhindered"], h),
        (_, true) => pick(&["Winter makes it worse", "In winter it is a killing place"], h),
    }
}

fn sky(w: Weather, night: bool, h: u64) -> &'static str {
    match (w, night) {
        (Weather::Fog, _) => pick(&["Fog lies thick, and shapes come and go in it", "A white fog has come down; you can hardly see your hand", "The fog muffles everything"], h),
        (Weather::Rain, _) => pick(&["A cold rain is falling", "Rain hisses on the ground and runs in every rut", "It is raining, steady and grey"], h),
        (Weather::Storm, _) => pick(&["A storm breaks overhead: thunder, and rain driven sideways", "Lightning cracks the sky open, and the rain comes down in sheets", "The wind howls and the storm beats at the land"], h),
        (Weather::Snow, _) => pick(&["Snow is falling, soft and endless", "Snow swirls on the wind and covers every track", "Big flakes drift down out of a white sky"], h),
        (Weather::Cloud, false) => pick(&["Low cloud hangs over everything", "The sky is grey and close", "Clouds race overhead"], h),
        (Weather::Cloud, true) => pick(&["No star shows through the cloud", "It is a black night, the clouds low"], h),
        (Weather::Clear, false) => pick(&["The sky is clear and the light is good", "The sun is out", "A clear day, and the air sharp"], h),
        (Weather::Clear, true) => pick(&["The stars are out, cold and many", "The night is clear and the moon throws shadows", "Stars wheel overhead"], h),
    }
}

impl Game {
    /// The paragraph told on first coming into a region (tile t).
    pub fn arrival(&self, t: (usize, usize)) -> String {
        let w = self.world.w;
        let k = t.1 * w + t.0;
        let land = super::surface::Land { info: &self.world, atlas: &self.atlas };
        let kind = if self.world.land.get(k).copied().unwrap_or(false) { land.kind(k) } else { Kind::Sea };
        let day = self.turn / super::land::DAY;
        let h = hash(self.seed ^ 0x9805E, t.0 as i64, t.1 as i64, day);
        let night = self.night();
        let weather = self.weather_at(t);
        let mut s = format!("{}. {}. {}.", land_line(kind, h), season_line(self.season(), kind, h >> 8), sky(weather, night, h >> 16));
        // What the eye falls on.
        if let Some(far) = self.far_seen.first() { s.push_str(&format!(" Far off you make out {}.", far)); }
        else if let Some(wd) = self.world.wonders.get(&k) { s.push_str(&format!(" Something here is older than the roads: {}.", wd.name)); }
        else if self.world.river.get(k).copied().unwrap_or(false) { s.push_str(pick(&[" A river runs through it, quick and brown.", " You hear a river before you see it.", " A river winds across the land, and the path follows it."], h >> 24)); }
        else if self.world.road.get(k).copied().unwrap_or(false) { s.push_str(pick(&[" A road runs through, rutted by carts.", " An old road crosses the land, its stones heaved by frost.", " The road here is well walked."], h >> 24)); }
        // What is wrong with it.
        let danger = self.world.danger.get(k).copied().unwrap_or(0);
        let shadow = self.world.shadow.get(k).copied().unwrap_or(0.0);
        if shadow > 0.4 { s.push_str(pick(&[" The land is sick: grey grass, black water, no birdsong.", " The Shadow has been here. Nothing grows right."], h >> 32)); }
        else if danger > 150 { s.push_str(pick(&[" No one lives out here, and it shows.", " This is far from any watch or wall; keep your blade loose.", " Things hunt in country like this."], h >> 32)); }
        else if self.sites.iter().any(|x| x.kind == SiteKind::Town && super::world::dist(x.tile, t, w) <= 1 && x.tile != t) { s.push_str(" Smoke rises from chimneys not far off."); }
        s
    }

    /// What a town feels like as one walks in: war, sickness, a feast day, how they look at the
    /// hero, and what travellers from the next town say.
    pub fn town_mood(&self, town: &SiteSpec) -> String {
        let day = self.turn / super::land::DAY;
        let h = hash(self.seed ^ 0x700D, town.id as i64, 0, day);
        let mut parts: Vec<String> = Vec::new();
        // At war (the history's living wars on its people).
        let war = self.history.as_ref().and_then(|hist| {
            let f = town.settlement.and_then(|sid| hist.settlements.values().find(|s| s.id.0 == sid).map(|s| s.faction))?;
            hist.wars.values().filter(|w| w.ended.is_none() && (w.aggressors.contains(&f) || w.defenders.contains(&f))).map(|w| w.name.clone()).next()
        });
        let sick = self.quests.iter().any(|q| q.town == town.id && q.state == super::quest::State::Open && matches!(&q.goal, super::quest::Goal::Tale(t) if t.kind == super::tales::TaleKind::Plague));
        if let Some(wn) = war { parts.push(format!("The walls of {} are manned and the gate watched: they are at war ({}).", town.name, wn)); }
        else if sick { parts.push(format!("A sickness is in {}: doors marked with chalk, and the priest's bell going.", town.name)); }
        else if (day + town.id as u64) % 9 == 0 { parts.push(pick(&["It is a feast day: flags on the houses, music from the inn, children underfoot.", "Market day: the square is loud with traders and their goods.", "A wedding fills the square with song."], h).to_string()); }
        else if let Some(n) = town.notes.last() { parts.push(format!("They still talk of it here: {}", n)); }
        else { parts.push(pick(&["The town goes about its business.", "Smoke from chimneys, a dog barking, a cart in the gate: an ordinary day.", "Folk look up as you pass, then back to their work."], h).to_string()); }
        let r = self.regard_of(town.id);
        if r >= 30 { parts.push("People nod to you in the street; someone calls your name.".into()); }
        else if r <= -20 { parts.push("Talk stops as you pass. Someone spits.".into()); }
        // Rumour from the next town over.
        let w = self.world.w;
        if let Some((o, n)) = self.sites.iter().filter(|o| o.kind == SiteKind::Town && o.id != town.id && super::world::dist(o.tile, town.tile, w) <= 8).filter_map(|o| o.notes.last().map(|n| (o, n))).min_by_key(|(o, _)| (super::world::dist(o.tile, town.tile, w), o.id)) {
            parts.push(format!("Travellers from {} say: {}", o.name, n));
        }
        parts.join(" ")
    }

    /// The first time into a place: what it is like.
    pub fn place_arrival(&self, spec: &SiteSpec) -> String {
        let h = hash(self.seed ^ 0x91ACE, spec.id as i64, 0, 0);
        let tier = match spec.tier { 0..=1 => "", 2 => " Something bigger than rats lives here.", 3 => " Claw marks score the walls, deep ones.", _ => " The silence here is the silence of a place where nothing dares to make a sound." };
        let first = match spec.kind {
            SiteKind::Cave => pick(&["The air is cold and smells of wet stone and animal.", "Water drips somewhere in the dark, slow as a heartbeat.", "The cave swallows your light a few steps in."], h),
            SiteKind::Lair => pick(&["Bones crunch underfoot at the mouth. Something lives here, and eats well.", "The stink of the beast hits you first."], h),
            SiteKind::Mine => pick(&["Old timbers groan over the gallery; rails run into the black.", "Picks rusted where they fell. The miners left in a hurry."], h),
            SiteKind::Ruin => pick(&["Broken walls, charred beams, and weeds in the hearths.", "Whoever lived here did not leave by choice."], h),
            SiteKind::Tomb => pick(&["Dust, and the dry sweet smell of old death.", "The dead are laid in rows, and not all of them are still."], h),
            SiteKind::Temple => pick(&["Incense still clings to the stones, though no priest has lit any in years.", "The god's face looks down from the wall, its eyes put out."], h),
            SiteKind::Shrine => pick(&["A small holy place, worn smooth by knees.", "Candles burn here that no one lit."], h),
            SiteKind::Castle => pick(&["Banners hang in rags from the walls.", "A keep of old stone, its gate long broken."], h),
            SiteKind::Labyrinth => pick(&["Passages turn and turn again; the walls are carved with a warning in a dead tongue.", "You are not the first to come in here. You may be the first to come out."], h),
            SiteKind::Camp => pick(&["A war camp: tents, a stockade, and the smell of smoke and leather.", "Banners on poles, fires burning, and voices that stop when you come in."], h),
            SiteKind::Halls => pick(&["Halls cut by dwarves, square and vast, the runes still sharp.", "Pillars march away into the dark, each one carved with a face."], h),
            SiteKind::DarkFortress => pick(&["Black stone, cold to the touch, and a weight on the air like a held breath.", "The Shadow's own house. Your torch burns small here."], h),
            SiteKind::Cellar => pick(&["Damp steps, and below them candles, and chanting that stops.", "A cellar where no honest work is done."], h),
            _ => "",
        };
        format!("{}{}", first, tier)
    }

    /// A fight line, varied (`what` the word for the blow, by the attacker's look).
    pub fn blow_word(look: &str, roll: u64) -> &'static str {
        match look {
            l if l.starts_with("folk") => pick(&["hits you", "strikes you", "lands a blow on you", "catches you", "batters you"], roll),
            "rat" | "giant rat" | "wolf" | "bear" | "wild boar" | "lion" | "leopard" | "white bear" | "crocodile" | "werewolf" => pick(&["bites you", "sinks its teeth into you", "tears at you", "snaps and connects"], roll),
            "spider" | "giant spider" => pick(&["bites you with its fangs", "stabs at you with its fangs"], roll),
            "bat" | "vulture" => pick(&["bites you", "rakes you", "dives at you"], roll),
            "adder" | "scorpion" | "giant scorpion" => pick(&["strikes at you", "stings you"], roll),
            _ => pick(&["hits you", "strikes you", "mauls you"], roll),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adventure::game::Tone;
    use crate::biomes::ExtendedBiome;

    /// Five lands of five kinds read as five different paragraphs, each of a few sentences.
    #[test]
    fn five_lands_read_five_ways() {
        let mut g = crate::adventure::land::tests::game();
        let w = g.world.w;
        let lands = [((5, 1), ExtendedBiome::Desert), ((6, 1), ExtendedBiome::Tundra), ((7, 1), ExtendedBiome::Marsh), ((5, 4), ExtendedBiome::TemperateForest), ((7, 4), ExtendedBiome::RazorPeaks)];
        for ((x, y), b) in lands { g.world.biome[y * w + x] = b; }
        let paras: Vec<String> = lands.iter().map(|(t, _)| g.arrival(*t)).collect();
        for p in &paras { assert!(p.matches(". ").count() >= 1 && p.len() > 80, "too thin: {}", p); }
        let mut uniq = paras.clone();
        uniq.sort(); uniq.dedup();
        assert_eq!(uniq.len(), 5, "{:#?}", paras);
        // The same land read twice on the same day reads the same.
        assert_eq!(g.arrival(lands[0].0), paras[0]);
    }

    /// A line said again is counted, not repeated; a long play has no line four times in a row.
    #[test]
    fn the_log_does_not_stutter() {
        let mut g = crate::adventure::land::tests::game();
        let n = g.log.len();
        for _ in 0..10 { g.say(Tone::Info, "You are hungry."); }
        assert_eq!(g.log.len(), n + 1);
        assert_eq!(g.log.last().unwrap().n, 9);
        let mut g = crate::adventure::land::tests::game();
        let mut all: Vec<String> = Vec::new();
        let mut b = crate::adventure::bot::Bot::default();
        for _ in 0..4000 {
            let before = g.log.len();
            if !b.step(&mut g) { g.act(crate::adventure::game::Action::Wait); }
            for l in g.log[before.min(g.log.len())..].iter() { all.push(l.text.clone()); }
        }
        assert!(all.len() > 200, "a short log: {}", all.len());
        for win in all.windows(4) { assert!(!(win[0] == win[1] && win[1] == win[2] && win[2] == win[3]), "said four times running: {}", win[0]); }
    }
}
