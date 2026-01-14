# Todo List

## Bugs & Critical Issues
- [ ] **Fix Unused Argument**: `stress_spread` CLI argument is parsed but never used. The `plates::spread_stress` function is also unused. Decide whether to integrate it or remove it.
- [ ] **Bounds Checking**: `Tilemap::index` does not check if `y` is within bounds, leading to panics. Add debug assertions or proper bounds checking.

## Refactoring
- [ ] **Deduplicate Gaussian Blur**: `plates::stress::smooth_stress` and `heightmap::convolve_gaussian` share nearly identical logic. `generate_gaussian_kernel` is also duplicated. Extract these into a common utility module (e.g., `utils.rs` or inside `tilemap.rs`).
- [ ] **Remove Unused Code**: If `plates::spread_stress` is not needed, remove it.
- [ ] **Clean Up Magic Numbers**: Extract hardcoded constants (elevation ranges, noise parameters, colors) into a configuration struct or constants module.

## Performance Improvements
- [ ] **Reduce Allocations**: 
    - `spread_stress` (if kept) clones the entire `Tilemap` in every iteration of the loop. Use double-buffering.
    - `enhance_stress` also clones.
- [ ] **Parallel Processing**: 
    - Use `rayon` to parallelize `generate_plates` (noise generation can be parallel).
    - Parallelize image export loops (especially `export_globe` which is computationally expensive).
    - Parallelize `calculate_stress` loops.

## Features & Enhancements
- [ ] **Configuration**: Support loading parameters from a config file (TOML/JSON) instead of just CLI args/hardcoded values.
- [ ] **Better Error Handling**: Replace `expect` calls with proper error propagation (e.g., using `anyhow`).
- [ ] **Progress Reporting**: Add a progress bar (e.g., `indicatif`) for long-running generation steps.
- [ ] **Enhanced CLI**: Add flags to control more generation parameters (ocean level, mountain height, etc.).

## Testing
- [ ] **Unit Tests**: Add unit tests for `Tilemap` (wrapping logic, bounds).
- [ ] **Integration Tests**: Add tests for the generation pipeline to ensure it runs without crashing and produces valid outputs.
