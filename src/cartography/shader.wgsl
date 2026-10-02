// Antique Old Paper Cartography WGSL Compute Shader

struct CartographyUniforms {
    width: u32,
    height: u32,
    paper_style: u32,
    paper_roughness: f32,
    paper_stains: f32,
    vignette_strength: f32,
    waterline_count: u32,
    waterline_spacing: f32,
    hachure_intensity: f32,
    hachure_density: f32,
    watercolor_opacity: f32,
    seed: u32,
};

@group(0) @binding(0) var<storage, read> heightmap: array<f32>;
@group(0) @binding(1) var<storage, read> biome_indices: array<u32>;
@group(0) @binding(2) var<storage, read> water_dist_map: array<f32>;
@group(0) @binding(3) var<storage, read> river_flags: array<u32>;
@group(0) @binding(4) var<storage, read> water_depth_map: array<f32>;
@group(0) @binding(5) var<uniform> uniforms: CartographyUniforms;
@group(0) @binding(6) var<storage, read_write> output_pixels: array<u32>;

// Pseudo-random hash function
fn hash(p: vec2<f32>) -> f32 {
    let p3 = fract(vec3<f32>(p.xyx) * 0.1031);
    let d = dot(p3, p3.yzx + 33.33);
    return fract((p3.x + p3.y) * d);
}

// 2D Value Noise
fn noise2d(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);

    let a = hash(i + vec2<f32>(0.0, 0.0));
    let b = hash(i + vec2<f32>(1.0, 0.0));
    let c = hash(i + vec2<f32>(0.0, 1.0));
    let d = hash(i + vec2<f32>(1.0, 1.0));

    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y) * 2.0 - 1.0;
}

// Fractional Brownian Motion for procedural paper texture
fn fbm2d(p: vec2<f32>, octaves: u32) -> f32 {
    var total = 0.0;
    var amp = 0.5;
    var pos = p;
    for (var i = 0u; i < octaves; i = i + 1u) {
        total = total + amp * noise2d(pos);
        pos = pos * 2.1;
        amp = amp * 0.5;
    }
    return total;
}

// Pack RGB float to u32 (RGBA8)
fn pack_rgb(r: f32, g: f32, b: f32) -> u32 {
    let ir = u32(clamp(r * 255.0, 0.0, 255.0));
    let ig = u32(clamp(g * 255.0, 0.0, 255.0));
    let ib = u32(clamp(b * 255.0, 0.0, 255.0));
    return (ir << 0u) | (ig << 8u) | (ib << 16u) | (255u << 24u);
}

@compute @workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let x = global_id.x;
    let y = global_id.y;
    let width = uniforms.width;
    let height = uniforms.height;

    if (x >= width || y >= height) {
        return;
    }

    let idx = y * width + x;
    let u = f32(x) / f32(width);
    let v = f32(y) / f32(height);
    let uv = vec2<f32>(u, v);

    let h = heightmap[idx];
    let is_river = river_flags[idx] != 0u;
    let water_depth = water_depth_map[idx];
    let is_lake = h >= 0.0 && water_depth > 0.5;
    let is_land = h >= 0.0 && !is_river && !is_lake;

    // Base paper colors
    var paper_light = vec3<f32>(0.965, 0.933, 0.855); // #f6eedb
    var paper_mid   = vec3<f32>(0.902, 0.847, 0.722); // #e6d8b8
    var paper_dark  = vec3<f32>(0.808, 0.729, 0.580); // #cfba94

    let ink_coast = vec3<f32>(0.149, 0.102, 0.063);
    let ink_hachure = vec3<f32>(0.372, 0.267, 0.188);
    let ink_waterline = vec3<f32>(0.353, 0.412, 0.463);
    let ink_river = vec3<f32>(0.125, 0.204, 0.290);

    // 1. Procedural Parchment Paper Texture
    let p_macro = fbm2d(uv * 14.0 + vec2<f32>(f32(uniforms.seed % 100u)), 3u);
    let p_fibers = fbm2d(uv * 85.0 + vec2<f32>(17.3), 3u);
    let p_speckles = noise2d(uv * 320.0);

    let paper_noise = (p_macro * 0.55 + p_fibers * 0.30 + p_speckles * 0.15) * uniforms.paper_roughness;
    var col = mix(paper_mid, paper_light, clamp(paper_noise, 0.0, 1.0));
    if (paper_noise < 0.0) {
        col = mix(paper_mid, paper_dark, clamp(-paper_noise, 0.0, 1.0));
    }

    // Tea stains
    if (uniforms.paper_stains > 0.05) {
        let stain = fbm2d(uv * 5.0 + vec2<f32>(55.0, 33.0), 3u);
        if (stain > 0.25) {
            let sf = min((stain - 0.25) * 2.5, 1.0) * uniforms.paper_stains * 0.40;
            col = mix(col, paper_dark * vec3<f32>(0.85, 0.78, 0.65), sf);
        }
    }

    // Edge vignette
    if (uniforms.vignette_strength > 0.05) {
        let dist_x = min(u, 1.0 - u) * 2.0;
        let dist_y = min(v, 1.0 - v) * 2.0;
        let edge_dist = clamp(min(dist_x, dist_y), 0.0, 1.0);
        let vignette = pow(1.0 - edge_dist, 1.6) * uniforms.vignette_strength * 0.65;
        col = mix(col, paper_dark * vec3<f32>(0.65, 0.52, 0.38), vignette);
    }

    // 2. Land & Biome Watercolor Wash
    if (is_land) {
        let b_id = biome_indices[idx];
        var wash = vec3<f32>(0.85, 0.80, 0.65); // Neutral default

        if (b_id == 3u || b_id == 15u) { // Ice / SnowyPeaks
            wash = vec3<f32>(0.96, 0.96, 0.94);
        } else if (b_id == 4u || b_id == 14u) { // Tundra
            wash = vec3<f32>(0.79, 0.81, 0.74);
        } else if (b_id == 5u || b_id == 12u) { // Boreal / Conifer
            wash = vec3<f32>(0.52, 0.57, 0.43);
        } else if (b_id == 6u || b_id == 13u) { // Grassland / Meadow
            wash = vec3<f32>(0.87, 0.81, 0.59);
        } else if (b_id == 7u || b_id == 10u || b_id == 17u) { // Forest / Foothills
            wash = vec3<f32>(0.60, 0.64, 0.46);
        } else if (b_id == 8u || b_id == 11u) { // Rainforest
            wash = vec3<f32>(0.46, 0.55, 0.38);
        } else if (b_id == 9u) { // Desert
            wash = vec3<f32>(0.89, 0.74, 0.52);
        } else if (b_id == 10u) { // Savanna
            wash = vec3<f32>(0.85, 0.75, 0.53);
        } else if (b_id == 20u || b_id == 21u || b_id == 22u) { // Wetlands
            wash = vec3<f32>(0.54, 0.58, 0.45);
        }

        let tinted = col * wash;
        col = mix(col, tinted, uniforms.watercolor_opacity);

        // 3. Mountain Engraving & Hachuring
        if (x > 0u && x < width - 1u && y > 0u && y < height - 1u) {
            let h_l = heightmap[idx - 1u];
            let h_r = heightmap[idx + 1u];
            let h_u = heightmap[idx - width];
            let h_d = heightmap[idx + width];

            let dzdx = (h_r - h_l) * 0.5;
            let dzdy = (h_d - h_u) * 0.5;
            let slope = sqrt(dzdx * dzdx + dzdy * dzdy);

            let nx_s = -dzdx * 0.03;
            let ny_s = -dzdy * 0.03;
            let nz_s = 1.0;
            let n_len = sqrt(nx_s * nx_s + ny_s * ny_s + nz_s * nz_s);
            let dot_l = (nx_s * -0.707 + ny_s * -0.707 + nz_s * 0.50) / n_len;

            if (slope > 16.0 && uniforms.hachure_intensity > 0.05) {
                let hatch_sin = sin((f32(x) + f32(y)) * uniforms.hachure_density * 0.45);
                let shadow = clamp((0.75 - dot_l), 0.0, 1.2);
                if (hatch_sin > 0.25 && shadow > 0.15) {
                    let slope_mod = min((slope - 16.0) / 35.0, 1.0);
                    let stroke = min(shadow * slope_mod * 0.75 * uniforms.hachure_intensity, 0.85);
                    col = mix(col, ink_hachure, stroke);
                }
                if (slope > 40.0 && dot_l < 0.4) {
                    col = mix(col, ink_coast, 0.65 * uniforms.hachure_intensity);
                }
            }
        }

        // Coastline edge detection
        var has_water_neighbor = false;
        if (x > 0u && x < width - 1u && y > 0u && y < height - 1u) {
            let n_l = heightmap[idx - 1u];
            let n_r = heightmap[idx + 1u];
            let n_u = heightmap[idx - width];
            let n_d = heightmap[idx + width];
            if (n_l < 0.0 || n_r < 0.0 || n_u < 0.0 || n_d < 0.0) {
                has_water_neighbor = true;
            }
        }
        if (has_water_neighbor) {
            col = mix(col, ink_coast, 0.90);
        }
    } else if (is_lake) {
        col = mix(col, vec3<f32>(0.67, 0.78, 0.82), 0.70);
    } else if (is_river) {
        col = ink_river;
    } else {
        // Ocean Waterlining
        let d = water_dist_map[idx];
        let spacing = uniforms.waterline_spacing;
        let count = uniforms.waterline_count;

        for (var r = 1u; r <= count; r = r + 1u) {
            var target_d = spacing * 0.7;
            var ring_alpha = 0.65;
            if (r == 2u) { target_d = spacing * 1.8; ring_alpha = 0.45; }
            else if (r == 3u) { target_d = spacing * 3.2; ring_alpha = 0.30; }
            else if (r == 4u) { target_d = spacing * 5.0; ring_alpha = 0.18; }
            else if (r == 5u) { target_d = spacing * 7.5; ring_alpha = 0.10; }

            let wave = noise2d(uv * 30.0 + vec2<f32>(f32(r), f32(r))) * 0.6;
            let diff = abs(d + wave - target_d);
            if (diff < 0.65) {
                let factor = (1.0 - diff / 0.65) * ring_alpha;
                col = mix(col, ink_waterline, factor);
            }
        }

        // Coastal stippling
        if (d < 12.0) {
            let hsh = hash(vec2<f32>(f32(x), f32(y))) * 100.0;
            let dens = ((12.0 - d) / 12.0) * 28.0;
            if (hsh < dens) {
                col = mix(col, ink_waterline, 0.50);
            }
        }
    }

    output_pixels[idx] = pack_rgb(col.r, col.g, col.b);
}
