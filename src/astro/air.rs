//! The world's air: what it is made of and what it does to the climate. Nitrogen comes with the
//! world's volatiles and is stripped by an active star where the magnetic field is weak; oxygen
//! follows life; CO2 is set by the carbonate-silicate cycle (`climate::thermostat`): volcanoes
//! breathe it out, rain and rock draw it down faster the warmer it is (Walker, Hays & Kasting
//! 1981), so a world far out in its zone builds a thick CO2 blanket and one near the inner edge
//! keeps almost none.

use serde::{Deserialize, Serialize};

/// Earth's pre-industrial CO2 (bar).
pub const EARTH_CO2_BAR: f32 = 2.8e-4;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Air {
    pub n2_bar: f32,
    pub o2_bar: f32,
    pub co2_bar: f32,
    pub argon_bar: f32,
}

impl Air {
    pub fn earth() -> Self { Air { n2_bar: 0.781, o2_bar: 0.209, co2_bar: EARTH_CO2_BAR, argon_bar: 0.0093 } }

    /// Surface pressure (bar).
    pub fn pressure_bar(&self) -> f32 { self.n2_bar + self.o2_bar + self.co2_bar + self.argon_bar }

    /// The air without its CO2 (bar).
    pub fn dry_bar(&self) -> f32 { self.n2_bar + self.o2_bar + self.argon_bar }
}

/// CO2's greenhouse forcing relative to 280 ppm (W/m2): the logarithmic law of the trace gas
/// (Myhre et al. 1998) plus a pressure-broadened term that takes over at tenths of a bar and
/// more, fitted to 1-D radiative-convective results (~48 W/m2 at 0.3 bar, ~130 at 8 bar).
pub fn co2_forcing_wm2(co2_bar: f32) -> f32 {
    let p = co2_bar.max(1e-7);
    5.35 * (p / EARTH_CO2_BAR).ln() + 30.0 * ((1.0 + p / 0.7).ln() - (1.0 + EARTH_CO2_BAR / 0.7).ln())
}

/// Pressure broadening by the background gas (W/m2): more nitrogen widens the absorption lines
/// of CO2 and water (Goldblatt et al. 2009: twice Earth's nitrogen is worth ~4 C).
pub fn broadening_wm2(pressure_bar: f32) -> f32 { 6.0 * pressure_bar.max(0.05).ln() }

/// Extra planetary albedo from Rayleigh scattering by an atmosphere of `pressure_bar` (zero at
/// Earth's): thick air scatters starlight back to space, more so under a blue-white star.
pub fn rayleigh_albedo(pressure_bar: f32, teff: f32) -> f32 {
    (0.03 * (pressure_bar - 1.0) * (teff / 5772.0).powi(2)).clamp(-0.03, 0.25)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn co2_warms_by_the_known_amounts() {
        assert!(co2_forcing_wm2(EARTH_CO2_BAR).abs() < 1e-4);
        let doubling = co2_forcing_wm2(2.0 * EARTH_CO2_BAR);
        assert!((3.5..4.0).contains(&doubling), "doubling {doubling}");
        let tenth = co2_forcing_wm2(0.3);
        assert!((40.0..60.0).contains(&tenth), "0.3 bar {tenth}");
        let thick = co2_forcing_wm2(8.0);
        assert!((115.0..145.0).contains(&thick), "8 bar {thick}");
    }
}
