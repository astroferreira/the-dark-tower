//! Glacial erosion simulation using the Shallow Ice Approximation (SIA).
//!
//! The SIA model treats ice as a viscous fluid flowing under gravity, with
//! the flow rate determined by ice thickness and surface slope. This creates
//! characteristic U-shaped valleys, cirques, and fjords.
//!
//! Key equations:
//! - Ice flux: q = -[(2A/(n+2)) * (ρg)^n * h^(n+2) * |∇s|^(n-1) + u_b*h] * ∇s
//! - Mass balance: ∂h/∂t = ȧ - ∇·q
//! - Erosion: ė = K * |u_b|^k

use crate::erosion::params::ErosionParams;
use crate::erosion::utils::{divergence_at_cell, surface_gradient_at_cell};
use crate::erosion::ErosionStats;
use crate::tilemap::Tilemap;

/// State for glacial erosion simulation
struct GlacialState {
    /// Bedrock elevation (modified by erosion)
    bedrock: Tilemap<f32>,
    /// Ice thickness
    ice_thickness: Tilemap<f32>,
    /// Ice flux in x direction
    flux_x: Tilemap<f32>,
    /// Ice flux in y direction
    flux_y: Tilemap<f32>,
    /// Basal sliding velocity magnitude
    sliding_velocity: Tilemap<f32>,
}

impl GlacialState {
    fn new(heightmap: &Tilemap<f32>) -> Self {
        let width = heightmap.width;
        let height = heightmap.height;

        Self {
            bedrock: heightmap.clone(),
            ice_thickness: Tilemap::new_with(width, height, 0.0f32),
            flux_x: Tilemap::new_with(width, height, 0.0f32),
            flux_y: Tilemap::new_with(width, height, 0.0f32),
            sliding_velocity: Tilemap::new_with(width, height, 0.0f32),
        }
    }

    fn width(&self) -> usize {
        self.bedrock.width
    }

    fn height(&self) -> usize {
        self.bedrock.height
    }

    /// Get surface elevation (bedrock + ice)
    fn surface(&self, x: usize, y: usize) -> f32 {
        *self.bedrock.get(x, y) + *self.ice_thickness.get(x, y)
    }
}

/// Run glacial erosion simulation.
///
/// Algorithm per timestep:
/// 1. Calculate mass balance field based on temperature/elevation
/// 2. Calculate surface gradient ∇s
/// 3. Calculate ice flux using SIA equation
/// 4. Update ice thickness using continuity equation
/// 5. Calculate basal sliding velocity from flux
/// 6. Apply erosion law to bedrock
pub fn simulate(
    heightmap: &mut Tilemap<f32>,
    temperature: &Tilemap<f32>,
    hardness: &Tilemap<f32>,
    params: &ErosionParams,
) -> ErosionStats {
    let mut stats = ErosionStats::default();
    stats.iterations = params.glacial_timesteps;

    // Initialize state
    let mut state = GlacialState::new(heightmap);

    // Pre-compute equilibrium line altitude (ELA) if not specified
    let ela = params.snowline_elevation.unwrap_or_else(|| {
        estimate_ela(temperature, heightmap)
    });

    // Run simulation timesteps
    for _ in 0..params.glacial_timesteps {
        // Step 1: Calculate mass balance
        let mass_balance = calculate_mass_balance(&state, temperature, ela, params);

        // Step 2 & 3: Calculate ice flux
        calculate_ice_flux(&mut state, params);

        // Step 4: Update ice thickness
        update_ice_thickness(&mut state, &mass_balance, params);

        // Step 5 & 6: Apply erosion
        let step_stats = apply_erosion(&mut state, hardness, params);
        stats.total_eroded += step_stats.total_eroded;
        stats.max_erosion = stats.max_erosion.max(step_stats.max_erosion);
    }

    // Copy eroded bedrock back to heightmap
    for y in 0..state.height() {
        for x in 0..state.width() {
            heightmap.set(x, y, *state.bedrock.get(x, y));
        }
    }

    stats
}

/// Estimate the Equilibrium Line Altitude (ELA) from temperature data.
/// ELA is approximately where mean annual temperature is at freezing threshold.
fn estimate_ela(temperature: &Tilemap<f32>, heightmap: &Tilemap<f32>) -> f32 {
    let mut sum_elevation = 0.0f64;
    let mut count = 0;

    for y in 0..temperature.height {
        for x in 0..temperature.width {
            let temp = *temperature.get(x, y);
            let elev = *heightmap.get(x, y);

            // Find cells near the glaciation temperature threshold
            if temp.abs() < 5.0 && elev > 0.0 {
                sum_elevation += elev as f64;
                count += 1;
            }
        }
    }

    if count > 0 {
        (sum_elevation / count as f64) as f32
    } else {
        // Default to a reasonable high elevation if no data
        2000.0
    }
}

/// Calculate mass balance (accumulation - ablation) for each cell.
/// Positive above ELA (accumulation), negative below (ablation).
fn calculate_mass_balance(
    state: &GlacialState,
    temperature: &Tilemap<f32>,
    ela: f32,
    params: &ErosionParams,
) -> Tilemap<f32> {
    let width = state.width();
    let height = state.height();
    let mut mass_balance = Tilemap::new_with(width, height, 0.0f32);

    for y in 0..height {
        for x in 0..width {
            let elevation = state.surface(x, y);
            let temp = *temperature.get(x, y);

            // Only accumulate ice in cold regions
            if temp > params.glaciation_temperature {
                // Too warm for glaciation
                if *state.ice_thickness.get(x, y) > 0.0 {
                    // Melt existing ice
                    mass_balance.set(x, y, -params.mass_balance_gradient * 10.0);
                }
                continue;
            }

            // Mass balance based on elevation relative to ELA
            let elevation_above_ela = elevation - ela;
            let balance = elevation_above_ela * params.mass_balance_gradient;

            // Clamp to reasonable bounds
            let clamped = balance.clamp(-5.0, 5.0);
            mass_balance.set(x, y, clamped);
        }
    }

    mass_balance
}

/// Calculate ice flux using the Shallow Ice Approximation.
/// q = -[(2A/(n+2)) * (ρg)^n * h^(n+2) * |∇s|^(n-1) + u_b*h] * ∇s
fn calculate_ice_flux(state: &mut GlacialState, params: &ErosionParams) {
    let width = state.width();
    let height = state.height();
    let n = params.glen_exponent;
    let a = params.ice_deform_coefficient;
    let u_b = params.ice_sliding_coefficient;

    // Simplified rho_g for our scaled simulation (not real physical units)
    let rho_g: f32 = 0.01;  // Scaled value for our heightmap units

    for y in 0..height {
        for x in 0..width {
            let h = *state.ice_thickness.get(x, y);

            if h <= 0.1 {
                // No significant ice, no flux
                state.flux_x.set(x, y, 0.0);
                state.flux_y.set(x, y, 0.0);
                state.sliding_velocity.set(x, y, 0.0);
                continue;
            }

            // Calculate surface gradient
            let (grad_x, grad_y) = surface_gradient_at_cell(
                &state.bedrock,
                &state.ice_thickness,
                x,
                y,
            );
            let grad_mag = (grad_x * grad_x + grad_y * grad_y).sqrt();

            if grad_mag < 1e-4 {
                // Flat surface, minimal flux
                state.flux_x.set(x, y, 0.0);
                state.flux_y.set(x, y, 0.0);
                state.sliding_velocity.set(x, y, 0.0);
                continue;
            }

            // Clamp ice thickness to reasonable values to prevent overflow
            let h_clamped = h.min(500.0);

            // SIA deformation term: (2A/(n+2)) * (ρg)^n * h^(n+2) * |∇s|^(n-1)
            let deform_coeff = (2.0 * a) / (n + 2.0);
            let rho_g_n = rho_g.powf(n);
            let h_n2 = h_clamped.powf(n + 2.0);
            let grad_n1 = grad_mag.powf(n - 1.0);
            let deform_term = (deform_coeff * rho_g_n * h_n2 * grad_n1).min(1e6);

            // Sliding term: u_b * h
            let sliding_term = (u_b * h_clamped).min(1e4);

            // Total flux magnitude (negative because flow is downslope)
            let flux_mag = -(deform_term + sliding_term);

            // Clamp flux to prevent numerical instability
            let flux_mag = flux_mag.clamp(-1e6, 1e6);

            // Flux components
            state.flux_x.set(x, y, flux_mag * grad_x);
            state.flux_y.set(x, y, flux_mag * grad_y);

            // Store sliding velocity for erosion calculation
            // Simplified sliding velocity proportional to ice thickness and slope
            let sliding_velocity = (u_b * h_clamped * grad_mag).min(100.0);
            state.sliding_velocity.set(x, y, sliding_velocity.abs());
        }
    }
}

/// Update ice thickness using the continuity equation.
/// ∂h/∂t = ȧ - ∇·q
fn update_ice_thickness(
    state: &mut GlacialState,
    mass_balance: &Tilemap<f32>,
    params: &ErosionParams,
) {
    let width = state.width();
    let height = state.height();
    let dt = params.glacial_dt;

    // Create a copy of ice thickness for updating
    let mut new_ice = state.ice_thickness.clone();

    for y in 0..height {
        for x in 0..width {
            let h = *state.ice_thickness.get(x, y);
            let m = *mass_balance.get(x, y);

            // Calculate flux divergence
            let div_q = divergence_at_cell(&state.flux_x, &state.flux_y, x, y);

            // Update: h_new = h + dt * (mass_balance - div_q)
            let dh = dt * (m - div_q);
            let h_new = (h + dh).max(0.0);  // Ice thickness can't be negative

            new_ice.set(x, y, h_new);
        }
    }

    state.ice_thickness = new_ice;
}

/// Apply erosion to bedrock based on basal sliding velocity.
/// ė = K * |u_b|^k * (1 - hardness)
fn apply_erosion(
    state: &mut GlacialState,
    hardness: &Tilemap<f32>,
    params: &ErosionParams,
) -> ErosionStats {
    let width = state.width();
    let height = state.height();
    let dt = params.glacial_dt;
    let k = params.erosion_coefficient;
    let exp = params.erosion_exponent;

    let mut stats = ErosionStats::default();

    // Maximum erosion per timestep
    let max_erosion_per_step = 5.0;

    for y in 0..height {
        for x in 0..width {
            let u_b = *state.sliding_velocity.get(x, y);
            let ice_thickness = *state.ice_thickness.get(x, y);

            if u_b <= 0.0 || ice_thickness < 10.0 {
                continue;
            }

            // Erosion rate: K * |u_b|^exp * ice_thickness_factor
            // Thicker ice = more pressure = more erosion
            let ice_factor = (ice_thickness / 200.0).min(1.5).max(0.1);
            let erosion_rate = k * u_b.powf(exp) * ice_factor;

            // Modulate by rock hardness (harder rock erodes less)
            let h = *hardness.get(x, y);
            let hardness_factor = 1.0 - h;
            let actual_erosion = (erosion_rate * hardness_factor * dt).min(max_erosion_per_step);

            if actual_erosion > 0.0 && actual_erosion.is_finite() {
                let current = *state.bedrock.get(x, y);
                state.bedrock.set(x, y, current - actual_erosion);

                stats.total_eroded += actual_erosion as f64;
                stats.max_erosion = stats.max_erosion.max(actual_erosion);
            }
        }
    }

    stats
}

/// Get the final ice thickness map (useful for visualization)
pub fn get_ice_thickness(
    heightmap: &Tilemap<f32>,
    temperature: &Tilemap<f32>,
    params: &ErosionParams,
) -> Tilemap<f32> {
    let mut state = GlacialState::new(heightmap);

    let ela = params.snowline_elevation.unwrap_or_else(|| {
        estimate_ela(temperature, heightmap)
    });

    // Run a few iterations to get equilibrium ice distribution
    for _ in 0..100 {
        let mass_balance = calculate_mass_balance(&state, temperature, ela, params);
        calculate_ice_flux(&mut state, params);
        update_ice_thickness(&mut state, &mass_balance, params);
    }

    state.ice_thickness
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ice_accumulates_in_cold() {
        let mut heightmap = Tilemap::new_with(32, 32, 0.0f32);
        // Create a simple bowl shape at high elevation
        for y in 0..32 {
            for x in 0..32 {
                let dx = x as f32 - 16.0;
                let dy = y as f32 - 16.0;
                let dist = (dx * dx + dy * dy).sqrt();
                let h = 3000.0 + dist * 10.0;  // Bowl at high elevation
                heightmap.set(x, y, h);
            }
        }

        // Very cold everywhere
        let temperature = Tilemap::new_with(32, 32, -20.0f32);
        let hardness = Tilemap::new_with(32, 32, 0.5f32);

        let params = ErosionParams {
            glacial_timesteps: 50,
            ..ErosionParams::default()
        };

        let stats = simulate(&mut heightmap, &temperature, &hardness, &params);

        // Should have eroded some bedrock
        assert!(stats.total_eroded > 0.0);
    }

    #[test]
    fn test_no_ice_in_warm() {
        let heightmap = Tilemap::new_with(32, 32, 100.0f32);  // Low elevation
        let temperature = Tilemap::new_with(32, 32, 20.0f32);  // Warm

        let params = ErosionParams::default();

        let ice = get_ice_thickness(&heightmap, &temperature, &params);

        // No ice should form in warm temperatures
        for (_, _, &h) in ice.iter() {
            assert!(h < 0.1, "Expected no ice, got {}", h);
        }
    }
}
