use eframe::egui::{self, ColorImage, TextureHandle, TextureOptions};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::f64::consts::PI;
use std::sync::mpsc::{channel, Receiver, Sender};
use std::thread;

use crate::{biomes, climate, erosion, export, heightmap, plates, scale, tileset};
use crate::biomes::{ExtendedBiome, WorldBiomeConfig, BiomeCategory};
use crate::erosion::ErosionParams;
use crate::plates::{Plate, PlateId};
use crate::tilemap::Tilemap;
use crate::tileset::Tileset;

/// View modes for the interactive viewer
#[derive(Clone, Copy, Debug, PartialEq)]
enum ViewMode {
    Terrain,      // Biome-colored terrain
    Shaded,       // Terrain with hillshading
    Fantasy,      // Fantasy/extended biomes
    FantasyShaded,// Fantasy biomes with hillshading
    Tileset,      // Pixel-art tileset rendering
    Globe,        // 3D globe projection
    Plates,       // Plate colors
    Stress,       // Stress at boundaries
    Heightmap,    // Raw heightmap (spectral colormap)
}

impl ViewMode {
    fn label(&self) -> &'static str {
        match self {
            ViewMode::Terrain => "Terrain (Biomes)",
            ViewMode::Shaded => "Shaded Terrain",
            ViewMode::Fantasy => "Fantasy Biomes",
            ViewMode::FantasyShaded => "Fantasy Shaded",
            ViewMode::Tileset => "Tileset (Pixel Art)",
            ViewMode::Globe => "Globe",
            ViewMode::Plates => "Plate Colors",
            ViewMode::Stress => "Plate Stress",
            ViewMode::Heightmap => "Heightmap",
        }
    }

    fn all() -> &'static [ViewMode] {
        &[
            ViewMode::Terrain,
            ViewMode::Shaded,
            ViewMode::Fantasy,
            ViewMode::FantasyShaded,
            ViewMode::Tileset,
            ViewMode::Globe,
            ViewMode::Plates,
            ViewMode::Stress,
            ViewMode::Heightmap,
        ]
    }
}

/// Cached planet data to avoid regenerating on view switch
struct PlanetData {
    heightmap: Tilemap<f32>,
    plate_map: Tilemap<PlateId>,
    plates: Vec<Plate>,
    stress_map: Tilemap<f32>,
    temperature: Tilemap<f32>,
    moisture: Tilemap<f32>,
    extended_biomes: Option<Tilemap<ExtendedBiome>>,
    // Upscaled versions (computed on demand)
    upscaled_heightmap: Option<Tilemap<f32>>,
    upscaled_temperature: Option<Tilemap<f32>>,
    upscaled_moisture: Option<Tilemap<f32>>,
    upscaled_stress: Option<Tilemap<f32>>,
    upscale_factor: usize,
}

/// Message types for async generation
enum GenerationMessage {
    Start { width: usize, height: usize, seed: u64, params: ErosionParams, map_scale: scale::MapScale },
    Done(PlanetData),
}

/// Main viewer application state
struct PlanetViewerApp {
    // Map dimensions
    width: usize,
    height: usize,
    last_width: usize,
    last_height: usize,

    // Current seed
    seed: u64,
    last_seed: u64,

    // Erosion parameters (editable via sliders)
    params: ErosionParams,
    last_params: ErosionParams,

    // Planet data (None while generating)
    planet: Option<PlanetData>,

    // View state
    view_mode: ViewMode,

    // Globe rotation
    globe_rotation: f64,
    globe_tilt: f64,

    // Rendered texture
    texture: Option<TextureHandle>,
    needs_render: bool,

    // Async generation
    gen_sender: Sender<GenerationMessage>,
    gen_receiver: Receiver<GenerationMessage>,
    is_generating: bool,

    // Auto-regenerate when params change
    auto_regenerate: bool,

    // Tileset for pixel-art rendering
    tileset: Option<Tileset>,

    // Animation time for water effects
    animation_time: f64,
    animate_water: bool,

    // Biome configuration for fantasy biomes
    biome_config: WorldBiomeConfig,
    last_biome_config_fantasy_intensity: f32,

    // Map scale configuration
    map_scale: scale::MapScale,
    scale_preset: scale::ScalePreset,

    // Upscaling settings
    upscale_factor: usize,
    upscale_detail: f32,
    upscale_detail_freq: f32,

    // Zoom and pan
    zoom: f32,
    pan_offset: egui::Vec2,
    is_panning: bool,
    last_mouse_pos: Option<egui::Pos2>,

    // Hover info for debugging
    hover_info: Option<String>,
}

impl PlanetViewerApp {
    fn new(cc: &eframe::CreationContext<'_>, width: usize, height: usize, initial_seed: Option<u64>) -> Self {
        // Set up async generation channel
        let (tx, rx) = channel();
        let (gen_tx, gen_rx) = channel();

        // Spawn generator thread
        let gen_sender = gen_tx.clone();
        thread::spawn(move || {
            loop {
                match rx.recv() {
                    Ok(GenerationMessage::Start { width, height, seed, params, map_scale }) => {
                        let planet = generate_planet_data_with_params(width, height, seed, &params, &map_scale);
                        let _ = gen_sender.send(GenerationMessage::Done(planet));
                    }
                    Ok(GenerationMessage::Done(_)) => {}
                    Err(_) => break,
                }
            }
        });

        let seed = initial_seed.unwrap_or_else(rand::random);
        let params = ErosionParams::default();
        let default_scale = scale::MapScale::default();

        // Load tileset
        let tileset = Tileset::load();
        if tileset.is_none() {
            eprintln!("Warning: Could not load tileset from docs/tileset.png");
        }

        // Start initial generation
        let _ = tx.send(GenerationMessage::Start {
            width,
            height,
            seed,
            params: params.clone(),
            map_scale: default_scale,
        });

        let biome_config = WorldBiomeConfig::default();

        Self {
            width,
            height,
            last_width: width,
            last_height: height,
            seed,
            last_seed: seed,
            params: params.clone(),
            last_params: params,
            planet: None,
            view_mode: ViewMode::Shaded,
            globe_rotation: 0.0,
            globe_tilt: 0.0,
            texture: None,
            needs_render: true,
            gen_sender: tx,
            gen_receiver: gen_rx,
            is_generating: true,
            auto_regenerate: true,
            tileset,
            animation_time: 0.0,
            animate_water: true,
            biome_config: biome_config.clone(),
            last_biome_config_fantasy_intensity: biome_config.fantasy_intensity,
            map_scale: scale::MapScale::regional(),
            scale_preset: scale::ScalePreset::Regional,
            upscale_factor: 1,
            upscale_detail: 0.3,
            upscale_detail_freq: 8.0,
            zoom: 1.0,
            pan_offset: egui::Vec2::ZERO,
            is_panning: false,
            last_mouse_pos: None,
            hover_info: None,
        }
    }

    fn regenerate(&mut self) {
        if self.is_generating {
            return;
        }

        self.is_generating = true;
        self.planet = None;
        self.texture = None;

        let _ = self.gen_sender.send(GenerationMessage::Start {
            width: self.width,
            height: self.height,
            seed: self.seed,
            params: self.params.clone(),
            map_scale: self.map_scale,
        });
    }

    fn render_to_texture(&mut self, ctx: &egui::Context) {
        let Some(planet) = &mut self.planet else { return };

        // Generate upscaled heightmap if needed
        if self.upscale_factor > 1 && planet.upscaled_heightmap.is_none() {
            println!("Upscaling heightmap {}x...", self.upscale_factor);
            planet.upscaled_heightmap = Some(planet.heightmap.upscale_with_detail(
                self.upscale_factor,
                self.upscale_detail,
                self.upscale_detail_freq,
                self.seed,
            ));
            planet.upscaled_temperature = Some(planet.temperature.upscale(self.upscale_factor));
            planet.upscaled_moisture = Some(planet.moisture.upscale(self.upscale_factor));
            planet.upscaled_stress = Some(planet.stress_map.upscale(self.upscale_factor));
            planet.upscale_factor = self.upscale_factor;
            println!("Upscaling complete: {}x{} -> {}x{}",
                planet.heightmap.width, planet.heightmap.height,
                planet.heightmap.width * self.upscale_factor,
                planet.heightmap.height * self.upscale_factor);
        }

        // Select which heightmap to use for rendering
        let render_heightmap = if self.upscale_factor > 1 {
            planet.upscaled_heightmap.as_ref().unwrap_or(&planet.heightmap)
        } else {
            &planet.heightmap
        };

        let render_temperature = if self.upscale_factor > 1 {
            planet.upscaled_temperature.as_ref().unwrap_or(&planet.temperature)
        } else {
            &planet.temperature
        };

        let render_moisture = if self.upscale_factor > 1 {
            planet.upscaled_moisture.as_ref().unwrap_or(&planet.moisture)
        } else {
            &planet.moisture
        };

        let render_stress = if self.upscale_factor > 1 {
            planet.upscaled_stress.as_ref().unwrap_or(&planet.stress_map)
        } else {
            &planet.stress_map
        };

        // Generate extended biomes if needed and not cached (use upscaled data)
        if (self.view_mode == ViewMode::Fantasy || self.view_mode == ViewMode::FantasyShaded)
            && planet.extended_biomes.is_none()
        {
            planet.extended_biomes = Some(biomes::generate_extended_biomes(
                render_heightmap,
                render_temperature,
                render_moisture,
                render_stress,
                &self.biome_config,
                self.seed,
            ));
        }

        let img = match self.view_mode {
            ViewMode::Terrain => export::render_terrain_map(render_heightmap),
            ViewMode::Shaded => {
                if self.animate_water {
                    export::render_terrain_shaded_animated(render_heightmap, self.animation_time)
                } else {
                    // Use extended shading with fantasy biomes
                    export::render_terrain_shaded_extended(
                        render_heightmap,
                        render_temperature,
                        render_stress,
                        &self.biome_config,
                        self.seed,
                    )
                }
            }
            ViewMode::Fantasy => {
                if let Some(ext_biomes) = &planet.extended_biomes {
                    export::render_terrain_extended(ext_biomes)
                } else {
                    export::render_terrain_map(render_heightmap)
                }
            }
            ViewMode::FantasyShaded => {
                if let Some(ext_biomes) = &planet.extended_biomes {
                    export::render_terrain_extended_shaded(render_heightmap, ext_biomes)
                } else {
                    export::render_terrain_shaded(render_heightmap)
                }
            }
            ViewMode::Tileset => {
                if let Some(ts) = &self.tileset {
                    // Always use BASE resolution for tileset (tiles represent discrete map cells)
                    // Using upscaled data would make tiles cover too many pixels
                    let base_heightmap = &planet.heightmap;
                    let base_temperature = &planet.temperature;
                    let base_moisture = &planet.moisture;

                    // Use scaled rendering for large maps to avoid huge images
                    let w = base_heightmap.width;
                    let h = base_heightmap.height;
                    let scale = if w > 128 || h > 64 {
                        (w / 64).max(h / 32).max(1)
                    } else {
                        1
                    };
                    tileset::render_tileset_map_scaled(
                        base_heightmap,
                        base_temperature,
                        base_moisture,
                        ts,
                        scale,
                    )
                } else {
                    // Fallback to terrain if tileset not loaded
                    export::render_terrain_map(render_heightmap)
                }
            }
            ViewMode::Globe => render_globe_3d(
                render_heightmap,
                self.globe_rotation,
                self.globe_tilt,
                render_heightmap.width,
                render_heightmap.height,
            ),
            ViewMode::Plates => export::render_plate_map(&planet.plate_map, &planet.plates),
            ViewMode::Stress => export::render_stress_map(render_stress),
            ViewMode::Heightmap => export::render_heightmap(render_heightmap),
        };

        // Convert to egui texture
        let size = [img.width() as usize, img.height() as usize];
        let pixels: Vec<egui::Color32> = img.pixels()
            .map(|p| egui::Color32::from_rgb(p[0], p[1], p[2]))
            .collect();

        let color_image = ColorImage {
            size,
            pixels,
        };

        self.texture = Some(ctx.load_texture(
            "planet_view",
            color_image,
            TextureOptions::LINEAR,
        ));

        self.needs_render = false;
    }
}

impl eframe::App for PlanetViewerApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Update animation time for water effects
        if self.animate_water && self.view_mode == ViewMode::Shaded && self.planet.is_some() {
            self.animation_time += ctx.input(|i| i.stable_dt) as f64;
            self.needs_render = true;
        }

        // Check for generation completion
        if let Ok(GenerationMessage::Done(planet)) = self.gen_receiver.try_recv() {
            self.planet = Some(planet);
            self.is_generating = false;
            self.needs_render = true;
            // Update last known state after successful generation
            self.last_params = self.params.clone();
            self.last_seed = self.seed;
            self.last_width = self.width;
            self.last_height = self.height;
        }

        // Auto-regenerate if params, seed, or resolution changed
        if self.auto_regenerate && !self.is_generating {
            if self.params != self.last_params || self.seed != self.last_seed
                || self.width != self.last_width || self.height != self.last_height {
                self.regenerate();
            }
        }

        // Re-render if needed
        if self.needs_render && self.planet.is_some() {
            self.render_to_texture(ctx);
        }

        // Side panel with controls
        egui::SidePanel::left("controls")
            .default_width(280.0)
            .show(ctx, |ui| {
                ui.heading("Planet Generator");
                ui.separator();

                // Seed controls
                ui.horizontal(|ui| {
                    ui.label("Seed:");
                    let mut seed_str = format!("{}", self.seed);
                    if ui.text_edit_singleline(&mut seed_str).changed() {
                        if let Ok(new_seed) = seed_str.parse() {
                            self.seed = new_seed;
                        }
                    }
                });

                ui.horizontal(|ui| {
                    if ui.button("Random Seed").clicked() {
                        self.seed = rand::random();
                    }
                    if ui.button("Regenerate").clicked() && !self.is_generating {
                        self.regenerate();
                    }
                });

                ui.checkbox(&mut self.auto_regenerate, "Auto-regenerate on change");

                if self.is_generating {
                    ui.spinner();
                    ui.label("Generating...");
                }

                ui.separator();

                // Resolution controls
                ui.heading("Resolution");

                ui.horizontal(|ui| {
                    ui.label("Width:");
                    let mut width_val = self.width as i32;
                    if ui.add(egui::DragValue::new(&mut width_val)
                        .range(128..=2048)
                        .speed(16)
                    ).changed() {
                        self.width = (width_val as usize).clamp(128, 2048);
                    }
                });

                ui.horizontal(|ui| {
                    ui.label("Height:");
                    let mut height_val = self.height as i32;
                    if ui.add(egui::DragValue::new(&mut height_val)
                        .range(64..=1024)
                        .speed(8)
                    ).changed() {
                        self.height = (height_val as usize).clamp(64, 1024);
                    }
                });

                // Preset buttons
                ui.horizontal(|ui| {
                    if ui.button("256x128").clicked() {
                        self.width = 256;
                        self.height = 128;
                    }
                    if ui.button("512x256").clicked() {
                        self.width = 512;
                        self.height = 256;
                    }
                });
                ui.horizontal(|ui| {
                    if ui.button("1024x512").clicked() {
                        self.width = 1024;
                        self.height = 512;
                    }
                    if ui.button("2048x1024").clicked() {
                        self.width = 2048;
                        self.height = 1024;
                    }
                });

                ui.separator();

                // Map Scale controls
                ui.heading("Map Scale");

                // Scale preset dropdown
                let mut scale_changed = false;
                ui.horizontal(|ui| {
                    ui.label("Preset:");
                    egui::ComboBox::from_id_salt("scale_preset")
                        .selected_text(self.scale_preset.display_name())
                        .show_ui(ui, |ui| {
                            for preset in scale::ScalePreset::all() {
                                if *preset != scale::ScalePreset::Custom {
                                    if ui.selectable_value(&mut self.scale_preset, *preset, preset.display_name()).clicked() {
                                        if let Some(new_scale) = preset.to_scale() {
                                            self.map_scale = new_scale;
                                            scale_changed = true;
                                        }
                                    }
                                }
                            }
                            ui.selectable_value(&mut self.scale_preset, scale::ScalePreset::Custom, "Custom");
                        });
                });

                // Custom scale slider (only shown when Custom is selected)
                if self.scale_preset == scale::ScalePreset::Custom {
                    ui.horizontal(|ui| {
                        ui.label("km/tile:");
                        let mut km = self.map_scale.km_per_tile;
                        if ui.add(egui::Slider::new(&mut km, 0.5..=100.0).logarithmic(true)).changed() {
                            self.map_scale = scale::MapScale::new(km);
                            // Note: don't auto-regenerate on slider drag, wait for release
                        }
                    });
                    // Button to apply custom scale
                    if ui.button("Apply Scale").clicked() {
                        scale_changed = true;
                    }
                }

                // Regenerate if scale preset changed
                if scale_changed && !self.is_generating {
                    self.regenerate();
                }

                // Display map size info
                let (w_km, h_km) = self.map_scale.map_size_km(self.width, self.height);
                ui.label(format!("Map size: {:.0} × {:.0} km", w_km, h_km));
                ui.label(format!("1 tile = {:.1} km", self.map_scale.km_per_tile));

                ui.separator();

                // Upscaling controls
                ui.heading("Upscaling");
                ui.horizontal(|ui| {
                    ui.label("Factor:");
                    let factors = [1, 2, 4, 8];
                    for &f in &factors {
                        let label = if f == 1 { "1x".to_string() } else { format!("{}x", f) };
                        if ui.selectable_label(self.upscale_factor == f, label).clicked() {
                            if self.upscale_factor != f {
                                self.upscale_factor = f;
                                // Invalidate cached upscaled data
                                if let Some(planet) = &mut self.planet {
                                    if planet.upscale_factor != f {
                                        planet.upscaled_heightmap = None;
                                        planet.upscaled_temperature = None;
                                        planet.upscaled_moisture = None;
                                        planet.upscaled_stress = None;
                                        planet.upscale_factor = f;
                                    }
                                }
                                self.needs_render = true;
                            }
                        }
                    }
                });

                if self.upscale_factor > 1 {
                    if let Some(planet) = &self.planet {
                        let base_res = format!("{}x{}", planet.heightmap.width, planet.heightmap.height);
                        let upscaled_res = format!("{}x{}",
                            planet.heightmap.width * self.upscale_factor,
                            planet.heightmap.height * self.upscale_factor);
                        ui.label(format!("{} -> {}", base_res, upscaled_res));
                    }

                    ui.add(egui::Slider::new(&mut self.upscale_detail, 0.0..=1.0)
                        .text("Detail noise")
                        .step_by(0.05));

                    ui.add(egui::Slider::new(&mut self.upscale_detail_freq, 1.0..=20.0)
                        .text("Detail freq")
                        .step_by(1.0));

                    if ui.button("Apply upscale").clicked() {
                        if let Some(planet) = &mut self.planet {
                            planet.upscaled_heightmap = None;
                            planet.upscale_factor = self.upscale_factor;
                        }
                        self.needs_render = true;
                    }
                }

                ui.separator();

                // View mode selection
                ui.heading("View Mode");
                for mode in ViewMode::all() {
                    if ui.selectable_label(self.view_mode == *mode, mode.label()).clicked() {
                        if self.view_mode != *mode {
                            self.view_mode = *mode;
                            self.needs_render = true;
                        }
                    }
                }

                // Shaded view controls (water animation)
                if self.view_mode == ViewMode::Shaded {
                    ui.separator();
                    ui.heading("Water Effects");
                    ui.checkbox(&mut self.animate_water, "Animate water");
                    if self.animate_water {
                        ui.label(format!("Time: {:.1}s", self.animation_time));
                        if ui.button("Reset animation").clicked() {
                            self.animation_time = 0.0;
                        }
                    }
                }

                // Globe controls (only show when in globe mode)
                if self.view_mode == ViewMode::Globe {
                    ui.separator();
                    ui.heading("Globe Controls");

                    let mut changed = false;
                    changed |= ui.add(egui::Slider::new(&mut self.globe_rotation, -PI..=PI)
                        .text("Rotation")).changed();
                    changed |= ui.add(egui::Slider::new(&mut self.globe_tilt, -1.4..=1.4)
                        .text("Tilt")).changed();

                    if changed {
                        self.needs_render = true;
                    }
                }

                // Fantasy biome controls (show when in Fantasy modes)
                if self.view_mode == ViewMode::Fantasy || self.view_mode == ViewMode::FantasyShaded {
                    ui.separator();
                    ui.heading("Fantasy Biomes");

                    // Fantasy intensity slider
                    let mut intensity_changed = false;
                    intensity_changed |= ui.add(egui::Slider::new(&mut self.biome_config.fantasy_intensity, 0.0..=1.0)
                        .text("Fantasy Intensity")
                        .step_by(0.05)).changed();

                    if intensity_changed {
                        // Invalidate cached extended biomes when config changes
                        if let Some(planet) = &mut self.planet {
                            planet.extended_biomes = None;
                        }
                        self.needs_render = true;
                    }

                    ui.add_space(4.0);

                    // Collapsible sections for each biome category
                    for category in BiomeCategory::all_fantasy() {
                        ui.collapsing(category.display_name(), |ui| {
                            for biome in ExtendedBiome::fantasy_biomes() {
                                if biome.category() == *category {
                                    if let Some(config) = self.biome_config.biomes.get_mut(biome) {
                                        ui.horizontal(|ui| {
                                            let checkbox_changed = ui.checkbox(&mut config.enabled, "").changed();
                                            ui.label(biome.display_name());

                                            if config.enabled {
                                                let slider_changed = ui.add(
                                                    egui::Slider::new(&mut config.rarity, 0.0..=1.0)
                                                        .show_value(false)
                                                        .fixed_decimals(2)
                                                ).changed();

                                                if checkbox_changed || slider_changed {
                                                    if let Some(planet) = &mut self.planet {
                                                        planet.extended_biomes = None;
                                                    }
                                                    self.needs_render = true;
                                                }
                                            } else if checkbox_changed {
                                                if let Some(planet) = &mut self.planet {
                                                    planet.extended_biomes = None;
                                                }
                                                self.needs_render = true;
                                            }
                                        });
                                    }
                                }
                            }
                        });
                    }
                }

                ui.separator();

                // Collapsible erosion parameters
                ui.collapsing("Erosion Parameters", |ui| {
                    ui.add_space(4.0);

                    // Erosion toggles
                    ui.checkbox(&mut self.params.enable_rivers, "Enable Rivers");
                    ui.checkbox(&mut self.params.enable_hydraulic, "Enable Hydraulic");
                    ui.checkbox(&mut self.params.enable_glacial, "Enable Glacial");
                    ui.checkbox(&mut self.params.use_gpu, "Use GPU (if available)");

                    ui.add_space(8.0);
                    ui.label("River Parameters");

                    ui.add(egui::Slider::new(&mut self.params.river_erosion_rate, 0.0..=5.0)
                        .text("Erosion Rate")
                        .step_by(0.1));

                    ui.add(egui::Slider::new(&mut self.params.river_capacity_factor, 1.0..=100.0)
                        .text("Capacity Factor")
                        .step_by(1.0));

                    ui.add(egui::Slider::new(&mut self.params.river_max_erosion, 10.0..=500.0)
                        .text("Max Erosion")
                        .step_by(10.0));

                    ui.add(egui::Slider::new(&mut self.params.river_max_deposition, 1.0..=100.0)
                        .text("Max Deposition")
                        .step_by(1.0));

                    ui.add(egui::Slider::new(&mut self.params.river_channel_width, 1..=10)
                        .text("Channel Width"));

                    ui.add_space(8.0);
                    ui.label("Hydraulic Parameters");

                    // Use logarithmic slider for iterations
                    let mut log_iters = (self.params.hydraulic_iterations as f32).log10();
                    if ui.add(egui::Slider::new(&mut log_iters, 4.0..=6.5)
                        .text("Iterations (log10)")
                        .step_by(0.1)).changed() {
                        self.params.hydraulic_iterations = 10f32.powf(log_iters) as usize;
                    }
                    ui.label(format!("  = {} iterations", self.params.hydraulic_iterations));

                    ui.add(egui::Slider::new(&mut self.params.droplet_inertia, 0.0..=1.0)
                        .text("Inertia")
                        .step_by(0.05));

                    ui.add(egui::Slider::new(&mut self.params.droplet_erosion_rate, 0.0..=1.0)
                        .text("Droplet Erosion")
                        .step_by(0.01));

                    ui.add(egui::Slider::new(&mut self.params.droplet_deposit_rate, 0.0..=1.0)
                        .text("Droplet Deposit")
                        .step_by(0.01));

                    ui.add(egui::Slider::new(&mut self.params.droplet_evaporation, 0.0..=0.1)
                        .text("Evaporation")
                        .step_by(0.005));

                    ui.add(egui::Slider::new(&mut self.params.droplet_capacity_factor, 1.0..=50.0)
                        .text("Capacity Factor")
                        .step_by(1.0));

                    ui.add_space(8.0);
                    ui.label("Glacial Parameters");

                    ui.add(egui::Slider::new(&mut self.params.glacial_timesteps, 0..=1000)
                        .text("Timesteps"));

                    ui.add(egui::Slider::new(&mut self.params.erosion_coefficient, 0.0..=0.001)
                        .text("Erosion Coeff"));

                    ui.add_space(8.0);
                    if ui.button("Reset to Defaults").clicked() {
                        self.params = ErosionParams::default();
                    }
                });

                ui.separator();

                // Status info
                ui.label(format!("Map: {}x{}", self.width, self.height));
                ui.label(format!("Seed: {}", self.seed));
            });

        // Main panel with the rendered image
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(texture) = &self.texture {
                // Calculate available size
                let available = ui.available_size();

                // Fit image to available space while maintaining aspect ratio (base size)
                let img_aspect = texture.size()[0] as f32 / texture.size()[1] as f32;
                let available_aspect = available.x / available.y;

                let (base_w, base_h) = if img_aspect > available_aspect {
                    // Image is wider - fit to width
                    (available.x, available.x / img_aspect)
                } else {
                    // Image is taller - fit to height
                    (available.y * img_aspect, available.y)
                };

                // Apply zoom to display size
                let disp_w = base_w * self.zoom;
                let disp_h = base_h * self.zoom;

                // Create a scrollable area for panning when zoomed
                egui::ScrollArea::both()
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .drag_to_scroll(false) // We'll handle this ourselves
                    .show(ui, |ui| {
                        // Center the image when not zoomed or partially zoomed
                        let padding_x = ((available.x - disp_w) / 2.0).max(0.0);
                        let padding_y = ((available.y - disp_h) / 2.0).max(0.0);

                        ui.add_space(padding_y);
                        ui.horizontal(|ui| {
                            ui.add_space(padding_x);

                            let response = ui.add(
                                egui::Image::new(texture)
                                    .fit_to_exact_size(egui::vec2(disp_w, disp_h))
                            );

                            // Handle mouse wheel zoom
                            let hover_pos = ui.input(|i| i.pointer.hover_pos());
                            if response.hovered() {
                                let scroll_delta = ui.input(|i| i.raw_scroll_delta.y);
                                if scroll_delta != 0.0 {
                                    let zoom_factor = 1.0 + scroll_delta * 0.001;
                                    let old_zoom = self.zoom;
                                    self.zoom = (self.zoom * zoom_factor).clamp(0.5, 8.0);

                                    // Adjust pan offset to zoom towards mouse position
                                    if let Some(pos) = hover_pos {
                                        let image_rect = response.rect;
                                        let rel_pos = pos - image_rect.center();
                                        let zoom_change = self.zoom / old_zoom;
                                        self.pan_offset = self.pan_offset * zoom_change + rel_pos * (1.0 - zoom_change);
                                    }
                                }
                            }

                            // Compute hover info for biome debugging
                            if response.hovered() {
                                if let Some(pos) = hover_pos {
                                    if let Some(planet) = &self.planet {
                                        let image_rect = response.rect;
                                        // Calculate pixel position in the displayed image
                                        let rel_x = (pos.x - image_rect.left()) / disp_w;
                                        let rel_y = (pos.y - image_rect.top()) / disp_h;

                                        if rel_x >= 0.0 && rel_x < 1.0 && rel_y >= 0.0 && rel_y < 1.0 {
                                            // Map to actual heightmap coordinates
                                            let map_w = planet.heightmap.width;
                                            let map_h = planet.heightmap.height;
                                            let mx = (rel_x * map_w as f32) as usize;
                                            let my = (rel_y * map_h as f32) as usize;

                                            let h = *planet.heightmap.get(mx, my);
                                            let temp = *planet.temperature.get(mx, my);
                                            let moist = *planet.moisture.get(mx, my);
                                            let stress = *planet.stress_map.get(mx, my);

                                            // Use extended biome classification for fantasy biomes
                                            use noise::{Perlin, Seedable};
                                            let biome_noise = Perlin::new(1).set_seed(self.seed as u32);
                                            let ext_biome = biomes::classify_extended(
                                                h, temp, moist, stress,
                                                mx, my, map_w, map_h,
                                                &self.biome_config, &biome_noise,
                                            );

                                            self.hover_info = Some(format!(
                                                "({}, {}) h:{:.0}m t:{:.1}°C m:{:.2} s:{:.0} => {}",
                                                mx, my, h, temp, moist, stress, ext_biome.display_name()
                                            ));
                                        }
                                    }
                                }
                            } else {
                                self.hover_info = None;
                            }

                            // Handle drag panning (for non-globe views or when zoomed)
                            if self.view_mode == ViewMode::Globe {
                                // Globe mode: drag rotates the globe
                                if response.dragged() {
                                    let delta = response.drag_delta();
                                    self.globe_rotation -= delta.x as f64 * 0.01;
                                    self.globe_tilt = (self.globe_tilt + delta.y as f64 * 0.01)
                                        .clamp(-PI / 2.0 + 0.1, PI / 2.0 - 0.1);
                                    self.needs_render = true;
                                }
                            } else if self.zoom > 1.0 {
                                // Other modes when zoomed: drag pans the view
                                if response.dragged_by(egui::PointerButton::Primary) {
                                    let delta = response.drag_delta();
                                    self.pan_offset += delta;

                                    // Constrain pan offset to keep image visible
                                    let max_pan_x = (disp_w - available.x).max(0.0) / 2.0;
                                    let max_pan_y = (disp_h - available.y).max(0.0) / 2.0;
                                    self.pan_offset.x = self.pan_offset.x.clamp(-max_pan_x, max_pan_x);
                                    self.pan_offset.y = self.pan_offset.y.clamp(-max_pan_y, max_pan_y);
                                }
                            }

                            ui.add_space(padding_x);
                        });
                        ui.add_space(padding_y);
                    });

                // Display zoom level indicator
                egui::Area::new(egui::Id::new("zoom_indicator"))
                    .anchor(egui::Align2::RIGHT_BOTTOM, egui::vec2(-10.0, -10.0))
                    .show(ctx, |ui| {
                        ui.horizontal(|ui| {
                            if ui.small_button("-").clicked() {
                                self.zoom = (self.zoom / 1.25).max(0.5);
                            }
                            ui.label(format!("{:.0}%", self.zoom * 100.0));
                            if ui.small_button("+").clicked() {
                                self.zoom = (self.zoom * 1.25).min(8.0);
                            }
                            if self.zoom != 1.0 && ui.small_button("Reset").clicked() {
                                self.zoom = 1.0;
                                self.pan_offset = egui::Vec2::ZERO;
                            }
                        });
                    });

                // Display hover info (biome debugging)
                if let Some(info) = &self.hover_info {
                    egui::Area::new(egui::Id::new("hover_info"))
                        .anchor(egui::Align2::LEFT_BOTTOM, egui::vec2(10.0, -10.0))
                        .show(ctx, |ui| {
                            egui::Frame::popup(ui.style()).show(ui, |ui| {
                                ui.label(info);
                            });
                        });
                }
            } else if self.is_generating {
                ui.centered_and_justified(|ui| {
                    ui.spinner();
                });
            }
        });

        // Request repaint while generating or globe is being dragged
        if self.is_generating || self.needs_render {
            ctx.request_repaint();
        }
    }
}

/// Run the interactive planet viewer with egui.
pub fn run_viewer(width: usize, height: usize, initial_seed: Option<u64>) {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 700.0])
            .with_title("Planet Generator"),
        ..Default::default()
    };

    eframe::run_native(
        "Planet Generator",
        options,
        Box::new(move |cc| {
            Ok(Box::new(PlanetViewerApp::new(cc, width, height, initial_seed)))
        }),
    ).expect("Failed to start viewer");
}

/// Generate planet data with custom erosion parameters
fn generate_planet_data_with_params(width: usize, height: usize, seed: u64, params: &ErosionParams, map_scale: &scale::MapScale) -> PlanetData {
    println!("Generating planet with seed: {} (scale: {} km/tile)...", seed, map_scale.km_per_tile);

    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    // Generate plates
    let (plate_map, plates) = plates::generate_plates(width, height, None, &mut rng);

    // Calculate stress
    let stress_map = plates::calculate_stress(&plate_map, &plates);

    // Generate heightmap with scale
    let mut heightmap = heightmap::generate_heightmap_scaled(&plate_map, &plates, &stress_map, seed, map_scale);

    // Generate temperature for erosion
    let temperature = climate::generate_temperature(&heightmap, width, height);

    // Run erosion with provided params
    let (_stats, _hardness) = erosion::simulate_erosion(
        &mut heightmap,
        &plate_map,
        &plates,
        &stress_map,
        &temperature,
        params,
        &mut rng,
        seed,
    );

    // Generate moisture with scale (after erosion, as heightmap has changed)
    let moisture = climate::generate_moisture_scaled(&heightmap, width, height, map_scale);

    // Regenerate temperature after erosion for accurate biome classification
    let temperature = climate::generate_temperature(&heightmap, width, height);

    println!("Done! Map size: {}x{} ({} km x {} km)", width, height,
        width as f32 * map_scale.km_per_tile,
        height as f32 * map_scale.km_per_tile);

    PlanetData {
        heightmap,
        plate_map,
        plates,
        stress_map,
        temperature,
        moisture,
        extended_biomes: None,
        upscaled_heightmap: None,
        upscaled_temperature: None,
        upscaled_moisture: None,
        upscaled_stress: None,
        upscale_factor: 1,
    }
}

/// Render globe with full 3D rotation (longitude and latitude tilt)
fn render_globe_3d(
    heightmap: &Tilemap<f32>,
    rotation: f64,
    tilt: f64,
    target_width: usize,
    target_height: usize,
) -> image::RgbImage {
    use image::{ImageBuffer, Rgb, RgbImage};

    // Make the globe fill the window better
    let size = target_width.min(target_height);
    let mut img: RgbImage = ImageBuffer::new(target_width as u32, target_height as u32);

    let radius = size as f64 / 2.0 - 10.0;
    let center_x = target_width as f64 / 2.0;
    let center_y = target_height as f64 / 2.0;

    // Light direction (from upper-right-front)
    let light_dir = normalize_vec3_f64(1.0, 1.0, 0.8);

    // Precompute rotation matrices
    let cos_rot = rotation.cos();
    let sin_rot = rotation.sin();
    let cos_tilt = tilt.cos();
    let sin_tilt = tilt.sin();

    for py in 0..target_height {
        for px in 0..target_width {
            let x = (px as f64 - center_x) / radius;
            let y = (center_y - py as f64) / radius;

            let r_squared = x * x + y * y;
            if r_squared > 1.0 {
                // Background - dark space
                img.put_pixel(px as u32, py as u32, Rgb([5, 5, 15]));
                continue;
            }

            // Z coordinate on sphere surface (pointing towards viewer)
            let z = (1.0 - r_squared).sqrt();

            // Apply inverse rotation to find the point on the original sphere
            // First apply inverse tilt (rotation around X axis)
            let y2 = y * cos_tilt + z * sin_tilt;
            let z2 = -y * sin_tilt + z * cos_tilt;

            // Then apply inverse longitude rotation (rotation around Y axis)
            let x3 = x * cos_rot - z2 * sin_rot;
            let z3 = x * sin_rot + z2 * cos_rot;

            // Convert the rotated point to latitude/longitude
            let lat = y2.asin();  // -PI/2 to PI/2
            let lon = x3.atan2(z3);  // -PI to PI

            // Normalize longitude to 0..2*PI
            let lon = ((lon % (2.0 * PI)) + 2.0 * PI) % (2.0 * PI);

            // Convert to map coordinates
            let map_x = (lon / (2.0 * PI) * heightmap.width as f64) as usize % heightmap.width;
            let map_y = ((0.5 - lat / PI) * heightmap.height as f64)
                .clamp(0.0, heightmap.height as f64 - 1.0) as usize;

            // Get height and color
            let height = *heightmap.get(map_x, map_y);

            // Use terrain colors based on height
            let base_color = terrain_color_for_height(height);

            // Calculate lighting (Lambert shading) using original surface normal
            let normal = (x, y, z);
            let diffuse = (normal.0 * light_dir.0 + normal.1 * light_dir.1 + normal.2 * light_dir.2)
                .max(0.0);

            // Ambient + diffuse lighting
            let ambient = 0.3;
            let light_intensity = ambient + (1.0 - ambient) * diffuse;

            // Apply lighting to color
            let r = ((base_color[0] as f64 * light_intensity).clamp(0.0, 255.0)) as u8;
            let g = ((base_color[1] as f64 * light_intensity).clamp(0.0, 255.0)) as u8;
            let b = ((base_color[2] as f64 * light_intensity).clamp(0.0, 255.0)) as u8;

            img.put_pixel(px as u32, py as u32, Rgb([r, g, b]));
        }
    }

    // Add atmosphere glow
    let glow_radius = radius * 1.15;
    for py in 0..target_height {
        for px in 0..target_width {
            let x = px as f64 - center_x;
            let y = py as f64 - center_y;
            let dist = (x * x + y * y).sqrt();

            if dist > radius && dist < glow_radius {
                let t = (dist - radius) / (glow_radius - radius);
                let glow_strength = (1.0 - t).powi(2) * 0.4;

                let pixel = img.get_pixel(px as u32, py as u32);
                let r = (pixel[0] as f64 + 100.0 * glow_strength).min(255.0) as u8;
                let g = (pixel[1] as f64 + 150.0 * glow_strength).min(255.0) as u8;
                let b = (pixel[2] as f64 + 255.0 * glow_strength).min(255.0) as u8;
                img.put_pixel(px as u32, py as u32, Rgb([r, g, b]));
            }
        }
    }

    img
}

fn normalize_vec3_f64(x: f64, y: f64, z: f64) -> (f64, f64, f64) {
    let len = (x * x + y * y + z * z).sqrt();
    (x / len, y / len, z / len)
}

/// Get terrain color based on height (similar to export.rs biome colors)
fn terrain_color_for_height(height: f32) -> [u8; 3] {
    if height < -500.0 {
        [20, 40, 80]       // Deep ocean
    } else if height < -100.0 {
        [30, 60, 120]      // Ocean
    } else if height < 0.0 {
        [60, 100, 150]     // Shallow water
    } else if height < 10.0 {
        [210, 190, 140]    // Beach
    } else if height < 40.0 {
        [80, 160, 60]      // Lowland
    } else if height < 80.0 {
        [100, 180, 80]     // Plains
    } else if height < 130.0 {
        [40, 120, 50]      // Forest
    } else if height < 200.0 {
        [110, 140, 70]     // Hills
    } else if height < 300.0 {
        [140, 130, 100]    // Highland
    } else if height < 450.0 {
        [120, 110, 100]    // Mountain
    } else {
        [240, 240, 245]    // Snowy peak
    }
}
