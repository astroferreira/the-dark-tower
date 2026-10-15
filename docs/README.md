# docs/

What is here and whether it is current (2026-10-15). The living notes are `CLAUDE.md` at the
root, the `CLAUDE.md` beside each module, `ROADMAP.md` and `TODO.md`.

## reference/ (current: used by the generation roadmap)
- `numerical_tests.md`: ten geomorphometric tests for rivers and erosion (Horton ratios,
  drainage density, Hack's law, hypsometry...). The basis of `--region-survey` (ROADMAP Stage 1).
- `realistic_map_indicators.md`: more indicators (hypsometric integral, geomorphons, Moran's
  I, slope histograms).
- `glacial_erosion.md`: the shallow-ice approximation for glacial erosion. The legacy
  `erosion/glacial.rs` implements it; Stage 2's glacial legacy at region scale reads it again.

## colony/ (paused since 2026-10-15; the direction before the generation roadmap)
- `ROADMAP.md`: the six updates toward an autonomous colony chronicle (the former root roadmap).
- `TODO.md`: the colony-era short-term list (the former root TODO).
- `FUN_REVIEW_2026-10-05.md`: the design review of the colony game and the cards it produced.
- `MYSTERY_BOX.md`: what the `mystery_box` branch (the January fortress prototype) is worth.

## archive/ (legacy: describe the January 2026 terminal-era code; not maintained)
- `ASCII.md`, `RENDERING.md`: the frozen terminal front end (`--legacy-explorer`).
- `EROSION.md`, `RIVERS.md`: the legacy erosion (`--legacy-erosion`) and the old hydrology
  write-up; the current notes are `src/erosion/CLAUDE.md` and `src/climate/CLAUDE.md`.
- `BIOMES.md`: the fantasy-era biome reference (the realistic default keeps most of it off).
- `FEATURE_EVIDENCE.md`: the January fauna/colonist prototype.
- `TASK_HISTORY.md`: the January plan for the history simulation (built since, differently).

`audit.md` at the root is the terrain audit of 2026-10-13 (terrain revision 2); it stays there
because the terrain notes and tests cite its sections.
