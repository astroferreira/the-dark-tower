//! Weather and the cold on the land (card adv-land-weather).
//!
//! The weather is a deterministic function of the region (3 x 3 tiles), the half-day and the
//! land's climate (`Game::weather_at`): the tile's temperature shifted by the season (from the
//! history's calendar when the living history runs, else from the clock) and the night, its
//! moisture for rain or snow, low wet country for fog. It changes what one sees (fog halves
//! sight, storms and snow cut it, rain a little: `sight_factor`), the going (rain turns earth and
//! grass to mud), what burns (rain puts fires out), and the cold: at night, or by day in a hard
//! frost, on cold land, without furs in the pack or a campfire within three cells, the hero stops
//! healing and loses life (down to a quarter of it: numb, not dead). A torch makes a campfire
//! (`Action::Camp`, B); resting by one mends twice as fast. In snow, a creature's fresh tracks
//! are seen and told.

use super::game::{Game, Tone};
use super::map::{Feature, Ground, Wall};
use super::surface::{hash, Kind};

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum Weather { Clear, Cloud, Rain, Storm, Snow, Fog }

impl Weather {
    pub fn word(self) -> &'static str {
        match self { Weather::Clear => "clear", Weather::Cloud => "cloudy", Weather::Rain => "rain", Weather::Storm => "storm", Weather::Snow => "snow", Weather::Fog => "fog" }
    }
    /// What it leaves of one's sight under the sky.
    pub fn sight_factor(self) -> f32 {
        match self { Weather::Fog => 0.5, Weather::Storm => 0.6, Weather::Snow => 0.7, Weather::Rain => 0.85, _ => 1.0 }
    }
    pub fn wet(self) -> bool { matches!(self, Weather::Rain | Weather::Storm) }
}

impl Game {
    /// The season: 0 spring, 1 summer, 2 autumn, 3 winter (the history's calendar if it runs).
    pub fn season(&self) -> u8 {
        if let Some(h) = &self.history {
            return match h.current_date.season { crate::seasons::Season::Spring => 0, crate::seasons::Season::Summer => 1, crate::seasons::Season::Autumn => 2, crate::seasons::Season::Winter => 3 };
        }
        ((self.turn / super::living::SEASON) % 4) as u8
    }

    /// The air at tile t now (degrees): the land's, the season's, the night's.
    pub fn temperature_at(&self, t: (usize, usize)) -> f32 {
        let k = t.1 * self.world.w + t.0;
        let base = self.world.temperature.get(k).copied().unwrap_or(10.0);
        base + [0.0, 6.0, -2.0, -10.0][self.season() as usize] - if self.night() { 5.0 } else { 0.0 }
    }

    /// The weather over tile t now.
    pub fn weather_at(&self, t: (usize, usize)) -> Weather {
        if let Some(w) = self.weather_set { return w; }
        let k = t.1 * self.world.w + t.0;
        if !self.world.land.get(k).copied().unwrap_or(false) { return Weather::Cloud; }
        let half_day = self.turn / (super::land::DAY / 2);
        let h = hash(self.seed ^ 0x3EA7, (t.0 / 3) as i64, (t.1 / 3) as i64, half_day) % 100;
        let wet = self.world.moisture.get(k).copied().unwrap_or(0.5).clamp(0.0, 1.0);
        let land = super::surface::Land { info: &self.world, atlas: &self.atlas };
        let kind = land.kind(k);
        let rain = (8.0 + wet * 34.0 + if self.season() == 2 { 6.0 } else { 0.0 } - if kind == Kind::Desert { 20.0 } else { 0.0 }).max(2.0) as u64;
        let fog = if matches!(kind, Kind::Swamp | Kind::Lake) { 16 } else if matches!(kind, Kind::Forest | Kind::Taiga | Kind::Mountain) { 8 } else { 4 };
        let cold = self.temperature_at(t) < 1.0;
        if h < rain { return if cold { Weather::Snow } else if h < rain / 5 { Weather::Storm } else { Weather::Rain }; }
        if h < rain + fog { return Weather::Fog; }
        if h < rain + fog + 22 { return Weather::Cloud; }
        Weather::Clear
    }

    /// The weather where the hero stands (clear under a roof).
    pub fn weather(&self) -> Weather {
        if !self.on_land() { return Weather::Clear; }
        self.weather_at(self.tile)
    }

    /// Too cold to heal, and losing life: cold land at night (or a hard frost by day), no furs,
    /// no fire near.
    pub fn freezing(&self) -> bool {
        if !self.on_land() { return false; }
        let t = self.temperature_at(self.tile) - if self.weather() == Weather::Snow { 3.0 } else { 0.0 };
        if t >= 1.0 || (!self.night() && t > -12.0) { return false; }
        if self.hero.count("furs") > 0 || self.hero.warm > 0 { return false; }
        !self.fire_near(3)
    }

    /// A campfire (or something burning) within r cells.
    pub fn fire_near(&self, r: i32) -> bool {
        let Some(f) = self.floor() else { return false };
        (-r..=r).any(|dy| (-r..=r).any(|dx| { let (x, y) = (self.x + dx, self.y + dy); f.inside(x, y) && (matches!(f.at(x, y).feature, Feature::Campfire | Feature::Brazier) || f.fire.contains_key(&(x, y))) }))
    }

    /// A turn of weather on the land: the cold bites, rain puts out fires, tracks in the snow.
    pub fn weather_tick(&mut self, ticks: u64) {
        if !self.on_land() || ticks == 0 { return; }
        let w = self.weather();
        if self.freezing() {
            for k in 0..ticks {
                let turn = self.turn / 100 - k;
                if turn % 6 == 0 && self.hero.hp > self.hero.max_hp() / 4 { self.hero.hp -= 1; }
                if turn % 40 == 0 { self.say(Tone::Danger, "The cold bites to the bone. Without furs or a fire you will not mend tonight (B makes a campfire with a torch)."); }
            }
        }
        if w.wet() {
            let z = self.z;
            if let Some(p) = self.place_mut() { let f = &mut p.floors[z]; for v in f.fire.values_mut() { *v = (*v).saturating_sub(1).max(1); } }
        }
        // Tracks: in the snow, something unseen nearby passed this way.
        if w == Weather::Snow && (self.turn / 100) % 25 == 0 {
            let (hx, hy) = (self.x, self.y);
            let seen = self.sight.clone();
            let near = self.place().and_then(|p| { let fw = p.floors[0].w; p.monsters.iter().filter(|m| m.hp > 0 && !m.boss && !seen.get(m.y as usize * fw + m.x as usize).copied().unwrap_or(false)).map(|m| ((m.x - hx).abs().max((m.y - hy).abs()), m.x - hx, m.y - hy, m.name.clone())).filter(|t| t.0 <= 24).min_by_key(|t| t.0) });
            if let Some((_, dx, dy, name)) = near { self.say(Tone::Info, format!("Fresh tracks in the snow: {} passed here, heading {}.", super::item::article(&name), super::wonders::way(dx, dy).trim_start_matches("to the "))); }
        }
    }

    /// Make a campfire beside one with a torch (B).
    pub fn camp(&mut self) -> Option<i32> {
        if !self.on_land() { self.say(Tone::Info, "Not here: a campfire wants open sky."); return None; }
        if self.hero.count("torch") == 0 && self.hero.torch <= 0 { self.say(Tone::Info, "You need a torch to start a fire."); return None; }
        if self.fire_near(1) { self.say(Tone::Info, "There is a fire here already."); return None; }
        let z = self.z;
        let (x0, y0) = (self.x, self.y);
        let spot = super::map::DIRS8.iter().map(|(dx, dy)| (x0 + dx, y0 + dy)).find(|&(x, y)| self.floor().map_or(false, |f| f.walkable(x, y) && f.at(x, y).feature == Feature::None && !matches!(f.at(x, y).ground, Ground::Water | Ground::Shallows)) && self.place().map_or(true, |p| !p.monsters.iter().any(|m| m.hp > 0 && (m.x, m.y) == (x, y)) && !p.npcs.iter().any(|n| (n.x, n.y) == (x, y))));
        let Some((x, y)) = spot else { self.say(Tone::Info, "No room for a fire here."); return None };
        if self.hero.torch <= 0 { self.hero.spend("torch", 1); }
        if let Some(p) = self.place_mut() { let c = p.floors[z].at_mut(x, y); c.feature = Feature::Campfire; if c.wall == Wall::Tree { c.wall = Wall::None; } }
        self.say(Tone::Info, if self.weather().wet() { "You coax a fire to life in the lee of your pack. It smokes, but it burns." } else { "You gather what will burn and set a campfire going. Warmth, and light." });
        self.look();
        Some(300)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::adventure::game::Action;

    /// Fog halves one's sight under the sky; a storm and snow cut it too.
    #[test]
    fn fog_halves_sight() {
        let mut g = crate::adventure::land::tests::game();
        g.land_at((4, 2), None);
        g.weather_set = Some(Weather::Clear);
        let clear = g.sight_radius();
        g.weather_set = Some(Weather::Fog);
        let fog = g.sight_radius();
        assert_eq!(fog * 2, clear, "fog {} clear {}", fog, clear);
        g.weather_set = Some(Weather::Storm);
        assert!(g.sight_radius() < clear);
    }

    /// A night in the snow without furs or a fire costs life (down to a quarter, no lower);
    /// by a campfire it costs nothing.
    #[test]
    fn a_night_in_the_snow_without_a_fire_costs_life() {
        let night = |fire: bool| -> (i32, i32) {
            let mut g = crate::adventure::land::tests::game();
            for t in g.world.temperature.iter_mut() { *t = -8.0; }
            g.turn = crate::adventure::land::DAY * 13 / 24; // 9 at night
            g.land_at((5, 2), None);
            g.weather_set = Some(Weather::Snow);
            g.hero.pack.retain(|i| i.id != "furs");
            if fire { g.hero.torch = 0; assert!(g.act(Action::Camp), "no campfire made"); }
            let start = g.hero.max_hp();
            g.hero.hp = start;
            for _ in 0..500 {
                if let Some(p) = g.land.as_mut() { p.monsters.clear(); }
                g.pass(100);
                if !g.night() { break; }
            }
            (start, g.hero.hp)
        };
        let (max, cold) = night(false);
        assert!(cold < max - 20 && cold >= max / 4, "a cold night: {} of {}", cold, max);
        let (max, warm) = night(true);
        assert_eq!(warm, max, "by the fire: {} of {}", warm, max);
    }
}
