//! Terminal-based world explorer using ratatui
//!
//! Roguelike-style terminal interface for exploring generated worlds.
//! Navigate with arrow keys or mouse, inspect tiles, change view modes.

use std::io::{self, stdout};
use std::error::Error;

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

use crate::ascii::{biome_char, AsciiMode, height_color, temperature_color, moisture_color, stress_color};
use crate::biomes::ExtendedBiome;
use crate::plates::PlateType;
use crate::world::{WorldData, generate_world};

/// Viewport for rendering a portion of the map
struct Viewport {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

/// Terminal explorer state
pub struct Explorer {
    world: WorldData,
    cursor_x: usize,
    cursor_y: usize,
    viewport: Viewport,
    view_mode: AsciiMode,
    running: bool,
    show_help: bool,
}

impl Explorer {
    pub fn new(world: WorldData) -> Self {
        let cursor_x = world.width / 2;
        let cursor_y = world.height / 2;

        Self {
            world,
            cursor_x,
            cursor_y,
            viewport: Viewport {
                x: 0,
                y: 0,
                width: 80,
                height: 20,
            },
            view_mode: AsciiMode::Biome,
            running: true,
            show_help: false,
        }
    }

    /// Regenerate the world with a new random seed
    pub fn regenerate_random(&mut self) {
        let new_seed: u64 = rand::random();
        let width = self.world.width;
        let height = self.world.height;

        self.world = generate_world(width, height, new_seed);

        // Reset cursor to center
        self.cursor_x = width / 2;
        self.cursor_y = height / 2;
        self.center_viewport();
    }

    pub fn run(&mut self) -> Result<(), Box<dyn Error>> {
        // Setup terminal
        terminal::enable_raw_mode()?;
        let mut stdout = stdout();
        execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
        let backend = CrosstermBackend::new(stdout);
        let mut terminal = Terminal::new(backend)?;

        // Center viewport on cursor
        self.center_viewport();

        // Main loop
        while self.running {
            // Get terminal size and update viewport
            let size = terminal.size()?;
            self.viewport.width = (size.width as usize).saturating_sub(4).min(self.world.width);
            self.viewport.height = (size.height as usize).saturating_sub(14).min(self.world.height);

            // Render
            terminal.draw(|frame| self.render(frame))?;

            // Handle input
            if event::poll(std::time::Duration::from_millis(50))? {
                match event::read()? {
                    Event::Key(key) => self.handle_key_input(key),
                    Event::Mouse(mouse) => self.handle_mouse_input(mouse),
                    Event::Resize(_, _) => {
                        self.adjust_viewport();
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

    fn center_viewport(&mut self) {
        let half_w = self.viewport.width / 2;
        let half_h = self.viewport.height / 2;

        self.viewport.x = self.cursor_x.saturating_sub(half_w);
        self.viewport.y = self.cursor_y.saturating_sub(half_h);

        self.clamp_viewport();
    }

    fn clamp_viewport(&mut self) {
        if self.viewport.x + self.viewport.width > self.world.width {
            self.viewport.x = self.world.width.saturating_sub(self.viewport.width);
        }
        if self.viewport.y + self.viewport.height > self.world.height {
            self.viewport.y = self.world.height.saturating_sub(self.viewport.height);
        }
    }

    fn adjust_viewport(&mut self) {
        let margin = 3;

        if self.cursor_x < self.viewport.x + margin {
            self.viewport.x = self.cursor_x.saturating_sub(margin);
        }
        if self.cursor_x >= self.viewport.x + self.viewport.width - margin {
            self.viewport.x = self.cursor_x.saturating_sub(self.viewport.width - margin - 1);
        }
        if self.cursor_y < self.viewport.y + margin {
            self.viewport.y = self.cursor_y.saturating_sub(margin);
        }
        if self.cursor_y >= self.viewport.y + self.viewport.height - margin {
            self.viewport.y = self.cursor_y.saturating_sub(self.viewport.height - margin - 1);
        }

        self.clamp_viewport();
    }

    fn handle_key_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('q') | KeyCode::Esc => self.running = false,

            KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('k') => {
                if self.cursor_y > 0 {
                    self.cursor_y -= 1;
                    self.adjust_viewport();
                }
            }
            KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('j') => {
                if self.cursor_y < self.world.height - 1 {
                    self.cursor_y += 1;
                    self.adjust_viewport();
                }
            }
            KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('h') => {
                if self.cursor_x > 0 {
                    self.cursor_x -= 1;
                    self.adjust_viewport();
                }
            }
            KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('l') => {
                if self.cursor_x < self.world.width - 1 {
                    self.cursor_x += 1;
                    self.adjust_viewport();
                }
            }

            KeyCode::PageUp => {
                self.cursor_y = self.cursor_y.saturating_sub(10);
                self.adjust_viewport();
            }
            KeyCode::PageDown => {
                self.cursor_y = (self.cursor_y + 10).min(self.world.height - 1);
                self.adjust_viewport();
            }
            KeyCode::Home => {
                self.cursor_x = self.cursor_x.saturating_sub(10);
                self.adjust_viewport();
            }
            KeyCode::End => {
                self.cursor_x = (self.cursor_x + 10).min(self.world.width - 1);
                self.adjust_viewport();
            }

            KeyCode::Char('v') => self.cycle_view_mode(),
            KeyCode::Char('?') | KeyCode::F(1) => self.show_help = !self.show_help,
            KeyCode::Char('c') => self.center_viewport(),
            KeyCode::Char('n') => self.regenerate_random(),
            KeyCode::Char('0') => {
                self.cursor_x = self.world.width / 2;
                self.cursor_y = self.world.height / 2;
                self.center_viewport();
            }
            _ => {}
        }
    }

    fn handle_mouse_input(&mut self, mouse: MouseEvent) {
        // Map area starts at row 2, column 1 (inside the border)
        const MAP_START_ROW: u16 = 2;
        const MAP_START_COL: u16 = 1;

        match mouse.kind {
            MouseEventKind::Down(MouseButton::Left) | MouseEventKind::Drag(MouseButton::Left) => {
                if mouse.row >= MAP_START_ROW && mouse.column >= MAP_START_COL {
                    let vx = (mouse.column - MAP_START_COL) as usize;
                    let vy = (mouse.row - MAP_START_ROW) as usize;

                    if vx < self.viewport.width && vy < self.viewport.height {
                        let world_x = self.viewport.x + vx;
                        let world_y = self.viewport.y + vy;

                        if world_x < self.world.width && world_y < self.world.height {
                            self.cursor_x = world_x;
                            self.cursor_y = world_y;
                        }
                    }
                }
            }
            MouseEventKind::Down(MouseButton::Right) => {
                if mouse.row >= MAP_START_ROW && mouse.column >= MAP_START_COL {
                    let vx = (mouse.column - MAP_START_COL) as usize;
                    let vy = (mouse.row - MAP_START_ROW) as usize;

                    if vx < self.viewport.width && vy < self.viewport.height {
                        let world_x = self.viewport.x + vx;
                        let world_y = self.viewport.y + vy;

                        if world_x < self.world.width && world_y < self.world.height {
                            self.cursor_x = world_x;
                            self.cursor_y = world_y;
                            self.center_viewport();
                        }
                    }
                }
            }
            MouseEventKind::ScrollUp => {
                self.viewport.y = self.viewport.y.saturating_sub(3);
                if self.cursor_y >= self.viewport.y + self.viewport.height {
                    self.cursor_y = self.viewport.y + self.viewport.height - 1;
                }
            }
            MouseEventKind::ScrollDown => {
                let max_y = self.world.height.saturating_sub(self.viewport.height);
                self.viewport.y = (self.viewport.y + 3).min(max_y);
                if self.cursor_y < self.viewport.y {
                    self.cursor_y = self.viewport.y;
                }
            }
            _ => {}
        }
    }

    fn cycle_view_mode(&mut self) {
        self.view_mode = match self.view_mode {
            AsciiMode::Biome => AsciiMode::Height,
            AsciiMode::Height => AsciiMode::Temperature,
            AsciiMode::Temperature => AsciiMode::Moisture,
            AsciiMode::Moisture => AsciiMode::Stress,
            AsciiMode::Stress => AsciiMode::Plates,
            AsciiMode::Plates => AsciiMode::Biome,
        };
    }

    /// Convert our color tuple to ratatui Color
    fn to_ratatui_color(rgb: (u8, u8, u8)) -> Color {
        Color::Rgb(rgb.0, rgb.1, rgb.2)
    }

    /// Get colors for a tile based on view mode
    fn get_tile_colors(&self, world_x: usize, world_y: usize) -> (Color, Color) {
        let biome = *self.world.biomes.get(world_x, world_y);

        match self.view_mode {
            AsciiMode::Biome => {
                let (r, g, b) = biome.color();
                let bg = Color::Rgb(
                    (r as f32 * 0.6) as u8,
                    (g as f32 * 0.6) as u8,
                    (b as f32 * 0.6) as u8,
                );
                let luminance = 0.299 * r as f32 + 0.587 * g as f32 + 0.114 * b as f32;
                let fg = if luminance > 128.0 {
                    Color::Rgb(
                        r.saturating_add(80).min(255),
                        g.saturating_add(80).min(255),
                        b.saturating_add(80).min(255),
                    )
                } else {
                    Color::Rgb(
                        (r as f32 * 1.5).min(255.0) as u8,
                        (g as f32 * 1.5).min(255.0) as u8,
                        (b as f32 * 1.5).min(255.0) as u8,
                    )
                };
                (fg, bg)
            }
            AsciiMode::Height => {
                let elev = *self.world.heightmap.get(world_x, world_y);
                let c = height_color(elev);
                (Self::to_ratatui_color(c), Color::Rgb(c.0 / 2, c.1 / 2, c.2 / 2))
            }
            AsciiMode::Temperature => {
                let temp = *self.world.temperature.get(world_x, world_y);
                let c = temperature_color(temp);
                (Self::to_ratatui_color(c), Color::Rgb(c.0 / 2, c.1 / 2, c.2 / 2))
            }
            AsciiMode::Moisture => {
                let moist = *self.world.moisture.get(world_x, world_y);
                let c = moisture_color(moist);
                (Self::to_ratatui_color(c), Color::Rgb(c.0 / 2, c.1 / 2, c.2 / 2))
            }
            AsciiMode::Stress => {
                let stress = *self.world.stress_map.get(world_x, world_y);
                let c = stress_color(stress);
                (Self::to_ratatui_color(c), Color::Rgb(c.0 / 2, c.1 / 2, c.2 / 2))
            }
            AsciiMode::Plates => {
                let plate_id = self.world.plate_map.get(world_x, world_y).0 as usize;
                let colors: [(u8, u8, u8); 16] = [
                    (230, 25, 75), (60, 180, 75), (255, 225, 25), (0, 130, 200),
                    (245, 130, 48), (145, 30, 180), (70, 240, 240), (240, 50, 230),
                    (210, 245, 60), (250, 190, 212), (0, 128, 128), (220, 190, 255),
                    (170, 110, 40), (255, 250, 200), (128, 0, 0), (170, 255, 195),
                ];
                let c = colors[plate_id % colors.len()];
                (Color::Rgb(c.0, c.1, c.2), Color::Rgb(c.0 / 3, c.1 / 3, c.2 / 3))
            }
        }
    }

    fn render(&self, frame: &mut Frame) {
        let size = frame.area();

        // Layout: header, map, info panel, controls
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),  // Header
                Constraint::Min(10),    // Map
                Constraint::Length(5),  // Info panel
                Constraint::Length(1),  // Controls
            ])
            .split(size);

        // Header
        let header = Paragraph::new(format!(
            "PLANET EXPLORER - Seed: {}  Size: {}x{} ({:.0} x {:.0} km)  [?] Help  [Q] Quit",
            self.world.seed,
            self.world.width,
            self.world.height,
            self.world.map_size_km().0,
            self.world.map_size_km().1,
        ))
        .style(Style::default().fg(Color::Cyan));
        frame.render_widget(header, chunks[0]);

        // Map area with border
        let map_block = Block::default()
            .borders(Borders::ALL)
            .title(format!(" Map - {} ", self.view_mode.name()));
        let map_inner = map_block.inner(chunks[1]);
        frame.render_widget(map_block, chunks[1]);

        // Render map tiles directly to buffer
        let map_width = map_inner.width as usize;
        let map_height = map_inner.height as usize;

        for vy in 0..map_height.min(self.viewport.height) {
            let world_y = self.viewport.y + vy;
            if world_y >= self.world.height {
                break;
            }

            for vx in 0..map_width.min(self.viewport.width) {
                let world_x = self.viewport.x + vx;
                if world_x >= self.world.width {
                    break;
                }

                let biome = *self.world.biomes.get(world_x, world_y);
                let ch = biome_char(&biome);
                let is_cursor = world_x == self.cursor_x && world_y == self.cursor_y;

                let (fg, bg) = if is_cursor {
                    (Color::Black, Color::Yellow)
                } else {
                    self.get_tile_colors(world_x, world_y)
                };

                let x = map_inner.x + vx as u16;
                let y = map_inner.y + vy as u16;

                if x < map_inner.x + map_inner.width && y < map_inner.y + map_inner.height {
                    frame.buffer_mut().set_string(
                        x, y,
                        ch.to_string(),
                        Style::default().fg(fg).bg(bg),
                    );
                }
            }
        }

        // Info panel
        let tile = self.world.get_tile_info(self.cursor_x, self.cursor_y);
        let (km_x, km_y) = self.world.get_physical_coords(self.cursor_x, self.cursor_y);

        let plate_info = if !tile.plate_id.is_none() {
            let plate = &self.world.plates[tile.plate_id.0 as usize];
            let ptype = if plate.plate_type == PlateType::Continental { "Cont" } else { "Ocean" };
            format!("{} #{}", ptype, tile.plate_id.0)
        } else {
            "None".to_string()
        };

        let (br, bg, bb) = tile.biome.color();
        let biome_style = Style::default()
            .fg(Color::Rgb(br, bg, bb))
            .add_modifier(Modifier::BOLD);

        // Water body color based on type
        let water_color = match tile.water_body_type {
            crate::water_bodies::WaterBodyType::Ocean => Color::Blue,
            crate::water_bodies::WaterBodyType::Lake => Color::Cyan,
            crate::water_bodies::WaterBodyType::River => Color::LightBlue,
            crate::water_bodies::WaterBodyType::None => Color::DarkGray,
        };

        let info_text = vec![
            Line::from(vec![
                Span::raw("Cursor: "),
                Span::styled(format!("({}, {})", self.cursor_x, self.cursor_y), Style::default().fg(Color::White)),
                Span::raw("  Pos: "),
                Span::styled(format!("{:.0} km E, {:.0} km S", km_x, km_y), Style::default().fg(Color::White)),
            ]),
            Line::from(vec![
                Span::raw("Biome: "),
                Span::styled(format!("{} {}", biome_char(&tile.biome), tile.biome.display_name()), biome_style),
                Span::raw("  Water: "),
                Span::styled(tile.water_body_str(), Style::default().fg(water_color)),
            ]),
            Line::from(vec![
                Span::raw("Elev: "),
                Span::styled(tile.elevation_str(), Style::default().fg(Color::Yellow)),
                Span::raw("  Temp: "),
                Span::styled(tile.temperature_str(), Style::default().fg(Color::Red)),
                Span::raw("  Moist: "),
                Span::styled(tile.moisture_str(), Style::default().fg(Color::Blue)),
            ]),
        ];

        let info_panel = Paragraph::new(info_text)
            .block(Block::default().borders(Borders::ALL).title(" Tile Info "));
        frame.render_widget(info_panel, chunks[2]);

        // Controls
        let controls = Paragraph::new("[←↑↓→/WASD] Move  [Click] Select  [V] View  [N] New Seed  [C] Center  [?] Help  [Q] Quit")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(controls, chunks[3]);

        // Help overlay
        if self.show_help {
            self.render_help(frame);
        }
    }

    fn render_help(&self, frame: &mut Frame) {
        let area = frame.area();

        // Center the help popup
        let popup_width = 44;
        let popup_height = 20;
        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

        // Clear the area behind the popup
        frame.render_widget(Clear, popup_area);

        let help_text = vec![
            Line::from("Keyboard:").style(Style::default().add_modifier(Modifier::BOLD)),
            Line::from("  Arrow keys / WASD / HJKL - Move"),
            Line::from("  PgUp/PgDn - Fast vertical"),
            Line::from("  Home/End  - Fast horizontal"),
            Line::from(""),
            Line::from("Mouse:").style(Style::default().add_modifier(Modifier::BOLD)),
            Line::from("  Left click  - Move cursor to tile"),
            Line::from("  Right click - Center on tile"),
            Line::from("  Scroll      - Pan the viewport"),
            Line::from("  Drag        - Select tiles"),
            Line::from(""),
            Line::from("Views (V to cycle):").style(Style::default().add_modifier(Modifier::BOLD)),
            Line::from("  Biome / Height / Temp / Moisture / Stress"),
            Line::from(""),
            Line::from("Other:").style(Style::default().add_modifier(Modifier::BOLD)),
            Line::from("  N - New random seed (regenerate)"),
            Line::from("  C - Center viewport on cursor"),
            Line::from("  0 - Jump to map center"),
            Line::from("  ? - Toggle this help"),
            Line::from("  Q/Esc - Quit"),
        ];

        let help_popup = Paragraph::new(help_text)
            .block(Block::default()
                .borders(Borders::ALL)
                .title(" Help ")
                .style(Style::default().bg(Color::DarkGray)))
            .style(Style::default().fg(Color::White).bg(Color::DarkGray));

        frame.render_widget(help_popup, popup_area);
    }
}

/// Run the terminal explorer
pub fn run_explorer(world: WorldData) -> Result<(), Box<dyn Error>> {
    let mut explorer = Explorer::new(world);
    explorer.run()
}
