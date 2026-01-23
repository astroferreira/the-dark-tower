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
    Rivers,
    BiomeBlend,
    Coastline,
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
            ViewMode::Rivers => "Rivers",
            ViewMode::BiomeBlend => "BiomeBlend",
            ViewMode::Coastline => "Coastline",
        }
    }

    fn next(&self) -> ViewMode {
        match self {
            ViewMode::Biome => ViewMode::Height,
            ViewMode::Height => ViewMode::Temperature,
            ViewMode::Temperature => ViewMode::Moisture,
            ViewMode::Moisture => ViewMode::Plates,
            ViewMode::Plates => ViewMode::Stress,
            ViewMode::Stress => ViewMode::Rivers,
            ViewMode::Rivers => ViewMode::BiomeBlend,
            ViewMode::BiomeBlend => ViewMode::Coastline,
            ViewMode::Coastline => ViewMode::Biome,
        }
    }
}

/// Explorer state
struct Explorer {
    world: WorldData,
    cursor_x: usize,
    cursor_y: usize,
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

        Explorer {
            world,
            cursor_x,
            cursor_y,
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

        // Reset cursor to center of map
        self.cursor_x = width / 2;
        self.cursor_y = height / 2;
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

                buf.get_mut(screen_x, screen_y).set_char(ch).set_style(style);
            }
        }
    }

    /// Get display character and colors for a tile
    fn get_tile_display(&self, x: usize, y: usize) -> (char, Color, Color) {
        let biome = *self.world.biomes.get(x, y);
        let height = *self.world.heightmap.get(x, y);
        let temp = *self.world.temperature.get(x, y);
        let moisture = *self.world.moisture.get(x, y);
        let stress = *self.world.stress_map.get(x, y);
        let plate_id = *self.world.plate_map.get(x, y);

        match self.view_mode {
            ViewMode::Biome => {
                let ch = biome_char(&biome);
                let (r, g, b) = biome.color();
                (ch, Color::Rgb(r, g, b), Color::Reset)
            }
            ViewMode::Height => {
                let ch = if height < 0.0 { '~' } else { '.' };
                let (r, g, b) = height_color(height);
                (ch, Color::Rgb(r, g, b), Color::Reset)
            }
            ViewMode::Temperature => {
                let ch = if height < 0.0 { '~' } else { '.' };
                let (r, g, b) = temperature_color(temp);
                (ch, Color::Rgb(r, g, b), Color::Reset)
            }
            ViewMode::Moisture => {
                let ch = if height < 0.0 { '~' } else { '.' };
                let (r, g, b) = moisture_color(moisture);
                (ch, Color::Rgb(r, g, b), Color::Reset)
            }
            ViewMode::Plates => {
                let ch = if height < 0.0 { '~' } else { '.' };
                let plate_idx = plate_id.0 as usize;
                if plate_idx < self.world.plates.len() {
                    let [r, g, b] = self.world.plates[plate_idx].color;
                    (ch, Color::Rgb(r, g, b), Color::Reset)
                } else {
                    (ch, Color::DarkGray, Color::Reset)
                }
            }
            ViewMode::Stress => {
                let ch = if height < 0.0 { '~' } else { '.' };
                let (r, g, b) = stress_color(stress);
                (ch, Color::Rgb(r, g, b), Color::Reset)
            }
            ViewMode::Rivers => {
                if height < 0.0 {
                    // Water
                    if let Some(ref river_network) = self.world.river_network {
                        let width = river_network.get_width_at(x as f32, y as f32, 2.0);
                        if width > 0.0 {
                            let intensity = (width * 30.0).min(255.0) as u8;
                            ('~', Color::Rgb(50, intensity, 255), Color::Rgb(0, 20, intensity / 2))
                        } else {
                            ('~', Color::Rgb(50, 100, 200), Color::Reset)
                        }
                    } else {
                        ('~', Color::Rgb(50, 100, 200), Color::Reset)
                    }
                } else {
                    // Land - check for rivers
                    if let Some(ref river_network) = self.world.river_network {
                        let width = river_network.get_width_at(x as f32, y as f32, 1.0);
                        if width > 0.0 {
                            ('~', Color::Rgb(0, 200, 255), Color::Reset)
                        } else {
                            let (r, g, b) = height_color(height);
                            ('.', Color::Rgb(r, g, b), Color::Reset)
                        }
                    } else {
                        let (r, g, b) = height_color(height);
                        ('.', Color::Rgb(r, g, b), Color::Reset)
                    }
                }
            }
            ViewMode::BiomeBlend => {
                if height < 0.0 {
                    ('~', Color::Rgb(50, 100, 200), Color::Reset)
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
                        ('*', Color::Rgb(255, 165, 0), Color::Reset)
                    } else {
                        let (r, g, b) = biome.color();
                        ('.', Color::Rgb(r, g, b), Color::Reset)
                    }
                }
            }
            ViewMode::Coastline => {
                if height < 0.0 {
                    // Water - check if coastal
                    let is_coastal = self.world.heightmap.neighbors_8(x, y).into_iter().any(|(nx, ny)| {
                        *self.world.heightmap.get(nx, ny) >= 0.0
                    });
                    if is_coastal {
                        ('~', Color::Rgb(0, 200, 200), Color::Rgb(0, 50, 50))
                    } else {
                        ('~', Color::Rgb(50, 100, 200), Color::Reset)
                    }
                } else {
                    // Land - check if coastal
                    let is_coastal = self.world.heightmap.neighbors_8(x, y).into_iter().any(|(nx, ny)| {
                        *self.world.heightmap.get(nx, ny) < 0.0
                    });
                    if is_coastal {
                        ('#', Color::Rgb(255, 255, 100), Color::Reset)
                    } else if height < 50.0 {
                        ('.', Color::Rgb(200, 150, 100), Color::Reset)
                    } else {
                        let (r, g, b) = height_color(height);
                        ('.', Color::Rgb(r, g, b), Color::Reset)
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
            "View Modes (V to cycle):",
            "  Biome, Height, Temperature, Moisture,",
            "  Plates, Stress, Rivers, BiomeBlend, Coastline",
            "",
            "Zoom:",
            "  +/- - Zoom in/out",
            "  F - Fit map to screen",
            "",
            "Other:",
            "  R - Regenerate world (new seed)",
            "  E - Export current view as PNG",
            "  T - Export top-down view as PNG",
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

            let (r, g, b) = match view_mode {
                ViewMode::Biome => biome.color(),
                ViewMode::Height => height_color(h),
                ViewMode::Temperature => temperature_color(temp),
                ViewMode::Moisture => moisture_color(moisture),
                ViewMode::Stress => stress_color(stress),
                ViewMode::Plates => {
                    let hue = (plate_id.0 as f32 * 37.0) % 360.0;
                    hsv_to_rgb(hue, 0.7, 0.8)
                }
                ViewMode::Rivers => {
                    if h < 0.0 {
                        (50, 100, 200)
                    } else {
                        height_color(h)
                    }
                }
                ViewMode::BiomeBlend => biome.color(),
                ViewMode::Coastline => {
                    if h < 0.0 {
                        if h > -50.0 { (0, 200, 200) } else { (50, 100, 200) }
                    } else if h < 50.0 {
                        (255, 255, 100)
                    } else {
                        height_color(h)
                    }
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
                " W:({},{}) | {} | {}{}{} | V:View  ?:Help  Q:Quit",
                explorer.cursor_x,
                explorer.cursor_y,
                explorer.view_mode.name(),
                explorer.tile_info(),
                zoom_str,
                msg_str,
            );
            let status_para = Paragraph::new(status)
                .style(Style::default().bg(Color::DarkGray).fg(Color::White));
            f.render_widget(status_para, status_area);

            // Render help if active
            if explorer.show_help {
                explorer.render_help(map_area, f.buffer_mut());
            }
        })?;

        // Clear message after display
        explorer.message = None;

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

                        // Zoom controls
                        KeyCode::Char('-') | KeyCode::Char('_') => explorer.zoom_out(),
                        KeyCode::Char('+') | KeyCode::Char('=') => explorer.zoom_in(),
                        KeyCode::Char('f') | KeyCode::Char('F') => {
                            let size = terminal.size()?;
                            explorer.fit_to_screen(size.width as usize, (size.height - 1) as usize);
                        }

                        // Export image
                        KeyCode::Char('e') | KeyCode::Char('E') => {
                            let filename = format!("world_{}.png", explorer.world.seed);
                            match export_map_image(&explorer.world, explorer.view_mode, &filename) {
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
