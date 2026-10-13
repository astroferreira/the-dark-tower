//! What the system looks like from the world and what it did to the world: the sun's size and
//! colour, daylight, the colour plants would take under that light, moons, eclipses and tides,
//! aurorae, the wanderers in the night sky; and the climate's notes (mean temperature, ice,
//! circulation cells). `StarSystem::account` writes it all as sections of plain sentences, for
//! the console (`--system`), the viewer's heavens card (H) and the journal.

use crate::climate::ClimateSimulation;
use crate::climate::ebm::row_latitude;
use crate::tilemap::Tilemap;

use super::planets::PlanetKind;
use super::{ClimateNotes, SpectralClass, StarSystem};

/// One AU in Earth radii.
const AU_EARTH_RADII: f32 = 23_481.0;

/// Summarise a simulated climate.
pub fn climate_notes(sim: &ClimateSimulation, hm: &Tilemap<f32>) -> ClimateNotes {
    let (w, h) = (hm.width, hm.height);
    let warmest = sim.warmest_season();
    let locked = sim.forcing.locked;
    let (mut total, mut sum_t, mut ice) = (0.0f64, 0.0f64, 0.0f64);
    let (mut eq, mut eq_w, mut pole, mut pole_w) = (0.0f64, 0.0f64, 0.0f64, 0.0f64);
    let (mut summer, mut winter, mut land_w) = (0.0f64, 0.0f64, 0.0f64);
    for y in 0..h {
        let lat = row_latitude(y, h);
        let area = lat.cos().max(0.001) as f64;
        for x in 0..w {
            let t = *sim.mean_temperature.get(x, y) as f64;
            total += area;
            sum_t += t * area;
            if *warmest.get(x, y) < -1.5 { ice += area; }
            // Bands: by latitude, or for a locked world by distance from the substellar point.
            let band = match locked {
                Some(lon0) => {
                    let lon = (x as f32 + 0.5) / w as f32 * std::f32::consts::TAU;
                    90.0 - (lat.cos() * (lon - lon0).cos()).clamp(-1.0, 1.0).acos().to_degrees()
                }
                None => 90.0 - lat.to_degrees().abs(),
            };
            // Equator: under the sun; poles: the night side's middle for a locked world.
            let is_pole = if locked.is_some() { band < -80.0 } else { band < 10.0 };
            if band > 80.0 { eq += t * area; eq_w += area; }
            if is_pole { pole += t * area; pole_w += area; }
            if *hm.get(x, y) > 0.0 {
                let seasons = sim.seasonal_temperatures.iter().map(|s| *s.get(x, y));
                let (lo, hi) = seasons.fold((f32::MAX, f32::MIN), |(a, b), v| (a.min(v), b.max(v)));
                summer += hi as f64 * area;
                winter += lo as f64 * area;
                land_w += area;
            }
        }
    }
    let avg = |s: f64, w: f64| if w > 0.0 { (s / w) as f32 } else { f32::NAN };
    ClimateNotes {
        mean_c: avg(sum_t, total),
        co2_bar: sim.air.co2_bar,
        pressure_bar: sim.air.pressure_bar,
        greenhouse_wm2: sim.air.greenhouse_wm2,
        hadley_deg: sim.circulation.hadley_deg,
        cells: sim.circulation.belts.len().saturating_sub(1),
        ice_share: avg(ice, total),
        equator_c: avg(eq, eq_w),
        pole_c: avg(pole, pole_w),
        summer_land_c: avg(summer, land_w),
        winter_land_c: avg(winter, land_w),
        tidal_flat_tiles: 0,
    }
}

/// A section of the account.
pub struct Section {
    pub title: &'static str,
    pub lines: Vec<String>,
}

/// A number to about three significant figures, never in exponent form ("0.00028", "12.4").
pub fn round_sig(v: f32) -> String {
    if v >= 100.0 { format!("{:.0}", v) } else if v >= 10.0 { format!("{:.1}", v) } else if v >= 1.0 { format!("{:.2}", v) }
    else if v <= 0.0 { "0".into() }
    else {
        let decimals = (-v.log10().floor()) as usize + 2;
        format!("{:.*}", decimals.min(8), v)
    }
}

impl StarSystem {
    /// The sun's angular diameter (degrees) from the world.
    pub fn sun_angular_deg(&self) -> f32 { 0.533 * self.star.radius_rsun / self.world.orbit.semi_major_au }

    /// The moon that lights the night: the widest seen from the ground, if it is a disc at all
    /// (0.1 degrees, a fifth of the Moon's); a moonlet is only a bright star.
    pub fn night_moon(&self) -> Option<&super::planets::Moon> {
        self.world.moons.iter().filter(|m| m.angular_deg(6371.0) >= 0.1)
            .max_by(|a, b| a.angular_deg(6371.0).partial_cmp(&b.angular_deg(6371.0)).unwrap())
    }

    /// How bright the day looks to the eye relative to Earth's.
    pub fn daylight(&self) -> f32 { self.world.flux_earth * self.host().visible_share() }

    /// A companion as seen from the world: its angular size (degrees) and apparent magnitude,
    /// and for a close pair the widest the two suns stand apart (degrees).
    pub fn companion_view(&self) -> Option<(f32, f32, f32)> {
        let c = self.companion.as_ref()?;
        let a = self.world.orbit.semi_major_au;
        let dist = if c.circumbinary { a } else { c.separation_au };
        let size = 0.533 * c.star.radius_rsun / dist;
        let flux = c.star.luminosity_lsun / (dist * dist) * c.star.visible_share();
        let mag = -26.74 - 2.5 * flux.max(1e-12).log10();
        let apart = if c.circumbinary { (c.separation_au / a).atan().to_degrees() } else { 0.0 };
        Some((size, mag, apart))
    }

    /// The colour leaves would take under this light (Kiang et al. 2007: pigments tuned to the
    /// light that reaches the ground: bluer suns favour reflecting blue-green, redder ones dark
    /// leaves that drink every colour).
    pub fn leaves(&self) -> &'static str {
        match self.star.spectral.class {
            SpectralClass::F => "blue-green and teal leaves, turning silver in drought",
            SpectralClass::G => "green leaves",
            SpectralClass::K => "leaves of russet, gold and olive",
            SpectralClass::M => "leaves near black, purple where the light falls through them",
        }
    }

    pub fn sky_colour(&self) -> &'static str {
        match self.star.spectral.class {
            SpectralClass::F => "a deep blue sky, violet at the zenith",
            SpectralClass::G => "a blue sky",
            SpectralClass::K => "a soft blue sky, gold at dawn and dusk",
            SpectralClass::M => "a dim, pale sky, salmon toward the horizon",
        }
    }

    /// Latitude (degrees) down to which aurorae are seen: the magnetopause stands where the
    /// field's pressure meets the star's wind (Earth: ~10 radii, aurorae at ~67 deg).
    pub fn aurora_latitude(&self) -> f32 {
        let wind = self.host().activity.sqrt() / (self.world.orbit.semi_major_au * self.world.orbit.semi_major_au);
        let standoff = 10.0 * (self.world.magnetic_field.powi(2) / wind.max(1e-3)).powf(1.0 / 6.0);
        if standoff < 1.3 { return 0.0; }
        90.0 - (1.0 / standoff).sqrt().asin().to_degrees()
    }

    /// Spring tidal range relative to Earth's: the moons' tides and, unless the world is locked
    /// (then the star's bulge stands still), the star's.
    pub fn tides(&self) -> f32 {
        let moons: f32 = self.world.moons.iter().map(|m| m.tide()).sum();
        let a = self.world.orbit.semi_major_au;
        let star = if self.world.spin.locked { 0.0 } else { 0.46 * self.host().mass_msun / (a * a * a) };
        (moons + star) / 1.46
    }

    /// A planet's brightest apparent magnitude from the world and its angular size then
    /// (arcseconds): an outer planet at opposition, an inner one at greatest elongation.
    pub fn planet_view(&self, i: usize) -> (f32, f32) {
        let p = &self.planets[i];
        let ah = self.world.orbit.semi_major_au;
        let ap = p.semi_major_au;
        let (dist, phase) = if ap > ah { (ap - ah, 1.0) } else { ((ah * ah - ap * ap).max(1e-6).sqrt(), 0.3) };
        let albedo = match p.kind { PlanetKind::GasGiant => 0.5, PlanetKind::IceGiant => 0.45, PlanetKind::Ice => 0.6, PlanetKind::Hothouse => 0.65, PlanetKind::MiniNeptune => 0.4, _ => 0.15 };
        let r_au = p.radius_earth / AU_EARTH_RADII;
        let ratio = albedo * (r_au / dist).powi(2) * phase * (ah / ap).powi(2);
        let m_star = -26.74 - 2.5 * self.daylight().max(1e-4).log10();
        let mag = m_star - 2.5 * ratio.max(1e-30).log10();
        (mag, 2.0 * r_au / dist * 206_265.0)
    }

    /// Sunrise to sunset and back: the day-night swing of temperature inland (C), from the
    /// day's length and the air's thickness (Earth's dry interiors: ~12).
    pub fn diurnal_swing(&self) -> Option<f32> {
        let day = self.world.spin.solar_day_hours(&self.world.orbit)?;
        Some((12.0 * (day / 24.0).sqrt() / self.world.air.pressure_bar().powf(0.4)).clamp(2.0, 70.0))
    }

    /// The account of the heavens, section by section.
    pub fn account(&self) -> Vec<Section> {
        let (s, w) = (&self.star, &self.world);
        let mut out = Vec::new();

        let mut sun = vec![format!(
            "{}, {} {} {} ({}): {} the Sun's mass, {} its light, {:.0} K.",
            s.name, a_an(&s.spectral.label()), s.spectral.label(), s.spectral.class.dwarf_word(), s.spectral.class.colour_word(),
            round_sig(s.mass_msun), round_sig(s.luminosity_lsun), s.teff_k
        )];
        sun.push(format!("{:.1} billion years old of a life of {:.0} billion; its metals {:.1}x the Sun's ([Fe/H] {:+.2}).",
            s.age_gyr, s.lifetime_gyr.min(999.0), 10f32.powf(s.metallicity), s.metallicity));
        if let Some(c) = &self.companion {
            let cs = &c.star;
            sun.push(if c.circumbinary {
                format!("With it {}, {} {} {}: the two circle each other every {} days, {} AU apart, and the world circles them both.",
                    cs.name, a_an(&cs.spectral.label()), cs.spectral.label(), cs.spectral.class.dwarf_word(), round_sig(c.period_days), round_sig(c.separation_au))
            } else {
                format!("Far off {}, {} {} {}, circles it every {} years at {} AU (as close as {} AU); it bounds the planets to {} AU.",
                    cs.name, a_an(&cs.spectral.label()), cs.spectral.label(), cs.spectral.class.dwarf_word(), round_sig(c.period_days / orbit_year()), round_sig(c.separation_au),
                    round_sig(c.separation_au * (1.0 - c.eccentricity)), round_sig(c.truncation_au()))
            });
        }
        sun.push(match s.activity {
            a if a < 2.0 => "A calm star: flares are rare and small.".to_string(),
            a if a < 15.0 => format!("A restless star ({:.0}x the Sun's storms): great flares every few years, aurorae often.", a),
            a => format!("A violent star ({:.0}x the Sun's storms): flares by the week that light the night side and strip the air.", a),
        });
        out.push(Section { title: "The sun", lines: sun });

        let (hz_in, hz_out) = self.host().hz_au();
        let o = &w.orbit;
        let mut orbit = vec![format!(
            "{} circles it at {:.3} AU, {} of its habitable zone ({:.2}-{:.2} AU); starlight {:.2}x the Earth's.",
            w.name, o.semi_major_au,
            match w.hz_position { h if h < 0.0 => "just inside the inner edge", h if h < 0.08 => "at the warm inner edge", h if h < 0.22 => "in the warm inner part", h if h < 0.5 => "in the cool middle", h if h <= 1.0 => "near the cold outer edge", _ => "just beyond the outer edge" },
            hz_in, hz_out, w.flux_earth
        )];
        let year_days = o.period_days;
        match w.spin.solar_day_hours(o) {
            Some(day) if w.spin.resonance => orbit.push(format!(
                "Held in a 3:2 dance with its sun, as Mercury is with ours: it turns three times every two years, so a day lasts two years ({:.0} Earth days). At every perihelion the sun hangs over the hot longitudes, {:.0}° E and {:.0}° E.",
                day / 24.0, w.spin.substellar_lon_deg, (w.spin.substellar_lon_deg + 180.0) % 360.0
            )),
            None => orbit.push(format!(
                "One face always to {}: endless day under the sun at {:.0}° E, endless night on the far side. A year of {:.0} Earth days passes without sunrise.",
                s.name, w.spin.substellar_lon_deg, year_days
            )),
            Some(day) => {
                let per_year = year_days * 24.0 / day;
                orbit.push(format!(
                    "A day of {:.1} hours (it spins in {:.1}{}); a year of {:.0} of its days ({:.0} Earth days).",
                    day, w.spin.sidereal_hours, if w.spin.retrograde { ", backwards: the sun rises in the west" } else { "" }, per_year, year_days
                ));
            }
        }
        if o.eccentricity >= 0.03 {
            let (peri, apo) = o.peri_apo_au();
            let season = ["northern spring", "northern summer", "northern autumn", "northern winter"][(((o.perihelion_lon_deg + 45.0).rem_euclid(360.0)) / 90.0) as usize % 4];
            orbit.push(format!(
                "An eccentric orbit (e {:.2}): {:.0}% more light at perihelion ({:.2} AU, in {}) than at aphelion ({:.2} AU).",
                o.eccentricity, ((apo / peri).powi(2) - 1.0) * 100.0, peri, season, apo
            ));
            let shares = o.season_shares();
            if let Some(day) = w.spin.solar_day_hours(o) {
                let d = |k: usize| shares[k] * year_days * 24.0 / day;
                orbit.push(format!("Unequal seasons (Kepler's second law): spring {:.0} days, summer {:.0}, autumn {:.0}, winter {:.0}.", d(0), d(1), d(2), d(3)));
            }
        }
        let tilt = w.spin.obliquity_deg;
        orbit.push(if w.spin.locked {
            "No tilt worth the name: no seasons but the orbit's.".to_string()
        } else if tilt < 8.0 {
            format!("Tilted {:.0}°: hardly any seasons; the poles never see the sun high.", tilt)
        } else if tilt < 35.0 {
            format!("Tilted {:.0}°: seasons {} Earth's.", tilt, if tilt < 18.0 { "milder than" } else if tilt < 28.0 { "like" } else { "harsher than" })
        } else if tilt < 54.0 {
            format!("Tilted {:.0}°: savage seasons; the midnight sun reaches {:.0}° latitude.", tilt, 90.0 - tilt)
        } else {
            format!("Tilted {:.0}°: over a year the poles take more sun than the equator; polar summers blaze, the tropics stay cool.", tilt)
        });
        out.push(Section { title: "Orbit and day", lines: orbit });

        let mut body = vec![format!(
            "Gravity {:.2} g, its iron core {:.0}% of its mass; internal heat {:.1}x the Earth's ({}).",
            w.gravity, w.core_fraction * 100.0, w.internal_heat,
            if w.internal_heat > 1.4 { "young and restless: many hotspots" } else if w.internal_heat < 0.7 { "old and cooling: few hotspots" } else { "like the Earth's" }
        )];
        body.push(format!(
            "Water {:.2}x the Earth's oceans{}.",
            w.water,
            if w.water < 0.7 { ": the seas are shallow and much land stands dry" } else if w.water > 1.4 { ": the seas drown all but the high land" } else { "" }
        ));
        body.push(format!("A magnetic field {:.1}x the Earth's{}.", w.magnetic_field, if w.magnetic_field < 0.5 { ": a thin shield against the star's wind" } else { "" }));
        out.push(Section { title: "The world", lines: body });

        let a = &w.air;
        let mut air = vec![format!(
            "Air of {:.2} bar: nitrogen {:.2}, oxygen {:.2}, CO2 {} bar ({:.0}x the Earth's).",
            a.pressure_bar(), a.n2_bar, a.o2_bar, round_sig(a.co2_bar), a.co2_bar / super::air::EARTH_CO2_BAR
        )];
        if !self.is_sol() {
            air.push(if a.co2_bar > 0.01 {
                "The carbonate-silicate cycle has built a thick CO2 blanket: the cold weathers rock too slowly to draw it down.".to_string()
            } else if a.co2_bar < 1e-4 {
                "The warmth weathers rock fast and has drawn the CO2 down to a trace.".to_string()
            } else {
                "Volcanoes and weathering hold the CO2 near the Earth's.".to_string()
            });
        }
        if a.o2_bar < 0.05 { air.push("Too little oxygen for fire to take hold easily.".into()); }
        else if a.o2_bar > 0.3 { air.push("Rich in oxygen: fires burn fierce.".into()); }
        out.push(Section { title: "The air", lines: air });

        if let Some(c) = &self.climate {
            let mut cl = vec![format!(
                "Mean {:.0} °C (Earth ~14 °C); {:.0}% of the surface under ice all year.",
                c.mean_c, c.ice_share * 100.0
            )];
            if w.spin.locked {
                cl.push(format!("Under the sun {:.0} °C, on the night side {:.0} °C.", c.equator_c, c.pole_c));
            } else {
                cl.push(format!("Equator {:.0} °C, poles {:.0} °C{}; on land summers {:.0} °C, winters {:.0} °C.",
                    c.equator_c, c.pole_c, if c.pole_c > c.equator_c { " (warmer than the equator)" } else { "" }, c.summer_land_c, c.winter_land_c));
                cl.push(if c.cells <= 1 {
                    "It spins too slowly for more than one great cell of air from equator to pole: no storm track, rain where the trades meet.".to_string()
                } else {
                    format!("{} bands of wind a hemisphere; the tropics' cell reaches {:.0}° (Earth 30°), with its deserts at its edge.", c.cells, c.hadley_deg)
                });
            }
            out.push(Section { title: "Climate", lines: cl });
        }

        let mut sky = vec![format!(
            "{} is {:.2}° across ({:.1}x the Sun from Earth), {}; daylight {:.2}x the Earth's to the eye.",
            s.name, self.sun_angular_deg(), self.sun_angular_deg() / 0.533, s.spectral.class.colour_word(), self.daylight()
        )];
        if let (Some(c), Some((size, mag, apart))) = (&self.companion, self.companion_view()) {
            sky.push(if c.circumbinary {
                format!("Two suns: {} {:.2}° across beside the first, at most {:.1}° apart; every shadow is doubled, and they eclipse each other when their orbit lines up.", c.star.name, size, apart)
            } else if mag < -12.0 {
                format!("{} shines at magnitude {:.0}, brighter than a full moon: by night it casts shadows, and in its season the night is never dark.", c.star.name, mag)
            } else {
                format!("{} shines at magnitude {:.0}, the brightest star of the night.", c.star.name, mag)
            });
        }
        sky.push(format!("{}; plants would grow {}.", capitalise(self.sky_colour()), self.leaves()));
        if let Some(swing) = self.diurnal_swing() {
            if swing > 20.0 { sky.push(format!("Long days and nights: inland the air swings {:.0} °C from noon to dawn.", swing)); }
        }
        let aur = self.aurora_latitude();
        sky.push(if aur <= 1.0 { "The star's wind beats on the air itself: aurorae over the whole sky.".into() } else { format!("Aurorae down to {:.0}° latitude.", aur) });
        out.push(Section { title: "The sky", lines: sky });

        let mut moons = Vec::new();
        if w.moons.is_empty() { moons.push("No moon: dark nights, and only the sun's tides.".into()); }
        for m in &w.moons {
            let ang = m.angular_deg(6371.0);
            let sun = self.sun_angular_deg();
            let eclipse = if ang > 1.03 * sun { "covers the sun: total eclipses" } else if ang >= 0.95 * sun { "just covers the sun: total and ring eclipses" } else if ang > 0.3 * sun { "rings of fire across the sun" } else { "a dark speck on the sun" };
            moons.push(format!("{}: {} Moons' mass, {:.2}° across, a month of {:.1} days; {}.", m.name, round_sig(m.mass_moons), ang, m.period_days, eclipse));
        }
        if !w.moons.is_empty() && self.night_moon().is_none() { moons.push("No moon shows as a disc: the nights are dark, and the month is counted by the stars.".into()); }
        let tides = self.tides();
        let flats = self.climate.as_ref().map_or(0, |c| c.tidal_flat_tiles);
        moons.push(format!("Tides {:.1}x the Earth's{}.", tides,
            if flats > 0 { format!(": the sea runs out over {} tiles of salt marsh and mangrove, and bores run up the rivers", flats) }
            else if tides > 3.0 { ": wide tidal flats and bores up the rivers".to_string() } else if tides < 0.3 { ": the sea barely moves".to_string() } else { String::new() }));
        out.push(Section { title: "Moons and tides", lines: moons });

        let mut wander = Vec::new();
        for (i, p) in self.planets.iter().enumerate() {
            if p.kind == PlanetKind::Home { continue; }
            let (mag, arcsec) = self.planet_view(i);
            let size = if p.kind == PlanetKind::GasGiant { format!(" ({:.1} Jupiters)", p.mass_earth / super::planets::JUPITER_EARTHS) } else { format!(" ({} Earths)", round_sig(p.mass_earth)) };
            let seen = if arcsec > 60.0 { format!(", a disc {:.0}' across at its nearest", arcsec / 60.0) } else if mag < -3.0 { ", brighter than any star".into() } else if mag < 1.0 { ", a bright wanderer".into() } else if mag < 6.0 { ", faint to the eye".into() } else { ", unseen without a glass".into() };
            wander.push(format!("{}, {} {}{}{} at {} AU, a year of {} days{}.", p.name, a_an(p.kind.word()), p.kind.word(), size, if p.rings { " with rings" } else { "" }, round_sig(p.semi_major_au), round_sig(p.period_days), seen));
        }
        for b in &self.belts {
            wander.push(format!("{}: {} from {} to {} AU.", capitalise(&b.name), if b.icy { "comets and ice" } else { "rubble of a world that never formed" }, round_sig(b.inner_au), round_sig(b.outer_au)));
        }
        out.push(Section { title: "Neighbours", lines: wander });
        for s in &mut out { for l in &mut s.lines { *l = capitalise(l); } }
        out
    }

    /// The neighbours in a line each, short (the heavens card's list).
    pub fn neighbour_lines(&self) -> Vec<String> {
        let mut out = Vec::new();
        for (i, p) in self.planets.iter().enumerate() {
            if p.kind == PlanetKind::Home { continue; }
            let (mag, arcsec) = self.planet_view(i);
            let mass = if p.kind == PlanetKind::GasGiant { format!("{} Jupiters", round_sig(p.mass_earth / super::planets::JUPITER_EARTHS)) } else { format!("{} Earths", round_sig(p.mass_earth)) };
            let seen = if arcsec > 60.0 { format!("a disc {:.0}' across", arcsec / 60.0) } else if mag < -3.0 { "brighter than any star".into() } else if mag < 1.0 { "a bright wanderer".into() } else if mag < 6.0 { "faint".into() } else { "unseen".into() };
            out.push(format!("{}: {}{}, {}, {} AU; {}", p.name, p.kind.word(), if p.rings { " with rings" } else { "" }, mass, round_sig(p.semi_major_au), seen));
        }
        for b in &self.belts { out.push(format!("{}: {} to {} AU", capitalise(&b.name), round_sig(b.inner_au), round_sig(b.outer_au))); }
        out
    }

    /// One line for the console when the world is made.
    pub fn summary(&self) -> String {
        let w = &self.world;
        let day = w.spin.solar_day_hours(&w.orbit).map(|d| format!("a {:.1}-hour day", d)).unwrap_or_else(|| "locked to its star".into());
        format!(
            "{} circles {}, {} {} {}: {:.3} AU (zone position {:.2}), starlight {:.2}x Earth's, tilt {:.0}°, {}, a year of {:.0} days, {} moon{}, {} neighbour planets",
            w.name, self.star.name, a_an(&self.star.spectral.label()), self.star.spectral.label(), self.star.spectral.class.dwarf_word(), w.orbit.semi_major_au,
            w.hz_position, w.flux_earth, w.spin.obliquity_deg, day, w.orbit.period_days, w.moons.len(), if w.moons.len() == 1 { "" } else { "s" },
            self.planets.len() - 1
        )
    }
}

fn orbit_year() -> f32 { super::orbit::EARTH_YEAR_DAYS }

/// "a" or "an" before a word or a spectral type ("an M3V", "an ice giant", "a K2V").
pub fn a_an(word: &str) -> &'static str {
    let w = word.trim_start();
    let first = w.chars().next().unwrap_or('x').to_ascii_lowercase();
    let spelled_vowel = matches!(first, 'f' | 'h' | 'l' | 'm' | 'n' | 'r' | 's' | 'x') && w.chars().nth(1).map_or(false, |c| c.is_ascii_digit() || c.is_ascii_uppercase());
    if matches!(first, 'a' | 'e' | 'i' | 'o' | 'u') || spelled_vowel { "an" } else { "a" }
}

/// The first letter in capitals.
pub fn capitalise_word(s: &str) -> String { capitalise(s) }

fn capitalise(s: &str) -> String {
    let mut c = s.chars();
    c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::astro::SystemSpec;

    #[test]
    fn the_suns_sky_is_the_earths() {
        let sol = StarSystem::sol();
        assert!((sol.sun_angular_deg() - 0.533).abs() < 0.01);
        assert!((sol.daylight() - 1.0).abs() < 0.01);
        assert!((sol.tides() - 1.0).abs() < 0.05, "tides {}", sol.tides());
        let aur = sol.aurora_latitude();
        assert!((65.0..75.0).contains(&aur), "aurora {aur}");
        let jupiter = sol.planets.iter().position(|p| p.name == "Jupiter").unwrap();
        let (mag, _) = sol.planet_view(jupiter);
        assert!((-3.3..-2.4).contains(&mag), "Jupiter at {mag}");
        let venus = sol.planets.iter().position(|p| p.name == "Venus").unwrap();
        assert!(sol.planet_view(venus).0 < -4.0);
    }

    #[test]
    fn articles_follow_the_sound() {
        assert_eq!(a_an("M3V"), "an");
        assert_eq!(a_an("F5V"), "an");
        assert_eq!(a_an("K2V"), "a");
        assert_eq!(a_an("G2V"), "a");
        assert_eq!(a_an("ice giant"), "an");
        assert_eq!(a_an("gas giant"), "a");
        assert_eq!(a_an("mini-neptune"), "a");
        assert_eq!(a_an("super-earth"), "a");
    }

    #[test]
    fn companions_and_resonances_are_told() {
        let mut found = (false, false, false);
        for seed in 0..400 {
            let sys = StarSystem::generate(seed, &SystemSpec::default());
            let text: String = sys.account().iter().flat_map(|s| s.lines.clone()).collect::<Vec<_>>().join(" ");
            if let Some(c) = &sys.companion {
                if c.circumbinary {
                    found.0 = true;
                    assert!(text.contains("Two suns"), "seed {seed}: {text}");
                    assert!(sys.world.orbit.semi_major_au > c.stability_limit_au(), "seed {seed}: the world inside the pair's limit");
                    assert!(sys.planets.iter().all(|p| p.semi_major_au >= c.stability_limit_au() * 0.999), "seed {seed}: a planet inside the limit");
                } else {
                    found.1 = true;
                    assert!(sys.planets.iter().all(|p| p.semi_major_au <= c.truncation_au().max(sys.world.orbit.semi_major_au * 1.05) * 1.001), "seed {seed}: a planet past the truncation");
                }
            }
            if sys.world.spin.resonance {
                found.2 = true;
                let day = sys.world.spin.solar_day_hours(&sys.world.orbit).unwrap();
                assert!((day / 24.0 / sys.world.orbit.period_days - 2.0).abs() < 0.01, "seed {seed}: a 3:2 day is two years");
                assert!(text.contains("3:2"));
            }
        }
        assert!(found.0 && found.1 && found.2, "close pairs, wide companions, resonances: {found:?}");
    }

    #[test]
    fn every_account_reads() {
        for seed in 0..40 {
            let sys = StarSystem::generate(seed, &SystemSpec::default());
            let acc = sys.account();
            assert!(acc.len() >= 6);
            for s in &acc { for l in &s.lines { assert!(!l.contains("NaN") && !l.contains("inf"), "seed {seed}: {l}"); } }
        }
    }
}

/// The sky over an embark, for the colony: how long its seasons are (in the colony's 24-hour
/// days, its year scaled from the world's as the colony's 120-day reckoning is from Earth's
/// 365), how long its day is, where its sun stands. The settlers keep a 24-hour rhythm (they
/// sleep by the clock, as people do through a polar summer); the light follows the sun.
#[derive(Clone, Debug)]
pub struct LocalSky {
    /// Days in spring, summer, autumn and winter (unequal on an eccentric orbit).
    pub season_days: [u64; 4],
    /// Sunrise to sunrise (hours); None when the sun never moves (a locked world).
    pub solar_day_hours: Option<f32>,
    /// On a locked world, the sun's height here (degrees; below zero, the night side).
    pub fixed_sun_deg: f32,
    pub latitude_deg: f32,
    pub tilt_deg: f32,
    /// The Earth under the Sun: the colony's original clock and light, unchanged.
    pub earth: bool,
    /// The great moon's month in colony days (full moon to full moon; 28 under the Sun).
    pub month_days: u64,
    /// No moon that shows as a disc (`StarSystem::night_moon`): the werebeast's night is the
    /// month's darkest.
    pub moonless: bool,
}

impl LocalSky {
    /// The original calendar: four 30-day seasons, a 24-hour day, night 21-5.
    pub fn earth(latitude_deg: f32) -> Self {
        LocalSky { season_days: [30; 4], solar_day_hours: Some(24.0), fixed_sun_deg: 0.0, latitude_deg, tilt_deg: 23.44, earth: true, month_days: 28, moonless: false }
    }

    pub fn year_days(&self) -> u64 { self.season_days.iter().sum() }

    /// The season (0 spring .. 3 winter) of a colony day (1-based) and the day it began on.
    pub fn season_of(&self, day: u64) -> (usize, u64) {
        let year = self.year_days().max(1);
        let into = (day.max(1) - 1) % year;
        let mut start = 0;
        for k in 0..4 {
            if into < start + self.season_days[k] { return (k, day.max(1) - (into - start)); }
            start += self.season_days[k];
        }
        (3, day)
    }

    /// Days into the year (0-based) of a colony day (1-based).
    pub fn into_year(&self, day: u64) -> u64 { (day.max(1) - 1) % self.year_days().max(1) }

    /// The day of the year (0-based) a season begins.
    pub fn season_start(&self, k: usize) -> u64 { self.season_days[..k.min(4)].iter().sum() }

    /// Whether a season begins on this day (festivals, the turn of the season).
    pub fn season_begins(&self, day: u64) -> bool { day > 1 && self.season_of(day).1 == day }

    /// The sun's height (degrees) at a moment, `hours` since the colony's first midnight.
    pub fn sun_height(&self, hours: f32) -> f32 {
        let Some(day) = self.solar_day_hours else { return self.fixed_sun_deg; };
        // Noon at midday of the first day, then one turn each solar day.
        let h = std::f32::consts::TAU * (hours - 12.0) / day;
        // The season's declination from the share of the year gone (spring equinox at 0).
        let year_h = self.year_days() as f32 * 24.0;
        let lambda = std::f32::consts::TAU * (hours / year_h).fract();
        let dec = (self.tilt_deg.to_radians().sin() * lambda.sin()).asin();
        let lat = self.latitude_deg.to_radians();
        (lat.sin() * dec.sin() + lat.cos() * dec.cos() * h.cos()).clamp(-1.0, 1.0).asin().to_degrees()
    }

    /// How dark it is, 0 by day to 0.55 deep in the night: the Earth's clock (dusk 19-21, dawn
    /// 5-7) under the Sun; under another sun its height, through a twilight from 4 above the
    /// horizon to 8 below.
    pub fn darkness(&self, hours: f32) -> f32 {
        if self.earth {
            let hm = hours % 24.0;
            let f = if hm >= 21.0 || hm < 5.0 { 1.0 } else if hm >= 19.0 { (hm - 19.0) / 2.0 } else if hm < 7.0 { (7.0 - hm) / 2.0 } else { 0.0 };
            return 0.55 * f;
        }
        let e = self.sun_height(hours);
        0.55 * ((4.0 - e) / 12.0).clamp(0.0, 1.0)
    }

    /// The sky in a line for the camp's card ("" under the Sun).
    pub fn line(&self, hours: f32) -> String {
        if self.earth { return String::new(); }
        match self.solar_day_hours {
            None if self.fixed_sun_deg > 6.0 => format!("The sun stands still, {:.0}° up; it never sets here.", self.fixed_sun_deg),
            None if self.fixed_sun_deg > -6.0 => "The sun sits on the horizon forever: an endless dusk.".into(),
            None => "The sun never rises here: an endless night.".into(),
            Some(d) => {
                let up = self.sun_height(hours) > 0.0;
                // When it next rises or sets, searching ahead an hour at a time.
                let next = (1..=(d as i64 * 2).max(48)).map(|k| k as f32).find(|k| (self.sun_height(hours + k) > 0.0) != up);
                match (up, next) {
                    (true, Some(k)) => format!("A day of {:.0} hours: the sun sets in {:.0}.", d, k),
                    (false, Some(k)) => format!("A day of {:.0} hours: the sun rises in {:.0}.", d, k),
                    (true, None) => "The midnight sun: it will not set this season.".into(),
                    (false, None) => "The polar night: it will not rise this season.".into(),
                }
            }
        }
    }
}

impl StarSystem {
    /// The sky over the map tile at (latitude, longitude) in degrees (longitude east of the
    /// map's left edge).
    pub fn local_sky(&self, latitude_deg: f32, longitude_deg: f32) -> LocalSky {
        if self.is_sol() { return LocalSky::earth(latitude_deg); }
        let w = &self.world;
        let year_days = (120.0 * w.orbit.years()).round().clamp(40.0, 480.0);
        let shares = w.orbit.season_shares();
        let season_days = shares.map(|s| ((s * year_days).round() as u64).max(4));
        let fixed_sun_deg = if w.spin.locked {
            let (lat, dlon) = (latitude_deg.to_radians(), (longitude_deg - w.spin.substellar_lon_deg).to_radians());
            90.0 - (lat.cos() * dlon.cos()).clamp(-1.0, 1.0).acos().to_degrees()
        } else { 0.0 };
        // The month: the night moon's synodic period (full moon to full moon), in days of 24
        // hours as the camp counts them (the Sun's camp keeps 28).
        let moon = self.night_moon();
        let month_days = moon.map(|m| {
            let synodic = 1.0 / (1.0 / m.period_days - 1.0 / w.orbit.period_days).abs().max(1e-6);
            synodic.round().clamp(6.0, 60.0) as u64
        }).unwrap_or(28);
        LocalSky { season_days, solar_day_hours: w.spin.solar_day_hours(&w.orbit), fixed_sun_deg, latitude_deg, tilt_deg: w.spin.obliquity_deg, earth: false, month_days, moonless: moon.is_none() }
    }
}

#[cfg(test)]
mod local_sky_tests {
    use super::*;
    use crate::astro::SystemSpec;

    #[test]
    fn the_suns_calendar_is_the_colonys_own() {
        let s = StarSystem::sol().local_sky(45.0, 10.0);
        assert!(s.earth && s.season_days == [30; 4] && s.year_days() == 120);
        assert_eq!(s.season_of(1), (0, 1));
        assert_eq!(s.season_of(31), (1, 31));
        assert_eq!(s.season_of(121), (0, 121));
        assert!(s.season_begins(31) && !s.season_begins(32) && !s.season_begins(1));
        assert_eq!(s.darkness(12.0), 0.0);
        assert_eq!(s.darkness(23.0), 0.55);
    }

    #[test]
    fn other_suns_keep_other_hours() {
        // A long year: long seasons.
        let f = StarSystem::generate(1, &SystemSpec { hz: Some(0.1), class: Some(crate::astro::SpectralClass::F), ..Default::default() });
        let fs = f.local_sky(30.0, 0.0);
        assert!(fs.year_days() > 120, "an F star's long year: {} days", fs.year_days());
        // A locked world: day under the star, night behind it.
        let m = StarSystem::generate(3, &SystemSpec { class: Some(crate::astro::SpectralClass::M), subclass: Some(4), ..Default::default() });
        assert!(m.world.spin.locked);
        let lon = m.world.spin.substellar_lon_deg;
        let day = m.local_sky(0.0, lon);
        let night = m.local_sky(0.0, lon + 180.0);
        assert!(day.darkness(3.0) == 0.0 && day.darkness(15.0) == 0.0, "always day under the star");
        assert!(night.darkness(3.0) > 0.5 && night.darkness(15.0) > 0.5, "always night behind it");
        assert!(day.line(0.0).contains("never sets") && night.line(0.0).contains("never rises"));
        // A slow day: the sun stays up for many of the settlers' days.
        let slow = LocalSky { season_days: [40; 4], solar_day_hours: Some(600.0), fixed_sun_deg: 0.0, latitude_deg: 10.0, tilt_deg: 10.0, earth: false, month_days: 28, moonless: false };
        let lit: Vec<bool> = (0..600).map(|h| slow.darkness(h as f32) < 0.1).collect();
        let share = lit.iter().filter(|l| **l).count();
        let mut run = 0;
        let mut best = 0;
        for l in lit.iter().chain(lit.iter()) { run = if *l { run + 1 } else { 0 }; best = best.max(run); }
        assert!((240..360).contains(&share) && best > 240, "a 600-hour day: {share} hours lit, the longest light {best} hours");
        // The polar night: high latitude in winter under a steep tilt.
        let polar = LocalSky { season_days: [30; 4], solar_day_hours: Some(24.0), fixed_sun_deg: 0.0, latitude_deg: 80.0, tilt_deg: 30.0, earth: false, month_days: 28, moonless: false };
        let winter = 3.0 * 30.0 * 24.0 + 24.0 * 15.0;
        assert!((0..24).all(|h| polar.sun_height(winter + h as f32) < 0.0), "no sunrise in the polar winter");
    }

    #[test]
    fn the_month_is_the_night_moons() {
        let sol = StarSystem::sol().local_sky(45.0, 0.0);
        assert!(sol.month_days == 28 && !sol.moonless);
        // The tilted dev sky: a great moon of 18.3 days around a world with a 333-day year.
        let tilted = StarSystem::generate(302, &SystemSpec::from_code("G+o700").unwrap());
        let m = tilted.night_moon().expect("a great moon");
        let sky = tilted.local_sky(30.0, 0.0);
        assert!(!sky.moonless && sky.month_days as f32 > m.period_days && sky.month_days < 22, "synodic month {} days", sky.month_days);
        // The eyeball's only moon is a speck: no full moon, dark nights.
        let eyeball = StarSystem::generate(106, &SystemSpec::from_code("M3").unwrap());
        assert!(!eyeball.world.moons.is_empty() && eyeball.night_moon().is_none());
        assert!(eyeball.local_sky(0.0, 0.0).moonless);
        assert!(eyeball.account().iter().any(|s| s.lines.iter().any(|l| l.starts_with("No moon shows as a disc"))));
    }
}
