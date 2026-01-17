/// A 2D tilemap grid with equirectangular projection (wraps horizontally).
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

                // Add fractal detail noise
                let nx = new_x as f64 * detail_frequency as f64 / new_width as f64;
                let ny = new_y as f64 * detail_frequency as f64 / new_height as f64;

                // Multi-octave noise for natural detail
                let detail = fbm_noise(&noise, nx, ny, 4, 0.5, 2.0) as f32;

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

        let gx = (hx_plus - hx_minus) / (2.0 * delta);
        let gy = (hy_plus - hy_minus) / (2.0 * delta);

        (gx * gx + gy * gy).sqrt()
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

/// Fractional Brownian Motion noise
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
