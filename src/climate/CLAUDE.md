# Climate and biomes (`src/climate/`)

## Climate (`climate/`)
- Temperature: Budyko-Sellers energy balance per season (`ebm.rs`), heat transport D = 3.0
  W/(m2 C) (zonal land means within ~2-4 C of Earth's), continentality, lapse rate. The
  annual mean is solved first; where its surface temperature is below -20 C
  (`PERENNIAL_ICE_C`) the seasons keep ice albedo, so ice sheets survive the polar summer
  (solving each season from scratch had melted them: +10 C summers at 80-90 deg). Seed 42
  warmest season on land: ~10 C at 60-70 deg, ~6 C at 70-80, -7 C at 80-90.
  `ClimateSimulation::warmest_season()` is the max of the four seasonal means.
- Precipitation (`moisture.rs`): vapour evaporates from the sea (toward 80% humidity), rides
  the steering-level wind (3.5x the surface wind, ~a cell per step: one step = a cell
  crossing, so the scheme is resolution-independent), and rains out at (step / 9-day
  residence) x 3 x humidity^2 x ascent, where ascent comes from sea-level pressure (ITCZ and
  subpolar lows wet, subtropical highs dry; capped at 1.15), plus orographic rain where the
  wind climbs the smoothed terrain. Land returns 92% of its rain to the air (40% in the cold),
  which carries rain into the interiors. 120 steps, mean of the last 80, 2-pass smoothing,
  precipitable water 2.5 mm per g/kg. Seed 42: land mean ~850 mm/yr (median ~390), ocean
  ~900, equatorial ocean ~2000, subtropical ~600, storm tracks ~1700.
- Closed basins: water-body detection marks a lake endorheic when its inflow can't match
  open-water evaporation (PET - P); it then covers only the area that balance allows.
  `water_bodies::apply_salt_flats` turns the rest of the basin floor (below the spill level,
  not under water, mean above 0 C) into `SaltFlats` (seed 42: 112 tiles, e.g. Lake Noubroorn in
  a rain shadow). Salt flats are no longer rolled onto random low desert.
- Runoff (`climate::runoff_mm`): precipitation minus actual evapotranspiration on Fu's Budyko
  curve (w 2.6), PET = 300 + 50 T mm/yr (`pet_mm`). Water-body detection sums it as flow
  (`water_bodies::RUNOFF_MM_PER_UNIT` = 3500 mm per unit keeps the river thresholds'
  scale); endorheic lakes balance inflow against open-water evaporation (PET - P).
- `terrain_lab` with `LAB_CLIMATE=1` stops after the climate and prints land precipitation
  quantiles, zonal land/ocean precipitation, precipitation and wind by distance from the
  coast, Budyko runoff, the biome mix, and writes `precip.png`.

## Biomes
- Lowland classification (`climate/biomes.rs::classify_lowland`) is a Whittaker diagram on
  mean annual temperature (ice below -22 C or -15 C when very wet, tundra to -10 C, taiga
  -10..4 C) and the moisture index P / (P + PET) (`climate::moisture_index`, the UNEP aridity
  index AI as AI / (1 + AI)): deserts below 0.17 (AI 0.2), steppe / savanna to 0.33-0.42,
  forests above ~0.4 (AI ~0.67), rainforest above 0.67 (AI 2). PET rises with temperature,
  so the same rain makes forest in the cold and steppe in the heat (Koppen's B rule).
  The polar band follows Koppen's summer rule when the warmest season is known
  (`Biome::classify_seasonal`, `generate_extended_biomes(.., Some(&warmest), ..)`): ice cap
  below -1.5 C, tundra below 8.5 C (a 3-month mean ~ a 10 C warmest month), taiga as far as
  summers stay warm however cold the winters. Seed 42: forests ~40%, desert 18%, grass/savanna
  28%, tundra 10% of land tiles; seed 7 (land on both poles) 24% ice. Embarks and the legacy
  world path still classify by annual means only.
- Seasonal rain (`biomes::apply_seasonal_biomes`, after biome generation, each hemisphere's own
  summer/winter): `MediterraneanShrubland` (winter 3x+ wetter than a dry summer, mild, 250-1500
  mm; replaces temperate grassland/forest; drawn as steppe with shrubs) and `MonsoonForest`
  (warm, summer 3x+ wetter, 1000-3500 mm; replaces seasonal tropical forest and wet savanna;
  savanna ground with deciduous trees). Seed 42: ~1700 Mediterranean tiles at 20-45 deg, ~1000
  monsoon tiles at 0-20 deg. Both appended at the end of `ExtendedBiome` (old saves load) and
  added to ecology, races, economy, road costs and gazetteer regions next to their analogues.
- `--fantasy 0..1` (default 0.2, old behaviour 0.5) scales fantasy/special biomes, including
  the rare-biome replacement pass. `--biome-stats` prints the land-biome mix, land
  temperature/moisture percentiles and a zonal temperature/land profile.
