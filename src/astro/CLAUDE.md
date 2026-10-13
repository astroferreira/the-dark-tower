# Star systems (`src/astro/`)

Every world circles a star of its own, drawn from the seed before the terrain
(`StarSystem::generate(seed, &SystemSpec)`); the climate, the tectonics, the viewer and the
journal read it. `StarSystem::sol()` is the Sun, the Earth (the Moon, the eight planets, the two
belts): its climate forcing is `PlanetForcing::default()`, the original model's arithmetic, so a
Sol world is bit-identical to one made before star systems (checked by diffing `--dev --present`
and `--dev --gazetteer` against `main`). `--dev` and world codes without a star part keep the Sun.

## What is drawn (`mod.rs`, separate RNG streams: star, orbit, spin, world, planets, names)
- Star (`star.rs`): class F 13%, G 34%, K 35%, M 18% (M0-M6; the habitable-zone fits stop at
  2600 K), subclass uniform; Pecaut & Mamajek's dwarf table (Teff, radius, mass), L = R^2 T^4;
  [Fe/H] ~ N(-0.05, 0.22); age 0.8 Gyr to 85% of the life (max 11 Gyr); Gough's brightening
  (30% fainter at birth); activity (XUV share vs the Sun's) = (age/4.57)^-1.5 M^-3, max 2000.
  Habitable zone: Kopparapu et al. 2014 (runaway greenhouse to maximum greenhouse).
- Orbit: zone position 22% in [-0.06, 0.04] (hot, at or just past the inner edge), 58% in
  [0.04, 0.2] (the Earth sits at 0.09), 20% in [0.2, 0.36]; flux = S_in (S_out/S_in)^h;
  eccentricity |N(0, 0.045)|, one in ten +0.08..0.25; a from the year-mean flux (1/sqrt(1-e^2)).
- Spin: primordial day lognormal around 12 h; braked by a moon's tides (x 1 + 1.6 age/4.5
  tide^0.6) and part of the way to synchronous by the star's (`tidal_lock_gyr`: Gladman et al.
  1996, Q 100, k2 0.3). Locked when the star is older than the lock time, except that around
  stars above 0.45 Msun the air's thermal tide holds 55% of them off (Leconte et al. 2015): a
  slow day of 4-40 Earth days. Locked worlds get tilt 0-3 deg and their eccentricity damped.
  Tilt: with a great moon N(23, 9) in 3..45; without, chaotic (Laskar): 35% 0-20, 42% 20-45,
  23% 45-85. 7% spin backwards (the sun rises in the west).
- Companions (stream 7): binary with p = 0.28 sqrt(M) (0.1-0.35); mass ratio 0.25-0.95 (type from
  `SpectralType::for_mass`, same age and metals). Three in ten are close pairs (0.04-0.25 AU x
  M^1/3, e < 0.3) that the world circles: `StarSystem::host()` sums their light and mass
  (`Star::pair`: light-weighted Teff) for the zone, the orbit, the climate and the planets; the
  pair shrinks until the world sits outside 0.75 of its stability limit ((2.3 + 3e) a_b, Holman &
  Wiegert), and no planet forms inside the limit. The rest are wide (40-1000 AU, e < 0.7), kept
  at least 6 a / (1 - e) out; they truncate the disk at a (1 - e)/3 and shine at magnitude -10
  to -18 (`companion_view`).
- 3:2 resonance: a world the tides would lock, on an orbit with e > 0.08, is caught at 3:2
  instead (65%; Mercury): it spins 1.5 times per orbit, a solar day lasts two years, tilt 0-3.
  `substellar_lon_deg` is then a hot longitude: `PlanetForcing::hot_longitudes` multiplies the
  zonal insolation by 1 + 1.5e cos(2 (lon - lon0)) (max 0.4), so the sea warms in two bands.
- Moons (`planets::moons`): a great moon 38% of the time (none when locked), 0-2 small ones.
- Bulk: radius is the Earth's (the map's scale); core fraction 0.32 + 0.12 [Fe/H] (+-0.04) ->
  density -> gravity (+-10%), which sets the lapse rate (6.5 C/km x g). Internal heat sqrt(M)
  exp(-(age-4.5)/8) 10^(0.2 [Fe/H]) -> hotspots 8 heat^0.8 (2-20) and CO2 outgassing (x
  lognormal 0.25). Magnetic field from the core and the spin.
- Water: lognormal(0.35) x exp(2 (a/snow line - 0.37)) x 10^(0.25 [Fe/H]), cut by a red dwarf's
  bright youth (M x0.35-0.9, K x0.8-1). It scales the style's land fraction by water^-0.7
  (0.45-1.9x; `TectonicParams::land_scale`, clamped 4-75%).
- Air (`air.rs`): N2 0.78 x lognormal(0.4) x g x retention (an active star strips it where the
  field is weak), 0.2-4 bar; O2 after 1.8 Gyr; CO2 from the climate's thermostat. CO2 forcing
  5.35 ln(p/280 ppm) + 30 ln(1 + p/0.7 bar) (48 W/m2 at 0.3 bar, 130 at 8); pressure broadening
  6 ln(p); Rayleigh albedo 0.03 (p - 1) (Teff/5772)^2.
- Neighbours (`planets.rs`): orbits stepped out and in from the world (x1.4-2.2, wider past the
  snow line at 2.7 sqrt(L) AU); rock inside the snow line, beyond it gas giants with p =
  0.3 M^0.8 10^(1.2 [Fe/H]) (Fischer & Valenti / Johnson), ice giants, icy dwarfs; pairs closer
  than 9 mutual Hill radii (12 with a giant) lose the lighter (never the world). Kinds by
  equilibrium temperature and mass (lava, hothouse, barren, cold desert, super-earth,
  mini-neptune). Belts inside the first giant and beyond the last.
- The sky (`sky.rs`): the sun's size and painted colour, daylight to the eye (visible share
  of a blackbody), sky colour and leaf colour by class (Kiang et al. 2007), aurora latitude
  from the magnetopause standoff, tides (moons + the star's unless locked), eclipses, each
  planet's brightest magnitude and disc, the day-night swing; `account()` is the prose
  (console `--system`, the heavens card, the journal's "The Heavens").

## Into the climate (`climate/forcing.rs`, `climate/thermostat.rs`; notes in `src/climate/CLAUDE.md`)
`StarSystem::climate_config` sets the solar constant (1361 L/a^2), the tilt and a
`PlanetForcing`: eccentricity and perihelion, spin, year, locked longitude, Teff, lapse rate and
the carbonate-silicate cycle's parameters (dry air, outgassing, beta 0.3-0.55, k 0.07-0.1/C).
`note_climate` stores what came out (`ClimateNotes`: mean, ice share, cells, equator/poles).

## Survey (2026-10-12, 30 seeds at 192x96)
4 frozen eyeball worlds (locked, 50-65% ice), 3 cold (20-31% ice), 7 hot (mean over 20 C, CO2
drawn down to ~10 ppm), the rest temperate; 5 slow turners (25-37-day days, one Hadley cell);
7 tilted over 45 deg (no polar caps; poles warmer than the equator over 54 deg). The thermostat's
zonal model and the 2-D climate agree within 1-3 C. Seed 42 (512x256) now circles a K6 dwarf,
locked, 68% ice; numbers quoted for seed 42 elsewhere are `--star sol`'s.

## Interfaces
- CLI: `--star auto|sol|F|G|K|M|K3|<code>`, `--hz`, `--obliquity`, `--day-hours`,
  `--eccentricity`, `--metallicity`, `--star-age`, `--locked true|false`, `--binary true|false`, `--system` (print the
  account, stop after the climate; works with `--load-world`), `--heavens-snapshot FILE`.
  Overrides on the Sun make a Sun-like generated system (G2, 4.57 Gyr, Earth's orbit) that
  follows them, through the new physics.
- World codes: an optional sixth part is `SystemSpec::code()` ("auto", "K3+h250+o450", no
  dots; "+b1"/"+b0" a companion); five-part codes are the Sun's. Plates carry `--star`. World files v7 append the system.
- Viewer: H on the world map opens the heavens card (`tiles/heavens.rs`; H/Esc close). Start
  screen: "The heavens" rows (the sun: from the seed / the Sun / a class; its orbit: from the
  seed / scorching / hot / like the Earth's / cool / cold), with the seed's star in the help;
  `PLANET_START_ROW=N` selects a row for `--start-snapshot`.

## The night's moon (`StarSystem::night_moon`)
- The widest moon seen as a disc (0.1 degrees or more; a moonlet is only a bright star). Its
  synodic month in days of 24 hours (clamped 6-60) is `LocalSky::month_days`, the camp's
  werebeast cycle (`colony/curse.rs`: the full moon on the middle day of the month; the Sun's
  camp keeps 28). With none, `LocalSky::moonless`: the werebeast comes "on the darkest night",
  the account says "No moon shows as a disc", and the history names nothing after a moon
  (`set_moonless`).

## Done since
- Tidal flats from the tidal range (`biomes::apply_tidal_flats`).
