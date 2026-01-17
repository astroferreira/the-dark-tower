//! GPU-accelerated hydraulic erosion using wgpu compute shaders
//!
//! This module implements the droplet-based hydraulic erosion algorithm on the GPU
//! for significant performance improvements on supported hardware.

use bytemuck::{Pod, Zeroable};
use std::borrow::Cow;
use wgpu::util::DeviceExt;

use crate::erosion::{ErosionParams, ErosionStats};
use crate::tilemap::Tilemap;

/// Parameters passed to the GPU compute shader
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct GpuErosionParams {
    width: u32,
    height: u32,
    inertia: f32,
    capacity_factor: f32,
    erosion_rate: f32,
    deposit_rate: f32,
    evaporation: f32,
    min_volume: f32,
    max_steps: u32,
    gravity: f32,
    erosion_radius: u32,
    base_seed: u32,
}

/// GPU context for erosion computation
pub struct GpuErosionContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl GpuErosionContext {
    /// Create a new GPU erosion context
    /// Returns None if GPU is not available
    pub fn new() -> Option<Self> {
        pollster::block_on(Self::new_async())
    }

    async fn new_async() -> Option<Self> {
        // Request GPU adapter
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..Default::default()
        });

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                compatible_surface: None,
                force_fallback_adapter: false,
            })
            .await?;

        println!("GPU Adapter: {}", adapter.get_info().name);

        // Request device with compute capability
        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Erosion GPU"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .ok()?;

        // Create compute shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Erosion Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(EROSION_SHADER)),
        });

        // Create bind group layout
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Erosion Bind Group Layout"),
            entries: &[
                // Heightmap buffer (read-write storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Hardness buffer (read-only storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Parameters uniform buffer
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Delta buffer for accumulating changes (read-write storage)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Erosion Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        // Create compute pipeline
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Erosion Compute Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        Some(Self {
            device,
            queue,
            pipeline,
            bind_group_layout,
        })
    }

    /// Run GPU-accelerated hydraulic erosion
    pub fn simulate(
        &self,
        heightmap: &mut Tilemap<f32>,
        hardness: &Tilemap<f32>,
        params: &ErosionParams,
        seed: u64,
    ) -> ErosionStats {
        let width = heightmap.width;
        let height = heightmap.height;

        // Prepare heightmap data as flat f32 array
        let mut heightmap_data: Vec<f32> = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                heightmap_data.push(*heightmap.get(x, y));
            }
        }

        // Prepare hardness data
        let mut hardness_data: Vec<f32> = Vec::with_capacity(width * height);
        for y in 0..height {
            for x in 0..width {
                hardness_data.push(*hardness.get(x, y));
            }
        }

        // Create GPU buffers
        let heightmap_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Heightmap Buffer"),
            contents: bytemuck::cast_slice(&heightmap_data),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::COPY_DST,
        });

        let hardness_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Hardness Buffer"),
            contents: bytemuck::cast_slice(&hardness_data),
            usage: wgpu::BufferUsages::STORAGE,
        });

        // Delta buffer for accumulating changes (initialized to zero)
        let delta_data: Vec<f32> = vec![0.0; width * height];
        let delta_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Delta Buffer"),
            contents: bytemuck::cast_slice(&delta_data),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
        });

        // Process in batches to allow delta accumulation
        let batch_size = 65536; // Process 64K droplets per batch
        let num_batches = (params.hydraulic_iterations + batch_size - 1) / batch_size;

        for batch in 0..num_batches {
            let batch_start = batch * batch_size;
            let batch_count = (params.hydraulic_iterations - batch_start).min(batch_size);

            // Update parameters for this batch
            let gpu_params = GpuErosionParams {
                width: width as u32,
                height: height as u32,
                inertia: params.droplet_inertia,
                capacity_factor: params.droplet_capacity_factor,
                erosion_rate: params.droplet_erosion_rate,
                deposit_rate: params.droplet_deposit_rate,
                evaporation: params.droplet_evaporation,
                min_volume: params.droplet_min_volume,
                max_steps: params.droplet_max_steps as u32,
                gravity: params.droplet_gravity,
                erosion_radius: params.droplet_erosion_radius as u32,
                base_seed: (seed.wrapping_add(batch_start as u64) & 0xFFFFFFFF) as u32,
            };

            let params_buffer = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("Params Buffer"),
                contents: bytemuck::bytes_of(&gpu_params),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            // Create bind group for this batch
            let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("Erosion Bind Group"),
                layout: &self.bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: heightmap_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: hardness_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 2,
                        resource: params_buffer.as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 3,
                        resource: delta_buffer.as_entire_binding(),
                    },
                ],
            });

            // Create command encoder and dispatch compute
            let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Erosion Encoder"),
            });

            {
                let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("Erosion Pass"),
                    timestamp_writes: None,
                });

                compute_pass.set_pipeline(&self.pipeline);
                compute_pass.set_bind_group(0, &bind_group, &[]);

                // Dispatch workgroups (256 threads per group)
                let workgroup_size = 256;
                let num_workgroups = (batch_count + workgroup_size - 1) / workgroup_size;
                compute_pass.dispatch_workgroups(num_workgroups as u32, 1, 1);
            }

            // Submit commands
            self.queue.submit(std::iter::once(encoder.finish()));

            // Wait for GPU to finish this batch
            self.device.poll(wgpu::Maintain::Wait);
        }

        // Read back results
        let staging_buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size: (width * height * std::mem::size_of::<f32>()) as u64,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Copy Encoder"),
        });
        encoder.copy_buffer_to_buffer(
            &heightmap_buffer,
            0,
            &staging_buffer,
            0,
            (width * height * std::mem::size_of::<f32>()) as u64,
        );
        self.queue.submit(std::iter::once(encoder.finish()));

        // Map and read the buffer
        let buffer_slice = staging_buffer.slice(..);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = sender.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);
        receiver.recv().unwrap().unwrap();

        let data = buffer_slice.get_mapped_range();
        let result: Vec<f32> = bytemuck::cast_slice(&data).to_vec();
        drop(data);
        staging_buffer.unmap();

        // Update heightmap with results
        let mut total_eroded = 0.0f64;
        let mut total_deposited = 0.0f64;
        let mut max_erosion = 0.0f32;
        let mut max_deposition = 0.0f32;

        for y in 0..height {
            for x in 0..width {
                let idx = y * width + x;
                let old_h = *heightmap.get(x, y);
                let new_h = result[idx].clamp(-5000.0, 2000.0);
                let diff = new_h - old_h;

                if diff < 0.0 {
                    total_eroded += (-diff) as f64;
                    max_erosion = max_erosion.max(-diff);
                } else if diff > 0.0 {
                    total_deposited += diff as f64;
                    max_deposition = max_deposition.max(diff);
                }

                heightmap.set(x, y, new_h);
            }
        }

        ErosionStats {
            total_eroded,
            total_deposited,
            max_erosion,
            max_deposition,
            iterations: params.hydraulic_iterations,
            river_lengths: Vec::new(),
            steps_taken: 0,
        }
    }
}

/// Check if GPU erosion is available on this system
pub fn is_gpu_available() -> bool {
    GpuErosionContext::new().is_some()
}

/// Run GPU-accelerated hydraulic erosion if available, otherwise fall back to CPU
pub fn simulate_gpu_or_cpu(
    heightmap: &mut Tilemap<f32>,
    hardness: &Tilemap<f32>,
    params: &ErosionParams,
    seed: u64,
) -> ErosionStats {
    if let Some(ctx) = GpuErosionContext::new() {
        println!("Using GPU-accelerated erosion");
        ctx.simulate(heightmap, hardness, params, seed)
    } else {
        println!("GPU not available, using CPU parallel erosion");
        super::hydraulic::simulate_parallel(heightmap, hardness, params, seed)
    }
}

/// WGSL compute shader for hydraulic erosion
const EROSION_SHADER: &str = r#"
struct Params {
    width: u32,
    height: u32,
    inertia: f32,
    capacity_factor: f32,
    erosion_rate: f32,
    deposit_rate: f32,
    evaporation: f32,
    min_volume: f32,
    max_steps: u32,
    gravity: f32,
    erosion_radius: u32,
    base_seed: u32,
}

@group(0) @binding(0) var<storage, read_write> heightmap: array<f32>;
@group(0) @binding(1) var<storage, read> hardness: array<f32>;
@group(0) @binding(2) var<uniform> params: Params;
@group(0) @binding(3) var<storage, read_write> delta: array<f32>;

// PCG random number generator
fn pcg_hash(input: u32) -> u32 {
    let state = input * 747796405u + 2891336453u;
    let word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

fn random_f32(seed: ptr<function, u32>) -> f32 {
    *seed = pcg_hash(*seed);
    return f32(*seed) / 4294967295.0;
}

// Get height at integer coordinates
fn get_height(x: i32, y: i32) -> f32 {
    let wx = ((x % i32(params.width)) + i32(params.width)) % i32(params.width);
    let wy = clamp(y, 0, i32(params.height) - 1);
    return heightmap[u32(wy) * params.width + u32(wx)];
}

// Bilinear interpolation for smooth height sampling
fn sample_height(x: f32, y: f32) -> f32 {
    let ix = i32(floor(x));
    let iy = i32(floor(y));
    let fx = x - f32(ix);
    let fy = y - f32(iy);

    let h00 = get_height(ix, iy);
    let h10 = get_height(ix + 1, iy);
    let h01 = get_height(ix, iy + 1);
    let h11 = get_height(ix + 1, iy + 1);

    let h0 = mix(h00, h10, fx);
    let h1 = mix(h01, h11, fx);
    return mix(h0, h1, fy);
}

// Compute gradient at position using central differences
fn sample_gradient(x: f32, y: f32) -> vec2<f32> {
    let eps = 0.5;
    let gx = sample_height(x + eps, y) - sample_height(x - eps, y);
    let gy = sample_height(x, y + eps) - sample_height(x, y - eps);
    return vec2<f32>(gx, gy);
}

@compute @workgroup_size(256)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let droplet_id = global_id.x;

    // Initialize RNG with unique seed per droplet
    var rng_state = params.base_seed ^ (droplet_id * 1664525u + 1013904223u);

    // Random starting position on land (above sea level)
    var pos_x = random_f32(&rng_state) * f32(params.width);
    var pos_y = random_f32(&rng_state) * f32(params.height);

    // Try to find a position above sea level
    for (var attempt = 0u; attempt < 10u; attempt++) {
        let h = sample_height(pos_x, pos_y);
        if (h > 0.0) {
            break;
        }
        pos_x = random_f32(&rng_state) * f32(params.width);
        pos_y = random_f32(&rng_state) * f32(params.height);
    }

    // Droplet state
    var dir_x = 0.0;
    var dir_y = 0.0;
    var velocity = 1.0;
    var water = 1.0;
    var sediment = 0.0;

    // Simulate droplet path
    for (var step = 0u; step < params.max_steps; step++) {
        // Stop if water evaporated
        if (water < params.min_volume) {
            break;
        }

        // Get current cell
        let cell_x = i32(floor(pos_x));
        let cell_y = i32(floor(pos_y));

        // Sample height and gradient
        let height_old = sample_height(pos_x, pos_y);
        let gradient = sample_gradient(pos_x, pos_y);

        // Stop if in ocean (below sea level)
        if (height_old < 0.0) {
            // Deposit all sediment at coast
            let wx = u32(((cell_x % i32(params.width)) + i32(params.width)) % i32(params.width));
            let wy = u32(clamp(cell_y, 0, i32(params.height) - 1));
            let idx = wy * params.width + wx;
            // Use atomicAdd for thread safety (approximate with regular add for now)
            delta[idx] = delta[idx] + sediment;
            break;
        }

        // Update direction with inertia
        dir_x = dir_x * params.inertia - gradient.x * (1.0 - params.inertia);
        dir_y = dir_y * params.inertia - gradient.y * (1.0 - params.inertia);

        // Normalize direction
        let len = sqrt(dir_x * dir_x + dir_y * dir_y);
        if (len > 0.0001) {
            dir_x = dir_x / len;
            dir_y = dir_y / len;
        } else {
            // Random direction if stuck
            let angle = random_f32(&rng_state) * 6.28318;
            dir_x = cos(angle);
            dir_y = sin(angle);
        }

        // Move droplet
        let new_pos_x = pos_x + dir_x;
        var new_pos_y = pos_y + dir_y;

        // Clamp Y, wrap X
        new_pos_y = clamp(new_pos_y, 0.0, f32(params.height) - 1.0);

        // Sample new height
        let height_new = sample_height(new_pos_x, new_pos_y);
        let height_diff = height_new - height_old;

        // Calculate sediment capacity
        let slope = max(-height_diff, 0.01);
        let capacity = max(slope, 0.01) * velocity * water * params.capacity_factor;

        // Get cell index for modification
        let wx = u32(((cell_x % i32(params.width)) + i32(params.width)) % i32(params.width));
        let wy = u32(clamp(cell_y, 0, i32(params.height) - 1));
        let idx = wy * params.width + wx;

        // Get hardness at this cell
        let hard = hardness[idx];

        if (sediment > capacity || height_diff > 0.0) {
            // Deposit sediment
            let deposit_amount = select(
                (sediment - capacity) * params.deposit_rate,
                min(height_diff, sediment),
                height_diff > 0.0
            );
            sediment = sediment - deposit_amount;
            delta[idx] = delta[idx] + deposit_amount;
        } else {
            // Erode terrain
            let erode_amount = min((capacity - sediment) * params.erosion_rate * (1.0 - hard), -height_diff);
            let clamped_erode = max(erode_amount, 0.0);
            sediment = sediment + clamped_erode;
            delta[idx] = delta[idx] - clamped_erode;
        }

        // Update velocity based on height change
        velocity = sqrt(max(velocity * velocity - height_diff * params.gravity, 0.01));

        // Evaporate water
        water = water * (1.0 - params.evaporation);

        // Update position
        pos_x = new_pos_x;
        pos_y = new_pos_y;
    }
}
"#;
