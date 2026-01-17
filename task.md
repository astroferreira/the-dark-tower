# Erosion Simulation Implementation Tasks

## Overview
Implement glacial and hydraulic erosion simulations for the planet generation project, integrating with the existing heightmap-based terrain system.

---

## Phase 1: Core Infrastructure
- [x] Create erosion module structure (`src/erosion/mod.rs`, etc.)
- [x] Define shared erosion parameter types and configuration
- [x] Extend `Tilemap` with gradient calculation utilities
- [x] Add erosion-related CLI arguments to `main.rs`

## Phase 1.5: Material / Rock Hardness Map
- [x] Create `RockType` enum (Basalt, Granite, Sandstone, Limestone, Shale, Sediment, Ice)
- [x] Implement hardness factor for each rock type
- [x] Generate material map based on plate types and terrain features
- [x] Create precomputed hardness map for erosion lookups
- [ ] Add material map export/visualization

## Phase 2: Hydraulic Erosion
- [x] Implement particle-based droplet erosion system
- [x] Add water droplet spawning and lifetime management
- [x] Implement gradient-following path tracing
- [x] Add sediment pickup (erosion) calculations with hardness factor
- [x] Implement sediment deposit mechanics
- [x] Add evaporation and inertia modeling
- [x] Create hydraulic erosion parameter tuning

## Phase 3: Glacial Erosion (SIA)
- [x] Implement ice thickness tracking layer
- [x] Add mass balance equation (accumulation/ablation)
- [x] Implement ice flux calculations (SIA)
- [x] Add basal sliding velocity computation
- [x] Implement bedrock erosion law
- [x] Create glacial erosion parameter configuration
- [x] Add temperature-dependent glaciation zones

## Phase 4: Integration & Pipeline
- [x] Integrate erosion into main generation pipeline
- [x] Add erosion iteration controls
- [ ] Implement erosion map exports
- [ ] Update viewer with erosion visualization modes
- [ ] Add before/after comparison exports

## Phase 5: Performance Optimization
- [ ] Profile erosion algorithms
- [ ] Add parallel iteration support with rayon
- [ ] Optimize memory usage in erosion passes
- [ ] Add early termination conditions

## Phase 6: Testing & Documentation
- [x] Test with various terrain parameters
- [x] Fix hydraulic erosion noise and improve river visibility
- [x] Improve river branching and meandering (natural rivulets)
- [x] Refine hardness map for variable erosion (noisy resistance)
- [ ] Document erosion parameters and tuning
- [ ] Create example outputs
- [ ] Update CLAUDE.md with new options
