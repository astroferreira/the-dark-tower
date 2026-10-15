/// A 2D tilemap grid with equirectangular projection (wraps horizontally).
#[derive(serde::Serialize, serde::Deserialize)]
#[derive(Clone)]
pub struct Tilemap<T> {
    pub width: usize,
    pub height: usize,
    data: Vec<T>,
}

impl<T: Clone + Default> Tilemap<T> {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![T::default(); width * height],
        }
    }
}

impl<T: Clone> Tilemap<T> {
    pub fn new_with(width: usize, height: usize, value: T) -> Self {
        Self {
            width,
            height,
            data: vec![value; width * height],
        }
    }

    /// Create a Tilemap from a pre-computed vector.
    /// The vector should be in row-major order (y * width + x).
    /// Panics if the vector length doesn't match width * height.
    pub fn from_vec(width: usize, height: usize, data: Vec<T>) -> Self {
        assert_eq!(data.len(), width * height, "Data length must match width * height");
        Self { width, height, data }
    }

    /// Get the index into the data array, handling horizontal wrapping.
    fn index(&self, x: usize, y: usize) -> usize {
        let x = x % self.width; // Wrap horizontally
        y * self.width + x
    }

    pub fn get(&self, x: usize, y: usize) -> &T {
        &self.data[self.index(x, y)]
    }

    pub fn get_mut(&mut self, x: usize, y: usize) -> &mut T {
        let idx = self.index(x, y);
        &mut self.data[idx]
    }

    pub fn set(&mut self, x: usize, y: usize, value: T) {
        let idx = self.index(x, y);
        self.data[idx] = value;
    }

    /// Fill the entire map with a value.
    pub fn fill(&mut self, value: T) where T: Clone {
        self.data.fill(value);
    }

    /// Get neighbors with horizontal wrapping (4-connectivity).
    /// Returns up to 4 neighbors (up, down, left, right).
    /// Top and bottom edges don't wrap.
    pub fn neighbors(&self, x: usize, y: usize) -> Vec<(usize, usize)> {
        let mut result = Vec::with_capacity(4);

        // Left (wraps)
        let left_x = if x == 0 { self.width - 1 } else { x - 1 };
        result.push((left_x, y));

        // Right (wraps)
        let right_x = if x == self.width - 1 { 0 } else { x + 1 };
        result.push((right_x, y));

        // Up (no wrap at top)
        if y > 0 {
            result.push((x, y - 1));
        }

        // Down (no wrap at bottom)
        if y < self.height - 1 {
            result.push((x, y + 1));
        }

        result
    }

    /// Get 8-connected neighbors with horizontal wrapping.
    /// Returns up to 8 neighbors (including diagonals).
    /// Top and bottom edges don't wrap vertically.
    /// Use this for more organic/circular expansion patterns.
    pub fn neighbors_8(&self, x: usize, y: usize) -> Vec<(usize, usize)> {
        let mut result = Vec::with_capacity(8);

        for dy in -1i32..=1 {
            for dx in -1i32..=1 {
                // Skip self
                if dx == 0 && dy == 0 {
                    continue;
                }

                // Handle X wrapping (horizontal)
                let nx = (x as i32 + dx).rem_euclid(self.width as i32) as usize;

                // Handle Y clamping (no vertical wrap)
                let ny = y as i32 + dy;
                if ny >= 0 && ny < self.height as i32 {
                    result.push((nx, ny as usize));
                }
            }
        }

        result
    }

    /// Iterate over all cells with their coordinates.
    pub fn iter(&self) -> impl Iterator<Item = (usize, usize, &T)> {
        self.data.iter().enumerate().map(move |(idx, val)| {
            let x = idx % self.width;
            let y = idx / self.width;
            (x, y, val)
        })
    }

    /// Iterate mutably over all cells with their coordinates.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (usize, usize, &mut T)> {
        let width = self.width;
        self.data.iter_mut().enumerate().map(move |(idx, val)| {
            let x = idx % width;
            let y = idx / width;
            (x, y, val)
        })
    }
}

/// Upscaling methods for f32 tilemaps
impl Tilemap<f32> {
    /// Upscale the tilemap by a factor using bicubic interpolation.
    /// Preserves terrain features while creating a smoother, higher-resolution map.
    pub fn upscale(&self, factor: usize) -> Self {
        if factor <= 1 {
            return self.clone();
        }

        let new_width = self.width * factor;
        let new_height = self.height * factor;
        let mut result = Tilemap::new_with(new_width, new_height, 0.0f32);

        for new_y in 0..new_height {
            for new_x in 0..new_width {
                // Map back to source coordinates (as floats)
                let src_x = new_x as f32 / factor as f32;
                let src_y = new_y as f32 / factor as f32;

                let value = self.sample_bicubic(src_x, src_y);
                result.set(new_x, new_y, value);
            }
        }

        result
    }

    /// Upscale with added fractal detail noise for more natural appearance.
    /// `detail_scale` controls how much high-frequency detail to add (0.0 = none, 1.0 = full).
    /// `detail_frequency` controls the noise frequency (higher = finer detail).
    pub fn upscale_with_detail(
        &self,
        factor: usize,
        detail_scale: f32,
        detail_frequency: f32,
        seed: u64,
    ) -> Self {
        use noise::{NoiseFn, Perlin, Seedable};

        if factor <= 1 {
            return self.clone();
        }

        let new_width = self.width * factor;
        let new_height = self.height * factor;
        let mut result = Tilemap::new_with(new_width, new_height, 0.0f32);

        // Create noise generator for detail
        let noise = Perlin::new(1).set_seed(seed as u32);

        // Calculate the typical elevation range for scaling detail noise
        let mut min_h = f32::MAX;
        let mut max_h = f32::MIN;
        for (_, _, &h) in self.iter() {
            if h < min_h { min_h = h; }
            if h > max_h { max_h = h; }
        }
        let range = (max_h - min_h).max(1.0);

        for new_y in 0..new_height {
            for new_x in 0..new_width {
                // Map back to source coordinates
                let src_x = new_x as f32 / factor as f32;
                let src_y = new_y as f32 / factor as f32;

                // Bicubic interpolated base value
                let base_value = self.sample_bicubic(src_x, src_y);

                // Multi-octave noise for natural detail, sampled on the sphere so the map's east
                // and west edges meet without a seam (`audit.md` 2.3; revision 1 took 2D noise)
                let detail = if crate::terrain_rev::v1() {
                    let nx = new_x as f64 * detail_frequency as f64 / new_width as f64;
                    let ny = new_y as f64 * detail_frequency as f64 / new_height as f64;
                    fbm_noise(&noise, nx, ny, 4, 0.5, 2.0) as f32
                } else {
                    let p = sphere_point(new_x, new_y, new_width, new_height, detail_frequency as f64 / std::f64::consts::TAU);
                    fbm_noise_3d(&noise, p, 4, 0.5, 2.0) as f32
                };

                // Scale detail based on local terrain (less detail in flat areas, more in mountains)
                let local_gradient = self.get_local_gradient(src_x, src_y);
                let gradient_factor = (local_gradient / 100.0).clamp(0.1, 1.0);

                // Apply detail noise scaled to terrain range
                let detail_amount = detail * detail_scale * range * 0.02 * gradient_factor;

                result.set(new_x, new_y, base_value + detail_amount);
            }
        }

        result
    }

    /// Upscale specifically for erosion simulation.
    /// Adds roughness ONLY in flat, low-elevation areas (river valleys/plains).
    /// Mountains, hills, and coastlines remain clean.
    pub fn upscale_for_erosion(
        &self,
        factor: usize,
        roughness_strength: f32,  // Try 15.0-30.0 for river valleys
        _warp_strength: f32,      // Deprecated
        seed: u64,
    ) -> Self {
        use noise::{NoiseFn, Perlin, Seedable};

        if factor <= 1 {
            return self.clone();
        }

        let new_width = self.width * factor;
        let new_height = self.height * factor;
        let mut result = Tilemap::new_with(new_width, new_height, 0.0f32);

        // Find elevation range for targeting low areas
        let mut max_land_h = 0.0f32;
        for (_, _, &h) in self.iter() {
            if h > 0.0 && h > max_land_h {
                max_land_h = h;
            }
        }

        // Noise for terrain roughness
        let noise_roughness = Perlin::new(1).set_seed(seed as u32);

        for new_y in 0..new_height {
            for new_x in 0..new_width {
                // Source coordinates (no warping - preserve terrain shape)
                let src_x = new_x as f32 / factor as f32;
                let src_y = new_y as f32 / factor as f32;

                // Bicubic interpolated base value
                let base_value = self.sample_bicubic(src_x, src_y);

                // Skip ocean
                if base_value <= 0.0 {
                    result.set(new_x, new_y, base_value);
                    continue;
                }

                // === Targeted Roughness ===
                // Only add roughness where rivers actually flow:
                // 1. Flat areas (low gradient) - rivers meander on plains
                // 2. Low elevation - valley floors, not mountain tops

                let local_gradient = self.get_local_gradient(src_x, src_y);

                // Flatness factor: 1.0 = flat, 0.0 = steep
                let flatness = (1.0 - (local_gradient / 100.0).min(1.0)).max(0.0);

                // Lowness factor: 1.0 = near sea level, 0.0 = high elevation
                // Rivers flow in low areas, so add roughness there
                let elevation_ratio = base_value / max_land_h.max(1.0);
                let lowness = (1.0 - elevation_ratio.powf(0.5)).max(0.0);

                // Combined factor: only apply roughness in flat AND low areas
                let river_zone_factor = flatness * lowness;

                // Skip if not in a river-prone zone
                if river_zone_factor < 0.2 {
                    result.set(new_x, new_y, base_value);
                    continue;
                }

                // Tuned frequency noise for meander-scale undulations, on the sphere so the
                // east and west edges meet (`audit.md` 2.3; revision 1 took 2D noise)
                let (n1, n2) = if crate::terrain_rev::v1() {
                    (noise_roughness.get([
                        new_x as f64 * 0.008,  // Low freq - main meander scale
                        new_y as f64 * 0.008,
                    ]) as f32,
                    noise_roughness.get([
                        new_x as f64 * 0.018 + 50.0,  // Medium freq - secondary bends
                        new_y as f64 * 0.018 + 50.0,
                    ]) as f32)
                } else {
                    let r = new_width as f64 / std::f64::consts::TAU;
                    let [a, b, c] = sphere_point(new_x, new_y, new_width, new_height, r * 0.008);
                    let [d, e, f] = sphere_point(new_x, new_y, new_width, new_height, r * 0.018);
                    (noise_roughness.get([a, b, c]) as f32, noise_roughness.get([d + 50.0, e + 50.0, f + 50.0]) as f32)
                };

                let roughness = n1 * 0.65 + n2 * 0.35;
                let roughness_amount = roughness * roughness_strength * river_zone_factor;

                result.set(new_x, new_y, base_value + roughness_amount);
            }
        }

        result
    }

    /// Sample the tilemap at fractional coordinates using bicubic interpolation.
    fn sample_bicubic(&self, x: f32, y: f32) -> f32 {
        let x0 = x.floor() as i32;
        let y0 = y.floor() as i32;

        let fx = x - x.floor();
        let fy = y - y.floor();

        // Sample 4x4 grid of points
        let mut values = [[0.0f32; 4]; 4];
        for j in 0..4 {
            for i in 0..4 {
                let sx = (x0 + i as i32 - 1).rem_euclid(self.width as i32) as usize;
                let sy = (y0 + j as i32 - 1).clamp(0, self.height as i32 - 1) as usize;
                values[j][i] = *self.get(sx, sy);
            }
        }

        // Bicubic interpolation using Catmull-Rom spline
        bicubic_interpolate(&values, fx, fy)
    }

    /// Sample using bilinear interpolation (faster but less smooth).
    pub fn sample_bilinear(&self, x: f32, y: f32) -> f32 {
        let x0 = x.floor() as i32;
        let y0 = y.floor() as i32;
        let x1 = x0 + 1;
        let y1 = y0 + 1;

        let fx = x - x.floor();
        let fy = y - y.floor();

        // Get four corner values (with wrapping)
        let sx0 = x0.rem_euclid(self.width as i32) as usize;
        let sx1 = x1.rem_euclid(self.width as i32) as usize;
        let sy0 = y0.clamp(0, self.height as i32 - 1) as usize;
        let sy1 = y1.clamp(0, self.height as i32 - 1) as usize;

        let v00 = *self.get(sx0, sy0);
        let v10 = *self.get(sx1, sy0);
        let v01 = *self.get(sx0, sy1);
        let v11 = *self.get(sx1, sy1);

        // Bilinear interpolation
        let v0 = v00 * (1.0 - fx) + v10 * fx;
        let v1 = v01 * (1.0 - fx) + v11 * fx;
        v0 * (1.0 - fy) + v1 * fy
    }

    /// Get the local gradient magnitude at a point (for detail scaling).
    fn get_local_gradient(&self, x: f32, y: f32) -> f32 {
        let delta = 1.0;
        let hx_plus = self.sample_bilinear(x + delta, y);
        let hx_minus = self.sample_bilinear(x - delta, y);
        let hy_plus = self.sample_bilinear(x, y + delta);
        let hy_minus = self.sample_bilinear(x, y - delta);

        // (A cell's east-west width is cos(latitude) of its north-south height: `audit.md` 2.2.)
        let gx = (hx_plus - hx_minus) / (2.0 * delta) / self.metric_cos(y);
        let gy = (hy_plus - hy_minus) / (2.0 * delta);

        (gx * gx + gy * gy).sqrt()
    }

    /// The east-west width of a cell in row `y` against its north-south height on the sphere,
    /// cos(latitude), at least 0.15 near the poles; 1 everywhere for terrain revision 1.
    pub fn metric_cos(&self, y: f32) -> f32 {
        if crate::terrain_rev::v1() { return 1.0; }
        let lat = std::f32::consts::FRAC_PI_2 - (y + 0.5) / self.height as f32 * std::f32::consts::PI;
        lat.cos().max(0.15)
    }

    /// Apply Gaussian blur to smooth microscopic ridges.
    /// Helps parallel streams merge by creating smooth gradients.
    pub fn gaussian_blur(&self, radius: usize) -> Self {
        if radius == 0 {
            return self.clone();
        }

        // Build 1D Gaussian kernel
        let sigma = radius as f32 / 2.0;
        let kernel_size = radius * 2 + 1;
        let mut kernel = vec![0.0f32; kernel_size];
        let mut sum = 0.0;

        for i in 0..kernel_size {
            let x = i as f32 - radius as f32;
            let g = (-x * x / (2.0 * sigma * sigma)).exp();
            kernel[i] = g;
            sum += g;
        }

        // Normalize kernel
        for k in &mut kernel {
            *k /= sum;
        }

        // Horizontal pass
        let mut temp = Tilemap::new_with(self.width, self.height, 0.0f32);
        let metric = !crate::terrain_rev::v1();
        for y in 0..self.height {
            // (On the sphere a row's cells narrow toward the poles: the kernel spans as many
            // kilometres east-west as north-south, so it widens by 1/cos(latitude) there.)
            let row_kernel: Option<(usize, Vec<f32>)> = if metric {
                let c = self.metric_cos(y as f32);
                let r = ((radius as f32 / c).round() as usize).clamp(radius, radius * 6).min(self.width / 2);
                let sx = sigma / c;
                let mut k: Vec<f32> = (0..=2 * r).map(|i| { let d = i as f32 - r as f32; (-d * d / (2.0 * sx * sx)).exp() }).collect();
                let s: f32 = k.iter().sum();
                for v in &mut k { *v /= s; }
                Some((r, k))
            } else { None };
            for x in 0..self.width {
                let mut val = 0.0;
                match &row_kernel {
                    Some((r, k)) => {
                        for (i, w) in k.iter().enumerate() {
                            let sx = (x as i32 + i as i32 - *r as i32).rem_euclid(self.width as i32) as usize;
                            val += *self.get(sx, y) * w;
                        }
                    }
                    None => {
                        for i in 0..kernel_size {
                            let sx = (x as i32 + i as i32 - radius as i32)
                                .rem_euclid(self.width as i32) as usize;
                            val += *self.get(sx, y) * kernel[i];
                        }
                    }
                }
                temp.set(x, y, val);
            }
        }

        // Vertical pass
        let mut result = Tilemap::new_with(self.width, self.height, 0.0f32);
        for y in 0..self.height {
            for x in 0..self.width {
                let mut val = 0.0;
                for i in 0..kernel_size {
                    let sy = (y as i32 + i as i32 - radius as i32)
                        .clamp(0, self.height as i32 - 1) as usize;
                    val += *temp.get(x, sy) * kernel[i];
                }
                result.set(x, y, val);
            }
        }

        result
    }
}

/// Bicubic interpolation using Catmull-Rom spline
fn bicubic_interpolate(values: &[[f32; 4]; 4], fx: f32, fy: f32) -> f32 {
    // Interpolate 4 rows
    let mut row_values = [0.0f32; 4];
    for j in 0..4 {
        row_values[j] = catmull_rom(values[j][0], values[j][1], values[j][2], values[j][3], fx);
    }

    // Interpolate the column
    catmull_rom(row_values[0], row_values[1], row_values[2], row_values[3], fy)
}

/// Catmull-Rom spline interpolation
fn catmull_rom(p0: f32, p1: f32, p2: f32, p3: f32, t: f32) -> f32 {
    let t2 = t * t;
    let t3 = t2 * t;

    0.5 * ((2.0 * p1)
        + (-p0 + p2) * t
        + (2.0 * p0 - 5.0 * p1 + 4.0 * p2 - p3) * t2
        + (-p0 + 3.0 * p1 - 3.0 * p2 + p3) * t3)
}

// =============================================================================
// 8-NEIGHBOR DIRECTIONAL ANALYSIS (Phase 4b)
// =============================================================================

/// Direction indices for 8-neighbor analysis
/// Order: N, NE, E, SE, S, SW, W, NW
pub const DIR_N: usize = 0;
pub const DIR_NE: usize = 1;
pub const DIR_E: usize = 2;
pub const DIR_SE: usize = 3;
pub const DIR_S: usize = 4;
pub const DIR_SW: usize = 5;
pub const DIR_W: usize = 6;
pub const DIR_NW: usize = 7;

/// Direction offsets for 8-neighbor analysis (dx, dy)
pub const DIR_OFFSETS: [(i32, i32); 8] = [
    (0, -1),  // N
    (1, -1),  // NE
    (1, 0),   // E
    (1, 1),   // SE
    (0, 1),   // S
    (-1, 1),  // SW
    (-1, 0),  // W
    (-1, -1), // NW
];

/// Rich context about surrounding terrain for adaptive feature placement
#[derive(Clone, Debug)]
pub struct DirectionalContext {
    /// Value at the center cell
    pub center_value: f32,
    /// Values of 8 neighbors (N, NE, E, SE, S, SW, W, NW), None if at map edge
    pub neighbors: [Option<f32>; 8],
    /// Gradient vector (dx, dy) pointing in direction of steepest ascent
    pub gradient: (f32, f32),
    /// Magnitude of the gradient (slope steepness)
    pub gradient_magnitude: f32,
    /// Curvature (Laplacian) - positive = convex/ridge, negative = concave/valley
    /// (terrain revision 1 had the sign the other way round: `audit.md` 2.1).
    pub curvature: f32,
    /// Plan (contour) curvature: positive where flow lines diverge across the slope (a spur, a
    /// ridge), negative where they converge (a swale, a hollow). Zero on flat ground.
    pub plan_curvature: f32,
    /// Profile curvature: positive where the slope steepens downhill (a convex break, a cliff's
    /// lip), negative where it eases (a concave footslope). Zero on flat ground.
    pub profile_curvature: f32,
    /// Aspect angle in radians - direction of steepest descent (0 = North, PI/2 = East)
    pub aspect: f32,
}

impl DirectionalContext {
    /// Check if this is a ridge (steep + convex)
    pub fn is_ridge(&self, slope_threshold: f32) -> bool {
        self.gradient_magnitude > slope_threshold && self.curvature > 0.0
    }

    /// Check if this is a valley (steep + concave)
    pub fn is_valley(&self, slope_threshold: f32) -> bool {
        self.gradient_magnitude > slope_threshold && self.curvature < 0.0
    }

    /// Check if this is a flat area
    pub fn is_flat(&self, slope_threshold: f32) -> bool {
        self.gradient_magnitude < slope_threshold
    }

    /// Get normalized gradient direction (unit vector)
    pub fn gradient_direction(&self) -> (f32, f32) {
        if self.gradient_magnitude > 0.0001 {
            (
                self.gradient.0 / self.gradient_magnitude,
                self.gradient.1 / self.gradient_magnitude,
            )
        } else {
            (0.0, 0.0)
        }
    }

    /// Get the direction index (0-7) of steepest descent
    pub fn steepest_descent_dir(&self) -> usize {
        // Aspect is direction of steepest descent, convert to index
        let angle = self.aspect.rem_euclid(std::f32::consts::TAU);
        let segment = std::f32::consts::TAU / 8.0;
        ((angle + segment / 2.0) / segment) as usize % 8
    }
}

/// Weights for computing gradients and curvature
#[derive(Clone, Debug)]
pub struct DirectionalWeights {
    /// Weight for cardinal neighbors (N, S, E, W)
    pub cardinal: f32,
    /// Weight for diagonal neighbors (NE, SE, SW, NW)
    pub diagonal: f32,
    /// Weight for the center cell in the smoothing estimates (`smoothed_value` takes its own);
    /// the gradient and curvature stencils give the center the sum of the neighbour weights.
    pub center: f32,
}

impl Default for DirectionalWeights {
    fn default() -> Self {
        // Sobel-like weights normalized for better gradient estimation
        Self {
            cardinal: 2.0,
            diagonal: 1.0,
            center: 4.0,
        }
    }
}

impl Tilemap<f32> {
    /// Analyze the 8-neighbor directional context at a point.
    /// Returns rich information about local terrain for adaptive decisions.
    pub fn analyze_directional_context(&self, x: usize, y: usize) -> DirectionalContext {
        self.analyze_directional_context_weighted(x, y, &DirectionalWeights::default())
    }

    /// Analyze directional context with custom weights
    pub fn analyze_directional_context_weighted(
        &self,
        x: usize,
        y: usize,
        weights: &DirectionalWeights,
    ) -> DirectionalContext {
        let center_value = *self.get(x, y);
        let mut neighbors = [None; 8];

        // Sample all 8 neighbors with wrapping
        for (i, &(dx, dy)) in DIR_OFFSETS.iter().enumerate() {
            let nx = (x as i32 + dx).rem_euclid(self.width as i32) as usize;
            let ny = y as i32 + dy;

            if ny >= 0 && ny < self.height as i32 {
                neighbors[i] = Some(*self.get(nx, ny as usize));
            }
        }

        // Compute gradient using Sobel-like operator
        // For missing neighbors, use center value (flat assumption)
        let get_or_center = |idx: usize| neighbors[idx].unwrap_or(center_value);

        // X gradient: (NE + 2*E + SE) - (NW + 2*W + SW)
        let gx = (get_or_center(DIR_NE) * weights.diagonal
            + get_or_center(DIR_E) * weights.cardinal
            + get_or_center(DIR_SE) * weights.diagonal)
            - (get_or_center(DIR_NW) * weights.diagonal
                + get_or_center(DIR_W) * weights.cardinal
                + get_or_center(DIR_SW) * weights.diagonal);

        // Y gradient: (SW + 2*S + SE) - (NW + 2*N + NE)
        let gy = (get_or_center(DIR_SW) * weights.diagonal
            + get_or_center(DIR_S) * weights.cardinal
            + get_or_center(DIR_SE) * weights.diagonal)
            - (get_or_center(DIR_NW) * weights.diagonal
                + get_or_center(DIR_N) * weights.cardinal
                + get_or_center(DIR_NE) * weights.diagonal);

        // Normalize by the sum of weights used
        let weight_sum = 2.0 * weights.diagonal + weights.cardinal;
        // (East-west steps are cos(latitude) as long as north-south ones: `audit.md` 2.2.)
        let cos_lat = self.metric_cos(y as f32);
        let gradient = (gx / weight_sum / cos_lat, gy / weight_sum);
        let gradient_magnitude = (gradient.0 * gradient.0 + gradient.1 * gradient.1).sqrt();

        // Compute curvature (Laplacian)
        // Using weighted sum of neighbors minus center
        let neighbor_sum = get_or_center(DIR_N) * weights.cardinal
            + get_or_center(DIR_S) * weights.cardinal
            + get_or_center(DIR_E) * weights.cardinal
            + get_or_center(DIR_W) * weights.cardinal
            + get_or_center(DIR_NE) * weights.diagonal
            + get_or_center(DIR_SE) * weights.diagonal
            + get_or_center(DIR_SW) * weights.diagonal
            + get_or_center(DIR_NW) * weights.diagonal;

        let total_weight = 4.0 * weights.cardinal + 4.0 * weights.diagonal;
        let neighbor_avg = neighbor_sum / total_weight;
        // Positive at a ridge or peak (the center above its neighbours), negative in a valley.
        // (Revision 1 subtracted the other way, so `is_ridge` found valleys: `audit.md` 2.1.)
        let curvature = if crate::terrain_rev::v1() { neighbor_avg - center_value } else { center_value - neighbor_avg };

        // Plan and profile curvature from the 3x3 second derivatives (Zevenbergen & Thorne
        // 1987), y down the map, east-west steps cos(latitude) long. Convex is positive.
        let (dxm, dym) = (cos_lat, 1.0f32);
        let (n, s_, e, w) = (get_or_center(DIR_N), get_or_center(DIR_S), get_or_center(DIR_E), get_or_center(DIR_W));
        let (ne, se, sw, nw) = (get_or_center(DIR_NE), get_or_center(DIR_SE), get_or_center(DIR_SW), get_or_center(DIR_NW));
        let zx = (e - w) / (2.0 * dxm);
        let zy = (s_ - n) / (2.0 * dym);
        let zxx = (e - 2.0 * center_value + w) / (dxm * dxm);
        let zyy = (s_ - 2.0 * center_value + n) / (dym * dym);
        let zxy = (se + nw - ne - sw) / (4.0 * dxm * dym);
        let p = zx * zx + zy * zy;
        let (plan_curvature, profile_curvature) = if p < 1e-9 { (0.0, 0.0) } else {
            (-(zxx * zy * zy - 2.0 * zxy * zx * zy + zyy * zx * zx) / p.powf(1.5),
             -(zxx * zx * zx + 2.0 * zxy * zx * zy + zyy * zy * zy) / (p * (1.0 + p).powf(1.5)))
        };

        // Compute aspect (direction of steepest descent)
        // atan2(dy, dx) gives angle, negate for descent direction
        let aspect = (-gradient.1).atan2(-gradient.0);

        DirectionalContext {
            center_value,
            neighbors,
            gradient,
            gradient_magnitude,
            curvature,
            plan_curvature,
            profile_curvature,
            aspect,
        }
    }

    /// Compute flow direction from directional context (D8 algorithm result)
    /// Returns direction index 0-7, or None if flat or sink
    pub fn compute_flow_direction_at(&self, x: usize, y: usize) -> Option<usize> {
        let context = self.analyze_directional_context(x, y);

        if context.gradient_magnitude < 0.0001 {
            return None; // Flat area
        }

        // Find the neighbor with the steepest descent
        let mut best_dir = None;
        let mut best_drop = 0.0f32;

        for (i, val) in context.neighbors.iter().enumerate() {
            if let Some(neighbor_val) = val {
                let drop = context.center_value - neighbor_val;
                // The step's length on the sphere: east-west parts cos(latitude) as long
                // (`audit.md` 2.2; revision 1 took 1 and 1.414 at every latitude).
                let dist = if crate::terrain_rev::v1() { if i % 2 == 1 { 1.414 } else { 1.0 } } else {
                    let (dx, dy) = DIR_OFFSETS[i];
                    let c = self.metric_cos(y as f32);
                    ((dx as f32 * c).powi(2) + (dy as f32).powi(2)).sqrt()
                };
                let slope = drop / dist;

                if slope > best_drop {
                    best_drop = slope;
                    best_dir = Some(i);
                }
            }
        }

        best_dir
    }

    /// Get the average value of all valid neighbors
    pub fn neighbor_average(&self, x: usize, y: usize) -> f32 {
        let mut sum = 0.0f32;
        let mut count = 0;

        for &(dx, dy) in DIR_OFFSETS.iter() {
            let nx = (x as i32 + dx).rem_euclid(self.width as i32) as usize;
            let ny = y as i32 + dy;

            if ny >= 0 && ny < self.height as i32 {
                sum += *self.get(nx, ny as usize);
                count += 1;
            }
        }

        if count > 0 {
            sum / count as f32
        } else {
            *self.get(x, y)
        }
    }

    /// Compute a smoothed value using weighted neighbor average
    pub fn smoothed_value(&self, x: usize, y: usize, center_weight: f32) -> f32 {
        let center = *self.get(x, y);
        let neighbor_avg = self.neighbor_average(x, y);
        let total_weight = center_weight + 1.0;
        (center * center_weight + neighbor_avg) / total_weight
    }
}

// =============================================================================
// DOWNSCALING METHODS (Phase 3 - Smart Downsampler)
// =============================================================================

impl Tilemap<f32> {
    /// Min-pooling downscale: always preserves lowest values (valleys/rivers).
    /// Use this when you want to ensure carved river channels are preserved.
    pub fn downscale_min_pool(&self, factor: usize) -> Self {
        if factor <= 1 {
            return self.clone();
        }

        let new_width = self.width / factor;
        let new_height = self.height / factor;
        let mut result = Tilemap::new_with(new_width, new_height, 0.0f32);

        for new_y in 0..new_height {
            for new_x in 0..new_width {
                let mut min_val = f32::MAX;
                for dy in 0..factor {
                    for dx in 0..factor {
                        let sx = new_x * factor + dx;
                        let sy = new_y * factor + dy;
                        if sx < self.width && sy < self.height {
                            min_val = min_val.min(*self.get(sx, sy));
                        }
                    }
                }
                result.set(new_x, new_y, min_val);
            }
        }
        result
    }

    /// Variance-aware downscale: min-pool for high variance (rivers), average for flat areas.
    /// This preserves sharp river channels while smoothing flat terrain.
    /// `variance_threshold` controls when to switch from average to min (lower = more aggressive).
    pub fn downscale_preserve_rivers(&self, factor: usize, variance_threshold: f32) -> Self {
        if factor <= 1 {
            return self.clone();
        }

        let new_width = self.width / factor;
        let new_height = self.height / factor;
        let mut result = Tilemap::new_with(new_width, new_height, 0.0f32);

        for new_y in 0..new_height {
            for new_x in 0..new_width {
                let mut values = Vec::with_capacity(factor * factor);
                let mut sum = 0.0f32;

                for dy in 0..factor {
                    for dx in 0..factor {
                        let sx = new_x * factor + dx;
                        let sy = new_y * factor + dy;
                        if sx < self.width && sy < self.height {
                            let v = *self.get(sx, sy);
                            values.push(v);
                            sum += v;
                        }
                    }
                }

                if values.is_empty() {
                    continue;
                }

                let mean = sum / values.len() as f32;
                let variance: f32 = values.iter()
                    .map(|v| (v - mean).powi(2))
                    .sum::<f32>() / values.len() as f32;

                let final_value = if variance > variance_threshold {
                    // High variance = river channel, use min to preserve the carved depth
                    values.iter().cloned().fold(f32::MAX, f32::min)
                } else {
                    mean
                };

                result.set(new_x, new_y, final_value);
            }
        }
        result
    }

    /// Flow-aware downscale: uses flow accumulation to identify river cells.
    /// River cells snap to their exact height; other cells use average.
    pub fn downscale_with_flow(
        &self,
        flow_map: &Tilemap<f32>,
        factor: usize,
        river_threshold: f32,
    ) -> Self {
        if factor <= 1 {
            return self.clone();
        }

        let new_width = self.width / factor;
        let new_height = self.height / factor;
        let mut result = Tilemap::new_with(new_width, new_height, 0.0f32);

        for new_y in 0..new_height {
            for new_x in 0..new_width {
                let mut heights = Vec::new();
                let mut river_height: Option<f32> = None;
                let mut max_flow = 0.0f32;

                for dy in 0..factor {
                    for dx in 0..factor {
                        let sx = new_x * factor + dx;
                        let sy = new_y * factor + dy;
                        if sx < self.width && sy < self.height {
                            let h = *self.get(sx, sy);
                            let f = *flow_map.get(sx, sy);
                            heights.push(h);

                            // Track the highest-flow river cell for snapping
                            if f > river_threshold && f > max_flow {
                                max_flow = f;
                                river_height = Some(h);
                            }
                        }
                    }
                }

                let final_value = river_height.unwrap_or_else(|| {
                    if heights.is_empty() {
                        0.0
                    } else {
                        heights.iter().sum::<f32>() / heights.len() as f32
                    }
                });

                result.set(new_x, new_y, final_value);
            }
        }
        result
    }

    /// Max-pooling downscale: always preserves highest values.
    /// Use this for flow accumulation to preserve river paths (high flow = river).
    pub fn downscale_max_pool(&self, factor: usize) -> Self {
        if factor <= 1 {
            return self.clone();
        }

        let new_width = self.width / factor;
        let new_height = self.height / factor;
        let mut result = Tilemap::new_with(new_width, new_height, 0.0f32);

        for new_y in 0..new_height {
            for new_x in 0..new_width {
                let mut max_val = f32::MIN;
                for dy in 0..factor {
                    for dx in 0..factor {
                        let sx = new_x * factor + dx;
                        let sy = new_y * factor + dy;
                        if sx < self.width && sy < self.height {
                            max_val = max_val.max(*self.get(sx, sy));
                        }
                    }
                }
                result.set(new_x, new_y, max_val);
            }
        }
        result
    }

    /// Simple average downscale (for comparison).
    pub fn downscale_average(&self, factor: usize) -> Self {
        if factor <= 1 {
            return self.clone();
        }

        let new_width = self.width / factor;
        let new_height = self.height / factor;
        let mut result = Tilemap::new_with(new_width, new_height, 0.0f32);

        for new_y in 0..new_height {
            for new_x in 0..new_width {
                let mut sum = 0.0f32;
                let mut count = 0;

                for dy in 0..factor {
                    for dx in 0..factor {
                        let sx = new_x * factor + dx;
                        let sy = new_y * factor + dy;
                        if sx < self.width && sy < self.height {
                            sum += *self.get(sx, sy);
                            count += 1;
                        }
                    }
                }

                if count > 0 {
                    result.set(new_x, new_y, sum / count as f32);
                }
            }
        }
        result
    }
}

/// Fractional Brownian Motion noise
/// The point on a sphere of `radius` under cell (`x`, `y`) of a `width` x `height`
/// equirectangular map (x the longitude, y the latitude from north to south): noise sampled
/// there wraps east to west and does not stretch toward the poles.
pub fn sphere_point(x: usize, y: usize, width: usize, height: usize, radius: f64) -> [f64; 3] {
    let lon = (x as f64 + 0.5) / width as f64 * std::f64::consts::TAU;
    let lat = std::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) / height as f64 * std::f64::consts::PI;
    [radius * lat.cos() * lon.cos(), radius * lat.cos() * lon.sin(), radius * lat.sin()]
}

/// Distance on the sphere from every cell to the nearest `sources` cell, travelling only through
/// cells `pass` allows (the sources themselves always count), in north-south cell heights (one
/// cell = pi / height radians of arc). Unreached cells are `f32::MAX`.
///
/// Each cell inherits its neighbour's nearest source and measures the great-circle distance to
/// it (vector propagation in a Dijkstra order), so the field is round and isotropic: a breadth-
/// first search counting every step as 1 measures max(|dx|, |dy|) and draws squares and
/// octagons (`audit.md` 3.2). East-west steps shrink with cos(latitude) as they do on the globe.
pub fn geodesic_distance(
    width: usize,
    height: usize,
    sources: impl IntoIterator<Item = (usize, usize)>,
    pass: impl Fn(usize, usize) -> bool,
) -> Tilemap<f32> {
    geodesic_nearest(width, height, sources, pass).0
}

/// `geodesic_distance` and, for every cell, the index (y * width + x) of its nearest source
/// (`usize::MAX` where unreached).
pub fn geodesic_nearest(
    width: usize,
    height: usize,
    sources: impl IntoIterator<Item = (usize, usize)>,
    pass: impl Fn(usize, usize) -> bool,
) -> (Tilemap<f32>, Vec<usize>) {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;
    let n = width * height;
    let cell = std::f64::consts::PI / height as f64;
    let lat: Vec<f64> = (0..height).map(|y| std::f64::consts::FRAC_PI_2 - (y as f64 + 0.5) * cell).collect();
    let lon = |x: usize| (x as f64 + 0.5) / width as f64 * std::f64::consts::TAU;
    let arc = |a: usize, b: usize| -> f64 {
        let (ya, yb) = (a / width, b / width);
        let dlat = lat[yb] - lat[ya];
        let dlon = lon(b % width) - lon(a % width);
        let h = (dlat * 0.5).sin().powi(2) + lat[ya].cos() * lat[yb].cos() * (dlon * 0.5).sin().powi(2);
        2.0 * h.sqrt().min(1.0).asin() / cell
    };
    let mut dist = vec![f32::MAX; n];
    let mut src = vec![usize::MAX; n];
    let mut heap = BinaryHeap::new();
    for (x, y) in sources {
        let i = y * width + x;
        if src[i] != usize::MAX { continue; }
        dist[i] = 0.0;
        src[i] = i;
        heap.push((Reverse(0u64), i));
    }
    while let Some((Reverse(k), i)) = heap.pop() {
        if (k as f64 / 1e4) as f32 > dist[i] + 1e-3 { continue; }
        let (x, y) = (i % width, i / width);
        for dy in -1i64..=1 {
            for dx in -1i64..=1 {
                if dx == 0 && dy == 0 { continue; }
                let ny = y as i64 + dy;
                if ny < 0 || ny >= height as i64 { continue; }
                let nx = (x as i64 + dx).rem_euclid(width as i64) as usize;
                let j = ny as usize * width + nx;
                if src[j] == j || !pass(nx, ny as usize) { continue; }
                let d = arc(j, src[i]) as f32;
                if d + 1e-4 < dist[j] {
                    dist[j] = d;
                    src[j] = src[i];
                    heap.push((Reverse((d as f64 * 1e4) as u64), j));
                }
            }
        }
    }
    (Tilemap::from_vec(width, height, dist), src)
}

/// Fractal Brownian motion in three dimensions (see `fbm_noise`).
fn fbm_noise_3d(noise: &impl noise::NoiseFn<f64, 3>, p: [f64; 3], octaves: u32, persistence: f64, lacunarity: f64) -> f64 {
    let (mut total, mut amplitude, mut frequency, mut max_value) = (0.0, 1.0, 1.0, 0.0);
    for _ in 0..octaves {
        total += amplitude * noise.get([p[0] * frequency, p[1] * frequency, p[2] * frequency]);
        max_value += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }
    total / max_value
}

fn fbm_noise(
    noise: &impl noise::NoiseFn<f64, 2>,
    x: f64,
    y: f64,
    octaves: u32,
    persistence: f64,
    lacunarity: f64,
) -> f64 {
    let mut total = 0.0;
    let mut amplitude = 1.0;
    let mut frequency = 1.0;
    let mut max_value = 0.0;

    for _ in 0..octaves {
        total += amplitude * noise.get([x * frequency, y * frequency]);
        max_value += amplitude;
        amplitude *= persistence;
        frequency *= lacunarity;
    }

    total / max_value
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A ridge is positive curvature and a valley negative (`audit.md` 2.1); a spur's plan
    /// curvature is positive and a hollow's negative.
    #[test]
    fn curvature_sign_ridge_and_valley() {
        // A north-south ridge on the equator: high at x = 8, falling east and west.
        let (w, h) = (32, 16);
        let mut ridge = Tilemap::new_with(w, h, 0.0f32);
        let mut valley = Tilemap::new_with(w, h, 0.0f32);
        for y in 0..h { for x in 0..w {
            let d = (x as f32 - 8.0).abs();
            ridge.set(x, y, 100.0 - 10.0 * d * d);
            valley.set(x, y, 10.0 * d * d);
        } }
        let r = ridge.analyze_directional_context(8, 8);
        let v = valley.analyze_directional_context(8, 8);
        assert!(r.curvature > 0.0 && v.curvature < 0.0, "ridge {} valley {}", r.curvature, v.curvature);
        // On the flank, sloping east, a spur (contours bulging downhill) diverges.
        let mut spur = Tilemap::new_with(w, h, 0.0f32);
        let mut hollow = Tilemap::new_with(w, h, 0.0f32);
        for y in 0..h { for x in 0..w {
            let dy = y as f32 - 8.0;
            spur.set(x, y, 200.0 - 5.0 * x as f32 - 2.0 * dy * dy);
            hollow.set(x, y, 200.0 - 5.0 * x as f32 + 2.0 * dy * dy);
        } }
        let s = spur.analyze_directional_context(10, 8);
        let o = hollow.analyze_directional_context(10, 8);
        assert!(s.plan_curvature > 0.0 && o.plan_curvature < 0.0, "spur {} hollow {}", s.plan_curvature, o.plan_curvature);
        assert!(r.is_ridge(0.0) || r.gradient_magnitude == 0.0);
        assert!(!r.is_valley(0.0));
    }

    /// East-west slopes steepen toward the poles by 1/cos(latitude) (`audit.md` 2.2).
    #[test]
    fn gradients_take_the_sphere_metric() {
        let (w, h) = (64, 32);
        let mut m = Tilemap::new_with(w, h, 0.0f32);
        for y in 0..h { for x in 0..w { m.set(x, y, (x as f32 * std::f32::consts::TAU / w as f32).sin() * 100.0); } }
        let eq = m.analyze_directional_context(0, h / 2).gradient_magnitude;
        let high = m.analyze_directional_context(0, 3).gradient_magnitude;
        let lat = std::f32::consts::FRAC_PI_2 - 3.5 / h as f32 * std::f32::consts::PI;
        let want = 1.0 / lat.cos();
        assert!((high / eq - want).abs() < 0.05 * want, "ratio {} want {}", high / eq, want);
        // The blur reaches further east-west near the poles than at the equator.
        let mut spike = Tilemap::new_with(w, h, 0.0f32);
        spike.set(10, 3, 1.0); spike.set(10, h / 2, 1.0);
        let b = spike.gaussian_blur(2);
        assert!(*b.get(13, 3) > 0.0 && *b.get(13, h / 2) == 0.0);
    }

    /// Distances are round, not square (`audit.md` 3.2), and shrink east-west near the poles.
    #[test]
    fn geodesic_distance_is_round() {
        let (w, h) = (360, 180);
        let d = geodesic_distance(w, h, [(180, 90)], |_, _| true);
        // On the equator 10 cells east, 10 north and 10 diagonally (7.07 + 7.07) are all ~10.
        let e = *d.get(190, 90);
        let nn = *d.get(180, 80);
        let diag = *d.get(187, 83);
        assert!((e - 10.0).abs() < 0.2 && (nn - 10.0).abs() < 0.2, "east {e} north {nn}");
        assert!((diag - 9.9).abs() < 0.3, "diagonal {diag}: a Chebyshev field would say 7");
        // At 60 degrees north, 10 cells east is ~5 cells away.
        let p = geodesic_distance(w, h, [(180, 30)], |_, _| true);
        assert!((*p.get(190, 30) - 5.0).abs() < 0.3, "east at 60N: {}", p.get(190, 30));
        // Walls stop it.
        let walled = geodesic_distance(w, h, [(180, 90)], |x, _| x != 185 && x != 175);
        assert_eq!(*walled.get(190, 90), f32::MAX);
    }

    /// The upscalers' detail meets itself across the map's east-west edge (`audit.md` 2.3).
    #[test]
    fn upscaled_detail_has_no_seam() {
        let (w, h) = (32, 16);
        // A flat map: what the upscaler adds is its detail noise alone.
        let m = Tilemap::new_with(w, h, 0.0f32);
        let up = m.upscale_with_detail(4, 1.0, 8.0, 7);
        let (uw, uh) = (up.width, up.height);
        let mut seam = 0.0f32; let mut inner = 0.0f32;
        for y in 0..uh {
            seam += (*up.get(uw - 1, y) - *up.get(0, y)).abs();
            inner += (*up.get(uw / 2, y) - *up.get(uw / 2 - 1, y)).abs();
        }
        assert!(inner > 0.0 && seam < inner * 2.0, "seam {seam} inner {inner}");
    }
}
