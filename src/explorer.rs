//! Terminal-based world map explorer using ratatui
//!
//! Simple roguelike-style terminal interface for exploring generated worlds.
//! Navigate with arrow keys, inspect tiles, change view modes.

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
use crate::world::{WorldData, generate_world};
use crate::zlevel::{self, ZTile, z_to_height, z_to_height_ceiling, z_level_description};

use image::{ImageBuffer, Rgb};

/// Viewport for rendering a portion of the map
struct Viewport {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

/// View mode for the map display
#[derive(Clone, Copy, PartialEq)]
enum ViewMode {
    Biome,
    Height,
    Temperature,
    Moisture,
    Plates,
    Stress,
    Factions,
    History,
}

impl ViewMode {
    fn name(&self) -> &'static str {
        match self {
            ViewMode::Biome => "Biome",
            ViewMode::Height => "Height",
            ViewMode::Temperature => "Temperature",
            ViewMode::Moisture => "Moisture",
            ViewMode::Plates => "Plates",
            ViewMode::Stress => "Stress",
            ViewMode::Factions => "Factions",
            ViewMode::History => "History",
        }
    }

    fn next(&self) -> ViewMode {
        match self {
            ViewMode::Biome => ViewMode::Height,
            ViewMode::Height => ViewMode::Temperature,
            ViewMode::Temperature => ViewMode::Moisture,
            ViewMode::Moisture => ViewMode::Plates,
            ViewMode::Plates => ViewMode::Stress,
            ViewMode::Stress => ViewMode::Factions,
            ViewMode::Factions => ViewMode::History,
            ViewMode::History => ViewMode::Biome,
        }
    }
}

/// Explorer state
struct Explorer {
    world: WorldData,
    cursor_x: usize,
    cursor_y: usize,
    cursor_z: i32,
    view_mode: ViewMode,
    show_help: bool,
    /// Zoom level: 1 = normal, 2 = 2x zoom out, 4 = 4x zoom out, etc.
    zoom: usize,
    /// Message to display temporarily
    message: Option<String>,
}

impl Explorer {
    fn new(world: WorldData) -> Self {
        let cursor_x = world.heightmap.width / 2;
        let cursor_y = world.heightmap.height / 2;
        // Start at surface level at the cursor position
        let cursor_z = *world.surface_z.get(cursor_x, cursor_y);

        Explorer {
            world,
            cursor_x,
            cursor_y,
            cursor_z,
            view_mode: ViewMode::Biome,
            show_help: false,
            zoom: 1,
            message: None,
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

        // Reset cursor to center of map at surface level
        self.cursor_x = width / 2;
        self.cursor_y = height / 2;
        self.cursor_z = *self.world.surface_z.get(self.cursor_x, self.cursor_y);
        self.zoom = 1;

        self.message = Some(format!("New world generated! Seed: {}", new_seed));
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

    /// Move Z-level up
    fn move_z_up(&mut self) {
        if self.cursor_z < zlevel::MAX_Z {
            self.cursor_z += 1;
        }
    }

    /// Move Z-level down
    fn move_z_down(&mut self) {
        if self.cursor_z > zlevel::MIN_Z {
            self.cursor_z -= 1;
        }
    }

    /// Go to sea level (Z = 0)
    fn go_to_sea_level(&mut self) {
        self.cursor_z = zlevel::SEA_LEVEL_Z;
    }

    /// Go to surface at current cursor position
    fn go_to_surface(&mut self) {
        self.cursor_z = *self.world.surface_z.get(self.cursor_x, self.cursor_y);
    }

    /// Get tile info at cursor
    fn tile_info(&self) -> String {
        let x = self.cursor_x;
        let y = self.cursor_y;

        let height = *self.world.heightmap.get(x, y);
        let temp = *self.world.temperature.get(x, y);
        let moisture = *self.world.moisture.get(x, y);
        let biome = *self.world.biomes.get(x, y);
        let surface_z = *self.world.surface_z.get(x, y);
        let ztile = *self.world.zlevels.get(x, y, self.cursor_z);

        // Show tile type based on what we're looking at
        let tile_name = ztile_name(ztile);

        // Get history info if available
        let history_str = if let Some(ref history) = self.world.history {
            let info = history.tile_info(x, y);
            info.summary().map(|s| format!(" | {}", s)).unwrap_or_default()
        } else {
            String::new()
        };

        if self.cursor_z == surface_z {
            // At surface - show biome
            format!(
                "({}, {}) | {} | {:?} | {:.0}m | {:.1}°C | {:.0}%{}",
                x, y, tile_name, biome, height, temp, moisture * 100.0, history_str,
            )
        } else {
            // Underground - show tile type
            format!(
                "({}, {}) | {} | Depth: {} | {:.1}°C | {:.0}%{}",
                x, y, tile_name, surface_z - self.cursor_z, temp, moisture * 100.0, history_str,
            )
        }
    }

    /// Get Z-level status string for the status bar
    fn z_level_status(&self) -> String {
        let floor_height = z_to_height(self.cursor_z);
        let ceiling_height = z_to_height_ceiling(self.cursor_z);
        let description = z_level_description(self.cursor_z);
        format!(
            "Z: {:+} ({:.0}m to {:.0}m) [{}]",
            self.cursor_z,
            floor_height,
            ceiling_height,
            description,
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

                buf.get_mut(screen_x, screen_y).set_char(ch).set_style(style);
            }
        }
    }

    /// Get display character and colors for a tile based on Z-level
    fn get_tile_display(&self, x: usize, y: usize) -> (char, Color, Color) {
        let ztile = *self.world.zlevels.get(x, y, self.cursor_z);
        let surface_z = *self.world.surface_z.get(x, y);
        let biome = *self.world.biomes.get(x, y);
        let height = *self.world.heightmap.get(x, y);
        let temp = *self.world.temperature.get(x, y);
        let moisture = *self.world.moisture.get(x, y);
        let stress = *self.world.stress_map.get(x, y);
        let plate_id = *self.world.plate_map.get(x, y);

        // For non-biome view modes, show layer information
        match self.view_mode {
            ViewMode::Biome => {
                // Show Z-level aware rendering
                match ztile {
                    ZTile::Air => (' ', Color::Black, Color::Black),
                    ZTile::Water => ('~', Color::Rgb(100, 150, 255), Color::Rgb(20, 40, 80)),
                    ZTile::Surface => {
                        // Use biome colors for surface
                        let ch = biome_char(&biome);
                        let (r, g, b) = biome.color();
                        (ch, Color::Rgb(r, g, b), Color::Reset)
                    }
                    ZTile::Solid => {
                        // Underground - show rock with depth shading
                        let depth_below = surface_z - self.cursor_z;
                        let shade = (80 - depth_below * 4).max(30) as u8;
                        ('#', Color::Rgb(shade, shade, shade), Color::Rgb(20, 20, 20))
                    }
                    ZTile::Aquifer => {
                        // Underground water reservoir - cyan/teal
                        ('≈', Color::Rgb(0, 200, 220), Color::Rgb(0, 60, 80))
                    }
                    ZTile::UndergroundRiver => {
                        // Flowing underground channel - light blue
                        ('~', Color::Rgb(100, 180, 255), Color::Rgb(20, 50, 100))
                    }
                    ZTile::WaterCave => {
                        // Water-filled cave chamber - teal
                        ('○', Color::Rgb(0, 180, 180), Color::Rgb(0, 40, 60))
                    }
                    ZTile::Spring => {
                        // Surface emergence point - bright cyan on green
                        ('◊', Color::Rgb(0, 255, 255), Color::Rgb(0, 80, 40))
                    }

                    // === Cave Structure ===
                    ZTile::CaveFloor => {
                        // Walkable cave floor - gray
                        ('.', Color::Rgb(60, 55, 50), Color::Rgb(15, 15, 20))
                    }
                    ZTile::CaveWall => {
                        // Cave wall - brown
                        ('#', Color::Rgb(80, 70, 60), Color::Rgb(20, 18, 15))
                    }

                    // === Speleothems (Cave Formations) ===
                    ZTile::Stalactite => {
                        // Hanging formation - cyan
                        ('▼', Color::Rgb(180, 200, 220), Color::Rgb(30, 30, 40))
                    }
                    ZTile::Stalagmite => {
                        // Rising formation - tan
                        ('▲', Color::Rgb(160, 140, 120), Color::Rgb(30, 25, 20))
                    }
                    ZTile::Pillar => {
                        // Merged column - light stone
                        ('│', Color::Rgb(200, 180, 160), Color::Rgb(40, 35, 30))
                    }
                    ZTile::Flowstone => {
                        // Sheet deposit - cream
                        ('=', Color::Rgb(180, 170, 150), Color::Rgb(35, 30, 25))
                    }

                    // === Cave Biomes ===
                    ZTile::FungalGrowth => {
                        // Glowing fungi - bright green
                        ('*', Color::Rgb(100, 255, 180), Color::Rgb(20, 60, 40))
                    }
                    ZTile::GiantMushroom => {
                        // Large mushroom (tower cap) - purple
                        ('♠', Color::Rgb(180, 100, 220), Color::Rgb(40, 20, 50))
                    }
                    ZTile::CrystalFormation => {
                        // Crystal growths - violet
                        ('◆', Color::Rgb(200, 100, 255), Color::Rgb(40, 20, 60))
                    }
                    ZTile::CaveMoss => {
                        // Bioluminescent moss - teal
                        ('\'', Color::Rgb(80, 200, 180), Color::Rgb(15, 40, 35))
                    }

                    // === Deep Features ===
                    ZTile::MagmaPool => {
                        // Molten rock - bright orange/red
                        ('≈', Color::Rgb(255, 100, 0), Color::Rgb(80, 20, 0))
                    }
                    ZTile::MagmaTube => {
                        // Lava tube passage - dark red
                        ('○', Color::Rgb(100, 40, 40), Color::Rgb(30, 15, 10))
                    }
                    ZTile::ObsidianFloor => {
                        // Cooled magma - dark gray/black
                        ('_', Color::Rgb(40, 40, 50), Color::Rgb(10, 10, 15))
                    }

                    // === Water Integration ===
                    ZTile::CaveLake => {
                        // Underground lake - dark blue
                        ('~', Color::Rgb(40, 80, 120), Color::Rgb(10, 20, 40))
                    }
                    ZTile::Waterfall => {
                        // Falling water - bright blue
                        ('|', Color::Rgb(150, 200, 255), Color::Rgb(30, 60, 100))
                    }

                    // === Vertical Passages ===
                    ZTile::RampUp => {
                        // Can go up - bright green arrow
                        ('↑', Color::Rgb(100, 255, 100), Color::Rgb(20, 50, 20))
                    }
                    ZTile::RampDown => {
                        // Can go down - bright red arrow
                        ('↓', Color::Rgb(255, 100, 100), Color::Rgb(50, 20, 20))
                    }
                    ZTile::RampBoth => {
                        // Can go both ways - bright yellow double arrow
                        ('↕', Color::Rgb(255, 255, 100), Color::Rgb(50, 50, 20))
                    }

                    // === Human-Made Structures ===

                    // Structure Walls
                    ZTile::StoneWall => {
                        ('#', Color::Rgb(140, 140, 145), Color::Rgb(50, 50, 55))
                    }
                    ZTile::BrickWall => {
                        ('#', Color::Rgb(160, 100, 80), Color::Rgb(60, 35, 25))
                    }
                    ZTile::WoodWall => {
                        ('#', Color::Rgb(180, 140, 100), Color::Rgb(70, 50, 35))
                    }
                    ZTile::RuinedWall => {
                        ('%', Color::Rgb(100, 95, 90), Color::Rgb(40, 38, 35))
                    }

                    // Structure Floors
                    ZTile::StoneFloor => {
                        ('.', Color::Rgb(130, 130, 135), Color::Rgb(45, 45, 50))
                    }
                    ZTile::WoodFloor => {
                        ('.', Color::Rgb(160, 130, 90), Color::Rgb(55, 45, 30))
                    }
                    ZTile::CobblestoneFloor => {
                        (',', Color::Rgb(120, 115, 110), Color::Rgb(40, 38, 35))
                    }
                    ZTile::DirtFloor => {
                        ('.', Color::Rgb(140, 110, 70), Color::Rgb(50, 40, 25))
                    }

                    // Structure Features
                    ZTile::Door => {
                        ('+', Color::Rgb(140, 100, 60), Color::Rgb(50, 35, 25))
                    }
                    ZTile::Window => {
                        ('□', Color::Rgb(180, 200, 220), Color::Rgb(40, 50, 60))
                    }
                    ZTile::StairsUp => {
                        ('<', Color::Rgb(200, 200, 200), Color::Rgb(60, 60, 65))
                    }
                    ZTile::StairsDown => {
                        ('>', Color::Rgb(200, 200, 200), Color::Rgb(60, 60, 65))
                    }
                    ZTile::Column => {
                        ('│', Color::Rgb(180, 175, 170), Color::Rgb(55, 55, 60))
                    }
                    ZTile::Rubble => {
                        ('*', Color::Rgb(90, 85, 80), Color::Rgb(30, 28, 25))
                    }
                    ZTile::Chest => {
                        ('□', Color::Rgb(200, 150, 50), Color::Rgb(60, 45, 15))
                    }
                    ZTile::Altar => {
                        ('╥', Color::Rgb(200, 180, 220), Color::Rgb(50, 45, 60))
                    }

                    // Roads
                    ZTile::DirtRoad => {
                        ('═', Color::Rgb(140, 120, 80), Color::Rgb(50, 40, 25))
                    }
                    ZTile::StoneRoad => {
                        ('═', Color::Rgb(150, 150, 155), Color::Rgb(55, 55, 60))
                    }
                    ZTile::Bridge => {
                        ('═', Color::Rgb(130, 90, 50), Color::Rgb(20, 30, 50))
                    }

                    // Cave Structures
                    ZTile::MinedTunnel => {
                        ('.', Color::Rgb(100, 80, 60), Color::Rgb(30, 25, 20))
                    }
                    ZTile::MinedRoom => {
                        ('.', Color::Rgb(110, 90, 70), Color::Rgb(35, 28, 22))
                    }
                    ZTile::MineSupport => {
                        ('║', Color::Rgb(120, 80, 40), Color::Rgb(35, 25, 15))
                    }
                    ZTile::Torch => {
                        ('☼', Color::Rgb(255, 200, 50), Color::Rgb(80, 40, 0))
                    }

                    // Mining structures
                    ZTile::MineShaft => {
                        ('○', Color::Rgb(80, 60, 40), Color::Rgb(20, 15, 10))
                    }
                    ZTile::MineLadder => {
                        ('H', Color::Rgb(140, 100, 60), Color::Rgb(30, 20, 10))
                    }
                    ZTile::MineRails => {
                        ('═', Color::Rgb(100, 100, 110), Color::Rgb(40, 35, 30))
                    }
                    ZTile::OreVein => {
                        ('*', Color::Rgb(180, 140, 80), Color::Rgb(60, 45, 25))
                    }
                    ZTile::RichOreVein => {
                        ('◆', Color::Rgb(255, 215, 0), Color::Rgb(80, 60, 20))
                    }
                    ZTile::MineEntrance => {
                        ('▼', Color::Rgb(90, 70, 50), Color::Rgb(40, 30, 20))
                    }

                    // Underground fortress
                    ZTile::FortressWall => {
                        ('█', Color::Rgb(80, 80, 90), Color::Rgb(40, 40, 50))
                    }
                    ZTile::FortressFloor => {
                        ('·', Color::Rgb(100, 100, 110), Color::Rgb(35, 35, 45))
                    }
                    ZTile::FortressGate => {
                        ('‡', Color::Rgb(120, 80, 40), Color::Rgb(45, 35, 25))
                    }
                    ZTile::Vault => {
                        ('$', Color::Rgb(200, 180, 80), Color::Rgb(50, 45, 30))
                    }
                    ZTile::BarracksFloor => {
                        ('░', Color::Rgb(110, 90, 70), Color::Rgb(40, 32, 24))
                    }
                    ZTile::ForgeFloor => {
                        ('▒', Color::Rgb(180, 80, 30), Color::Rgb(60, 30, 10))
                    }
                    ZTile::Cistern => {
                        ('≈', Color::Rgb(60, 120, 180), Color::Rgb(20, 40, 60))
                    }

                    // === Historical Evidence ===

                    // Battlefield evidence
                    ZTile::BoneField => {
                        ('☠', Color::Rgb(200, 190, 170), Color::Rgb(40, 35, 30))
                    }
                    ZTile::RustedWeapons => {
                        ('†', Color::Rgb(150, 100, 80), Color::Rgb(45, 30, 25))
                    }
                    ZTile::WarMemorial => {
                        ('╬', Color::Rgb(160, 160, 170), Color::Rgb(50, 50, 55))
                    }
                    ZTile::Crater => {
                        ('○', Color::Rgb(80, 70, 60), Color::Rgb(25, 22, 18))
                    }

                    // Cultural markers
                    ZTile::BoundaryStone => {
                        ('◙', Color::Rgb(140, 140, 145), Color::Rgb(45, 45, 50))
                    }
                    ZTile::MileMarker => {
                        ('│', Color::Rgb(150, 145, 140), Color::Rgb(50, 48, 45))
                    }
                    ZTile::Shrine => {
                        ('╥', Color::Rgb(180, 160, 200), Color::Rgb(50, 45, 60))
                    }
                    ZTile::Statue => {
                        ('♀', Color::Rgb(170, 170, 180), Color::Rgb(55, 55, 60))
                    }
                    ZTile::Obelisk => {
                        ('↑', Color::Rgb(140, 140, 150), Color::Rgb(45, 45, 50))
                    }

                    // Monster evidence
                    ZTile::BoneNest => {
                        ('☠', Color::Rgb(180, 170, 150), Color::Rgb(35, 30, 25))
                    }
                    ZTile::WebCluster => {
                        ('▓', Color::Rgb(200, 200, 210), Color::Rgb(60, 60, 65))
                    }
                    ZTile::SlimeTrail => {
                        ('~', Color::Rgb(100, 180, 80), Color::Rgb(30, 50, 25))
                    }
                    ZTile::TerritoryMarking => {
                        ('!', Color::Rgb(180, 130, 60), Color::Rgb(55, 40, 20))
                    }
                    ZTile::AntMound => {
                        ('▲', Color::Rgb(140, 100, 60), Color::Rgb(45, 32, 20))
                    }
                    ZTile::BeeHive => {
                        ('◆', Color::Rgb(200, 180, 60), Color::Rgb(60, 55, 20))
                    }
                    ZTile::ClawMarks => {
                        ('≡', Color::Rgb(120, 100, 80), Color::Rgb(40, 32, 25))
                    }
                    ZTile::CursedGround => {
                        ('†', Color::Rgb(100, 60, 100), Color::Rgb(30, 18, 30))
                    }
                    ZTile::CharredGround => {
                        ('░', Color::Rgb(50, 50, 50), Color::Rgb(20, 18, 15))
                    }

                    // Trade/resource evidence
                    ZTile::AbandonedCart => {
                        ('□', Color::Rgb(120, 90, 60), Color::Rgb(40, 30, 20))
                    }
                    ZTile::WaystationRuin => {
                        ('■', Color::Rgb(100, 95, 90), Color::Rgb(35, 33, 30))
                    }
                    ZTile::DriedWell => {
                        ('○', Color::Rgb(110, 100, 90), Color::Rgb(38, 35, 30))
                    }
                    ZTile::OvergrownGarden => {
                        ('♣', Color::Rgb(80, 140, 80), Color::Rgb(25, 45, 25))
                    }

                    // Graveyards
                    ZTile::Gravestone => {
                        ('†', Color::Rgb(140, 140, 145), Color::Rgb(45, 45, 48))
                    }
                    ZTile::Tomb => {
                        ('╬', Color::Rgb(130, 125, 130), Color::Rgb(42, 40, 42))
                    }
                    ZTile::Mausoleum => {
                        ('▓', Color::Rgb(150, 150, 155), Color::Rgb(48, 48, 52))
                    }
                    ZTile::Ossuary => {
                        ('☠', Color::Rgb(200, 195, 180), Color::Rgb(50, 48, 45))
                    }
                    ZTile::MassGrave => {
                        ('▓', Color::Rgb(90, 80, 70), Color::Rgb(30, 25, 22))
                    }

                    // === Artifact Containers ===
                    ZTile::ArtifactPedestal => {
                        ('╦', Color::Rgb(200, 180, 100), Color::Rgb(50, 45, 25))
                    }
                    ZTile::TreasureChest => {
                        ('▣', Color::Rgb(180, 140, 60), Color::Rgb(50, 40, 20))
                    }
                    ZTile::BookShelf => {
                        ('▤', Color::Rgb(140, 100, 60), Color::Rgb(40, 28, 18))
                    }
                    ZTile::RelicShrine => {
                        ('╥', Color::Rgb(180, 160, 200), Color::Rgb(45, 40, 55))
                    }
                    ZTile::ScrollCase => {
                        ('▥', Color::Rgb(160, 140, 100), Color::Rgb(45, 40, 28))
                    }

                    // === Statue Variants ===
                    ZTile::HeroStatue => {
                        ('♀', Color::Rgb(180, 180, 190), Color::Rgb(50, 50, 55))
                    }
                    ZTile::RuinedStatue => {
                        ('♀', Color::Rgb(100, 95, 90), Color::Rgb(35, 33, 30))
                    }

                    // === Dungeon Markers ===
                    ZTile::DungeonEntrance => {
                        ('▼', Color::Rgb(120, 80, 100), Color::Rgb(40, 25, 35))
                    }
                    ZTile::TreasureHoard => {
                        ('$', Color::Rgb(255, 215, 0), Color::Rgb(80, 55, 0))
                    }
                }
            }
            ViewMode::Height => {
                match ztile {
                    ZTile::Air => (' ', Color::Black, Color::Black),
                    ZTile::Water => ('~', Color::Rgb(50, 100, 200), Color::Reset),
                    ZTile::Aquifer => ('≈', Color::Rgb(0, 200, 220), Color::Rgb(0, 60, 80)),
                    ZTile::UndergroundRiver => ('~', Color::Rgb(100, 180, 255), Color::Rgb(20, 50, 100)),
                    ZTile::WaterCave | ZTile::CaveLake => ('○', Color::Rgb(0, 180, 180), Color::Rgb(0, 40, 60)),
                    ZTile::Spring | ZTile::Waterfall => ('◊', Color::Rgb(0, 255, 255), Color::Rgb(0, 80, 40)),
                    ZTile::MagmaPool => ('≈', Color::Rgb(255, 100, 0), Color::Rgb(80, 20, 0)),
                    ZTile::Surface | ZTile::Solid => {
                        let ch = if ztile == ZTile::Surface { '#' } else { '.' };
                        let (r, g, b) = height_color(height);
                        (ch, Color::Rgb(r, g, b), Color::Reset)
                    }
                    // Cave tiles in height mode - show with height coloring
                    _ => {
                        let ch = cave_tile_char(ztile);
                        let (r, g, b) = height_color(height);
                        (ch, Color::Rgb(r, g, b), Color::Rgb(15, 15, 20))
                    }
                }
            }
            ViewMode::Temperature => {
                match ztile {
                    ZTile::Air => (' ', Color::Black, Color::Black),
                    ZTile::Water => ('~', Color::Rgb(50, 100, 200), Color::Reset),
                    ZTile::Aquifer => ('≈', Color::Rgb(0, 200, 220), Color::Rgb(0, 60, 80)),
                    ZTile::UndergroundRiver => ('~', Color::Rgb(100, 180, 255), Color::Rgb(20, 50, 100)),
                    ZTile::WaterCave | ZTile::CaveLake => ('○', Color::Rgb(0, 180, 180), Color::Rgb(0, 40, 60)),
                    ZTile::Spring | ZTile::Waterfall => ('◊', Color::Rgb(0, 255, 255), Color::Rgb(0, 80, 40)),
                    ZTile::MagmaPool => ('≈', Color::Rgb(255, 100, 0), Color::Rgb(80, 20, 0)),
                    ZTile::Surface | ZTile::Solid => {
                        let ch = if ztile == ZTile::Surface { '.' } else { '#' };
                        let (r, g, b) = temperature_color(temp);
                        (ch, Color::Rgb(r, g, b), Color::Reset)
                    }
                    // Cave tiles in temperature mode - show with temp coloring
                    _ => {
                        let ch = cave_tile_char(ztile);
                        let (r, g, b) = temperature_color(temp);
                        (ch, Color::Rgb(r, g, b), Color::Rgb(15, 15, 20))
                    }
                }
            }
            ViewMode::Moisture => {
                match ztile {
                    ZTile::Air => (' ', Color::Black, Color::Black),
                    ZTile::Water => ('~', Color::Rgb(50, 100, 200), Color::Reset),
                    ZTile::Aquifer => ('≈', Color::Rgb(0, 200, 220), Color::Rgb(0, 60, 80)),
                    ZTile::UndergroundRiver => ('~', Color::Rgb(100, 180, 255), Color::Rgb(20, 50, 100)),
                    ZTile::WaterCave | ZTile::CaveLake => ('○', Color::Rgb(0, 180, 180), Color::Rgb(0, 40, 60)),
                    ZTile::Spring | ZTile::Waterfall => ('◊', Color::Rgb(0, 255, 255), Color::Rgb(0, 80, 40)),
                    ZTile::MagmaPool => ('≈', Color::Rgb(255, 100, 0), Color::Rgb(80, 20, 0)),
                    ZTile::Surface | ZTile::Solid => {
                        let ch = if ztile == ZTile::Surface { '.' } else { '#' };
                        let (r, g, b) = moisture_color(moisture);
                        (ch, Color::Rgb(r, g, b), Color::Reset)
                    }
                    // Cave tiles in moisture mode - show with moisture coloring
                    _ => {
                        let ch = cave_tile_char(ztile);
                        let (r, g, b) = moisture_color(moisture);
                        (ch, Color::Rgb(r, g, b), Color::Rgb(15, 15, 20))
                    }
                }
            }
            ViewMode::Plates => {
                match ztile {
                    ZTile::Air => (' ', Color::Black, Color::Black),
                    ZTile::Water => ('~', Color::Rgb(50, 100, 200), Color::Reset),
                    ZTile::Aquifer => ('≈', Color::Rgb(0, 200, 220), Color::Rgb(0, 60, 80)),
                    ZTile::UndergroundRiver => ('~', Color::Rgb(100, 180, 255), Color::Rgb(20, 50, 100)),
                    ZTile::WaterCave | ZTile::CaveLake => ('○', Color::Rgb(0, 180, 180), Color::Rgb(0, 40, 60)),
                    ZTile::Spring | ZTile::Waterfall => ('◊', Color::Rgb(0, 255, 255), Color::Rgb(0, 80, 40)),
                    ZTile::MagmaPool => ('≈', Color::Rgb(255, 100, 0), Color::Rgb(80, 20, 0)),
                    ZTile::Surface | ZTile::Solid => {
                        let ch = if ztile == ZTile::Surface { '#' } else { '.' };
                        // Color by plate ID
                        let hue = (plate_id.0 as f32 * 137.5) % 360.0;
                        let (r, g, b) = hsv_to_rgb(hue, 0.7, 0.9);
                        (ch, Color::Rgb(r, g, b), Color::Reset)
                    }
                    // Cave tiles in plates mode - show with plate coloring
                    _ => {
                        let ch = cave_tile_char(ztile);
                        let hue = (plate_id.0 as f32 * 137.5) % 360.0;
                        let (r, g, b) = hsv_to_rgb(hue, 0.5, 0.7);
                        (ch, Color::Rgb(r, g, b), Color::Rgb(15, 15, 20))
                    }
                }
            }
            ViewMode::Stress => {
                match ztile {
                    ZTile::Air => (' ', Color::Black, Color::Black),
                    ZTile::Water => ('~', Color::Rgb(50, 100, 200), Color::Reset),
                    ZTile::Aquifer => ('≈', Color::Rgb(0, 200, 220), Color::Rgb(0, 60, 80)),
                    ZTile::UndergroundRiver => ('~', Color::Rgb(100, 180, 255), Color::Rgb(20, 50, 100)),
                    ZTile::WaterCave | ZTile::CaveLake => ('○', Color::Rgb(0, 180, 180), Color::Rgb(0, 40, 60)),
                    ZTile::Spring | ZTile::Waterfall => ('◊', Color::Rgb(0, 255, 255), Color::Rgb(0, 80, 40)),
                    ZTile::MagmaPool => ('≈', Color::Rgb(255, 100, 0), Color::Rgb(80, 20, 0)),
                    ZTile::Surface | ZTile::Solid => {
                        let ch = if ztile == ZTile::Surface { '.' } else { '#' };
                        let (r, g, b) = stress_color(stress);
                        (ch, Color::Rgb(r, g, b), Color::Reset)
                    }
                    // Cave tiles in stress mode - show with stress coloring
                    _ => {
                        let ch = cave_tile_char(ztile);
                        let (r, g, b) = stress_color(stress);
                        (ch, Color::Rgb(r, g, b), Color::Rgb(15, 15, 20))
                    }
                }
            }
            ViewMode::Factions => {
                // Color tiles by faction territory
                match ztile {
                    ZTile::Air => (' ', Color::Black, Color::Black),
                    ZTile::Water => ('~', Color::Rgb(50, 100, 200), Color::Reset),
                    _ => {
                        // Get faction color if available
                        if let Some(ref history) = self.world.history {
                            if let Some(faction) = history.faction_at(x, y) {
                                let (r, g, b) = faction.color;
                                let ch = if ztile == ZTile::Surface { '#' } else { cave_tile_char(ztile) };
                                (ch, Color::Rgb(r, g, b), Color::Reset)
                            } else {
                                // Unclaimed territory - gray
                                let ch = if ztile == ZTile::Surface { '.' } else { cave_tile_char(ztile) };
                                (ch, Color::Rgb(80, 80, 80), Color::Reset)
                            }
                        } else {
                            // No history - show as gray
                            ('.', Color::Rgb(80, 80, 80), Color::Reset)
                        }
                    }
                }
            }
            ViewMode::History => {
                // Highlight tiles with historical significance
                match ztile {
                    ZTile::Air => (' ', Color::Black, Color::Black),
                    ZTile::Water => ('~', Color::Rgb(50, 100, 200), Color::Reset),
                    _ => {
                        if let Some(ref history) = self.world.history {
                            let info = history.tile_info(x, y);
                            if info.settlement.is_some() {
                                // Settlement - bright yellow
                                ('*', Color::Rgb(255, 215, 0), Color::Reset)
                            } else if info.lair.is_some() {
                                // Monster lair - red
                                ('!', Color::Rgb(255, 50, 50), Color::Reset)
                            } else if !info.events.is_empty() {
                                // Historical event - purple
                                ('+', Color::Rgb(200, 100, 255), Color::Reset)
                            } else if info.trade_route {
                                // Trade route - orange
                                ('=', Color::Rgb(255, 165, 0), Color::Reset)
                            } else if info.resource.is_some() {
                                // Resource site - cyan
                                ('o', Color::Rgb(0, 200, 200), Color::Reset)
                            } else if info.faction.is_some() {
                                // Just territory - dim faction color
                                let ch = if ztile == ZTile::Surface { '.' } else { cave_tile_char(ztile) };
                                (ch, Color::Rgb(60, 60, 60), Color::Reset)
                            } else {
                                // No history
                                let ch = if ztile == ZTile::Surface { '.' } else { cave_tile_char(ztile) };
                                (ch, Color::Rgb(40, 40, 40), Color::Reset)
                            }
                        } else {
                            ('.', Color::Rgb(40, 40, 40), Color::Reset)
                        }
                    }
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
            "Z-Level Navigation:",
            "  > / . - Go up one Z-level",
            "  < / , - Go down one Z-level",
            "  0 - Go to sea level (Z=0)",
            "  S - Go to surface at cursor",
            "",
            "View Modes:",
            "  V - Cycle view mode (Biome/Height/Temp/Moisture/Plates/Stress)",
            "",
            "Other:",
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
}

/// Convert HSV to RGB
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

/// Get display character for cave tiles
fn cave_tile_char(tile: ZTile) -> char {
    match tile {
        ZTile::CaveFloor => '.',
        ZTile::CaveWall => '#',
        ZTile::Stalactite => '▼',
        ZTile::Stalagmite => '▲',
        ZTile::Pillar => '│',
        ZTile::Flowstone => '=',
        ZTile::FungalGrowth => '*',
        ZTile::GiantMushroom => '♠',
        ZTile::CrystalFormation => '◆',
        ZTile::CaveMoss => '\'',
        ZTile::MagmaPool => '≈',
        ZTile::MagmaTube => '○',
        ZTile::ObsidianFloor => '_',
        ZTile::CaveLake => '~',
        ZTile::Waterfall => '|',
        ZTile::RampUp => '↑',
        ZTile::RampDown => '↓',
        ZTile::RampBoth => '↕',
        // Structure tiles
        ZTile::StoneWall | ZTile::BrickWall | ZTile::WoodWall => '#',
        ZTile::RuinedWall => '%',
        ZTile::StoneFloor | ZTile::WoodFloor | ZTile::DirtFloor => '.',
        ZTile::CobblestoneFloor => ',',
        ZTile::Door => '+',
        ZTile::Window => '□',
        ZTile::StairsUp => '<',
        ZTile::StairsDown => '>',
        ZTile::Column => '│',
        ZTile::Rubble => '*',
        ZTile::Chest => '□',
        ZTile::Altar => '╥',
        ZTile::DirtRoad | ZTile::StoneRoad | ZTile::Bridge => '═',
        ZTile::MinedTunnel | ZTile::MinedRoom => '.',
        ZTile::MineSupport => '║',
        ZTile::Torch => '☼',
        // Artifact containers
        ZTile::ArtifactPedestal => '╦',
        ZTile::TreasureChest => '▣',
        ZTile::BookShelf => '▤',
        ZTile::RelicShrine => '╥',
        ZTile::ScrollCase => '▥',
        // Statues
        ZTile::HeroStatue | ZTile::RuinedStatue => '♀',
        // Dungeon markers
        ZTile::DungeonEntrance => '▼',
        ZTile::TreasureHoard => '$',
        _ => '?', // Fallback for any unexpected tile
    }
}

/// Get human-readable name for a ZTile
fn ztile_name(tile: ZTile) -> &'static str {
    match tile {
        ZTile::Air => "Air",
        ZTile::Surface => "Surface",
        ZTile::Solid => "Solid Rock",
        ZTile::Water => "Water",
        ZTile::Aquifer => "Aquifer",
        ZTile::UndergroundRiver => "Underground River",
        ZTile::WaterCave => "Water Cave",
        ZTile::Spring => "Spring",
        // Cave structure
        ZTile::CaveFloor => "Cave Floor",
        ZTile::CaveWall => "Cave Wall",
        // Speleothems
        ZTile::Stalactite => "Stalactite (▼)",
        ZTile::Stalagmite => "Stalagmite (▲)",
        ZTile::Pillar => "Pillar (│)",
        ZTile::Flowstone => "Flowstone (=)",
        // Cave biomes
        ZTile::FungalGrowth => "Glowing Fungi (*)",
        ZTile::GiantMushroom => "Giant Mushroom (♠)",
        ZTile::CrystalFormation => "Crystal (◆)",
        ZTile::CaveMoss => "Cave Moss (')",
        // Deep features
        ZTile::MagmaPool => "Magma Pool (≈)",
        ZTile::MagmaTube => "Lava Tube (○)",
        ZTile::ObsidianFloor => "Obsidian Floor (_)",
        // Water integration
        ZTile::CaveLake => "Cave Lake (~)",
        ZTile::Waterfall => "Waterfall (|)",
        // Vertical passages
        ZTile::RampUp => "Ramp Up (↑) - ascend",
        ZTile::RampDown => "Ramp Down (↓) - descend",
        ZTile::RampBoth => "Ramp (↕) - up/down",
        // Human-made structures
        ZTile::StoneWall => "Stone Wall (#)",
        ZTile::BrickWall => "Brick Wall (#)",
        ZTile::WoodWall => "Wood Wall (#)",
        ZTile::RuinedWall => "Ruined Wall (%)",
        ZTile::StoneFloor => "Stone Floor (.)",
        ZTile::WoodFloor => "Wood Floor (.)",
        ZTile::CobblestoneFloor => "Cobblestone (,)",
        ZTile::DirtFloor => "Dirt Floor (.)",
        ZTile::Door => "Door (+)",
        ZTile::Window => "Window (□)",
        ZTile::StairsUp => "Stairs Up (<)",
        ZTile::StairsDown => "Stairs Down (>)",
        ZTile::Column => "Column (│)",
        ZTile::Rubble => "Rubble (*)",
        ZTile::Chest => "Chest (□)",
        ZTile::Altar => "Altar (╥)",
        ZTile::DirtRoad => "Dirt Road (═)",
        ZTile::StoneRoad => "Stone Road (═)",
        ZTile::Bridge => "Bridge (═)",
        ZTile::MinedTunnel => "Mine Tunnel (.)",
        ZTile::MinedRoom => "Mine Chamber (.)",
        ZTile::MineSupport => "Mine Support (║)",
        ZTile::Torch => "Torch (☼)",
        ZTile::MineShaft => "Mine Shaft (○)",
        ZTile::MineLadder => "Mine Ladder (H)",
        ZTile::MineRails => "Mine Rails (═)",
        ZTile::OreVein => "Ore Vein (*)",
        ZTile::RichOreVein => "Rich Ore (◆)",
        ZTile::MineEntrance => "Mine Entrance (▼)",
        ZTile::FortressWall => "Fortress Wall (█)",
        ZTile::FortressFloor => "Fortress Floor (·)",
        ZTile::FortressGate => "Fortress Gate (‡)",
        ZTile::Vault => "Treasure Vault ($)",
        ZTile::BarracksFloor => "Barracks (░)",
        ZTile::ForgeFloor => "Forge (▒)",
        ZTile::Cistern => "Cistern (≈)",
        // Historical evidence tiles
        ZTile::BoneField => "Bone Field (☠)",
        ZTile::RustedWeapons => "Rusted Weapons (†)",
        ZTile::WarMemorial => "War Memorial (╬)",
        ZTile::Crater => "Crater (○)",
        ZTile::BoundaryStone => "Boundary Stone (◙)",
        ZTile::MileMarker => "Mile Marker (│)",
        ZTile::Shrine => "Shrine (╥)",
        ZTile::Statue => "Statue (♀)",
        ZTile::Obelisk => "Obelisk (↑)",
        ZTile::BoneNest => "Bone Nest (☠)",
        ZTile::WebCluster => "Web Cluster (▓)",
        ZTile::SlimeTrail => "Slime Trail (~)",
        ZTile::TerritoryMarking => "Territory Marking (!)",
        ZTile::AntMound => "Ant Mound (▲)",
        ZTile::BeeHive => "Bee Hive (◆)",
        ZTile::ClawMarks => "Claw Marks (≡)",
        ZTile::CursedGround => "Cursed Ground (†)",
        ZTile::CharredGround => "Charred Ground (░)",
        ZTile::AbandonedCart => "Abandoned Cart (□)",
        ZTile::WaystationRuin => "Waystation Ruin (■)",
        ZTile::DriedWell => "Dried Well (○)",
        ZTile::OvergrownGarden => "Overgrown Garden (♣)",
        ZTile::Gravestone => "Gravestone (†)",
        ZTile::Tomb => "Tomb (╬)",
        ZTile::Mausoleum => "Mausoleum (▓)",
        ZTile::Ossuary => "Ossuary (☠)",
        ZTile::MassGrave => "Mass Grave (▓)",
        // Artifact containers
        ZTile::ArtifactPedestal => "Artifact Pedestal (╦)",
        ZTile::TreasureChest => "Treasure Chest (▣)",
        ZTile::BookShelf => "Book Shelf (▤)",
        ZTile::RelicShrine => "Relic Shrine (╥)",
        ZTile::ScrollCase => "Scroll Case (▥)",
        // Statues
        ZTile::HeroStatue => "Hero Statue (♀)",
        ZTile::RuinedStatue => "Ruined Statue (♀)",
        // Dungeon markers
        ZTile::DungeonEntrance => "Dungeon Entrance (▼)",
        ZTile::TreasureHoard => "Treasure Hoard ($)",
    }
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

/// Export the entire map as a PNG image at the given Z-level
pub fn export_map_image(
    world: &WorldData,
    z: i32,
    view_mode: ViewMode,
    filename: &str,
) -> Result<(), Box<dyn Error>> {
    let width = world.heightmap.width;
    let height = world.heightmap.height;

    let mut img = ImageBuffer::new(width as u32, height as u32);

    for y in 0..height {
        for x in 0..width {
            let mut ztile = *world.zlevels.get(x, y, z);
            let surface_z = *world.surface_z.get(x, y);
            let biome = *world.biomes.get(x, y);
            let h = *world.heightmap.get(x, y);
            let temp = *world.temperature.get(x, y);
            let moisture = *world.moisture.get(x, y);
            let stress = *world.stress_map.get(x, y);
            let plate_id = *world.plate_map.get(x, y);

            // If current Z is empty (Air), find the highest visible tile below
            let mut display_z = z;
            if ztile == ZTile::Air {
                // Search downward from current z to find the first non-Air tile
                for check_z in (-16..=z).rev() {
                    let check_tile = *world.zlevels.get(x, y, check_z);
                    if check_tile != ZTile::Air {
                        ztile = check_tile;
                        display_z = check_z;
                        break;
                    }
                }
            }

            // Get the color based on view mode
            let (r, g, b) = match view_mode {
                ViewMode::Biome => {
                    // Use tile colors for biome view
                    let (_ch, fg, bg) = get_tile_color_for_export(ztile, surface_z, display_z, &biome, h);
                    // Blend fg and bg
                    let (fr, fg_g, fb) = color_to_rgb(fg);
                    let (br, bg_g, bb) = color_to_rgb(bg);
                    // Use foreground primarily, with bg influence
                    (
                        ((fr as u16 * 3 + br as u16) / 4) as u8,
                        ((fg_g as u16 * 3 + bg_g as u16) / 4) as u8,
                        ((fb as u16 * 3 + bb as u16) / 4) as u8,
                    )
                }
                ViewMode::Height => {
                    height_color(h)
                }
                ViewMode::Temperature => {
                    temperature_color(temp)
                }
                ViewMode::Moisture => {
                    moisture_color(moisture)
                }
                ViewMode::Stress => {
                    stress_color(stress)
                }
                ViewMode::Plates => {
                    // Simple plate coloring
                    let hue = (plate_id.0 as f32 * 37.0) % 360.0;
                    hsv_to_rgb(hue, 0.7, 0.8)
                }
                ViewMode::Factions => {
                    // Faction territory coloring (use gray for export since we don't have history access)
                    (80, 80, 80)
                }
                ViewMode::History => {
                    // History view (use gray for export)
                    (60, 60, 60)
                }
            };

            img.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
        }
    }

    img.save(filename)?;
    println!("Exported map to {}", filename);
    Ok(())
}

/// Export an aesthetic top-down view showing the highest tile at each position
/// This creates a satellite-like view of the world from above
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
            let surface_z = *world.surface_z.get(x, y);

            // Find the highest non-Air tile from top down
            let mut top_z = surface_z;
            let mut top_tile = ZTile::Air;
            for check_z in (zlevel::MIN_Z..=zlevel::MAX_Z).rev() {
                let tile = *world.zlevels.get(x, y, check_z);
                if tile != ZTile::Air {
                    top_z = check_z;
                    top_tile = tile;
                    break;
                }
            }

            // Get base color from the tile
            let (base_r, base_g, base_b) = get_topdown_tile_color(top_tile, &biome, h, top_z, surface_z);

            // Apply smooth hillshading using a larger kernel to avoid noise
            // Sample heights in a 5x5 area for smoother gradients
            let mut shade = 1.0f32;
            if x >= 2 && y >= 2 && x < width - 2 && y < height - 2 {
                // Average height to the top-left (light source direction)
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

                // Gentle shading based on slope
                let slope = (h_botright - h_topleft) / 200.0;
                shade = (1.0 + slope).clamp(0.85, 1.15);
            }

            // Subtle elevation-based brightness
            let elevation_factor = if h > 0.0 {
                // Land: very subtle height variation
                0.95 + (h / 4000.0).min(0.1)
            } else {
                // Water: slightly darker for depth
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

/// Get color for top-down aesthetic view
fn get_topdown_tile_color(
    tile: ZTile,
    biome: &crate::biomes::ExtendedBiome,
    height: f32,
    tile_z: i32,
    surface_z: i32,
) -> (u8, u8, u8) {
    match tile {
        // Natural terrain - use biome colors
        ZTile::Surface | ZTile::Air => biome.color(),

        // Water bodies
        ZTile::Water => {
            // Deeper water = darker blue
            let depth = (-height).max(0.0);
            let depth_factor = (1.0 - depth / 500.0).max(0.3);
            (
                (40.0 * depth_factor) as u8,
                (80.0 + 60.0 * depth_factor) as u8,
                (150.0 + 80.0 * depth_factor) as u8,
            )
        }

        // Underground solid (shouldn't appear in top-down, but just in case)
        ZTile::Solid => (80, 75, 70),

        // Structures - use distinctive colors
        ZTile::StoneWall | ZTile::FortressWall => (120, 115, 110),
        ZTile::BrickWall => (140, 90, 70),
        ZTile::WoodWall => (160, 120, 80),
        ZTile::RuinedWall | ZTile::Rubble => (90, 85, 80),

        ZTile::StoneFloor | ZTile::FortressFloor => (140, 135, 130),
        ZTile::WoodFloor => (170, 140, 100),
        ZTile::CobblestoneFloor => (130, 125, 120),
        ZTile::DirtFloor => (140, 120, 90),

        // Roads stand out
        ZTile::DirtRoad => (160, 140, 100),
        ZTile::StoneRoad => (170, 165, 160),
        ZTile::Bridge => (150, 110, 70),

        // Special structure features
        ZTile::Door | ZTile::FortressGate => (120, 80, 50),
        ZTile::Column => (180, 175, 170),
        ZTile::Chest | ZTile::Vault => (180, 150, 50),
        ZTile::Altar => (200, 200, 220),

        // Cave/underground features (if exposed)
        ZTile::CaveFloor => (100, 90, 80),
        ZTile::Aquifer => (80, 180, 200),
        ZTile::UndergroundRiver => (60, 140, 200),
        ZTile::MagmaPool => (255, 80, 0),

        // Mine features
        ZTile::MineEntrance => (90, 70, 50),
        ZTile::MineShaft | ZTile::MinedTunnel | ZTile::MinedRoom => (70, 60, 50),
        ZTile::MineLadder | ZTile::MineSupport => (120, 80, 40),
        ZTile::MineRails => (100, 100, 110),
        ZTile::OreVein => (180, 160, 80),
        ZTile::RichOreVein => (220, 180, 60),
        ZTile::ForgeFloor => (80, 60, 50),
        ZTile::BarracksFloor => (110, 100, 90),
        ZTile::Cistern => (60, 100, 140),

        // Cave formations
        ZTile::Stalactite | ZTile::Stalagmite | ZTile::Pillar | ZTile::Flowstone => (160, 150, 140),
        ZTile::CrystalFormation => (200, 180, 255),
        ZTile::GiantMushroom => (100, 255, 150),
        ZTile::FungalGrowth | ZTile::CaveMoss => (80, 120, 60),

        // Misc
        ZTile::Torch => (255, 200, 100),
        ZTile::Window => (180, 200, 220),
        ZTile::StairsUp | ZTile::StairsDown => (150, 145, 140),

        // Fallback
        _ => (128, 128, 128),
    }
}

/// Get tile color for export (simplified version of get_tile_display)
fn get_tile_color_for_export(
    ztile: ZTile,
    surface_z: i32,
    current_z: i32,
    biome: &crate::biomes::ExtendedBiome,
    height: f32,
) -> (char, Color, Color) {
    match ztile {
        ZTile::Air => (' ', Color::Black, Color::Black),
        ZTile::Water => ('~', Color::Rgb(100, 150, 255), Color::Rgb(20, 40, 80)),
        ZTile::Surface => {
            let (r, g, b) = biome.color();
            ('.', Color::Rgb(r, g, b), Color::Rgb(r / 2, g / 2, b / 2))
        }
        ZTile::Solid => {
            let depth_below = surface_z - current_z;
            let shade = (80 - depth_below * 4).max(30) as u8;
            ('#', Color::Rgb(shade, shade, shade), Color::Rgb(20, 20, 20))
        }
        ZTile::CaveFloor => ('.', Color::Rgb(100, 90, 80), Color::Rgb(30, 28, 25)),
        ZTile::Aquifer => ('≈', Color::Rgb(0, 200, 220), Color::Rgb(0, 60, 80)),
        ZTile::UndergroundRiver => ('~', Color::Rgb(100, 180, 255), Color::Rgb(20, 50, 100)),
        ZTile::MagmaPool => ('≈', Color::Rgb(255, 100, 0), Color::Rgb(80, 20, 0)),
        ZTile::StoneWall => ('#', Color::Rgb(140, 140, 145), Color::Rgb(50, 50, 55)),
        ZTile::BrickWall => ('#', Color::Rgb(160, 100, 80), Color::Rgb(60, 35, 25)),
        ZTile::WoodWall => ('#', Color::Rgb(180, 140, 100), Color::Rgb(70, 50, 35)),
        ZTile::StoneFloor => ('.', Color::Rgb(130, 130, 135), Color::Rgb(45, 45, 50)),
        ZTile::WoodFloor => ('.', Color::Rgb(160, 130, 90), Color::Rgb(55, 45, 30)),
        ZTile::CobblestoneFloor => (',', Color::Rgb(120, 115, 110), Color::Rgb(40, 38, 35)),
        ZTile::MinedTunnel => ('.', Color::Rgb(100, 80, 60), Color::Rgb(30, 25, 20)),
        ZTile::MinedRoom => ('.', Color::Rgb(110, 90, 70), Color::Rgb(35, 28, 22)),
        ZTile::MineShaft => ('○', Color::Rgb(80, 60, 40), Color::Rgb(20, 15, 10)),
        ZTile::OreVein => ('*', Color::Rgb(180, 140, 80), Color::Rgb(60, 45, 25)),
        ZTile::RichOreVein => ('◆', Color::Rgb(255, 215, 0), Color::Rgb(80, 60, 20)),
        ZTile::FortressWall => ('█', Color::Rgb(80, 80, 90), Color::Rgb(40, 40, 50)),
        ZTile::FortressFloor => ('·', Color::Rgb(100, 100, 110), Color::Rgb(35, 35, 45)),
        ZTile::DirtRoad | ZTile::StoneRoad => ('═', Color::Rgb(140, 120, 80), Color::Rgb(50, 40, 25)),
        _ => {
            // Default for other tiles
            let (r, g, b) = height_color(height);
            ('.', Color::Rgb(r, g, b), Color::Rgb(r / 2, g / 2, b / 2))
        }
    }
}

/// Run the explorer
pub fn run_explorer(world: WorldData) -> Result<(), Box<dyn Error>> {
    // Setup terminal
    terminal::enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut explorer = Explorer::new(world);

    loop {
        // Render
        terminal.draw(|f| {
            let size = f.area();

            // Main layout: map area + status bar
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Min(1),
                    Constraint::Length(1),
                ])
                .split(size);

            let map_area = chunks[0];
            let status_area = chunks[1];

            // Render map
            explorer.render_map(map_area, f.buffer_mut());

            // Render status bar
            let zoom_str = if explorer.zoom > 1 { format!(" | Zoom:{}x", explorer.zoom) } else { String::new() };
            let msg_str = explorer.message.as_ref().map(|m| format!(" | {}", m)).unwrap_or_default();
            let status = format!(
                " {} | {}{} | {}{} | -/+ Zoom | F Fit | E Export | T TopDown | R New | Q Quit",
                explorer.z_level_status(),
                explorer.view_mode.name(),
                zoom_str,
                explorer.tile_info(),
                msg_str,
            );
            let status_para = Paragraph::new(status)
                .style(Style::default().bg(Color::DarkGray).fg(Color::White));
            f.render_widget(status_para, status_area);

            // Clear message after display
            explorer.message = None;

            // Render help if active
            if explorer.show_help {
                explorer.render_help(map_area, f.buffer_mut());
            }
        })?;

        // Handle input
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => {
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

                        // Z-level navigation
                        KeyCode::Char('>') | KeyCode::Char('.') => explorer.move_z_up(),
                        KeyCode::Char('<') | KeyCode::Char(',') => explorer.move_z_down(),
                        KeyCode::Char('0') => explorer.go_to_sea_level(),
                        KeyCode::Char('S') => explorer.go_to_surface(),

                        // Zoom controls
                        KeyCode::Char('-') | KeyCode::Char('_') => explorer.zoom_out(),
                        KeyCode::Char('+') | KeyCode::Char('=') => explorer.zoom_in(),
                        KeyCode::Char('f') | KeyCode::Char('F') => {
                            let size = terminal.size()?;
                            explorer.fit_to_screen(size.width as usize, (size.height - 1) as usize);
                        }

                        // Export image
                        KeyCode::Char('e') | KeyCode::Char('E') => {
                            let filename = format!("world_z{}.png", explorer.cursor_z);
                            match export_map_image(&explorer.world, explorer.cursor_z, explorer.view_mode, &filename) {
                                Ok(_) => explorer.message = Some(format!("Exported: {}", filename)),
                                Err(e) => explorer.message = Some(format!("Export failed: {}", e)),
                            }
                        }

                        // Regenerate world with new seed
                        KeyCode::Char('r') | KeyCode::Char('R') => {
                            explorer.regenerate();
                        }

                        // Top-down aesthetic export
                        KeyCode::Char('t') | KeyCode::Char('T') => {
                            let filename = format!("world_topdown_{}.png", explorer.world.seed);
                            match export_topdown_image(&explorer.world, &filename) {
                                Ok(_) => explorer.message = Some(format!("Exported: {}", filename)),
                                Err(e) => explorer.message = Some(format!("Export failed: {}", e)),
                            }
                        }

                        _ => {}
                    }
                }
                Event::Mouse(MouseEvent { kind: MouseEventKind::Down(MouseButton::Left), column, row, .. }) => {
                    // Click to move cursor
                    let size = terminal.size()?;
                    if row < size.height - 1 {
                        let view_width = size.width as usize;
                        let view_height = (size.height - 1) as usize;

                        let start_x = if explorer.cursor_x >= view_width / 2 {
                            explorer.cursor_x - view_width / 2
                        } else {
                            0
                        };
                        let start_y = if explorer.cursor_y >= view_height / 2 {
                            explorer.cursor_y - view_height / 2
                        } else {
                            0
                        };

                        let new_x = (start_x + column as usize) % explorer.world.heightmap.width;
                        let new_y = (start_y + row as usize).min(explorer.world.heightmap.height - 1);

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
