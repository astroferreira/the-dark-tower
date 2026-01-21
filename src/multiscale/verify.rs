//! Verification system for world/local map consistency.
//!
//! This module provides a robust verification system to ensure consistency between
//! world map and local maps, including adjacent chunk boundaries and z-level continuity.
//!
//! # Verification Categories
//!
//! - **Structure Presence**: Verify WorldHistory structures appear in local maps
//! - **Boundary Coherence**: Verify adjacent chunks match at edges
//! - **Z-Level Reachability**: Verify paths exist from surface to underground
//! - **Geology Consistency**: Verify geology parameters match world data
//! - **Feature Continuity**: Verify no jarring transitions at boundaries

use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;

use crate::world::WorldData;

use super::local::{LocalChunk, LocalFeature, LocalTerrain};
use super::geology::derive_geology;
use super::LOCAL_SIZE;

/// Severity of a verification issue
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Low severity - minor visual inconsistency
    Low,
    /// Medium severity - noticeable issue but not game-breaking
    Medium,
    /// High severity - significant issue affecting gameplay
    High,
    /// Critical severity - structure/feature missing entirely
    Critical,
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Severity::Low => write!(f, "LOW"),
            Severity::Medium => write!(f, "MEDIUM"),
            Severity::High => write!(f, "HIGH"),
            Severity::Critical => write!(f, "CRITICAL"),
        }
    }
}

/// Category of verification check
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VerifyCategory {
    /// Structure from WorldHistory should appear in local map
    StructurePresence,
    /// Adjacent chunk boundaries should match
    BoundaryCoherence,
    /// Underground areas should be reachable from surface
    ZLevelReachability,
    /// Geology parameters should match world data
    GeologyConsistency,
    /// Features should be continuous across boundaries
    FeatureContinuity,
}

impl fmt::Display for VerifyCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            VerifyCategory::StructurePresence => write!(f, "Structure Presence"),
            VerifyCategory::BoundaryCoherence => write!(f, "Boundary Coherence"),
            VerifyCategory::ZLevelReachability => write!(f, "Z-Level Reachability"),
            VerifyCategory::GeologyConsistency => write!(f, "Geology Consistency"),
            VerifyCategory::FeatureContinuity => write!(f, "Feature Continuity"),
        }
    }
}

/// Result of a single verification check
#[derive(Clone, Debug)]
pub struct VerifyResult {
    /// Whether this check passed
    pub passed: bool,
    /// Category of the check
    pub category: VerifyCategory,
    /// Human-readable message describing the result
    pub message: String,
    /// Location in world coordinates (world_x, world_y, optional z)
    pub location: Option<(usize, usize, Option<i16>)>,
    /// Severity of the issue (if failed)
    pub severity: Severity,
}

impl VerifyResult {
    /// Create a passing result
    pub fn pass(category: VerifyCategory, message: impl Into<String>) -> Self {
        Self {
            passed: true,
            category,
            message: message.into(),
            location: None,
            severity: Severity::Low, // Irrelevant for passing
        }
    }

    /// Create a failing result
    pub fn fail(
        category: VerifyCategory,
        message: impl Into<String>,
        severity: Severity,
    ) -> Self {
        Self {
            passed: false,
            category,
            message: message.into(),
            location: None,
            severity,
        }
    }

    /// Add location to this result
    pub fn at(mut self, world_x: usize, world_y: usize) -> Self {
        self.location = Some((world_x, world_y, None));
        self
    }

    /// Add location with z-level to this result
    pub fn at_z(mut self, world_x: usize, world_y: usize, z: i16) -> Self {
        self.location = Some((world_x, world_y, Some(z)));
        self
    }
}

/// Direction for boundary checks
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    North,
    South,
    East,
    West,
}

impl Direction {
    /// Get the offset for this direction
    pub fn offset(&self) -> (i32, i32) {
        match self {
            Direction::North => (0, -1),
            Direction::South => (0, 1),
            Direction::East => (1, 0),
            Direction::West => (-1, 0),
        }
    }

    /// Get the opposite direction
    pub fn opposite(&self) -> Direction {
        match self {
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::East => Direction::West,
            Direction::West => Direction::East,
        }
    }
}

/// Overall verification status
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationStatus {
    /// All checks passed
    Passed,
    /// Some issues found but all are low/medium severity
    PartialPass,
    /// High or critical severity issues found
    Failed,
}

/// Statistics for a verification category
#[derive(Clone, Debug, Default)]
pub struct CategoryStats {
    /// Number of checks performed
    pub checks: usize,
    /// Number of checks that passed
    pub passed: usize,
    /// Number of checks that failed
    pub failed: usize,
}

impl CategoryStats {
    fn record(&mut self, passed: bool) {
        self.checks += 1;
        if passed {
            self.passed += 1;
        } else {
            self.failed += 1;
        }
    }
}

/// Complete verification report
#[derive(Clone, Debug)]
pub struct VerificationReport {
    /// World seed being verified
    pub seed: u64,
    /// Map dimensions
    pub map_size: (usize, usize),
    /// Number of chunks verified
    pub chunks_verified: usize,
    /// Number of iteration loops run
    pub iterations: usize,
    /// Overall status
    pub status: VerificationStatus,
    /// All issues found (failures only)
    pub issues: Vec<VerifyResult>,
    /// Statistics per category
    pub category_stats: HashMap<VerifyCategory, CategoryStats>,
}

impl VerificationReport {
    /// Create a new empty report
    pub fn new(seed: u64, map_size: (usize, usize)) -> Self {
        let mut category_stats = HashMap::new();
        category_stats.insert(VerifyCategory::StructurePresence, CategoryStats::default());
        category_stats.insert(VerifyCategory::BoundaryCoherence, CategoryStats::default());
        category_stats.insert(VerifyCategory::ZLevelReachability, CategoryStats::default());
        category_stats.insert(VerifyCategory::GeologyConsistency, CategoryStats::default());
        category_stats.insert(VerifyCategory::FeatureContinuity, CategoryStats::default());

        Self {
            seed,
            map_size,
            chunks_verified: 0,
            iterations: 0,
            status: VerificationStatus::Passed,
            issues: Vec::new(),
            category_stats,
        }
    }

    /// Add a verification result
    pub fn add_result(&mut self, result: VerifyResult) {
        if let Some(stats) = self.category_stats.get_mut(&result.category) {
            stats.record(result.passed);
        }

        if !result.passed {
            // Update status based on severity
            match result.severity {
                Severity::High | Severity::Critical => {
                    self.status = VerificationStatus::Failed;
                }
                Severity::Medium | Severity::Low => {
                    if self.status == VerificationStatus::Passed {
                        self.status = VerificationStatus::PartialPass;
                    }
                }
            }
            self.issues.push(result);
        }
    }

    /// Get issues filtered by severity
    pub fn issues_by_severity(&self, min_severity: Severity) -> Vec<&VerifyResult> {
        self.issues
            .iter()
            .filter(|r| r.severity >= min_severity)
            .collect()
    }

    /// Format report as a string for display
    pub fn format(&self) -> String {
        let mut output = String::new();

        output.push_str("═══════════════════════════════════════════\n");
        output.push_str("       WORLD/LOCAL VERIFICATION REPORT\n");
        output.push_str("═══════════════════════════════════════════\n\n");

        output.push_str(&format!("Seed: {}\n", self.seed));
        output.push_str(&format!("Map Size: {}x{}\n", self.map_size.0, self.map_size.1));
        output.push_str(&format!("Chunks Verified: {}\n", self.chunks_verified));
        output.push_str(&format!("Iterations: {}\n\n", self.iterations));

        output.push_str("SUMMARY:\n");
        for (category, stats) in &self.category_stats {
            let status = if stats.failed == 0 { "✓" } else { "✗" };
            output.push_str(&format!(
                "  {} {}: {}/{} passed\n",
                status,
                category,
                stats.passed,
                stats.checks
            ));
        }
        output.push('\n');

        if !self.issues.is_empty() {
            output.push_str(&format!("ISSUES ({}):\n", self.issues.len()));
            for issue in &self.issues {
                let loc_str = issue
                    .location
                    .map(|(x, y, z)| {
                        if let Some(z) = z {
                            format!(" at ({}, {}, z={})", x, y, z)
                        } else {
                            format!(" at ({}, {})", x, y)
                        }
                    })
                    .unwrap_or_default();

                output.push_str(&format!(
                    "  [{}] {}{}\n",
                    issue.severity, issue.message, loc_str
                ));
            }
            output.push('\n');
        }

        let status_str = match self.status {
            VerificationStatus::Passed => "PASSED",
            VerificationStatus::PartialPass => {
                let high_count = self.issues_by_severity(Severity::High).len();
                if high_count > 0 {
                    &format!("PARTIAL PASS ({} high-severity issues)", high_count)
                } else {
                    "PARTIAL PASS (minor issues only)"
                }
            }
            VerificationStatus::Failed => "FAILED",
        };

        output.push_str(&format!("STATUS: {}\n", status_str));
        output.push_str("═══════════════════════════════════════════\n");

        output
    }
}

// =============================================================================
// STRUCTURE PRESENCE VERIFICATION
// =============================================================================

/// Verify that structures from WorldHistory appear in local maps
pub fn verify_structure_presence(
    world: &WorldData,
    chunk: &LocalChunk,
    world_x: usize,
    world_y: usize,
) -> Vec<VerifyResult> {
    let mut results = Vec::new();

    // Get history if available
    let history = match &world.history {
        Some(h) => h,
        None => return results, // No history, nothing to verify
    };

    // Check for dungeons
    if let Some(dungeon_id) = history.dungeons.dungeons_by_location.get(&(world_x, world_y)) {
        if let Some(dungeon) = history.dungeons.dungeons.get(dungeon_id) {
            let has_entrance = chunk_has_feature(chunk, |f| {
                matches!(f, LocalFeature::StairsDown | LocalFeature::RampDown)
            });

            if has_entrance {
                results.push(
                    VerifyResult::pass(
                        VerifyCategory::StructurePresence,
                        format!("Dungeon '{}' entrance present", dungeon.name),
                    )
                    .at(world_x, world_y),
                );
            } else {
                results.push(
                    VerifyResult::fail(
                        VerifyCategory::StructurePresence,
                        format!("Dungeon '{}' missing entrance", dungeon.name),
                        Severity::Critical,
                    )
                    .at(world_x, world_y),
                );
            }
        }
    }

    // Check for monster lairs
    for lair in history.monsters.lairs.values() {
        if lair.x == world_x && lair.y == world_y {
            // Monster lairs should have some evidence (rubble, special features)
            let has_lair_evidence = chunk_has_feature(chunk, |f| {
                matches!(
                    f,
                    LocalFeature::Rubble
                        | LocalFeature::Boulder
                        | LocalFeature::Chest
                        | LocalFeature::StairsDown
                )
            });

            if has_lair_evidence {
                results.push(
                    VerifyResult::pass(
                        VerifyCategory::StructurePresence,
                        format!("Monster lair '{}' evidence present", lair.name),
                    )
                    .at(world_x, world_y),
                );
            } else {
                results.push(
                    VerifyResult::fail(
                        VerifyCategory::StructurePresence,
                        format!("Monster lair '{}' missing evidence", lair.name),
                        Severity::High,
                    )
                    .at(world_x, world_y),
                );
            }
        }
    }

    // Check for settlements (villages)
    for settlement in history.territories.settlements.values() {
        let dx = (settlement.x as i32 - world_x as i32).abs();
        let dy = (settlement.y as i32 - world_y as i32).abs();
        if dx <= 2 && dy <= 2 {
            // Settlement area should have constructed features
            let has_construction = chunk_has_terrain(chunk, |t| {
                matches!(
                    t,
                    LocalTerrain::ConstructedFloor { .. } | LocalTerrain::ConstructedWall { .. }
                )
            });

            if has_construction {
                results.push(
                    VerifyResult::pass(
                        VerifyCategory::StructurePresence,
                        format!("Settlement '{}' construction present", settlement.name),
                    )
                    .at(world_x, world_y),
                );
            } else if dx == 0 && dy == 0 {
                // Only fail at exact settlement location
                results.push(
                    VerifyResult::fail(
                        VerifyCategory::StructurePresence,
                        format!("Settlement '{}' missing construction", settlement.name),
                        Severity::High,
                    )
                    .at(world_x, world_y),
                );
            }
            break; // Only check one settlement per tile
        }
    }

    results
}

/// Check if chunk has any feature matching the predicate
fn chunk_has_feature<F>(chunk: &LocalChunk, predicate: F) -> bool
where
    F: Fn(&LocalFeature) -> bool,
{
    for z in chunk.z_min..=chunk.z_max {
        for y in 0..LOCAL_SIZE {
            for x in 0..LOCAL_SIZE {
                let tile = chunk.get(x, y, z);
                if predicate(&tile.feature) {
                    return true;
                }
            }
        }
    }
    false
}

/// Check if chunk has any terrain matching the predicate
fn chunk_has_terrain<F>(chunk: &LocalChunk, predicate: F) -> bool
where
    F: Fn(&LocalTerrain) -> bool,
{
    for z in chunk.z_min..=chunk.z_max {
        for y in 0..LOCAL_SIZE {
            for x in 0..LOCAL_SIZE {
                let tile = chunk.get(x, y, z);
                if predicate(&tile.terrain) {
                    return true;
                }
            }
        }
    }
    false
}

// =============================================================================
// BOUNDARY COHERENCE VERIFICATION
// =============================================================================

/// Verify that adjacent chunks match at their shared boundary
pub fn verify_boundary_coherence(
    chunk_a: &LocalChunk,
    chunk_b: &LocalChunk,
    direction: Direction,
) -> Vec<VerifyResult> {
    let mut results = Vec::new();

    // Get edge tiles based on direction
    let (edge_a, edge_b) = get_boundary_edges(chunk_a, chunk_b, direction);

    let world_x = chunk_a.world_x;
    let world_y = chunk_a.world_y;

    // Check surface_z consistency along the boundary
    let mut z_mismatch_count = 0;

    for z in chunk_a.z_min..=chunk_a.z_max {
        for i in 0..LOCAL_SIZE {
            let tile_a = &edge_a[i];
            let tile_b = &edge_b[i];

            // Check terrain type at surface
            if z == chunk_a.surface_z {
                let a_solid = tile_a[z as usize - chunk_a.z_min as usize].terrain.is_solid();
                let b_solid = tile_b[z as usize - chunk_b.z_min as usize].terrain.is_solid();

                if a_solid != b_solid {
                    z_mismatch_count += 1;
                }
            }
        }
    }

    // Surface z difference
    let surface_diff = (chunk_a.surface_z - chunk_b.surface_z).abs();
    if surface_diff > 2 {
        results.push(
            VerifyResult::fail(
                VerifyCategory::BoundaryCoherence,
                format!(
                    "Surface z mismatch at {:?} boundary: {} vs {}",
                    direction, chunk_a.surface_z, chunk_b.surface_z
                ),
                if surface_diff > 4 {
                    Severity::High
                } else {
                    Severity::Medium
                },
            )
            .at(world_x, world_y),
        );
    } else {
        results.push(
            VerifyResult::pass(
                VerifyCategory::BoundaryCoherence,
                format!("Surface z consistent at {:?} boundary", direction),
            )
            .at(world_x, world_y),
        );
    }

    // Check for major terrain mismatches
    if z_mismatch_count > LOCAL_SIZE / 4 {
        results.push(
            VerifyResult::fail(
                VerifyCategory::BoundaryCoherence,
                format!(
                    "Terrain mismatch at {:?} boundary: {} tiles differ",
                    direction, z_mismatch_count
                ),
                Severity::Medium,
            )
            .at(world_x, world_y),
        );
    }

    results
}

/// Get the edge tiles for both chunks at their shared boundary
fn get_boundary_edges(
    chunk_a: &LocalChunk,
    chunk_b: &LocalChunk,
    direction: Direction,
) -> (Vec<Vec<super::local::LocalTile>>, Vec<Vec<super::local::LocalTile>>) {
    let z_count = (chunk_a.z_max - chunk_a.z_min + 1) as usize;

    let mut edge_a = Vec::with_capacity(LOCAL_SIZE);
    let mut edge_b = Vec::with_capacity(LOCAL_SIZE);

    match direction {
        Direction::East => {
            // chunk_a's eastern edge (x=47) vs chunk_b's western edge (x=0)
            for y in 0..LOCAL_SIZE {
                let mut col_a = Vec::with_capacity(z_count);
                let mut col_b = Vec::with_capacity(z_count);
                for z in chunk_a.z_min..=chunk_a.z_max {
                    col_a.push(*chunk_a.get(LOCAL_SIZE - 1, y, z));
                    col_b.push(*chunk_b.get(0, y, z));
                }
                edge_a.push(col_a);
                edge_b.push(col_b);
            }
        }
        Direction::West => {
            // chunk_a's western edge (x=0) vs chunk_b's eastern edge (x=47)
            for y in 0..LOCAL_SIZE {
                let mut col_a = Vec::with_capacity(z_count);
                let mut col_b = Vec::with_capacity(z_count);
                for z in chunk_a.z_min..=chunk_a.z_max {
                    col_a.push(*chunk_a.get(0, y, z));
                    col_b.push(*chunk_b.get(LOCAL_SIZE - 1, y, z));
                }
                edge_a.push(col_a);
                edge_b.push(col_b);
            }
        }
        Direction::South => {
            // chunk_a's southern edge (y=47) vs chunk_b's northern edge (y=0)
            for x in 0..LOCAL_SIZE {
                let mut col_a = Vec::with_capacity(z_count);
                let mut col_b = Vec::with_capacity(z_count);
                for z in chunk_a.z_min..=chunk_a.z_max {
                    col_a.push(*chunk_a.get(x, LOCAL_SIZE - 1, z));
                    col_b.push(*chunk_b.get(x, 0, z));
                }
                edge_a.push(col_a);
                edge_b.push(col_b);
            }
        }
        Direction::North => {
            // chunk_a's northern edge (y=0) vs chunk_b's southern edge (y=47)
            for x in 0..LOCAL_SIZE {
                let mut col_a = Vec::with_capacity(z_count);
                let mut col_b = Vec::with_capacity(z_count);
                for z in chunk_a.z_min..=chunk_a.z_max {
                    col_a.push(*chunk_a.get(x, 0, z));
                    col_b.push(*chunk_b.get(x, LOCAL_SIZE - 1, z));
                }
                edge_a.push(col_a);
                edge_b.push(col_b);
            }
        }
    }

    (edge_a, edge_b)
}

// =============================================================================
// Z-LEVEL REACHABILITY VERIFICATION
// =============================================================================

/// Verify that all non-solid z-levels are reachable from the surface
pub fn verify_z_reachability(chunk: &LocalChunk) -> Vec<VerifyResult> {
    let mut results = Vec::new();
    let surface_z = chunk.surface_z;

    // Build connectivity graph using BFS
    let mut reachable: HashSet<i16> = HashSet::new();
    let mut queue: VecDeque<i16> = VecDeque::new();

    // Start from surface
    queue.push_back(surface_z);
    reachable.insert(surface_z);

    // BFS through stairs/ramps/ladders
    while let Some(z) = queue.pop_front() {
        // Check all tiles at this z-level for vertical movement features
        for y in 0..LOCAL_SIZE {
            for x in 0..LOCAL_SIZE {
                let tile = chunk.get(x, y, z);

                match tile.feature {
                    LocalFeature::StairsDown | LocalFeature::RampDown => {
                        let next_z = z - 1;
                        if next_z >= chunk.z_min && !reachable.contains(&next_z) {
                            reachable.insert(next_z);
                            queue.push_back(next_z);
                        }
                    }
                    LocalFeature::StairsUp | LocalFeature::RampUp => {
                        let next_z = z + 1;
                        if next_z <= chunk.z_max && !reachable.contains(&next_z) {
                            reachable.insert(next_z);
                            queue.push_back(next_z);
                        }
                    }
                    LocalFeature::Ladder => {
                        // Ladders allow movement in both directions
                        for next_z in [z - 1, z + 1] {
                            if next_z >= chunk.z_min
                                && next_z <= chunk.z_max
                                && !reachable.contains(&next_z)
                            {
                                reachable.insert(next_z);
                                queue.push_back(next_z);
                            }
                        }
                    }
                    _ => {}
                }
            }
        }
    }

    // Check for unreachable z-levels that have non-solid tiles
    let mut unreachable_levels = Vec::new();
    for z in chunk.z_min..=chunk.z_max {
        if !reachable.contains(&z) && has_non_solid_tiles(chunk, z) {
            unreachable_levels.push(z);
        }
    }

    if unreachable_levels.is_empty() {
        results.push(VerifyResult::pass(
            VerifyCategory::ZLevelReachability,
            "All non-solid z-levels reachable from surface",
        ));
    } else {
        // Report unreachable levels (limit to first 5)
        for &z in unreachable_levels.iter().take(5) {
            results.push(
                VerifyResult::fail(
                    VerifyCategory::ZLevelReachability,
                    format!("Unreachable z-level {} from surface", z),
                    Severity::High,
                )
                .at_z(chunk.world_x, chunk.world_y, z),
            );
        }

        if unreachable_levels.len() > 5 {
            results.push(VerifyResult::fail(
                VerifyCategory::ZLevelReachability,
                format!(
                    "...and {} more unreachable z-levels",
                    unreachable_levels.len() - 5
                ),
                Severity::High,
            ));
        }
    }

    results
}

/// Check if a z-level has any non-solid tiles
fn has_non_solid_tiles(chunk: &LocalChunk, z: i16) -> bool {
    for y in 0..LOCAL_SIZE {
        for x in 0..LOCAL_SIZE {
            let tile = chunk.get(x, y, z);
            if !tile.terrain.is_solid() {
                return true;
            }
        }
    }
    false
}

// =============================================================================
// GEOLOGY CONSISTENCY VERIFICATION
// =============================================================================

/// Verify that chunk geology parameters match world data
pub fn verify_geology_consistency(
    world: &WorldData,
    chunk: &LocalChunk,
    world_x: usize,
    world_y: usize,
) -> Vec<VerifyResult> {
    let mut results = Vec::new();

    // Derive expected geology from world data
    let expected = derive_geology(world, world_x, world_y);
    let actual = &chunk.geology;

    // Check surface_z
    let surface_diff = (expected.surface_z - actual.surface_z).abs();
    if surface_diff > 2 {
        results.push(
            VerifyResult::fail(
                VerifyCategory::GeologyConsistency,
                format!(
                    "Surface z mismatch: expected {}, got {}",
                    expected.surface_z, actual.surface_z
                ),
                Severity::Medium,
            )
            .at(world_x, world_y),
        );
    } else {
        results.push(
            VerifyResult::pass(
                VerifyCategory::GeologyConsistency,
                "Surface z consistent with world data",
            )
            .at(world_x, world_y),
        );
    }

    // Check biome
    if expected.biome != actual.biome {
        results.push(
            VerifyResult::fail(
                VerifyCategory::GeologyConsistency,
                format!(
                    "Biome mismatch: expected {:?}, got {:?}",
                    expected.biome, actual.biome
                ),
                Severity::Medium,
            )
            .at(world_x, world_y),
        );
    }

    // Check temperature (within reasonable tolerance)
    let temp_diff = (expected.temperature - actual.temperature).abs();
    if temp_diff > 5.0 {
        results.push(
            VerifyResult::fail(
                VerifyCategory::GeologyConsistency,
                format!(
                    "Temperature mismatch: expected {:.1}C, got {:.1}C",
                    expected.temperature, actual.temperature
                ),
                Severity::Low,
            )
            .at(world_x, world_y),
        );
    }

    // Check primary stone type
    if expected.primary_stone != actual.primary_stone {
        results.push(
            VerifyResult::fail(
                VerifyCategory::GeologyConsistency,
                format!(
                    "Stone type mismatch: expected {:?}, got {:?}",
                    expected.primary_stone, actual.primary_stone
                ),
                Severity::Low,
            )
            .at(world_x, world_y),
        );
    }

    results
}

// =============================================================================
// MAIN VERIFICATION FUNCTIONS
// =============================================================================

/// Run all verification checks on a single chunk
pub fn verify_chunk(
    world: &WorldData,
    chunk: &LocalChunk,
    world_x: usize,
    world_y: usize,
) -> Vec<VerifyResult> {
    let mut results = Vec::new();

    // Structure presence
    results.extend(verify_structure_presence(world, chunk, world_x, world_y));

    // Z-level reachability
    results.extend(verify_z_reachability(chunk));

    // Geology consistency
    results.extend(verify_geology_consistency(world, chunk, world_x, world_y));

    results
}

/// Run verification on the world with sample locations
///
/// This is the main entry point for verification. It samples locations
/// across the world to verify consistency.
pub fn verify_world_sample(
    world: &WorldData,
    generate_chunk: impl Fn(&WorldData, usize, usize) -> LocalChunk,
    sample_count: usize,
) -> VerificationReport {
    let mut report = VerificationReport::new(world.seed, (world.width, world.height));

    // Generate sample locations
    let samples = get_verification_samples(world, sample_count);

    for (world_x, world_y) in &samples {
        // Generate chunk
        let chunk = generate_chunk(world, *world_x, *world_y);
        report.chunks_verified += 1;

        // Run verification
        let results = verify_chunk(world, &chunk, *world_x, *world_y);
        for result in results {
            report.add_result(result);
        }

        // Check boundaries with adjacent chunks
        let neighbors = [
            (*world_x + 1, *world_y, Direction::East),
            (world_x.saturating_sub(1), *world_y, Direction::West),
            (*world_x, *world_y + 1, Direction::South),
            (*world_x, world_y.saturating_sub(1), Direction::North),
        ];

        for (adj_x, adj_y, direction) in neighbors {
            if adj_x < world.width && adj_y < world.height && (adj_x, adj_y) != (*world_x, *world_y)
            {
                let adj_chunk = generate_chunk(world, adj_x, adj_y);
                let boundary_results = verify_boundary_coherence(&chunk, &adj_chunk, direction);
                for result in boundary_results {
                    report.add_result(result);
                }
            }
        }
    }

    report.iterations = 1;
    report
}

/// Get sample locations for verification
fn get_verification_samples(world: &WorldData, count: usize) -> Vec<(usize, usize)> {
    use rand::Rng;
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    let mut rng = ChaCha8Rng::seed_from_u64(world.seed.wrapping_add(0xBEEF));
    let mut samples = Vec::with_capacity(count);

    // Always include corners and center
    let corners = [
        (10, 10),
        (world.width - 10, 10),
        (10, world.height - 10),
        (world.width - 10, world.height - 10),
        (world.width / 2, world.height / 2),
    ];

    for (x, y) in corners {
        if x < world.width && y < world.height {
            samples.push((x, y));
        }
    }

    // Add locations with structures from history
    if let Some(ref history) = world.history {
        // Add dungeon locations
        for (loc, _) in history.dungeons.dungeons_by_location.iter().take(5) {
            if !samples.contains(loc) {
                samples.push(*loc);
            }
        }

        // Add settlement locations
        for settlement in history.territories.settlements.values().take(5) {
            let loc = (settlement.x, settlement.y);
            if !samples.contains(&loc) {
                samples.push(loc);
            }
        }

        // Add monster lair locations
        for lair in history.monsters.lairs.values().take(5) {
            let loc = (lair.x, lair.y);
            if !samples.contains(&loc) {
                samples.push(loc);
            }
        }
    }

    // Fill remaining with random locations
    while samples.len() < count {
        let x = rng.gen_range(5..world.width - 5);
        let y = rng.gen_range(5..world.height - 5);
        let loc = (x, y);
        if !samples.contains(&loc) {
            samples.push(loc);
        }
    }

    samples.truncate(count);
    samples
}

/// Quick verification with minimal samples (for testing)
pub fn verify_world_quick(
    world: &WorldData,
    generate_chunk: impl Fn(&WorldData, usize, usize) -> LocalChunk,
) -> VerificationReport {
    verify_world_sample(world, generate_chunk, 10)
}

/// Thorough verification with many samples
pub fn verify_world_thorough(
    world: &WorldData,
    generate_chunk: impl Fn(&WorldData, usize, usize) -> LocalChunk,
) -> VerificationReport {
    verify_world_sample(world, generate_chunk, 64)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_chunk(world_x: usize, world_y: usize, surface_z: i16) -> LocalChunk {
        LocalChunk::new(world_x, world_y, surface_z)
    }

    #[test]
    fn test_verify_result_creation() {
        let pass = VerifyResult::pass(VerifyCategory::StructurePresence, "Test passed");
        assert!(pass.passed);

        let fail = VerifyResult::fail(
            VerifyCategory::BoundaryCoherence,
            "Test failed",
            Severity::High,
        )
        .at(10, 20);

        assert!(!fail.passed);
        assert_eq!(fail.location, Some((10, 20, None)));
    }

    #[test]
    fn test_verification_report() {
        let mut report = VerificationReport::new(42, (512, 256));

        report.add_result(VerifyResult::pass(
            VerifyCategory::StructurePresence,
            "Test",
        ));
        report.add_result(VerifyResult::fail(
            VerifyCategory::BoundaryCoherence,
            "Test fail",
            Severity::Medium,
        ));

        assert_eq!(report.status, VerificationStatus::PartialPass);
        assert_eq!(report.issues.len(), 1);

        let formatted = report.format();
        assert!(formatted.contains("PARTIAL PASS"));
    }

    #[test]
    fn test_z_reachability() {
        let mut chunk = make_test_chunk(0, 0, 5);

        // Add stairs going down from surface
        chunk.get_mut(24, 24, 5).feature = LocalFeature::StairsDown;

        // Make level 4 have non-solid tiles (passable floor)
        chunk.get_mut(24, 24, 4).terrain = LocalTerrain::CaveFloor;

        let results = verify_z_reachability(&chunk);

        // Level 4 should be reachable via stairs from level 5
        // So z=4 should NOT appear in failures
        let z4_failures: Vec<_> = results
            .iter()
            .filter(|r| !r.passed && r.location.map(|(_, _, z)| z == Some(4)).unwrap_or(false))
            .collect();

        assert!(
            z4_failures.is_empty(),
            "Z-level 4 should be reachable via stairs from surface, but got failures: {:?}",
            z4_failures
        );
    }

    #[test]
    fn test_severity_ordering() {
        assert!(Severity::Critical > Severity::High);
        assert!(Severity::High > Severity::Medium);
        assert!(Severity::Medium > Severity::Low);
    }
}
