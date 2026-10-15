# Zoomed regions (`src/region/`)

## Zoomed regions (`region/zoom.rs`)
- `--zoom X,Y` (world tile at the window centre) or
  `--zoom auto`; `--zoom-tiles N` (default 8), `--zoom-scale S` cells per world tile (default
  128, ~610 m/cell on a 512-wide world), `--zoom-erosion I` iterations (default 40).
  Writes `zoom_<seed>_<x>_<y>.png` and a 16-bit heightmap into `--map-output-dir`.
- Seamless: terrain is assembled from fixed world-tile chunks (each simulated with a one-tile
  border, cached in memory) blended with tent weights, so a cell's height depends only on its
  world position; separately generated regions match exactly (tested). Hydrology runs on the
  blended terrain with a margin (below). The date line too (2026-10-12): chunks are kept and
  simulated by their wrapped tile (`wrap` in `generate_zoom`), and the climate's and biome
  colours' warp noise is read at the wrapped position, so a region across the date line reads
  the same chunks as one on the other side (`--embark-seams`: the same embark from the region
  round 95,10 and round 1,10 of the dev world differs 0.00 m; it had been 17 m).
- The hydrology margin is 3 tiles on the audited terrain (2 on revision 1,
  `final_margin_tiles`): its relief drains in longer catchments, and with 2 regions 3 tiles
  apart differed on 3 of 8 test worlds (up to 3.4% of pixels; `region_swap_is_invisible`). A
  cold 8x8 region still takes ~0.6 s.
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
