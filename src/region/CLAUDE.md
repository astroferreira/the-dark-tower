# Zoomed regions (`src/region/`)

## Zoomed regions (`region/zoom.rs`)
- `--zoom X,Y` (world tile at the window centre) or
  `--zoom auto`; `--zoom-tiles N` (default 8), `--zoom-scale S` cells per world tile (default
  128, ~610 m/cell on a 512-wide world), `--zoom-erosion I` iterations (default 40).
  Writes `zoom_<seed>_<x>_<y>.png` and a 16-bit heightmap into `--map-output-dir`.
- Seamless: terrain is assembled from fixed world-tile chunks (each simulated with a one-tile
  border, cached in memory) blended with tent weights, so a cell's height depends only on its
  world position; separately generated regions match exactly (tested). Hydrology runs on the
  blended terrain with a two-tile margin. Known exception: the date line (x wrap).
- Pipeline (per chunk): bicubic world heights + relief-scaled fractal detail -> world Bezier rivers carve
  valleys and inject upstream area where they enter -> shallow hollows filled -> implicit
  stream-power erosion + hillslope diffusion on a priority-flood drainage tree -> lakes and
  rivers (moisture-dependent threshold, width ~ 0.8 sqrt(A km2) m). ~7 s for a cold
  1024x1024 region; neighbouring regions reuse cached chunks.
- Render uses the climate colour LUT only (world biome colours and raw world climate are
  tile-blocky when upsampled; climate is smoothed over ~1 tile first).
- `--province-snapshot PREFIX` (dev embark, or `--tiles-center X,Y`) renders the colony's
  theatre: the zoomed region around the embark, a 250 km ring, lines to its three nearest
  living places with the days to walk each (25 km/day x 1.3 winding), and prints them. See the
  play-scale decision in ROADMAP (Update 5).
