//! LEGACY - FROZEN (2026-10-03). Part of the old terminal (ASCII) front end, kept only behind
//! `--legacy-explorer`. Do not add features, fix visuals or port new systems here: the tile
//! viewer (`src/tiles/`) is the only maintained front end. Only change this file if the build
//! breaks; anything still needed from it (e.g. image exporters used by `main`) should be moved
//! out first.
//!
//! Terminal-based world map explorer using ratatui
//!
//! Simple roguelike-style terminal interface for exploring generated worlds.
//! Navigate with arrow keys, inspect tiles, change view modes.

use std::collections::HashMap;
use std::io::{self, stdout};
use std::error::Error;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, MouseEvent, MouseEventKind, MouseButton, EnableMouseCapture, DisableMouseCapture},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Clear},
    style::{Color, Style, Modifier},
};

use crate::ascii::{biome_char, height_color, temperature_color, moisture_color, stress_color};
use crate::heightmap::{LavaState, VolcanoType};
use crate::world::{WorldData, generate_world};
use crate::weather_zones::ExtremeWeatherType;
use crate::region::{RegionMap, RegionCache};
use crate::region::zoom::{generate_zoom, ZoomParams, ZoomRegion};
use crate::map_export::get_family_color;
use crate::underground_water::SpringType;
use crate::history::world_state::WorldHistory;
use crate::history::{FactionId, SettlementId, LegendaryCreatureId};
use crate::history::legends::LegendsMode;
use crate::history::legends::renderer::render_legends;

use image::{ImageBuffer, Rgb};
use crate::map_export::{export_base_map_image, export_freshwater_network_image};

/// Create a darker background color from a foreground color for better contrast.
/// The background is darkened significantly to make the foreground character pop.
fn make_bg_color(r: u8, g: u8, b: u8) -> Color {
    // Darken the color significantly for background (30-40% of original)
    let factor = 0.35;
    let br = (r as f32 * factor) as u8;
    let bg = (g as f32 * factor) as u8;
    let bb = (b as f32 * factor) as u8;
    Color::Rgb(br, bg, bb)
}

/// Generate a distinct RGB color for a faction based on its ID.
/// Uses golden angle hue distribution for maximum visual separation.
fn faction_color(faction_id: FactionId) -> (u8, u8, u8) {
    // Golden angle (~137.5°) distributes hues evenly
    let hue = ((faction_id.0 as f32) * 137.508) % 360.0;
    let s = 0.65;
    let v = 0.85;

    // HSV to RGB conversion
    let h = hue / 60.0;
    let i = h.floor() as u32;
    let f = h - i as f32;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));

    let (r, g, b) = match i % 6 {
        0 => (v, t, p),
        1 => (q, v, p),
        2 => (p, v, t),
        3 => (p, q, v),
        4 => (t, p, v),
        _ => (v, p, q),
    };

    ((r * 255.0) as u8, (g * 255.0) as u8, (b * 255.0) as u8)
}

/// Create a slightly lighter/brighter foreground for better visibility on dark backgrounds.
fn make_fg_color(r: u8, g: u8, b: u8) -> Color {
    // Brighten slightly for foreground to ensure contrast
    let brighten = |c: u8| -> u8 {
        let boosted = c as u16 + 40;
        boosted.min(255) as u8
    };
    Color::Rgb(brighten(r), brighten(g), brighten(b))
}

/// Viewport for rendering a portion of the map
struct Viewport {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

/// View mode for the map display
#[derive(Clone, Copy, PartialEq)]
pub enum ViewMode {
    Biome,
    BaseBiome,
    Height,
    Temperature,
    Moisture,
    Plates,
    Stress,
    Rivers,
    BiomeBlend,
    Coastline,
    WeatherZones,
    Microclimate,
    SeasonalTemp,
    Lava,
    Factions,
    Roads,
}

impl ViewMode {
    fn name(&self) -> &'static str {
        match self {
            ViewMode::Biome => "Biome",
            ViewMode::BaseBiome => "Base",
            ViewMode::Height => "Height",
            ViewMode::Temperature => "Temperature",
            ViewMode::Moisture => "Moisture",
            ViewMode::Plates => "Plates",
            ViewMode::Stress => "Stress",
            ViewMode::Rivers => "Rivers",
            ViewMode::BiomeBlend => "BiomeBlend",
            ViewMode::Coastline => "Coastline",
            ViewMode::WeatherZones => "Weather",
            ViewMode::Microclimate => "Micro",
            ViewMode::SeasonalTemp => "Season",
            ViewMode::Lava => "Lava",
            ViewMode::Factions => "Factions",
            ViewMode::Roads => "Roads",
        }
    }

    fn next(&self) -> ViewMode {
        match self {
            ViewMode::Biome => ViewMode::BaseBiome,
            ViewMode::BaseBiome => ViewMode::Height,
            ViewMode::Height => ViewMode::Temperature,
            ViewMode::Temperature => ViewMode::Moisture,
            ViewMode::Moisture => ViewMode::Plates,
            ViewMode::Plates => ViewMode::Stress,
            ViewMode::Stress => ViewMode::Rivers,
            ViewMode::Rivers => ViewMode::BiomeBlend,
            ViewMode::BiomeBlend => ViewMode::Coastline,
            ViewMode::Coastline => ViewMode::WeatherZones,
            ViewMode::WeatherZones => ViewMode::Microclimate,
            ViewMode::Microclimate => ViewMode::SeasonalTemp,
            ViewMode::SeasonalTemp => ViewMode::Lava,
            ViewMode::Lava => ViewMode::Factions,
            ViewMode::Factions => ViewMode::Roads,
            ViewMode::Roads => ViewMode::Biome,
        }
    }
}

/// Explorer state
struct Explorer {
    world: WorldData,
    /// Optional world history for faction view mode
    history: Option<WorldHistory>,
    cursor_x: usize,
    cursor_y: usize,
    view_mode: ViewMode,
    show_help: bool,
    /// Show the tile info panel on the left
    show_panel: bool,
    /// Zoom level: 1 = normal, 2 = 2x zoom out, 4 = 4x zoom out, etc.
    zoom: usize,
    /// Message to display temporarily
    message: Option<String>,
    /// Show the region map overlay
    show_region_map: bool,
    /// Region cache for seamless multi-region generation
    region_cache: RegionCache,
    /// Last frame render time in milliseconds
    frame_time_ms: f32,
    /// Rolling average frame time in milliseconds
    avg_frame_time: f32,
    /// Legends mode overlay (browsing world history)
    legends_mode: Option<LegendsMode>,
    /// Dirty flag — when false, skip rendering to save CPU
    needs_redraw: bool,
    /// Position-indexed settlement lookup for O(1) per-pixel access
    settlement_positions: HashMap<(usize, usize), SettlementId>,
    /// Position-indexed legendary creature lair lookup for O(1) per-pixel access
    creature_lair_positions: HashMap<(usize, usize), LegendaryCreatureId>,
    /// Show the world minimap in the corner of the map view
    show_minimap: bool,
    /// Per-tile minimap colours, keyed by world seed
    minimap_colors: Option<(u64, Vec<[u8; 3]>)>,
    /// Last generated high-resolution zoom region (kept for reuse)
    zoom_view: Option<ZoomView>,
    /// Whether the zoom view is currently shown instead of the world map
    zoom_active: bool,
    /// A zoom was requested; it is generated after the "generating" frame is drawn
    zoom_pending: bool,
}

impl Explorer {
    fn new(world: WorldData, history: Option<WorldHistory>) -> Self {
        let cursor_x = world.heightmap.width / 2;
        let cursor_y = world.heightmap.height / 2;
        let seed = world.seed();

        // Build position indexes for O(1) lookups during rendering
        let mut settlement_positions = HashMap::new();
        let mut creature_lair_positions = HashMap::new();
        if let Some(ref hist) = history {
            for (id, s) in &hist.settlements {
                if !s.is_destroyed() {
                    settlement_positions.insert(s.location, *id);
                }
            }
            for (id, c) in &hist.legendary_creatures {
                if c.is_alive() {
                    if let Some(loc) = c.lair_location {
                        creature_lair_positions.insert(loc, *id);
                    }
                }
            }
        }

        Explorer {
            world,
            history,
            cursor_x,
            cursor_y,
            view_mode: ViewMode::Biome,
            show_help: false,
            show_panel: true,  // Panel visible by default
            zoom: 1,
            message: None,
            show_region_map: false,
            region_cache: RegionCache::new(seed),
            frame_time_ms: 0.0,
            avg_frame_time: 0.0,
            legends_mode: None,
            needs_redraw: true,
            settlement_positions,
            creature_lair_positions,
            show_minimap: true,
            minimap_colors: None,
            zoom_view: None,
            zoom_active: false,
            zoom_pending: false,
        }
    }

    /// Zoom out (show more of the map)
    fn zoom_out(&mut self) {
        if self.zoom < 16 {
            self.zoom *= 2;
            self.message = Some(format!("Zoom: {}x", self.zoom));
        }
    }

    /// Zoom in (show less of the map, more detail)
    fn zoom_in(&mut self) {
        if self.zoom > 1 {
            self.zoom /= 2;
            self.message = Some(format!("Zoom: {}x", self.zoom));
        }
    }

    /// Fit entire map on screen
    fn fit_to_screen(&mut self, screen_width: usize, screen_height: usize) {
        let map_width = self.world.heightmap.width;
        let map_height = self.world.heightmap.height;

        // Calculate zoom needed to fit
        let zoom_x = (map_width / screen_width).max(1);
        let zoom_y = (map_height / screen_height).max(1);
        self.zoom = zoom_x.max(zoom_y).next_power_of_two();
        self.message = Some(format!("Fit to screen: {}x zoom", self.zoom));
    }

    /// Regenerate the world with a new random seed
    fn regenerate(&mut self) {
        let width = self.world.width;
        let height = self.world.height;
        let new_seed: u64 = rand::random();

        self.message = Some(format!("Generating new world (seed: {})...", new_seed));
        self.world = generate_world(width, height, new_seed);
        self.history = None; // History is invalidated by regeneration

        // Reset cursor to center of map
        self.cursor_x = width / 2;
        self.cursor_y = height / 2;
        self.zoom = 1;

        self.message = Some(format!("New world generated! Seed: {}", self.world.seed()));
    }

    /// Cycle to the next season
    fn next_season(&mut self) {
        self.world.next_season();
        self.message = Some(format!("Season: {}", self.world.current_season.name()));
    }

    /// Cycle to the previous season
    fn prev_season(&mut self) {
        self.world.prev_season();
        self.message = Some(format!("Season: {}", self.world.current_season.name()));
    }

    /// Move cursor with wrapping
    fn move_cursor(&mut self, dx: i32, dy: i32) {
        let width = self.world.heightmap.width;
        let height = self.world.heightmap.height;

        // Horizontal wrapping
        self.cursor_x = ((self.cursor_x as i32 + dx).rem_euclid(width as i32)) as usize;
        // Vertical clamping
        self.cursor_y = (self.cursor_y as i32 + dy).clamp(0, height as i32 - 1) as usize;
    }

    /// Get tile info at cursor
    fn tile_info(&self) -> String {
        let x = self.cursor_x;
        let y = self.cursor_y;

        let height = *self.world.heightmap.get(x, y);
        let temp = *self.world.temperature.get(x, y);
        let moisture = *self.world.moisture.get(x, y);
        let biome = *self.world.biomes.get(x, y);

        format!(
            "({}, {}) | {:?} | {:.0}m | {:.1}C | {:.0}%",
            x, y, biome, height, temp, moisture * 100.0,
        )
    }

    /// Render the map
    fn render_map(&self, area: Rect, buf: &mut Buffer) {
        let width = self.world.heightmap.width;
        let height = self.world.heightmap.height;
        let zoom = self.zoom;

        // Calculate viewport centered on cursor, accounting for zoom
        let view_width = area.width as usize;
        let view_height = area.height as usize;

        // Map area visible = screen size * zoom
        let map_view_width = view_width * zoom;
        let map_view_height = view_height * zoom;

        let start_x = if self.cursor_x >= map_view_width / 2 {
            self.cursor_x - map_view_width / 2
        } else {
            0
        };
        let start_y = if self.cursor_y >= map_view_height / 2 {
            self.cursor_y - map_view_height / 2
        } else {
            0
        };

        for dy in 0..view_height {
            for dx in 0..view_width {
                // Sample from the map at zoom intervals
                let map_x = (start_x + dx * zoom) % width;
                let map_y = (start_y + dy * zoom).min(height - 1);

                if map_y >= height {
                    continue;
                }

                let screen_x = area.x + dx as u16;
                let screen_y = area.y + dy as u16;

                if screen_x >= area.x + area.width || screen_y >= area.y + area.height {
                    continue;
                }

                let (ch, fg, bg) = self.get_tile_display(map_x, map_y);

                // Highlight cursor position (check if cursor is in this cell's range)
                let cursor_in_cell = self.cursor_x >= map_x && self.cursor_x < map_x + zoom
                    && self.cursor_y >= map_y && self.cursor_y < map_y + zoom;
                let style = if cursor_in_cell {
                    Style::default().fg(Color::Black).bg(Color::Yellow)
                } else {
                    Style::default().fg(fg).bg(bg)
                };

                buf[(screen_x, screen_y)].set_char(ch).set_style(style);
            }
        }
    }

    /// Check if a tile is submerged (water body: ocean, lake, or flooded area)
    /// This correctly identifies alpine lakes which have height > 0 but water_depth > 0
    fn is_submerged(&self, x: usize, y: usize) -> bool {
        let height = *self.world.heightmap.get(x, y);
        let water_depth = *self.world.water_depth.get(x, y);
        // Submerged if: below sea level OR has water depth (alpine lakes)
        height < 0.0 || water_depth > 0.5  // 0.5m threshold to avoid float noise
    }

    /// Get display character and colors for a tile
    fn get_tile_display(&self, x: usize, y: usize) -> (char, Color, Color) {
        let biome = *self.world.biomes.get(x, y);
        let height = *self.world.heightmap.get(x, y);
        let temp = *self.world.temperature.get(x, y);
        let moisture = *self.world.moisture.get(x, y);
        let stress = *self.world.stress_map.get(x, y);
        let plate_id = *self.world.plate_map.get(x, y);
        let water_depth = *self.world.water_depth.get(x, y);
        let is_water = height < 0.0 || water_depth > 0.5;

        match self.view_mode {
            ViewMode::Biome => {
                // Check if river using the river network
                let has_river = self.world.river_tile_cache.as_ref()
                    .map_or(false, |c| *c.get(x, y));

                // Use elevation as source of truth for water type
                if height < 0.0 {
                    // OCEAN (below sea level)
                    if has_river && height > -100.0 {
                        // River mouth in shallow coastal water
                        ('≈', Color::Rgb(80, 180, 255), Color::Rgb(20, 60, 120))
                    } else {
                        // Deep ocean
                        let depth_factor = ((-height) / 500.0).min(1.0);
                        let blue = (120.0 + depth_factor * 80.0) as u8;
                        ('~', Color::Rgb(60, 100, blue + 50), Color::Rgb(20, 40, blue))
                    }
                } else {
                    // ABOVE SEA LEVEL (land, lake, or river - never ocean)
                    if has_river {
                        // River on land
                        let (r, g, b) = biome.color();
                        // Use blue foreground for river visibility
                        ('~', Color::Rgb(100, 200, 255), make_bg_color(r, g, b))
                    } else if water_depth > 0.5 {
                        // Lake (alpine lake, etc.)
                        ('~', Color::Rgb(80, 150, 220), Color::Rgb(30, 70, 140))
                    } else {
                        // Land biome
                        let ch = biome_char(&biome);
                        let (r, g, b) = biome.color();
                        (ch, make_fg_color(r, g, b), make_bg_color(r, g, b))
                    }
                }
            }
            ViewMode::BaseBiome => {
                // Show parent/base biome instead of extended biome
                let parent = biome.parent_biome();
                let has_river = self.world.river_tile_cache.as_ref()
                    .map_or(false, |c| *c.get(x, y));
                
                let ch = if is_water || has_river { '~' } else { '.' };
                let (r, g, b) = if has_river && !is_water {
                     // Blue river on land
                     (100, 180, 255)
                } else {
                     parent.color()
                };
                (ch, make_fg_color(r, g, b), make_bg_color(r, g, b))
            }
            ViewMode::Height => {
                let ch = if is_water { '~' } else { '.' };
                let (r, g, b) = height_color(height);
                (ch, make_fg_color(r, g, b), make_bg_color(r, g, b))
            }
            ViewMode::Temperature => {
                let ch = if is_water { '~' } else { '.' };
                let (r, g, b) = temperature_color(temp);
                (ch, make_fg_color(r, g, b), make_bg_color(r, g, b))
            }
            ViewMode::Moisture => {
                let ch = if is_water { '~' } else { '.' };
                let (r, g, b) = moisture_color(moisture);
                (ch, make_fg_color(r, g, b), make_bg_color(r, g, b))
            }
            ViewMode::Plates => {
                let ch = if is_water { '~' } else { '.' };
                let plate_idx = plate_id.0 as usize;
                if plate_idx < self.world.plates.len() {
                    let [r, g, b] = self.world.plates[plate_idx].color;
                    (ch, make_fg_color(r, g, b), make_bg_color(r, g, b))
                } else {
                    (ch, Color::Gray, Color::Rgb(30, 30, 30))
                }
            }
            ViewMode::Stress => {
                let ch = if is_water { '~' } else { '.' };
                let (r, g, b) = stress_color(stress);
                (ch, make_fg_color(r, g, b), make_bg_color(r, g, b))
            }
            ViewMode::Rivers => {
                // Use river network consistently
                let has_river = self.world.river_tile_cache.as_ref()
                    .map_or(false, |c| *c.get(x, y));

                // Use elevation as source of truth
                if height < 0.0 {
                    // OCEAN (below sea level)
                    if height < -100.0 {
                        // Deep ocean - no rivers visible
                        let depth_factor = ((-height) / 500.0).min(1.0);
                        let blue = (100.0 + depth_factor * 100.0) as u8;
                        ('~', Color::Rgb(20, 40, blue), Color::Rgb(10, 20, 40 + (depth_factor * 40.0) as u8))
                    } else if has_river {
                        // River mouth in shallow coastal water
                        ('≈', Color::Rgb(80, 180, 255), Color::Rgb(40, 80, 150))
                    } else {
                        // Shallow coastal water
                        ('~', Color::Rgb(60, 120, 180), Color::Rgb(20, 50, 100))
                    }
                } else {
                    // ABOVE SEA LEVEL (land, lake, river - never ocean)
                    if has_river {
                        // River on land
                        ('~', Color::Rgb(60, 180, 255), Color::Rgb(20, 80, 140))
                    } else if water_depth > 0.5 {
                        // Lake
                        ('~', Color::Rgb(80, 150, 220), Color::Rgb(20, 60, 120))
                    } else {
                        // Land
                        let (r, g, b) = height_color(height);
                        ('.', make_fg_color(r, g, b), make_bg_color(r, g, b))
                    }
                }
            }
            ViewMode::BiomeBlend => {
                if is_water {
                    ('~', Color::Rgb(80, 140, 220), Color::Rgb(20, 40, 80))
                } else {
                    // Check if any neighbor has different biome
                    let mut is_edge = false;
                    for (dx, dy) in [(-1i32, 0), (1, 0), (0, -1), (0, 1)] {
                        let nx = (x as i32 + dx).rem_euclid(self.world.width as i32) as usize;
                        let ny = (y as i32 + dy).clamp(0, self.world.height as i32 - 1) as usize;
                        if *self.world.biomes.get(nx, ny) != biome {
                            is_edge = true;
                            break;
                        }
                    }
                    if is_edge {
                        ('*', Color::Rgb(255, 200, 80), Color::Rgb(100, 60, 20))
                    } else {
                        let (r, g, b) = biome.color();
                        ('.', make_fg_color(r, g, b), make_bg_color(r, g, b))
                    }
                }
            }
            ViewMode::Coastline => {
                if is_water {
                    // Water - check if coastal (near dry land)
                    let is_coastal = self.world.heightmap.neighbors_8(x, y).into_iter().any(|(nx, ny)| {
                        let nh = *self.world.heightmap.get(nx, ny);
                        let nwd = *self.world.water_depth.get(nx, ny);
                        nh >= 0.0 && nwd < 0.5  // Neighbor is dry land
                    });
                    if is_coastal {
                        ('~', Color::Rgb(80, 220, 220), Color::Rgb(0, 70, 70))
                    } else {
                        ('~', Color::Rgb(80, 140, 220), Color::Rgb(20, 40, 80))
                    }
                } else {
                    // Land - check if coastal (near water)
                    let is_coastal = self.world.heightmap.neighbors_8(x, y).into_iter().any(|(nx, ny)| {
                        let nh = *self.world.heightmap.get(nx, ny);
                        let nwd = *self.world.water_depth.get(nx, ny);
                        nh < 0.0 || nwd > 0.5  // Neighbor is water
                    });
                    if is_coastal {
                        ('#', Color::Rgb(255, 255, 140), Color::Rgb(100, 100, 40))
                    } else if height < 50.0 {
                        ('.', Color::Rgb(220, 180, 130), Color::Rgb(80, 55, 35))
                    } else {
                        let (r, g, b) = height_color(height);
                        ('.', make_fg_color(r, g, b), make_bg_color(r, g, b))
                    }
                }
            }
            ViewMode::WeatherZones => {
                if is_water {
                    // Show hurricane risk in water
                    if let Some(ref wz) = self.world.weather_zones {
                        let zone = wz.get(x, y);
                        if zone.has_risk() {
                            let (r, g, b): (u8, u8, u8) = zone.primary.color();
                            let intensity = (zone.risk_factor * 255.0) as u8;
                            ('!', Color::Rgb(r.saturating_add(intensity/2), g, b), make_bg_color(r, g, b))
                        } else {
                            ('~', Color::Rgb(80, 140, 220), Color::Rgb(20, 40, 80))
                        }
                    } else {
                        ('~', Color::Rgb(80, 140, 220), Color::Rgb(20, 40, 80))
                    }
                } else {
                    // Show weather risk on land
                    if let Some(ref wz) = self.world.weather_zones {
                        let zone = wz.get(x, y);
                        if zone.has_risk() {
                            let ch = match zone.primary {
                                ExtremeWeatherType::Monsoon => 'M',
                                ExtremeWeatherType::Blizzard => 'B',
                                ExtremeWeatherType::Tornado => 'T',
                                ExtremeWeatherType::Sandstorm => 'S',
                                _ => '!',
                            };
                            let (r, g, b) = zone.primary.color();
                            (ch, make_fg_color(r, g, b), make_bg_color(r, g, b))
                        } else {
                            let (r, g, b) = height_color(height);
                            ('.', make_fg_color(r, g, b), make_bg_color(r, g, b))
                        }
                    } else {
                        let (r, g, b) = height_color(height);
                        ('.', make_fg_color(r, g, b), make_bg_color(r, g, b))
                    }
                }
            }
            ViewMode::Microclimate => {
                if is_water {
                    ('~', Color::Rgb(80, 140, 220), Color::Rgb(20, 40, 80))
                } else if let Some(ref mc) = self.world.microclimate {
                    let modifiers = mc.get(x, y);
                    // Color based on temperature modifier
                    let temp_mod = modifiers.temperature_mod;
                    if temp_mod > 1.0 {
                        // Valley warmth - orange/red
                        let intensity = ((temp_mod / 3.0).min(1.0) * 200.0) as u8;
                        ('v', Color::Rgb(255, 200 - intensity/2, 100 - intensity/2), Color::Rgb(80, 40, 20))
                    } else if temp_mod < -0.5 {
                        // Ridge cooling - blue
                        let intensity = (((-temp_mod) / 2.0).min(1.0) * 200.0) as u8;
                        ('^', Color::Rgb(150 - intensity/2, 200, 255), Color::Rgb(30, 50, 80))
                    } else if modifiers.moisture_mod > 0.05 {
                        // Lake effect / forest moisture - green
                        ('~', Color::Rgb(100, 200, 150), Color::Rgb(30, 60, 40))
                    } else {
                        let (r, g, b) = height_color(height);
                        ('.', make_fg_color(r, g, b), make_bg_color(r, g, b))
                    }
                } else {
                    let (r, g, b) = height_color(height);
                    ('.', make_fg_color(r, g, b), make_bg_color(r, g, b))
                }
            }
            ViewMode::SeasonalTemp => {
                // Show seasonal temperature (uses current season from world)
                let seasonal_temp = self.world.get_seasonal_temperature(x, y);
                let ch = if is_water { '~' } else { '.' };
                let (r, g, b) = temperature_color(seasonal_temp);
                (ch, make_fg_color(r, g, b), make_bg_color(r, g, b))
            }
            ViewMode::Lava => {
                // Show lava map overlay
                if let Some(ref lava_map) = self.world.lava_map {
                    use crate::heightmap::LavaState;
                    let lava_state = *lava_map.get(x, y);
                    match lava_state {
                        LavaState::Molten => {
                            // Bright orange/yellow for molten lava in crater
                            ('*', Color::Rgb(255, 200, 0), Color::Rgb(255, 100, 0))
                        }
                        LavaState::Flowing => {
                            // Orange/red for flowing lava
                            ('~', Color::Rgb(255, 150, 0), Color::Rgb(200, 50, 0))
                        }
                        LavaState::Cooled => {
                            // Dark gray/black for cooled basalt
                            ('#', Color::Rgb(80, 70, 70), Color::Rgb(40, 35, 35))
                        }
                        LavaState::None => {
                            // No lava - show regular terrain
                            if is_water {
                                ('~', Color::Rgb(80, 140, 220), Color::Rgb(20, 40, 80))
                            } else {
                                let (r, g, b) = height_color(height);
                                ('.', make_fg_color(r, g, b), make_bg_color(r, g, b))
                            }
                        }
                    }
                } else {
                    // No lava map available
                    if is_water {
                        ('~', Color::Rgb(80, 140, 220), Color::Rgb(20, 40, 80))
                    } else {
                        let (r, g, b) = height_color(height);
                        ('.', make_fg_color(r, g, b), make_bg_color(r, g, b))
                    }
                }
            }
            ViewMode::Factions => {
                if let Some(ref history) = self.history {
                    // Check if a faction owns this tile
                    let tile_hist = history.tile_history.get(x, y);

                    // Check for settlement at this tile (O(1) lookup)
                    let settlement = self.settlement_positions.get(&(x, y))
                        .and_then(|id| history.settlements.get(id));

                    // Check for legendary creature lair (O(1) lookup)
                    let lair = self.creature_lair_positions.get(&(x, y))
                        .and_then(|id| history.legendary_creatures.get(id));

                    if is_water {
                        // Water tiles stay water even in faction view
                        ('~', Color::Rgb(60, 100, 160), Color::Rgb(15, 30, 60))
                    } else if let Some(settlement) = settlement {
                        // Settlement marker with faction color
                        let (r, g, b) = faction_color(settlement.faction);
                        let ch = match settlement.settlement_type {
                            crate::history::civilizations::settlement::SettlementType::Capital => '#',
                            crate::history::civilizations::settlement::SettlementType::City => '@',
                            crate::history::civilizations::settlement::SettlementType::Town => 'O',
                            crate::history::civilizations::settlement::SettlementType::Village => 'o',
                            crate::history::civilizations::settlement::SettlementType::Fort => '+',
                            crate::history::civilizations::settlement::SettlementType::Outpost => '.',
                            _ => '*',
                        };
                        (ch, Color::Rgb(255, 255, 255), Color::Rgb(r, g, b))
                    } else if let Some(_lair) = lair {
                        // Legendary creature lair marker
                        ('!', Color::Rgb(255, 80, 80), Color::Rgb(80, 20, 20))
                    } else if let Some(owner) = tile_hist.current_owner {
                        // Territory tile colored by faction
                        let (r, g, b) = faction_color(owner);
                        ('.', make_fg_color(r, g, b), make_bg_color(r, g, b))
                    } else {
                        // Unclaimed wilderness
                        ('.', Color::Rgb(80, 80, 80), Color::Rgb(25, 25, 25))
                    }
                } else {
                    // No history available
                    let (r, g, b) = height_color(height);
                    ('.', make_fg_color(r, g, b), make_bg_color(r, g, b))
                }
            }
            ViewMode::Roads => {
                if let Some(ref history) = self.history {
                    // Check if this tile has a road (permanent infrastructure)
                    let has_road = history.tile_history.has_road(x, y);

                    // Check for settlement at this tile (O(1) lookup)
                    let settlement = self.settlement_positions.get(&(x, y))
                        .and_then(|id| history.settlements.get(id));

                    if is_water {
                        // Water tiles
                        ('~', Color::Rgb(60, 100, 160), Color::Rgb(15, 30, 60))
                    } else if let Some(settlement) = settlement {
                        // Settlement marker (gold for trade hub)
                        let ch = match settlement.settlement_type {
                            crate::history::civilizations::settlement::SettlementType::Capital => '#',
                            crate::history::civilizations::settlement::SettlementType::City => '@',
                            crate::history::civilizations::settlement::SettlementType::Town => 'O',
                            crate::history::civilizations::settlement::SettlementType::Village => 'o',
                            _ => '*',
                        };
                        // Check if this settlement is a trade endpoint
                        let is_trade_hub = history.trade_routes.values()
                            .filter(|r| r.is_active())
                            .any(|r| r.endpoints.0 == settlement.id || r.endpoints.1 == settlement.id);
                        if is_trade_hub {
                            (ch, Color::Rgb(255, 215, 0), Color::Rgb(120, 90, 0)) // Gold
                        } else {
                            (ch, Color::Rgb(200, 200, 200), Color::Rgb(60, 60, 60))
                        }
                    } else if has_road {
                        // Trade route path - golden road
                        ('·', Color::Rgb(255, 200, 50), Color::Rgb(100, 70, 20))
                    } else {
                        // Regular terrain (muted)
                        let (r, g, b) = height_color(height);
                        // Desaturate for background
                        let gray = ((r as u16 + g as u16 + b as u16) / 3) as u8;
                        ('.', Color::Rgb(gray, gray, gray), Color::Rgb(gray / 4, gray / 4, gray / 4))
                    }
                } else {
                    // No history available
                    let (r, g, b) = height_color(height);
                    ('.', make_fg_color(r, g, b), make_bg_color(r, g, b))
                }
            }
        }
    }

    /// Render help overlay
    fn render_help(&self, area: Rect, buf: &mut Buffer) {
        let help_text = vec![
            "=== World Map Explorer ===",
            "",
            "Navigation:",
            "  Arrow keys / WASD / HJKL - Move cursor",
            "  PgUp/PgDn - Fast vertical movement",
            "  Home/End - Fast horizontal movement",
            "",
            "View Modes (V to cycle):",
            "  Biome, Base, Height, Temperature,",
            "  Moisture, Plates, Stress, Rivers,",
            "  BiomeBlend, Coastline, Weather,",
            "  Micro, Season, Lava, Factions, Roads",
            "",
            "Season (for Season view):",
            "  [ / ] - Previous/Next season",
            "",
            "Zoom:",
            "  +/- - Zoom in/out",
            "  F - Fit map to screen",
            "",
            "Other:",
            "  I / Tab - Toggle info panel",
            "  M - Toggle region map panel",
            "  N - Toggle minimap",
            "  Z - Zoom: simulate the region around the cursor",
            "      (in zoom: arrows pan, +/- scale, F fit,",
            "       X save PNG, Esc/Z back)",
            "  L - Legends mode (browse history)",
            "  R - Regenerate world (new seed)",
            "  E - Export current view as PNG",
            "  T - Export top-down view as PNG",
            "  w - Export water network (rivers+lakes)",
            "  W - Export freshwater only (no ocean)",
            "  ? - Toggle this help",
            "  Q / Esc - Quit",
            "",
            "Press any key to close",
        ];

        let width = 50;
        let height = help_text.len() as u16 + 2;
        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y + (area.height.saturating_sub(height)) / 2;

        let help_area = Rect::new(x, y, width, height);

        // Clear background
        Clear.render(help_area, buf);

        let block = Block::default()
            .title(" Help ")
            .borders(Borders::ALL)
            .style(Style::default().bg(Color::DarkGray));

        let inner = block.inner(help_area);
        block.render(help_area, buf);

        for (i, line) in help_text.iter().enumerate() {
            if i as u16 >= inner.height {
                break;
            }
            buf.set_string(inner.x, inner.y + i as u16, line, Style::default().fg(Color::White));
        }
    }

    /// Render the tile information panel on the left side
    fn render_tile_panel(&self, area: Rect, buf: &mut Buffer) {
        use crate::water_bodies::WaterBodyType;

        let x = self.cursor_x;
        let y = self.cursor_y;

        // Get tile data
        let tile = self.world.get_tile_info(x, y);
        let biome = *self.world.biomes.get(x, y);
        let plate_id = *self.world.plate_map.get(x, y);

        // Get plate info
        let plate = self.world.plates.iter().find(|p| p.id == plate_id);
        let plate_type_str = plate.map(|p| format!("{:?}", p.plate_type)).unwrap_or_else(|| "Unknown".to_string());

        // Calculate latitude (0 = equator, ±90 = poles)
        let lat_normalized = (y as f32 / self.world.height as f32 - 0.5) * 2.0;
        let latitude = lat_normalized * 90.0;
        let lat_dir = if latitude >= 0.0 { "S" } else { "N" };

        // Calculate longitude (0-360 wrapping)
        let longitude = (x as f32 / self.world.width as f32) * 360.0;

        // Get seasonal temperature if available
        let seasonal_temp = self.world.get_seasonal_temperature(x, y);

        // Get weather zone info
        let weather_str = if let Some(zone) = self.world.get_weather_zone(x, y) {
            if zone.has_risk() {
                format!("{} ({:.0}%)", zone.primary.display_name(), zone.risk_factor * 100.0)
            } else {
                "None".to_string()
            }
        } else {
            "N/A".to_string()
        };

        // Get microclimate info
        let micro_str = if let Some(ref micro_map) = self.world.microclimate {
            let m = micro_map.get(x, y);
            format!("{:+.1}°C", m.temperature_mod)
        } else {
            "N/A".to_string()
        };

        // Get water body info
        let water_str = match tile.water_body_type {
            WaterBodyType::None => "None".to_string(),
            WaterBodyType::Ocean => "Ocean".to_string(),
            WaterBodyType::Lake => format!("Lake ({} tiles)", tile.water_body_size.unwrap_or(0)),
            WaterBodyType::River => "River".to_string(),
        };

        // Build the panel content
        let mut lines: Vec<(String, Style)> = vec![];

        // Header with biome color
        let (br, bg, bb) = biome.color();
        let biome_style = Style::default().fg(Color::Rgb(br, bg, bb)).add_modifier(Modifier::BOLD);
        lines.push((format!(" {}", biome.display_name()), biome_style));

        // Show parent biome if this is a special biome
        if biome.is_special() {
            let parent = biome.parent_biome();
            let (pr, pg, pb) = parent.color();
            let parent_style = Style::default().fg(Color::Rgb(pr, pg, pb));
            lines.push((format!("  Base: {:?}", parent), parent_style));
        }
        lines.push(("".to_string(), Style::default()));

        // Location section
        lines.push((" Location".to_string(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        lines.push((format!("  Position: ({}, {})", x, y), Style::default().fg(Color::White)));
        lines.push((format!("  Lat: {:.1}°{}", latitude.abs(), lat_dir), Style::default().fg(Color::White)));
        lines.push((format!("  Lon: {:.1}°", longitude), Style::default().fg(Color::White)));
        lines.push(("".to_string(), Style::default()));

        // Terrain section
        lines.push((" Terrain".to_string(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        // Always show terrain elevation relative to sea level
        let elev_str = format!("  Elevation: {:.0}m", tile.elevation);
        lines.push((elev_str, Style::default().fg(Color::White)));
        // Show water depth if tile is submerged (water_depth > 0)
        if tile.water_depth > 0.0 {
            let depth_str = format!("  Water Depth: {:.0}m", tile.water_depth);
            lines.push((depth_str, Style::default().fg(Color::Cyan)));
        }
        lines.push((format!("  Stress: {:.2}", tile.stress), Style::default().fg(Color::White)));
        if let Some(h) = tile.hardness {
            lines.push((format!("  Hardness: {:.2}", h), Style::default().fg(Color::White)));
        }
        lines.push(("".to_string(), Style::default()));

        // Climate section
        lines.push((" Climate".to_string(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        lines.push((format!("  Temperature: {:.1}°C", tile.temperature), Style::default().fg(Color::White)));
        lines.push((format!("  Seasonal: {:.1}°C", seasonal_temp), Style::default().fg(Color::Gray)));
        lines.push((format!("  Moisture: {:.0}%", tile.moisture * 100.0), Style::default().fg(Color::White)));
        lines.push((format!("  Microclimate: {}", micro_str), Style::default().fg(Color::Gray)));
        lines.push((format!("  Weather Risk: {}", weather_str), Style::default().fg(Color::White)));
        lines.push(("".to_string(), Style::default()));

        // Geology section
        lines.push((" Geology".to_string(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        lines.push((format!("  Plate ID: {}", plate_id.0), Style::default().fg(Color::White)));
        lines.push((format!("  Plate Type: {}", plate_type_str), Style::default().fg(Color::White)));

        // Check if this tile is a volcano
        let volcano = self.world.volcanoes.iter().find(|v| v.x == x && v.y == y);
        if let Some(v) = volcano {
            let type_str = match v.volcano_type {
                VolcanoType::Shield => "Shield",
                VolcanoType::Stratovolcano => "Stratovolcano",
                VolcanoType::Caldera => "Caldera",
            };
            let active_str = if v.is_active { " (Active)" } else { " (Dormant)" };
            lines.push((format!("  Volcano: {}{}", type_str, active_str), Style::default().fg(Color::Red)));
            lines.push((format!("    Peak: +{:.0}m", v.peak_height), Style::default().fg(Color::Gray)));
        }

        // Lava state
        if let Some(ref lava_map) = self.world.lava_map {
            let lava_state = *lava_map.get(x, y);
            match lava_state {
                LavaState::Molten => {
                    lines.push(("  Lava: Molten".to_string(), Style::default().fg(Color::Rgb(255, 150, 0))));
                }
                LavaState::Flowing => {
                    lines.push(("  Lava: Flowing".to_string(), Style::default().fg(Color::Rgb(255, 100, 0))));
                }
                LavaState::Cooled => {
                    lines.push(("  Lava: Cooled (Basalt)".to_string(), Style::default().fg(Color::Rgb(100, 80, 80))));
                }
                LavaState::None => {}
            }
        }
        lines.push(("".to_string(), Style::default()));

        // Water section
        lines.push((" Water".to_string(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        lines.push((format!("  Body: {}", water_str), Style::default().fg(Color::White)));

        // Flow accumulation (drainage area)
        if let Some(ref flow_acc) = self.world.flow_accumulation {
            let flow = *flow_acc.get(x, y);
            if flow > 10.0 {
                let river_str = if flow > 1000.0 {
                    format!("  Flow: {:.0} (Major River)", flow)
                } else if flow > 100.0 {
                    format!("  Flow: {:.0} (River)", flow)
                } else if flow > 50.0 {
                    format!("  Flow: {:.0} (Stream)", flow)
                } else {
                    format!("  Flow: {:.0} (Drainage)", flow)
                };
                lines.push((river_str, Style::default().fg(Color::Cyan)));
            }
        }

        // Underground water features
        if tile.water_features.has_any() {
            if tile.water_features.aquifer.is_present() {
                let aq = &tile.water_features.aquifer;
                lines.push((format!("  Aquifer: {}", aq.aquifer_type.display_name()), Style::default().fg(Color::Cyan)));
                lines.push((format!("    Depth: {:.0}m", aq.depth), Style::default().fg(Color::Gray)));
            }
            if tile.water_features.spring.is_present() {
                let sp = &tile.water_features.spring;
                let temp_str = if sp.temperature_mod > 0.0 {
                    format!(" (+{:.0}°C)", sp.temperature_mod)
                } else {
                    String::new()
                };
                lines.push((format!("  Spring: {}{}", sp.spring_type.display_name(), temp_str), Style::default().fg(Color::Blue)));
            }
            if tile.water_features.waterfall.is_present {
                let wf = &tile.water_features.waterfall;
                lines.push((format!("  Waterfall: {:.0}m drop", wf.drop_height), Style::default().fg(Color::LightBlue)));
            }
        }
        lines.push(("".to_string(), Style::default()));

        // Season indicator
        lines.push((" Season".to_string(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)));
        lines.push((format!("  {}", self.world.current_season.name()), Style::default().fg(Color::Cyan)));

        // History / Faction info
        if let Some(ref history) = self.history {
            lines.push(("".to_string(), Style::default()));
            lines.push((" History".to_string(), Style::default().fg(Color::Magenta).add_modifier(Modifier::BOLD)));

            let tile_hist = history.tile_history.get(x, y);
            if let Some(owner_id) = tile_hist.current_owner {
                if let Some(faction) = history.factions.get(&owner_id) {
                    let (fr, fg, fb) = faction_color(owner_id);
                    lines.push((format!("  Faction: {}", faction.name), Style::default().fg(Color::Rgb(fr, fg, fb))));
                    lines.push((format!("  Pop: {}", faction.total_population), Style::default().fg(Color::White)));
                }
            } else {
                lines.push(("  Unclaimed".to_string(), Style::default().fg(Color::DarkGray)));
            }

            // Settlement at cursor (O(1) lookup)
            if let Some(settlement) = self.settlement_positions.get(&(x, y))
                .and_then(|id| history.settlements.get(id))
            {
                lines.push((format!("  {:?}: {}", settlement.settlement_type, settlement.name), Style::default().fg(Color::Cyan)));
                lines.push((format!("    Pop: {}", settlement.population), Style::default().fg(Color::White)));
            }

            // Legendary creature lair (O(1) lookup)
            if let Some(creature) = self.creature_lair_positions.get(&(x, y))
                .and_then(|id| history.legendary_creatures.get(id))
            {
                lines.push((format!("  Lair: {}", creature.name), Style::default().fg(Color::Red)));
            }

            // Tile event count
            let event_count = tile_hist.events.len();
            if event_count > 0 {
                lines.push((format!("  Events: {}", event_count), Style::default().fg(Color::Gray)));
            }
        }

        // Draw the panel background and border
        let block = Block::default()
            .title(" Tile Info ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::DarkGray))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(area);
        block.render(area, buf);

        // Render each line
        for (i, (line, style)) in lines.iter().enumerate() {
            if i as u16 >= inner.height {
                break;
            }
            // Truncate line if too long
            let display_line: String = line.chars().take(inner.width as usize).collect();
            buf.set_string(inner.x, inner.y + i as u16, &display_line, *style);
        }
    }

    /// Ensure the region map is cached for current cursor position
    /// Uses RegionCache which generates this region and its neighbors together
    /// for seamless stitching across tile boundaries
    fn ensure_region_cached(&mut self) {
        // The cache handles checking if the region exists and generating neighbors
        // We just trigger it by calling get_region
        self.region_cache.get_region(&self.world, self.cursor_x, self.cursor_y);
    }

    /// Render the region map as a panel
    fn render_region_panel(&mut self, area: Rect, buf: &mut Buffer) {
        // Get region from cache (generates if needed with neighbors for seamless stitching)
        let region = self.region_cache.get_region(&self.world, self.cursor_x, self.cursor_y);

        // Draw border block
        let biome = self.world.biomes.get(self.cursor_x, self.cursor_y);
        let title = format!(" Region - {:?} ", biome);
        let block = Block::default()
            .title(title.as_str())
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(area);
        block.render(area, buf);

        // Display dimensions - use full panel area
        let display_width = inner.width.saturating_sub(0) as usize;
        let display_height = inner.height.saturating_sub(2) as usize; // Leave room for info line

        let scale_x = region.size as f32 / display_width as f32;
        let scale_y = region.size as f32 / display_height as f32;

        // Get biome colors for base terrain
        let base_biome = *self.world.biomes.get(self.cursor_x, self.cursor_y);
        let (biome_r, biome_g, biome_b) = base_biome.color();

        for dy in 0..display_height {
            for dx in 0..display_width {
                let rx = ((dx as f32 * scale_x) as usize).min(region.size - 1);
                let ry = ((dy as f32 * scale_y) as usize).min(region.size - 1);

                let height = region.get_height(rx, ry);
                let h_norm = region.get_height_normalized(rx, ry);
                let river = region.get_river(rx, ry);
                let vegetation = region.get_vegetation(rx, ry);
                let rocks = region.get_rocks(rx, ry);
                let slope = region.get_slope(rx, ry);
                let spring = region.get_spring(rx, ry);
                let waterfall = region.get_waterfall(rx, ry);

                // Determine character and color based on terrain features
                // Priority: waterfall > spring > river > other terrain
                let (ch, fg, bg) = if waterfall.is_present {
                    // Waterfall - dramatic cascading water
                    let intensity = (waterfall.drop_height / 50.0).clamp(0.5, 1.0);
                    ('▼', Color::Rgb(180, (200.0 + intensity * 55.0) as u8, 255),
                     Color::Rgb(40, (80.0 + intensity * 40.0) as u8, 140))
                } else if spring.spring_type.is_present() {
                    // Spring - water emerging from ground
                    use crate::underground_water::SpringType;
                    match spring.spring_type {
                        SpringType::Thermal => {
                            // Hot spring - warm colors
                            ('◎', Color::Rgb(255, 180, 100), Color::Rgb(120, 60, 30))
                        }
                        SpringType::Artesian => {
                            // Pressurized spring - bright blue
                            ('◉', Color::Rgb(100, 200, 255), Color::Rgb(30, 80, 120))
                        }
                        SpringType::Karst => {
                            // Cave spring - darker blue-green
                            ('○', Color::Rgb(80, 180, 200), Color::Rgb(20, 60, 80))
                        }
                        _ => {
                            // Seepage spring - gentle blue
                            ('●', Color::Rgb(120, 180, 220), Color::Rgb(30, 60, 90))
                        }
                    }
                } else if river > 0.5 {
                    // Strong river/water
                    let depth = (river * 0.5 + 0.5).min(1.0);
                    ('≈', Color::Rgb(80, (140.0 + depth * 60.0) as u8, 255),
                     Color::Rgb(15, 35, (80.0 + depth * 40.0) as u8))
                } else if river > 0.2 {
                    // Stream/shallow water
                    ('~', Color::Rgb(100, 180, 240), Color::Rgb(20, 50, 100))
                } else if height < -50.0 {
                    // Deep water
                    ('≋', Color::Rgb(40, 80, 180), Color::Rgb(10, 20, 60))
                } else if height < 0.0 {
                    // Shallow water
                    ('~', Color::Rgb(60, 120, 200), Color::Rgb(15, 35, 80))
                } else if rocks > 0.5 {
                    // Rocky terrain
                    let shade = (h_norm * 60.0) as u8;
                    ('▲', Color::Rgb(140 + shade, 130 + shade, 120 + shade),
                     Color::Rgb(60 + shade/2, 55 + shade/2, 50 + shade/2))
                } else if rocks > 0.3 {
                    // Some rocks
                    let shade = (h_norm * 50.0) as u8;
                    ('∆', Color::Rgb(130 + shade, 125 + shade, 115 + shade),
                     Color::Rgb(50 + shade/2, 48 + shade/2, 45 + shade/2))
                } else if vegetation > 0.7 {
                    // Dense forest - use biome color with tree character
                    let shade = 1.0 + h_norm * 0.2 - (1.0 - vegetation) * 0.1;
                    let r = ((biome_r as f32 * shade * 0.9) as u8).min(255);
                    let g = ((biome_g as f32 * shade * 1.1) as u8).min(255);
                    let b = ((biome_b as f32 * shade * 0.8) as u8).min(255);
                    ('♣', Color::Rgb(r, g, b), Color::Rgb(r/3, g/3, b/3))
                } else if vegetation > 0.5 {
                    // Medium forest
                    let shade = 1.0 + h_norm * 0.15;
                    let r = ((biome_r as f32 * shade * 0.95) as u8).min(255);
                    let g = ((biome_g as f32 * shade) as u8).min(255);
                    let b = ((biome_b as f32 * shade * 0.85) as u8).min(255);
                    ('↟', Color::Rgb(r, g, b), Color::Rgb(r/3, g/3, b/3))
                } else if vegetation > 0.3 {
                    // Light vegetation/shrubs
                    let shade = 1.0 + h_norm * 0.1;
                    let r = ((biome_r as f32 * shade) as u8).min(255);
                    let g = ((biome_g as f32 * shade * 0.95) as u8).min(255);
                    let b = ((biome_b as f32 * shade * 0.9) as u8).min(255);
                    ('*', Color::Rgb(r, g, b), Color::Rgb(r/3, g/3, b/3))
                } else if slope > 15.0 {
                    // Steep bare slope
                    let shade = (h_norm * 80.0) as u8;
                    ('/', Color::Rgb(150 + shade/2, 140 + shade/2, 130 + shade/2),
                     Color::Rgb(60 + shade/3, 55 + shade/3, 50 + shade/3))
                } else {
                    // Open terrain - use biome color with height shading
                    let shade = 0.8 + h_norm * 0.4;
                    let r = ((biome_r as f32 * shade) as u8).min(255);
                    let g = ((biome_g as f32 * shade) as u8).min(255);
                    let b = ((biome_b as f32 * shade) as u8).min(255);

                    // Choose character based on vegetation hint
                    let ch = if vegetation > 0.15 {
                        '.'
                    } else if vegetation > 0.05 {
                        ','
                    } else {
                        ' '
                    };
                    (ch, Color::Rgb(r, g, b), Color::Rgb(r/3, g/3, b/3))
                };

                let style = Style::default().fg(fg).bg(bg);
                buf[(inner.x + dx as u16, inner.y + dy as u16)]
                    .set_char(ch)
                    .set_style(style);
            }
        }

        // Info line at bottom (compact for panel)
        let info_y = inner.y + display_height as u16;
        if info_y < inner.y + inner.height {
            let info = format!(
                "{:.0}m-{:.0}m ≈River ●Spring ▼Fall ♣Forest",
                region.height_min, region.height_max
            );
            // Truncate to fit panel width
            let info_truncated: String = info.chars().take(inner.width as usize).collect();
            buf.set_string(inner.x, info_y, &info_truncated, Style::default().fg(Color::DarkGray));
        }
    }
}

/// Convert HSV to RGB

/// Full-screen view of a re-simulated high-resolution region (see `region::zoom`).
struct ZoomView {
    region: ZoomRegion,
    rgb: Vec<[u8; 3]>,
    /// World seed and centre tile it was generated for (reused when unchanged).
    world_seed: u64,
    tile: (usize, usize),
    /// View centre in region cells.
    center_x: f32,
    center_y: f32,
    /// Region cells per screen pixel (each character cell shows 1x2 pixels).
    scale: f32,
}

impl ZoomView {
    /// Scale and centre so the whole region fits in `area`.
    fn fit(&mut self, area: Rect) {
        let px_w = area.width.max(1) as f32;
        let px_h = area.height.max(1) as f32 * 2.0;
        self.scale = (self.region.width as f32 / px_w).max(self.region.height as f32 / px_h);
        self.center_x = self.region.width as f32 / 2.0;
        self.center_y = self.region.height as f32 / 2.0;
    }

    fn pan(&mut self, dx_px: f32, dy_px: f32) {
        self.center_x = (self.center_x + dx_px * self.scale).clamp(0.0, self.region.width as f32);
        self.center_y = (self.center_y + dy_px * self.scale).clamp(0.0, self.region.height as f32);
    }

    fn rescale(&mut self, factor: f32) {
        self.scale = (self.scale * factor).clamp(0.125, 16.0);
    }

    /// Colour of the screen pixel centred on region position (x, y). When zoomed out, a few
    /// stratified samples are averaged so thin rivers don't flicker in and out.
    fn sample(&self, x: f32, y: f32) -> Option<[u8; 3]> {
        let (w, h) = (self.region.width, self.region.height);
        if x < 0.0 || y < 0.0 || x >= w as f32 || y >= h as f32 {
            return None;
        }
        let n = (self.scale.ceil() as usize).clamp(1, 3);
        let mut acc = [0u32; 3];
        for sy in 0..n {
            for sx in 0..n {
                let fx = x + ((sx as f32 + 0.5) / n as f32 - 0.5) * self.scale.max(1.0);
                let fy = y + ((sy as f32 + 0.5) / n as f32 - 0.5) * self.scale.max(1.0);
                let ix = (fx.max(0.0) as usize).min(w - 1);
                let iy = (fy.max(0.0) as usize).min(h - 1);
                let c = self.rgb[iy * w + ix];
                for k in 0..3 { acc[k] += c[k] as u32; }
            }
        }
        let m = (n * n) as u32;
        Some([(acc[0] / m) as u8, (acc[1] / m) as u8, (acc[2] / m) as u8])
    }
}

impl Explorer {
    /// Top-left world tile of the map viewport (same rule as `render_map`).
    fn viewport_origin(&self, area: Rect) -> (usize, usize) {
        let vw = area.width as usize * self.zoom;
        let vh = area.height as usize * self.zoom;
        (self.cursor_x.saturating_sub(vw / 2), self.cursor_y.saturating_sub(vh / 2))
    }

    fn ensure_minimap_colors(&mut self) {
        let seed = self.world.seed();
        if self.minimap_colors.as_ref().map(|(s, _)| *s == seed).unwrap_or(false) {
            return;
        }
        let (w, h) = (self.world.width, self.world.height);
        let hm = &self.world.heightmap;
        let mut colors = Vec::with_capacity(w * h);
        for y in 0..h {
            for x in 0..w {
                let e = *hm.get(x, y);
                let c = if e <= 0.0 {
                    let t = (-e / 5000.0).clamp(0.0, 1.0).sqrt();
                    [(70.0 - 60.0 * t) as u8, (140.0 - 110.0 * t) as u8, (200.0 - 115.0 * t) as u8]
                } else {
                    let (r, g, b) = get_family_color(*self.world.biomes.get(x, y));
                    // Light from the west: brighten west-facing slopes, darken east-facing.
                    let dx = *hm.get((x + w - 1) % w, y) - *hm.get((x + 1) % w, y);
                    let shade = (1.0 + dx / 2500.0).clamp(0.7, 1.3);
                    let f = |v: u8| (v as f32 * shade).min(255.0) as u8;
                    [f(r), f(g), f(b)]
                };
                colors.push(c);
            }
        }
        self.minimap_colors = Some((seed, colors));
    }

    /// World minimap in the bottom-right corner of `area`, with the visible viewport outlined
    /// and the zoom window around the cursor marked.
    fn render_minimap(&mut self, area: Rect, buf: &mut Buffer) {
        let (ww, wh) = (self.world.width, self.world.height);
        let px_w = ((area.width as usize).saturating_sub(4) / 3).min(72);
        if px_w < 16 {
            return;
        }
        let mut px_h = (px_w * wh / ww).max(2);
        px_h += px_h % 2;
        let ch_h = px_h / 2;
        if ch_h + 2 > area.height as usize / 2 {
            return;
        }
        self.ensure_minimap_colors();
        let colors = &self.minimap_colors.as_ref().unwrap().1;

        // Average world tiles into minimap pixels.
        let mut px = vec![[0u8; 3]; px_w * px_h];
        for py in 0..px_h {
            let (y0, y1) = (py * wh / px_h, ((py + 1) * wh / px_h).max(py * wh / px_h + 1));
            for pxx in 0..px_w {
                let (x0, x1) = (pxx * ww / px_w, ((pxx + 1) * ww / px_w).max(pxx * ww / px_w + 1));
                let mut acc = [0u32; 3];
                let mut cnt = 0u32;
                for y in y0..y1.min(wh) {
                    for x in x0..x1.min(ww) {
                        let c = colors[y * ww + x];
                        for k in 0..3 { acc[k] += c[k] as u32; }
                        cnt += 1;
                    }
                }
                let cnt = cnt.max(1);
                px[py * px_w + pxx] = [(acc[0] / cnt) as u8, (acc[1] / cnt) as u8, (acc[2] / cnt) as u8];
            }
        }

        // Overlays: viewport outline (yellow) and zoom window (white) around the cursor.
        let to_px = |x: i64, y: i64| -> (usize, usize) {
            let xr = x.rem_euclid(ww as i64) as usize;
            let yr = y.clamp(0, wh as i64 - 1) as usize;
            (xr * px_w / ww, (yr * px_h / wh).min(px_h - 1))
        };
        let outline = |x0: i64, y0: i64, x1: i64, y1: i64, color: [u8; 3], px: &mut Vec<[u8; 3]>| {
            let (ax, ay) = to_px(x0, y0);
            let (bx, by) = to_px(x1, y1);
            let span_x = if bx >= ax { bx - ax } else { bx + px_w - ax };
            for i in 0..=span_x {
                let xx = (ax + i) % px_w;
                px[ay * px_w + xx] = color;
                px[by * px_w + xx] = color;
            }
            for yy in ay.min(by)..=ay.max(by) {
                px[yy * px_w + ax] = color;
                px[yy * px_w + bx] = color;
            }
        };
        let (sx, sy) = self.viewport_origin(area);
        // Clamp to the world so a view wider than the map doesn't wrap the frame onto itself.
        let vw = (area.width as i64 * self.zoom as i64).min(ww as i64);
        let vh = (area.height as i64 * self.zoom as i64).min(wh as i64);
        if (vw as usize) < ww || (vh as usize) < wh {
            outline(sx as i64, sy as i64, sx as i64 + vw - 1, sy as i64 + vh - 1, [240, 210, 60], &mut px);
        }
        let half = (ZoomParams::default().tiles / 2) as i64;
        let (cx, cy) = (self.cursor_x as i64, self.cursor_y as i64);
        outline(cx - half, cy - half, cx + half - 1, cy + half - 1, [255, 255, 255], &mut px);

        // Frame and half-block pixels (upper half = fg, lower half = bg).
        let box_w = px_w as u16 + 2;
        let box_h = ch_h as u16 + 2;
        let rect = Rect::new(area.x + area.width - box_w, area.y + area.height - box_h, box_w, box_h);
        Clear.render(rect, buf);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" Map (N) ")
            .border_style(Style::default().fg(Color::Gray));
        let inner = block.inner(rect);
        block.render(rect, buf);
        for row in 0..ch_h {
            for col in 0..px_w {
                let top = px[(2 * row) * px_w + col];
                let bot = px[(2 * row + 1) * px_w + col];
                buf[(inner.x + col as u16, inner.y + row as u16)]
                    .set_char('▀')
                    .set_fg(Color::Rgb(top[0], top[1], top[2]))
                    .set_bg(Color::Rgb(bot[0], bot[1], bot[2]));
            }
        }
    }

    /// Generate (or reuse) the zoom region around the cursor and switch to the zoom view.
    fn run_pending_zoom(&mut self, content: Rect) {
        self.zoom_pending = false;
        let seed = self.world.seed();
        let tile = (self.cursor_x, self.cursor_y);
        let reuse = matches!(&self.zoom_view, Some(z) if z.world_seed == seed && z.tile == tile);
        if !reuse {
            let t0 = std::time::Instant::now();
            let params = ZoomParams { center_x: tile.0, center_y: tile.1, seed, ..Default::default() };
            let region = generate_zoom(&self.world, &params);
            let rgb = region.render_rgb();
            let mut view = ZoomView { region, rgb, world_seed: seed, tile, center_x: 0.0, center_y: 0.0, scale: 1.0 };
            view.fit(content);
            self.zoom_view = Some(view);
            self.message = Some(format!("Region simulated in {:.1}s", t0.elapsed().as_secs_f32()));
        }
        self.zoom_active = true;
        self.needs_redraw = true;
    }

    fn render_zoom_view(&self, area: Rect, buf: &mut Buffer) {
        let Some(z) = self.zoom_view.as_ref() else { return };
        let half_w = area.width as f32 / 2.0;
        let half_h = area.height as f32; // in pixels: 2 per row
        for row in 0..area.height {
            for col in 0..area.width {
                let rx = z.center_x + (col as f32 + 0.5 - half_w) * z.scale;
                let ry_top = z.center_y + (2.0 * row as f32 + 0.5 - half_h) * z.scale;
                let ry_bot = z.center_y + (2.0 * row as f32 + 1.5 - half_h) * z.scale;
                let top = z.sample(rx, ry_top).unwrap_or([12, 12, 16]);
                let bot = z.sample(rx, ry_bot).unwrap_or([12, 12, 16]);
                buf[(area.x + col, area.y + row)]
                    .set_char('▀')
                    .set_fg(Color::Rgb(top[0], top[1], top[2]))
                    .set_bg(Color::Rgb(bot[0], bot[1], bot[2]));
            }
        }
        // Crosshair at the probe point.
        let (cx, cy) = (area.x + area.width / 2, area.y + area.height / 2);
        buf[(cx, cy)].set_char('+').set_fg(Color::Rgb(255, 60, 60));
    }

    fn zoom_status(&self) -> String {
        let Some(z) = self.zoom_view.as_ref() else { return String::new() };
        let r = &z.region;
        let (ix, iy) = (
            (z.center_x as usize).min(r.width - 1),
            (z.center_y as usize).min(r.height - 1),
        );
        let k = iy * r.width + ix;
        let e = r.elevation_m[k];
        let water = if e <= 0.0 {
            format!("sea {:.0} m deep", -e)
        } else if r.lake_depth_m[k] > 0.0 {
            format!("lake {:.0} m deep", r.lake_depth_m[k])
        } else if r.river_width_m[k] > 0.0 {
            format!("river ~{:.0} m wide", r.river_width_m[k])
        } else {
            "dry land".to_string()
        };
        let km = r.cell_m / 1000.0;
        let msg = self.message.as_ref().map(|m| format!(" | {m}")).unwrap_or_default();
        format!(
            " ZOOM tile ({},{}) | {}x{} @ {:.0} m | pos {:.1},{:.1} km | {:.0} m, {} | {:.1}°C, moist {:.2} | 1px={:.2} cells | arrows:pan +/-:scale F:fit X:PNG Esc:back{}",
            z.tile.0, z.tile.1, r.width, r.height, r.cell_m,
            ix as f32 * km, iy as f32 * km, e, water,
            r.temperature_c[k], r.moisture[k], z.scale, msg,
        )
    }
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> (u8, u8, u8) {
    let c = v * s;
    let x = c * (1.0 - ((h / 60.0) % 2.0 - 1.0).abs());
    let m = v - c;

    let (r, g, b) = if h < 60.0 {
        (c, x, 0.0)
    } else if h < 120.0 {
        (x, c, 0.0)
    } else if h < 180.0 {
        (0.0, c, x)
    } else if h < 240.0 {
        (0.0, x, c)
    } else if h < 300.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };

    (
        ((r + m) * 255.0) as u8,
        ((g + m) * 255.0) as u8,
        ((b + m) * 255.0) as u8,
    )
}

/// Convert ratatui Color to RGB values
fn color_to_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::Red => (255, 0, 0),
        Color::Green => (0, 255, 0),
        Color::Yellow => (255, 255, 0),
        Color::Blue => (0, 0, 255),
        Color::Magenta => (255, 0, 255),
        Color::Cyan => (0, 255, 255),
        Color::Gray => (128, 128, 128),
        Color::DarkGray => (64, 64, 64),
        Color::LightRed => (255, 128, 128),
        Color::LightGreen => (128, 255, 128),
        Color::LightYellow => (255, 255, 128),
        Color::LightBlue => (128, 128, 255),
        Color::LightMagenta => (255, 128, 255),
        Color::LightCyan => (128, 255, 255),
        Color::White => (255, 255, 255),
        Color::Reset => (128, 128, 128),
        _ => (128, 128, 128),
    }
}

/// Export the entire map as a PNG image
pub fn export_map_image(
    world: &WorldData,
    view_mode: ViewMode,
    filename: &str,
) -> Result<(), Box<dyn Error>> {
    let width = world.heightmap.width;
    let height = world.heightmap.height;

    let mut img = ImageBuffer::new(width as u32, height as u32);

    for y in 0..height {
        for x in 0..width {
            let biome = *world.biomes.get(x, y);
            let h = *world.heightmap.get(x, y);
            let temp = *world.temperature.get(x, y);
            let moisture = *world.moisture.get(x, y);
            let stress = *world.stress_map.get(x, y);
            let plate_id = *world.plate_map.get(x, y);
            let water_depth = *world.water_depth.get(x, y);
            // Submerged = below sea level OR alpine lake (water_depth > 0)
            let is_water = h < 0.0 || water_depth > 0.5;

            let (r, g, b) = match view_mode {
                ViewMode::Biome => biome.color(),
                ViewMode::BaseBiome => biome.parent_biome().color(),
                ViewMode::Height => height_color(h),
                ViewMode::Temperature => temperature_color(temp),
                ViewMode::Moisture => moisture_color(moisture),
                ViewMode::Stress => stress_color(stress),
                ViewMode::Plates => {
                    let hue = (plate_id.0 as f32 * 37.0) % 360.0;
                    hsv_to_rgb(hue, 0.7, 0.8)
                }
                ViewMode::Rivers => {
                    if is_water {
                        // Water body - darker blue for deeper water
                        let depth_factor = (water_depth / 100.0).min(1.0);
                        let blue = (100.0 + depth_factor * 100.0) as u8;
                        (50, (80.0 + depth_factor * 40.0) as u8, blue)
                    } else {
                        height_color(h)
                    }
                }
                ViewMode::BiomeBlend => biome.color(),
                ViewMode::Coastline => {
                    if is_water {
                        if water_depth < 50.0 { (0, 200, 200) } else { (50, 100, 200) }
                    } else if h < 50.0 {
                        (255, 255, 100)
                    } else {
                        height_color(h)
                    }
                }
                ViewMode::WeatherZones => {
                    if let Some(ref wz) = world.weather_zones {
                        let zone = wz.get(x, y);
                        if zone.has_risk() {
                            zone.primary.color()
                        } else if is_water {
                            (50, 100, 200)
                        } else {
                            height_color(h)
                        }
                    } else if is_water {
                        (50, 100, 200)
                    } else {
                        height_color(h)
                    }
                }
                ViewMode::Microclimate => {
                    if let Some(ref mc) = world.microclimate {
                        let modifiers = mc.get(x, y);
                        let temp_mod = modifiers.temperature_mod;
                        if is_water {
                            (50, 100, 200)
                        } else if temp_mod > 1.0 {
                            // Valley warmth - orange
                            (255, 180, 100)
                        } else if temp_mod < -0.5 {
                            // Ridge cooling - blue
                            (150, 200, 255)
                        } else if modifiers.moisture_mod > 0.05 {
                            // Lake/forest effect - green
                            (100, 200, 150)
                        } else {
                            height_color(h)
                        }
                    } else {
                        height_color(h)
                    }
                }
                ViewMode::SeasonalTemp => {
                    let seasonal_temp = world.get_seasonal_temperature(x, y);
                    temperature_color(seasonal_temp)
                }
                ViewMode::Lava => {
                    use crate::heightmap::LavaState;
                    if let Some(ref lava_map) = world.lava_map {
                        match *lava_map.get(x, y) {
                            LavaState::Molten => (255, 200, 0),
                            LavaState::Flowing => (255, 100, 0),
                            LavaState::Cooled => (60, 50, 50),
                            LavaState::None => {
                                if is_water { (50, 100, 200) } else { height_color(h) }
                            }
                        }
                    } else if is_water {
                        (50, 100, 200)
                    } else {
                        height_color(h)
                    }
                }
                ViewMode::Factions => {
                    // Faction view requires history context; fall back to height for export
                    if is_water { (50, 100, 200) } else { height_color(h) }
                }
                ViewMode::Roads => {
                    // Roads view requires history context; fall back to height for export
                    if is_water { (50, 100, 200) } else { height_color(h) }
                }
            };

            img.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
        }
    }

    img.save(filename)?;
    println!("Exported map to {}", filename);
    Ok(())
}

/// Export an aesthetic top-down view showing biome colors with hillshading
pub fn export_topdown_image(
    world: &WorldData,
    filename: &str,
) -> Result<(), Box<dyn Error>> {
    let width = world.heightmap.width;
    let height = world.heightmap.height;

    let mut img = ImageBuffer::new(width as u32, height as u32);

    for y in 0..height {
        for x in 0..width {
            let biome = *world.biomes.get(x, y);
            let h = *world.heightmap.get(x, y);

            // Get base color from biome
            let (base_r, base_g, base_b) = biome.color();

            // Apply smooth hillshading
            let mut shade = 1.0f32;
            if x >= 2 && y >= 2 && x < width - 2 && y < height - 2 {
                let mut h_topleft = 0.0f32;
                let mut h_botright = 0.0f32;
                for dy in 0..3 {
                    for dx in 0..3 {
                        h_topleft += *world.heightmap.get(x - 2 + dx, y - 2 + dy);
                        h_botright += *world.heightmap.get(x + dx, y + dy);
                    }
                }
                h_topleft /= 9.0;
                h_botright /= 9.0;

                let slope = (h_botright - h_topleft) / 200.0;
                shade = (1.0 + slope).clamp(0.85, 1.15);
            }

            // Subtle elevation-based brightness
            let elevation_factor = if h > 0.0 {
                0.95 + (h / 4000.0).min(0.1)
            } else {
                0.95 + (h / 1000.0).max(-0.15)
            };

            let final_factor = (elevation_factor * shade).clamp(0.7, 1.2);

            let r = ((base_r as f32 * final_factor).min(255.0)) as u8;
            let g = ((base_g as f32 * final_factor).min(255.0)) as u8;
            let b = ((base_b as f32 * final_factor).min(255.0)) as u8;

            img.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
        }
    }

    img.save(filename)?;
    println!("Exported top-down view to {}", filename);
    Ok(())
}

/// Export only rivers and lakes on a black background
/// - Rivers (from flow_accumulation OR river_network): Cyan, intensity based on flow
/// - Lakes (water_depth > 0): Blue, intensity based on depth
/// - Ocean (height < 0): Dark blue
/// - Everything else: Black
pub fn export_water_network_image(
    world: &WorldData,
    filename: &str,
) -> Result<(), Box<dyn Error>> {
    let width = world.heightmap.width;
    let height = world.heightmap.height;
    const RIVER_THRESHOLD: f32 = 50.0;

    let mut img = ImageBuffer::new(width as u32, height as u32);

    for y in 0..height {
        for x in 0..width {
            let h = *world.heightmap.get(x, y);
            let water_depth = *world.water_depth.get(x, y);

            // Get flow accumulation (primary river detection)
            let flow_acc = world.flow_accumulation.as_ref()
                .map(|fa| *fa.get(x, y))
                .unwrap_or(0.0);

            // Check Bezier river network as fallback
            let river_width = if let Some(ref river_network) = world.river_network {
                river_network.get_width_at(x as f32, y as f32, 1.0)
            } else {
                0.0
            };

            let is_river = flow_acc > RIVER_THRESHOLD || river_width > 0.0;

            let (r, g, b) = if is_river && h >= 0.0 {
                // River on land - bright cyan, intensity based on flow
                let intensity = (flow_acc.log2().max(0.0) * 20.0).min(255.0) as u8;
                (0, intensity.saturating_add(100), 255)
            } else if water_depth > 0.5 {
                // Lake (alpine or otherwise) - blue, intensity based on depth
                let depth_factor = (water_depth / 50.0).min(1.0);
                let blue = (150.0 + depth_factor * 105.0) as u8;
                let green = (80.0 + depth_factor * 40.0) as u8;
                (30, green, blue)
            } else if h < 0.0 {
                // Ocean - dark blue (rivers in ocean show as brighter)
                if is_river {
                    let intensity = (flow_acc.log2().max(0.0) * 15.0).min(100.0) as u8;
                    (20, 50 + intensity, 150 + intensity)
                } else {
                    let depth_factor = ((-h) / 2000.0).min(1.0);
                    let blue = (80.0 + depth_factor * 80.0) as u8;
                    (10, 30, blue)
                }
            } else {
                // Land - black
                (0, 0, 0)
            };

            img.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
        }
    }

    img.save(filename)?;
    println!("Exported water network to {}", filename);
    Ok(())
}

/// Run the explorer
pub fn run_explorer(world: WorldData, history: Option<WorldHistory>) -> Result<(), Box<dyn Error>> {
    // Setup terminal
    terminal::enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut explorer = Explorer::new(world, history);

    loop {
        // Measure frame render time
        let frame_start = std::time::Instant::now();

        // Only render when state has changed
        if explorer.needs_redraw {
        // Render
        terminal.draw(|f| {
            let size = f.area();

            // Main layout: content area + status bar
            let main_chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(1),
                    Constraint::Length(1),
                ])
                .split(size);

            let content_area = main_chunks[0];
            let status_area = main_chunks[1];

            if explorer.zoom_active && explorer.zoom_view.is_some() {
                explorer.render_zoom_view(content_area, f.buffer_mut());
                let status_para = Paragraph::new(explorer.zoom_status())
                    .style(Style::default().bg(Color::DarkGray).fg(Color::White));
                f.render_widget(status_para, status_area);
                return;
            }

            // Content layout: map + side panel on right (panel is optional)
            // When region map is shown, make the panel wider to fit it
            let panel_width = if explorer.show_region_map { 68 } else { 28 };

            let map_area = if explorer.show_panel || explorer.show_region_map {
                let content_chunks = Layout::default()
                    .direction(Direction::Horizontal)
                    .constraints([
                        Constraint::Min(1),              // Map takes remaining space
                        Constraint::Length(panel_width), // Panel width (wider when region shown)
                    ])
                    .split(content_area);

                let panel_area = content_chunks[1];

                // Split panel vertically if region map is shown
                if explorer.show_region_map {
                    // Ensure region is cached for current cursor position
                    explorer.ensure_region_cached();

                    let panel_chunks = Layout::default()
                        .direction(Direction::Vertical)
                        .constraints([
                            Constraint::Length(22), // Tile info panel (compact, room for volcano/lava info)
                            Constraint::Min(20),    // Region map takes rest
                        ])
                        .split(panel_area);

                    // Render compact tile info panel at top
                    if explorer.show_panel {
                        explorer.render_tile_panel(panel_chunks[0], f.buffer_mut());
                    }

                    // Render region map panel below
                    explorer.render_region_panel(panel_chunks[1], f.buffer_mut());
                } else if explorer.show_panel {
                    // Just render tile info panel (full height)
                    explorer.render_tile_panel(panel_area, f.buffer_mut());
                }

                content_chunks[0]
            } else {
                content_area
            };

            // Render map
            explorer.render_map(map_area, f.buffer_mut());
            if explorer.show_minimap {
                explorer.render_minimap(map_area, f.buffer_mut());
            }
            if explorer.zoom_pending {
                let text = format!(" Simulating region around ({}, {})... ", explorer.cursor_x, explorer.cursor_y);
                let w = (text.len() as u16 + 2).min(map_area.width);
                let rect = Rect::new(map_area.x + (map_area.width - w) / 2, map_area.y + map_area.height / 2, w, 3);
                f.render_widget(Clear, rect);
                f.render_widget(
                    Paragraph::new(text).block(Block::default().borders(Borders::ALL)).style(Style::default().fg(Color::Yellow)),
                    rect,
                );
            }

            // Render status bar
            let zoom_str = if explorer.zoom > 1 { format!(" | Zoom:{}x", explorer.zoom) } else { String::new() };
            let msg_str = explorer.message.as_ref().map(|m| format!(" | {}", m)).unwrap_or_default();

            // Show season info for seasonal views
            let season_str = if matches!(explorer.view_mode, ViewMode::SeasonalTemp | ViewMode::WeatherZones) {
                format!(" | {}", explorer.world.current_season.name())
            } else {
                String::new()
            };

            // Show weather zone info at cursor
            let weather_str = if explorer.view_mode == ViewMode::WeatherZones {
                if let Some(zone) = explorer.world.get_weather_zone(explorer.cursor_x, explorer.cursor_y) {
                    if zone.has_risk() {
                        format!(" | {}: {:.0}%", zone.primary.display_name(), zone.risk_factor * 100.0)
                    } else {
                        String::new()
                    }
                } else {
                    String::new()
                }
            } else {
                String::new()
            };

            // Build compact status when panel is shown, full info when hidden
            let tile_str = if explorer.show_panel {
                String::new()  // Panel shows detailed info
            } else {
                format!(" | {}", explorer.tile_info())
            };

            let panel_hint = if explorer.show_panel { "" } else { "  I:Panel" };
            let region_hint = if explorer.show_region_map { " [M]" } else { "" };

            let legends_hint = if explorer.history.is_some() { "  L:Legends" } else { "" };
            let status = format!(
                " ({},{}) | {}{}{}{}{}{}{} | V:View  M:Region  Z:Zoom  N:Map{}  [/]:Season  ?:Help{}  Q:Quit | {:.1}ms (avg:{:.1})",
                explorer.cursor_x,
                explorer.cursor_y,
                explorer.view_mode.name(),
                region_hint,
                tile_str,
                zoom_str,
                season_str,
                weather_str,
                msg_str,
                legends_hint,
                panel_hint,
                explorer.frame_time_ms,
                explorer.avg_frame_time,
            );
            let status_para = Paragraph::new(status)
                .style(Style::default().bg(Color::DarkGray).fg(Color::White));
            f.render_widget(status_para, status_area);

            // Render legends mode overlay
            if let Some(ref legends) = explorer.legends_mode {
                render_legends(legends, content_area, f.buffer_mut());
            }

            // Render help if active (as overlay on map)
            if explorer.show_help && explorer.legends_mode.is_none() {
                explorer.render_help(map_area, f.buffer_mut());
            }
        })?;

        // Update frame timing
        explorer.frame_time_ms = frame_start.elapsed().as_secs_f32() * 1000.0;
        explorer.avg_frame_time = explorer.avg_frame_time * 0.9 + explorer.frame_time_ms * 0.1;

        // Clear message after display
        explorer.message = None;
        explorer.needs_redraw = false;
        } // end if needs_redraw

        // A zoom was requested: the "simulating" frame is on screen now, so do the work.
        if explorer.zoom_pending {
            let size = terminal.size()?;
            let content = Rect::new(0, 0, size.width, size.height.saturating_sub(1));
            explorer.run_pending_zoom(content);
            continue;
        }

        // Handle input
        if event::poll(Duration::from_millis(50))? {
            let event = event::read()?;
            explorer.needs_redraw = true;
            match event {
                Event::Key(key) => {
                    // Zoom view has its own controls
                    if explorer.zoom_active {
                        let size = terminal.size()?;
                        let content = Rect::new(0, 0, size.width, size.height.saturating_sub(1));
                        let step = (size.width as f32 / 6.0).max(4.0);
                        if let Some(z) = explorer.zoom_view.as_mut() {
                            match key.code {
                                KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('z') | KeyCode::Char('Z') => {
                                    explorer.zoom_active = false;
                                }
                                KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('k') => z.pan(0.0, -step),
                                KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('j') => z.pan(0.0, step),
                                KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('h') => z.pan(-step, 0.0),
                                KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('l') => z.pan(step, 0.0),
                                KeyCode::Char('+') | KeyCode::Char('=') => z.rescale(0.5),
                                KeyCode::Char('-') | KeyCode::Char('_') => z.rescale(2.0),
                                KeyCode::Char('f') | KeyCode::Char('F') => z.fit(content),
                                KeyCode::Char('x') | KeyCode::Char('X') => {
                                    let name = format!("zoom_{}_{}_{}.png", z.world_seed, z.tile.0, z.tile.1);
                                    explorer.message = Some(match z.region.save_png(std::path::Path::new(&name)) {
                                        Ok(()) => format!("Saved {name}"),
                                        Err(e) => format!("Save failed: {e}"),
                                    });
                                }
                                _ => {}
                            }
                        }
                        continue;
                    }

                    // Handle legends mode input first
                    if explorer.legends_mode.is_some() {
                        let size = terminal.size()?;
                        let visible_height = size.height.saturating_sub(4) as usize;
                        match key.code {
                            KeyCode::Esc | KeyCode::Char('q') | KeyCode::Char('Q') => {
                                let legends = explorer.legends_mode.as_mut().unwrap();
                                if legends.nav_stack.is_empty() {
                                    // Exit legends mode
                                    explorer.legends_mode = None;
                                } else {
                                    // Pop navigation stack
                                    legends.go_back();
                                }
                            }
                            KeyCode::Up | KeyCode::Char('k') => {
                                explorer.legends_mode.as_mut().unwrap().move_up();
                            }
                            KeyCode::Down | KeyCode::Char('j') => {
                                explorer.legends_mode.as_mut().unwrap().move_down(visible_height);
                            }
                            KeyCode::Enter => {
                                if let Some(ref history) = explorer.history {
                                    explorer.legends_mode.as_mut().unwrap().select(history);
                                }
                            }
                            KeyCode::Backspace => {
                                if let Some(ref history) = explorer.history {
                                    explorer.legends_mode.as_mut().unwrap().search_pop(history);
                                }
                            }
                            KeyCode::Char(ch) => {
                                if let Some(ref history) = explorer.history {
                                    explorer.legends_mode.as_mut().unwrap().search_push(history, ch);
                                }
                            }
                            _ => {}
                        }
                        continue;
                    }

                    if explorer.show_help {
                        explorer.show_help = false;
                        continue;
                    }

                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => break,
                        KeyCode::Char('?') => explorer.show_help = true,
                        KeyCode::Char('v') | KeyCode::Char('V') => {
                            explorer.view_mode = explorer.view_mode.next();
                        }

                        // Movement
                        KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('k') => {
                            explorer.move_cursor(0, -1);
                        }
                        KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('j') => {
                            explorer.move_cursor(0, 1);
                        }
                        KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('h') => {
                            explorer.move_cursor(-1, 0);
                        }
                        KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('l') => {
                            explorer.move_cursor(1, 0);
                        }

                        // Fast movement
                        KeyCode::PageUp => explorer.move_cursor(0, -20),
                        KeyCode::PageDown => explorer.move_cursor(0, 20),
                        KeyCode::Home => explorer.move_cursor(-20, 0),
                        KeyCode::End => explorer.move_cursor(20, 0),

                        // Zoom controls
                        KeyCode::Char('-') | KeyCode::Char('_') => explorer.zoom_out(),
                        KeyCode::Char('+') | KeyCode::Char('=') => explorer.zoom_in(),
                        KeyCode::Char('f') | KeyCode::Char('F') => {
                            let size = terminal.size()?;
                            explorer.fit_to_screen(size.width as usize, (size.height - 1) as usize);
                        }

                        // Export image
                        KeyCode::Char('e') | KeyCode::Char('E') => {
                            let filename = format!("world_{}.png", explorer.world.seed());
                            match export_map_image(&explorer.world, explorer.view_mode, &filename) {
                                Ok(_) => explorer.message = Some(format!("Exported: {}", filename)),
                                Err(e) => explorer.message = Some(format!("Export failed: {}", e)),
                            }
                        }

                        // Export freshwater network (rivers + lakes)
                        KeyCode::Char('W') => {
                            let filename = format!("freshwater_{}.png", explorer.world.seed());
                            match export_freshwater_network_image(&explorer.world, &filename) {
                                Ok(_) => explorer.message = Some(format!("Exported: {}", filename)),
                                Err(e) => explorer.message = Some(format!("Export failed: {}", e)),
                            }
                        }

                        // Export antique old paper cartography map
                        KeyCode::Char('o') | KeyCode::Char('O') | KeyCode::Char('p') | KeyCode::Char('P') => {
                            let filename = format!("world_cartography_{}.png", explorer.world.seed());
                            match crate::cartography::export_cartography_image(&explorer.world, &filename, None) {
                                Ok(_) => explorer.message = Some(format!("Exported Cartography: {}", filename)),
                                Err(e) => explorer.message = Some(format!("Export failed: {}", e)),
                            }
                        }

                        // Regenerate world with new seed
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            explorer.regenerate();
                        }

                        // Toggle info panel
                        KeyCode::Tab | KeyCode::Char('i') | KeyCode::Char('I') => {
                            explorer.show_panel = !explorer.show_panel;
                            explorer.message = Some(if explorer.show_panel {
                                "Panel: ON".to_string()
                            } else {
                                "Panel: OFF".to_string()
                            });
                        }

                        // Top-down aesthetic export
                        KeyCode::Char('t') | KeyCode::Char('T') => {
                            let filename = format!("world_topdown_{}.png", explorer.world.seed());
                            match export_topdown_image(&explorer.world, &filename) {
                                Ok(_) => explorer.message = Some(format!("Exported: {}", filename)),
                                Err(e) => explorer.message = Some(format!("Export failed: {}", e)),
                            }
                        }

                        // Base map export (flat colors, no shading)
                        KeyCode::Char('b') | KeyCode::Char('B') => {
                            let filename = format!("world_base_{}.png", explorer.world.seed());
                            match export_base_map_image(&explorer.world, &filename) {
                                Ok(_) => explorer.message = Some(format!("Exported: {}", filename)),
                                Err(e) => explorer.message = Some(format!("Export failed: {}", e)),
                            }
                        }

                        // Season cycling
                        KeyCode::Char('[') | KeyCode::Char('{') => {
                            explorer.prev_season();
                        }
                        KeyCode::Char(']') | KeyCode::Char('}') => {
                            explorer.next_season();
                        }

                        // Legends mode
                        KeyCode::Char('L') => {
                            if explorer.history.is_some() {
                                explorer.legends_mode = Some(LegendsMode::new());
                            } else {
                                explorer.message = Some("No history (run with --history-years)".to_string());
                            }
                        }

                        // Minimap toggle
                        KeyCode::Char('n') | KeyCode::Char('N') => {
                            explorer.show_minimap = !explorer.show_minimap;
                        }

                        // Zoom: re-simulate the region around the cursor at high resolution
                        KeyCode::Char('z') | KeyCode::Char('Z') => {
                            explorer.zoom_pending = true;
                        }

                        // Region map panel toggle
                        KeyCode::Char('m') | KeyCode::Char('M') => {
                            explorer.show_region_map = !explorer.show_region_map;
                            if explorer.show_region_map {
                                explorer.ensure_region_cached();
                            }
                        }

                        _ => {}
                    }
                }
                Event::Mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), .. }) if explorer.zoom_active => {}
                Event::Mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column, row, .. }) => {
                    // Click to move cursor
                    let size = terminal.size()?;
                    if row < size.height - 1 {
                        let view_width = size.width as usize;
                        let view_height = (size.height - 1) as usize;
                        let zoom = explorer.zoom;

                        let map_view_width = view_width * zoom;
                        let map_view_height = view_height * zoom;

                        let start_x = if explorer.cursor_x >= map_view_width / 2 {
                            explorer.cursor_x - map_view_width / 2
                        } else {
                            0
                        };
                        let start_y = if explorer.cursor_y >= map_view_height / 2 {
                            explorer.cursor_y - map_view_height / 2
                        } else {
                            0
                        };

                        let new_x = (start_x + column as usize * zoom) % explorer.world.heightmap.width;
                        let new_y = (start_y + row as usize * zoom).min(explorer.world.heightmap.height - 1);

                        explorer.cursor_x = new_x;
                        explorer.cursor_y = new_y;
                    }
                }
                _ => {}
            }
        }
    }

    // Cleanup
    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Convert a rendered buffer into an image (half-block cells -> 2 pixels: fg over bg).
    fn buffer_to_image(buf: &Buffer) -> image::RgbImage {
        let area = buf.area;
        let rgb = |c: Color, fallback: [u8; 3]| match c {
            Color::Rgb(r, g, b) => [r, g, b],
            _ => fallback,
        };
        ImageBuffer::from_fn(area.width as u32, area.height as u32 * 2, |x, y| {
            let cell = &buf[(area.x + x as u16, area.y + (y / 2) as u16)];
            let bg = rgb(cell.bg, [0, 0, 0]);
            let fg = rgb(cell.fg, [220, 220, 220]);
            let px = match cell.symbol() {
                "▀" => if y % 2 == 0 { fg } else { bg },
                " " => bg,
                _ => fg,
            };
            Rgb(px)
        })
    }

    #[test]
    fn minimap_and_zoom_view_render() {
        let world = crate::world::generate_world_with_style(128, 64, 7, crate::plates::WorldStyle::Earthlike);
        let mut ex = Explorer::new(world, None);
        let area = Rect::new(0, 0, 160, 48);

        let mut buf = Buffer::empty(area);
        ex.render_map(area, &mut buf);
        ex.render_minimap(area, &mut buf);
        let half_blocks = |b: &Buffer| b.content().iter().filter(|c| c.symbol() == "▀").count();
        assert!(half_blocks(&buf) > 100, "minimap should be drawn");

        let (cx, cy) = crate::region::zoom::pick_interesting_window(&ex.world, ZoomParams::default().tiles);
        ex.cursor_x = cx;
        ex.cursor_y = cy;
        ex.zoom_pending = true;
        ex.run_pending_zoom(area);
        assert!(ex.zoom_active && ex.zoom_view.is_some());
        let mut zbuf = Buffer::empty(area);
        ex.render_zoom_view(area, &mut zbuf);
        assert!(half_blocks(&zbuf) > (160 * 48) / 2, "zoom view should fill the screen");
        assert!(!ex.zoom_status().is_empty());

        if let Ok(dir) = std::env::var("EXPLORER_SNAPSHOT_DIR") {
            buffer_to_image(&buf).save(format!("{dir}/explorer_minimap.png")).unwrap();
            buffer_to_image(&zbuf).save(format!("{dir}/explorer_zoom.png")).unwrap();
            println!("{}", ex.zoom_status());
        }
    }
}
