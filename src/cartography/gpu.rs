//! GPU-accelerated cartography shader using wgpu compute pipeline

use bytemuck::{Pod, Zeroable};
use image::{ImageBuffer, Rgb};
use std::borrow::Cow;
use wgpu::util::DeviceExt;

use crate::world::WorldData;
use super::params::CartographyParams;
use super::decorations::{
    render_rhumb_lines, render_graticule, render_compass_rose, render_vintage_border,
    find_ocean_centers,
};

const CARTOGRAPHY_SHADER: &str = include_str!("shader.wgsl");

/// Uniform parameters passed to the WGSL compute shader
#[repr(C)]
#[derive(Copy, Clone, Debug, Pod, Zeroable)]
struct GpuCartographyUniforms {
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
}

/// GPU compute context for cartography map rendering
pub struct GpuCartographyContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
    bind_group_layout: wgpu::BindGroupLayout,
}

impl GpuCartographyContext {
    /// Initialize GPU context for cartography rendering
    pub fn new() -> Option<Self> {
        pollster::block_on(Self::new_async())
    }

    async fn new_async() -> Option<Self> {
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

        let (device, queue) = adapter
            .request_device(
                &wgpu::DeviceDescriptor {
                    label: Some("Cartography GPU Device"),
                    required_features: wgpu::Features::empty(),
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                },
                None,
            )
            .await
            .ok()?;

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Cartography Compute Shader"),
            source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(CARTOGRAPHY_SHADER)),
        });

        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("Cartography Bind Group Layout"),
            entries: &[
                // 0: Heightmap (read)
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 1: Biome indices (read)
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
                // 2: Water distance map (read)
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 3: River flags (read)
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 4: Water depth map (read)
                wgpu::BindGroupLayoutEntry {
                    binding: 4,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 5: Uniforms (uniform)
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // 6: Output pixels (read_write)
                wgpu::BindGroupLayoutEntry {
                    binding: 6,
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

        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("Cartography Pipeline Layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });

        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("Cartography Compute Pipeline"),
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

    /// Render cartography map on GPU
    pub fn render(
        &self,
        world: &WorldData,
        params: &CartographyParams,
    ) -> Option<ImageBuffer<Rgb<u8>, Vec<u8>>> {
        let width = world.width as u32;
        let height = world.height as u32;
        let total_cells = (width * height) as usize;

        // Flatten data buffers
        let mut height_vec: Vec<f32> = Vec::with_capacity(total_cells);
        let mut biome_vec: Vec<u32> = Vec::with_capacity(total_cells);
        let mut water_depth_vec: Vec<f32> = Vec::with_capacity(total_cells);
        let mut river_vec: Vec<u32> = Vec::with_capacity(total_cells);

        for y in 0..world.height {
            for x in 0..world.width {
                height_vec.push(*world.heightmap.get(x, y));
                biome_vec.push(*world.biomes.get(x, y) as u32);
                water_depth_vec.push(*world.water_depth.get(x, y));

                let is_in_river_cache = world.river_tile_cache.as_ref().map(|c| *c.get(x, y));
                let flow_acc = world.flow_accumulation.as_ref().map(|fa| *fa.get(x, y)).unwrap_or(0.0);
                let is_river = *world.heightmap.get(x, y) >= 0.0 && match is_in_river_cache {
                    Some(cached) => cached,
                    None => flow_acc > 45.0,
                };
                river_vec.push(if is_river { 1 } else { 0 });
            }
        }

        // Compute signed water distance map using fast BFS
        let water_dist_tilemap = super::cpu::compute_water_coast_distance(&world.heightmap, &world.water_depth, 32);
        let mut water_dist_vec: Vec<f32> = Vec::with_capacity(total_cells);
        for y in 0..world.height {
            for x in 0..world.width {
                water_dist_vec.push(*water_dist_tilemap.get(x, y));
            }
        }

        let uniforms = GpuCartographyUniforms {
            width,
            height,
            paper_style: 0,
            paper_roughness: params.paper_roughness,
            paper_stains: params.paper_stains,
            vignette_strength: params.vignette_strength,
            waterline_count: params.waterline_count as u32,
            waterline_spacing: params.waterline_spacing,
            hachure_intensity: params.hachure_intensity,
            hachure_density: params.hachure_density,
            watercolor_opacity: params.watercolor_opacity,
            seed: world.seeds.master as u32,
        };

        // Create GPU buffers
        let height_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Height Buffer"),
            contents: bytemuck::cast_slice(&height_vec),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let biome_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Biome Buffer"),
            contents: bytemuck::cast_slice(&biome_vec),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let water_dist_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Water Dist Buffer"),
            contents: bytemuck::cast_slice(&water_dist_vec),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let river_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("River Buffer"),
            contents: bytemuck::cast_slice(&river_vec),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let depth_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Depth Buffer"),
            contents: bytemuck::cast_slice(&water_depth_vec),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let uniform_buf = self.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::bytes_of(&uniforms),
            usage: wgpu::BufferUsages::UNIFORM,
        });

        let out_byte_size = (total_cells * std::mem::size_of::<u32>()) as u64;
        let output_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Output Buffer"),
            size: out_byte_size,
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });

        let staging_buf = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size: out_byte_size,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let bind_group = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("Cartography Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: height_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: biome_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: water_dist_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 3, resource: river_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 4, resource: depth_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 5, resource: uniform_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 6, resource: output_buf.as_entire_binding() },
            ],
        });

        let mut encoder = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("Cartography Encoder"),
        });

        {
            let mut compute_pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("Cartography Pass"),
                timestamp_writes: None,
            });
            compute_pass.set_pipeline(&self.pipeline);
            compute_pass.set_bind_group(0, &bind_group, &[]);
            let workgroups_x = (width + 15) / 16;
            let workgroups_y = (height + 15) / 16;
            compute_pass.dispatch_workgroups(workgroups_x, workgroups_y, 1);
        }

        encoder.copy_buffer_to_buffer(&output_buf, 0, &staging_buf, 0, out_byte_size);
        self.queue.submit(Some(encoder.finish()));

        // Read back staging buffer
        let buffer_slice = staging_buf.slice(..);
        let (tx, rx) = std::sync::mpsc::channel();
        buffer_slice.map_async(wgpu::MapMode::Read, move |result| {
            let _ = tx.send(result);
        });
        self.device.poll(wgpu::Maintain::Wait);
        rx.recv().ok()?.ok()?;

        let mapped_view = buffer_slice.get_mapped_range();
        let packed_pixels: &[u32] = bytemuck::cast_slice(&mapped_view);

        let mut img: ImageBuffer<Rgb<u8>, Vec<u8>> = ImageBuffer::new(width, height);
        for y in 0..height {
            for x in 0..width {
                let pixel_val = packed_pixels[(y * width + x) as usize];
                let r = ((pixel_val >> 0) & 0xFF) as u8;
                let g = ((pixel_val >> 8) & 0xFF) as u8;
                let b = ((pixel_val >> 16) & 0xFF) as u8;
                img.put_pixel(x, y, Rgb([r, g, b]));
            }
        }
        drop(mapped_view);
        staging_buf.unmap();

        // Overlay vector decorations (rhumb lines, compass rose, vintage border)
        let border_w = if params.show_vintage_border { params.border_width } else { 0 };

        if params.show_rhumb_lines {
            let ocean_centers = find_ocean_centers(&world.heightmap, 3);
            render_rhumb_lines(&mut img, &ocean_centers, border_w);
        }

        if params.show_graticule {
            render_graticule(&mut img, border_w);
        }

        if params.show_compass_rose {
            let ocean_centers = find_ocean_centers(&world.heightmap, 2);
            if let Some(&center) = ocean_centers.first() {
                let radius = ((width.min(height) as f32) * 0.14).clamp(6.0, 75.0);
                render_compass_rose(&mut img, center, radius);
            }
        }

        if params.show_vintage_border {
            render_vintage_border(&mut img, params.border_width);
        }

        Some(img)
    }
}
