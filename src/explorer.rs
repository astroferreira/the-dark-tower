//! Terminal-based world explorer using ratatui
//!
//! Roguelike-style terminal interface for exploring generated worlds.
//! Navigate with arrow keys or mouse, inspect tiles, change view modes.
//! Includes civilization simulation mode for watching tribes evolve.

use std::io::{self, stdout};
use std::error::Error;
use std::time::{Duration, Instant};

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, MouseEvent, MouseEventKind, MouseButton, EnableMouseCapture, DisableMouseCapture},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph, Clear, Gauge},
    style::{Color, Style, Modifier},
};
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;

use crate::ascii::{biome_char, AsciiMode, height_color, temperature_color, moisture_color, stress_color};
use crate::biomes::ExtendedBiome;
use crate::local::{generate_local_map_default, LocalMap, LocalMapCache};
use crate::plates::PlateType;
use crate::world::{WorldData, generate_world};
use crate::simulation::{SimulationState, SimulationParams, TileCoord, TribeId};
use crate::simulation::types::{GlobalLocalCoord, LOCAL_MAP_SIZE};

/// Viewport for rendering a portion of the map
struct Viewport {
    x: usize,
    y: usize,
    width: usize,
    height: usize,
}

/// Simulation speed settings
#[derive(Clone, Copy, PartialEq)]
enum SimSpeed {
    Paused,
    Slow,      // 1 tick per 2 seconds
    Normal,    // 1 tick per 500ms
    Fast,      // 1 tick per 100ms
    VeryFast,  // 1 tick per 20ms
}

impl SimSpeed {
    fn tick_interval(&self) -> Option<Duration> {
        match self {
            SimSpeed::Paused => None,
            SimSpeed::Slow => Some(Duration::from_millis(2000)),
            SimSpeed::Normal => Some(Duration::from_millis(500)),
            SimSpeed::Fast => Some(Duration::from_millis(100)),
            SimSpeed::VeryFast => Some(Duration::from_millis(20)),
        }
    }

    fn name(&self) -> &'static str {
        match self {
            SimSpeed::Paused => "Paused",
            SimSpeed::Slow => "Slow",
            SimSpeed::Normal => "Normal",
            SimSpeed::Fast => "Fast",
            SimSpeed::VeryFast => "Very Fast",
        }
    }

    fn next(&self) -> SimSpeed {
        match self {
            SimSpeed::Paused => SimSpeed::Slow,
            SimSpeed::Slow => SimSpeed::Normal,
            SimSpeed::Normal => SimSpeed::Fast,
            SimSpeed::Fast => SimSpeed::VeryFast,
            SimSpeed::VeryFast => SimSpeed::Paused,
        }
    }

    fn prev(&self) -> SimSpeed {
        match self {
            SimSpeed::Paused => SimSpeed::VeryFast,
            SimSpeed::Slow => SimSpeed::Paused,
            SimSpeed::Normal => SimSpeed::Slow,
            SimSpeed::Fast => SimSpeed::Normal,
            SimSpeed::VeryFast => SimSpeed::Fast,
        }
    }
}

/// View mode for the explorer
#[derive(Clone, Copy, PartialEq)]
enum ViewMode {
    /// Local map primary view (default)
    Local,
    /// World map overview
    World,
}

/// Terminal explorer state
pub struct Explorer {
    world: WorldData,

    // World-level cursor (for world view mode)
    cursor_x: usize,
    cursor_y: usize,
    viewport: Viewport,

    // Local-primary view state
    view_mode_type: ViewMode,
    /// Camera position in global local coordinates
    camera: GlobalLocalCoord,
    /// Cursor position in global local coordinates
    cursor: GlobalLocalCoord,
    /// Local map cache
    local_cache: LocalMapCache,
    /// Show minimap overlay
    show_minimap: bool,
    /// Minimap size (world tiles)
    minimap_width: usize,
    minimap_height: usize,

    // Legacy local map state (for backward compatibility)
    local_mode: bool,
    current_local_map: Option<LocalMap>,
    local_cursor_x: usize,
    local_cursor_y: usize,
    local_viewport: Viewport,

    // Rendering options
    view_mode: AsciiMode,
    running: bool,
    show_help: bool,

    // Simulation state
    sim_mode: bool,
    sim_state: Option<SimulationState>,
    sim_params: SimulationParams,
    sim_rng: ChaCha8Rng,
    sim_speed: SimSpeed,
    sim_last_tick: Instant,
    sim_show_territories: bool,
    selected_tribe: Option<TribeId>,
    // Combat log display
    show_combat_log: bool,
}

impl Explorer {
    pub fn new(world: WorldData) -> Self {
        let cursor_x = world.width / 2;
        let cursor_y = world.height / 2;
        let seed = world.seed;
        let world_width = world.width;
        let world_height = world.height;

        // Initialize camera at center of the world in local coordinates
        let center_tile = TileCoord::new(cursor_x, cursor_y);
        let camera = GlobalLocalCoord::from_world_tile(center_tile);
        let cursor = camera;

        // Create local map cache
        let local_cache = LocalMapCache::new(world_width, world_height);

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

            // Local-primary view state
            view_mode_type: ViewMode::Local,
            camera,
            cursor,
            local_cache,
            show_minimap: true,
            minimap_width: 40,
            minimap_height: 20,

            // Legacy local map state
            local_mode: false,
            current_local_map: None,
            local_cursor_x: 32,
            local_cursor_y: 32,
            local_viewport: Viewport {
                x: 0,
                y: 0,
                width: 80,
                height: 20,
            },

            view_mode: AsciiMode::Biome,
            running: true,
            show_help: false,

            // Simulation fields
            sim_mode: false,
            sim_state: None,
            sim_params: SimulationParams::default(),
            sim_rng: ChaCha8Rng::seed_from_u64(seed),
            sim_speed: SimSpeed::Paused,
            sim_last_tick: Instant::now(),
            sim_show_territories: true,
            selected_tribe: None,
            // Combat log display
            show_combat_log: true, // Show by default in simulation mode
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

        // Reset local-primary view
        let center_tile = TileCoord::new(self.cursor_x, self.cursor_y);
        self.camera = GlobalLocalCoord::from_world_tile(center_tile);
        self.cursor = self.camera;
        self.local_cache = LocalMapCache::new(width, height);

        // Reset simulation if active
        if self.sim_mode {
            self.sim_state = None;
            self.sim_mode = false;
        }
    }

    /// Toggle simulation mode on/off
    fn toggle_simulation(&mut self) {
        if self.sim_mode {
            // Turn off simulation
            self.sim_mode = false;
            self.sim_state = None;
            self.selected_tribe = None;
        } else {
            // Initialize simulation
            let mut sim_state = SimulationState::new(self.world.seed);

            // Initialize with world and spawn tribes
            sim_state.initialize(&self.world, &self.sim_params, &mut self.sim_rng);

            self.sim_state = Some(sim_state);
            self.sim_mode = true;
            self.sim_speed = SimSpeed::Paused;
            self.sim_last_tick = Instant::now();
        }
    }

    /// Process a single simulation tick
    fn simulation_tick(&mut self) {
        if let Some(ref mut sim) = self.sim_state {
            sim.tick(&self.world, &self.sim_params, &mut self.sim_rng);
            self.sim_last_tick = Instant::now();
        }
    }

    /// Check if it's time for a simulation tick and process it
    fn update_simulation(&mut self) {
        if !self.sim_mode {
            return;
        }

        if let Some(interval) = self.sim_speed.tick_interval() {
            if self.sim_last_tick.elapsed() >= interval {
                self.simulation_tick();
            }
        }
    }

    /// Get the tribe at a specific coordinate
    fn tribe_at_coord(&self, x: usize, y: usize) -> Option<TribeId> {
        self.sim_state.as_ref().and_then(|sim| {
            sim.territory_map.get(&TileCoord::new(x, y)).copied()
        })
    }

    /// Update selected tribe based on cursor position
    fn update_selected_tribe(&mut self) {
        if self.sim_mode {
            self.selected_tribe = self.tribe_at_coord(self.cursor_x, self.cursor_y);
        }
    }

    /// Get a distinct color for a tribe based on its ID
    fn tribe_color(tribe_id: TribeId) -> (u8, u8, u8) {
        let colors: [(u8, u8, u8); 16] = [
            (230, 25, 75),   // Red
            (60, 180, 75),   // Green
            (255, 225, 25),  // Yellow
            (0, 130, 200),   // Blue
            (245, 130, 48),  // Orange
            (145, 30, 180),  // Purple
            (70, 240, 240),  // Cyan
            (240, 50, 230),  // Magenta
            (210, 245, 60),  // Lime
            (250, 190, 212), // Pink
            (0, 128, 128),   // Teal
            (220, 190, 255), // Lavender
            (170, 110, 40),  // Brown
            (255, 250, 200), // Beige
            (128, 0, 0),     // Maroon
            (170, 255, 195), // Mint
        ];
        colors[tribe_id.0 as usize % colors.len()]
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

            // Update simulation if running
            self.update_simulation();

            // Render
            terminal.draw(|frame| self.render(frame))?;

            // Handle input with shorter poll time for smoother simulation
            let poll_time = if self.sim_mode && self.sim_speed != SimSpeed::Paused {
                Duration::from_millis(16) // ~60fps for smooth animation
            } else {
                Duration::from_millis(50)
            };

            if event::poll(poll_time)? {
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
        // Handle Escape specially
        if key.code == KeyCode::Esc {
            match self.view_mode_type {
                ViewMode::Local => {
                    self.running = false;
                }
                ViewMode::World => {
                    if self.local_mode {
                        self.local_mode = false;
                        self.current_local_map = None;
                    } else {
                        self.running = false;
                    }
                }
            }
            return;
        }

        // Handle view mode switching
        match key.code {
            KeyCode::Char('w') | KeyCode::Char('W') if key.modifiers.contains(crossterm::event::KeyModifiers::NONE) && self.view_mode_type == ViewMode::Local => {
                // Don't switch if just pressing w for movement - only switch on W
                if key.code == KeyCode::Char('W') {
                    self.view_mode_type = ViewMode::World;
                    // Sync world cursor to local cursor position
                    let tile = self.cursor.world_tile();
                    self.cursor_x = tile.x;
                    self.cursor_y = tile.y;
                    self.center_viewport();
                    return;
                }
            }
            KeyCode::Char('l') | KeyCode::Char('L') if self.view_mode_type == ViewMode::World && !self.local_mode => {
                self.view_mode_type = ViewMode::Local;
                // Sync local cursor to world cursor position
                let tile = TileCoord::new(self.cursor_x, self.cursor_y);
                self.cursor = GlobalLocalCoord::from_world_tile(tile);
                self.camera = self.cursor;
                return;
            }
            _ => {}
        }

        // Handle mode-specific input
        match self.view_mode_type {
            ViewMode::Local => self.handle_local_primary_key_input(key),
            ViewMode::World => {
                if self.local_mode {
                    self.handle_local_key_input(key);
                } else {
                    self.handle_world_key_input(key);
                }
            }
        }
    }

    /// Handle keyboard input for local-primary view
    fn handle_local_primary_key_input(&mut self, key: KeyEvent) {
        let total_local_width = self.world.width as u32 * LOCAL_MAP_SIZE;
        let total_local_height = self.world.height as u32 * LOCAL_MAP_SIZE;

        match key.code {
            KeyCode::Char('q') => self.running = false,

            // View controls
            KeyCode::Char('m') | KeyCode::Char('M') => {
                self.show_minimap = !self.show_minimap;
            }
            KeyCode::Char('W') => {
                // Switch to world view
                self.view_mode_type = ViewMode::World;
                let tile = self.cursor.world_tile();
                self.cursor_x = tile.x;
                self.cursor_y = tile.y;
                self.center_viewport();
            }

            // Simulation controls
            KeyCode::Char('S') => self.toggle_simulation(),
            KeyCode::Char(' ') if self.sim_mode => {
                if self.sim_speed == SimSpeed::Paused {
                    self.simulation_tick();
                } else {
                    self.sim_speed = SimSpeed::Paused;
                }
            }
            KeyCode::Char('+') | KeyCode::Char('=') if self.sim_mode => {
                self.sim_speed = self.sim_speed.next();
            }
            KeyCode::Char('-') | KeyCode::Char('_') if self.sim_mode => {
                self.sim_speed = self.sim_speed.prev();
            }
            KeyCode::Char('t') | KeyCode::Char('T') if self.sim_mode => {
                self.sim_show_territories = !self.sim_show_territories;
            }
            KeyCode::Char('L') if self.sim_mode => {
                self.show_combat_log = !self.show_combat_log;
            }

            // Navigation - move cursor in local space
            KeyCode::Up | KeyCode::Char('k') => {
                if self.cursor.y > 0 {
                    self.cursor.y -= 1;
                    self.camera = self.cursor;
                }
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if self.cursor.y < total_local_height - 1 {
                    self.cursor.y += 1;
                    self.camera = self.cursor;
                }
            }
            KeyCode::Left | KeyCode::Char('h') => {
                // Wrap horizontally
                self.cursor.x = if self.cursor.x == 0 {
                    total_local_width - 1
                } else {
                    self.cursor.x - 1
                };
                self.camera = self.cursor;
            }
            KeyCode::Right | KeyCode::Char('l') => {
                // Wrap horizontally
                self.cursor.x = (self.cursor.x + 1) % total_local_width;
                self.camera = self.cursor;
            }
            // WASD support (but not 'w' alone as it might conflict with World view)
            KeyCode::Char('w') => {
                if self.cursor.y > 0 {
                    self.cursor.y -= 1;
                    self.camera = self.cursor;
                }
            }
            KeyCode::Char('s') if !self.sim_mode => {
                if self.cursor.y < total_local_height - 1 {
                    self.cursor.y += 1;
                    self.camera = self.cursor;
                }
            }
            KeyCode::Char('a') => {
                self.cursor.x = if self.cursor.x == 0 {
                    total_local_width - 1
                } else {
                    self.cursor.x - 1
                };
                self.camera = self.cursor;
            }
            KeyCode::Char('d') => {
                self.cursor.x = (self.cursor.x + 1) % total_local_width;
                self.camera = self.cursor;
            }

            // Fast movement
            KeyCode::PageUp => {
                self.cursor.y = self.cursor.y.saturating_sub(10);
                self.camera = self.cursor;
            }
            KeyCode::PageDown => {
                self.cursor.y = (self.cursor.y + 10).min(total_local_height - 1);
                self.camera = self.cursor;
            }
            KeyCode::Home => {
                self.cursor.x = ((self.cursor.x as i32 - 10).rem_euclid(total_local_width as i32)) as u32;
                self.camera = self.cursor;
            }
            KeyCode::End => {
                self.cursor.x = (self.cursor.x + 10) % total_local_width;
                self.camera = self.cursor;
            }

            // Other controls
            KeyCode::Char('c') | KeyCode::Char('C') => {
                self.camera = self.cursor;
            }
            KeyCode::Char('?') | KeyCode::F(1) => {
                self.show_help = !self.show_help;
            }
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.regenerate_random();
            }
            KeyCode::Char('0') => {
                // Jump to center of world
                let center_x = total_local_width / 2;
                let center_y = total_local_height / 2;
                self.cursor = GlobalLocalCoord::new(center_x, center_y);
                self.camera = self.cursor;
            }

            _ => {}
        }
    }

    fn handle_world_key_input(&mut self, key: KeyEvent) {
        use crossterm::event::KeyModifiers;

        match key.code {
            KeyCode::Char('q') => self.running = false,

            // Simulation controls
            KeyCode::Char('S') => self.toggle_simulation(),
            KeyCode::Char(' ') if self.sim_mode => {
                // Space = single step when paused, or pause when running
                if self.sim_speed == SimSpeed::Paused {
                    self.simulation_tick();
                } else {
                    self.sim_speed = SimSpeed::Paused;
                }
            }
            KeyCode::Char('+') | KeyCode::Char('=') if self.sim_mode => {
                self.sim_speed = self.sim_speed.next();
            }
            KeyCode::Char('-') | KeyCode::Char('_') if self.sim_mode => {
                self.sim_speed = self.sim_speed.prev();
            }
            KeyCode::Char('t') if self.sim_mode => {
                self.sim_show_territories = !self.sim_show_territories;
            }
            KeyCode::Char('L') if self.sim_mode => {
                self.show_combat_log = !self.show_combat_log;
            }

            KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('k') => {
                if self.cursor_y > 0 {
                    self.cursor_y -= 1;
                    self.adjust_viewport();
                    self.update_selected_tribe();
                }
            }
            KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('j') => {
                if self.cursor_y < self.world.height - 1 {
                    self.cursor_y += 1;
                    self.adjust_viewport();
                    self.update_selected_tribe();
                }
            }
            KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('h') => {
                if self.cursor_x > 0 {
                    self.cursor_x -= 1;
                    self.adjust_viewport();
                    self.update_selected_tribe();
                }
            }
            KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('l') => {
                if self.cursor_x < self.world.width - 1 {
                    self.cursor_x += 1;
                    self.adjust_viewport();
                    self.update_selected_tribe();
                }
            }

            KeyCode::PageUp => {
                self.cursor_y = self.cursor_y.saturating_sub(10);
                self.adjust_viewport();
                self.update_selected_tribe();
            }
            KeyCode::PageDown => {
                self.cursor_y = (self.cursor_y + 10).min(self.world.height - 1);
                self.adjust_viewport();
                self.update_selected_tribe();
            }
            KeyCode::Home => {
                self.cursor_x = self.cursor_x.saturating_sub(10);
                self.adjust_viewport();
                self.update_selected_tribe();
            }
            KeyCode::End => {
                self.cursor_x = (self.cursor_x + 10).min(self.world.width - 1);
                self.adjust_viewport();
                self.update_selected_tribe();
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
            // Switch to local primary view (only 'L' to avoid conflict with movement)
            KeyCode::Char('o') => {
                self.view_mode_type = ViewMode::Local;
                let tile = TileCoord::new(self.cursor_x, self.cursor_y);
                self.cursor = GlobalLocalCoord::from_world_tile(tile);
                self.camera = self.cursor;
            }
            _ => {}
        }
    }

    fn handle_local_key_input(&mut self, key: KeyEvent) {
        let local_map = match &self.current_local_map {
            Some(map) => map,
            None => return,
        };
        let local_size = local_map.width;

        match key.code {
            KeyCode::Char('q') => {
                self.local_mode = false;
                self.current_local_map = None;
            }

            KeyCode::Up | KeyCode::Char('w') | KeyCode::Char('k') => {
                if self.local_cursor_y > 0 {
                    self.local_cursor_y -= 1;
                    self.adjust_local_viewport();
                }
            }
            KeyCode::Down | KeyCode::Char('s') | KeyCode::Char('j') => {
                if self.local_cursor_y < local_size - 1 {
                    self.local_cursor_y += 1;
                    self.adjust_local_viewport();
                }
            }
            KeyCode::Left | KeyCode::Char('a') | KeyCode::Char('h') => {
                if self.local_cursor_x > 0 {
                    self.local_cursor_x -= 1;
                    self.adjust_local_viewport();
                }
            }
            KeyCode::Right | KeyCode::Char('d') | KeyCode::Char('l') => {
                if self.local_cursor_x < local_size - 1 {
                    self.local_cursor_x += 1;
                    self.adjust_local_viewport();
                }
            }

            KeyCode::PageUp => {
                self.local_cursor_y = self.local_cursor_y.saturating_sub(10);
                self.adjust_local_viewport();
            }
            KeyCode::PageDown => {
                self.local_cursor_y = (self.local_cursor_y + 10).min(local_size - 1);
                self.adjust_local_viewport();
            }
            KeyCode::Home => {
                self.local_cursor_x = self.local_cursor_x.saturating_sub(10);
                self.adjust_local_viewport();
            }
            KeyCode::End => {
                self.local_cursor_x = (self.local_cursor_x + 10).min(local_size - 1);
                self.adjust_local_viewport();
            }

            KeyCode::Char('?') | KeyCode::F(1) => self.show_help = !self.show_help,
            KeyCode::Char('c') => self.center_local_viewport(),
            KeyCode::Char('0') => {
                self.local_cursor_x = local_size / 2;
                self.local_cursor_y = local_size / 2;
                self.center_local_viewport();
            }
            _ => {}
        }
    }

    fn toggle_local_mode(&mut self) {
        if self.local_mode {
            // Exit local mode
            self.local_mode = false;
            self.current_local_map = None;
        } else {
            // Enter local mode - generate local map for current tile
            let local_map = generate_local_map_default(&self.world, self.cursor_x, self.cursor_y);
            let size = local_map.width;
            self.current_local_map = Some(local_map);
            self.local_mode = true;
            self.local_cursor_x = size / 2;
            self.local_cursor_y = size / 2;
            self.center_local_viewport();
        }
    }

    fn center_local_viewport(&mut self) {
        let half_w = self.local_viewport.width / 2;
        let half_h = self.local_viewport.height / 2;

        self.local_viewport.x = self.local_cursor_x.saturating_sub(half_w);
        self.local_viewport.y = self.local_cursor_y.saturating_sub(half_h);

        self.clamp_local_viewport();
    }

    fn clamp_local_viewport(&mut self) {
        if let Some(local_map) = &self.current_local_map {
            if self.local_viewport.x + self.local_viewport.width > local_map.width {
                self.local_viewport.x = local_map.width.saturating_sub(self.local_viewport.width);
            }
            if self.local_viewport.y + self.local_viewport.height > local_map.height {
                self.local_viewport.y = local_map.height.saturating_sub(self.local_viewport.height);
            }
        }
    }

    fn adjust_local_viewport(&mut self) {
        let margin = 3;

        if self.local_cursor_x < self.local_viewport.x + margin {
            self.local_viewport.x = self.local_cursor_x.saturating_sub(margin);
        }
        if self.local_cursor_x >= self.local_viewport.x + self.local_viewport.width - margin {
            self.local_viewport.x = self.local_cursor_x.saturating_sub(self.local_viewport.width - margin - 1);
        }
        if self.local_cursor_y < self.local_viewport.y + margin {
            self.local_viewport.y = self.local_cursor_y.saturating_sub(margin);
        }
        if self.local_cursor_y >= self.local_viewport.y + self.local_viewport.height - margin {
            self.local_viewport.y = self.local_cursor_y.saturating_sub(self.local_viewport.height - margin - 1);
        }

        self.clamp_local_viewport();
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

    fn render(&mut self, frame: &mut Frame) {
        match self.view_mode_type {
            ViewMode::Local => self.render_local_primary(frame),
            ViewMode::World => {
                if self.local_mode {
                    self.render_local(frame);
                } else {
                    self.render_world(frame);
                }
            }
        }
    }

    /// Render the local-map primary view with seamless tile transitions
    fn render_local_primary(&mut self, frame: &mut Frame) {
        let size = frame.area();

        // Update cache based on camera position
        self.local_cache.update_camera(&self.world, self.camera);

        // Layout: header, simulation bar (if active), map, info panel, controls
        let constraints = if self.sim_mode && self.show_combat_log {
            vec![
                Constraint::Length(1),  // Header
                Constraint::Length(1),  // Simulation status bar
                Constraint::Min(10),    // Map
                Constraint::Length(6),  // Combat log panel
                Constraint::Length(5),  // Info panel
                Constraint::Length(1),  // Controls
            ]
        } else if self.sim_mode {
            vec![
                Constraint::Length(1),  // Header
                Constraint::Length(1),  // Simulation status bar
                Constraint::Min(10),    // Map
                Constraint::Length(5),  // Info panel
                Constraint::Length(1),  // Controls
            ]
        } else {
            vec![
                Constraint::Length(1),  // Header
                Constraint::Min(10),    // Map
                Constraint::Length(5),  // Info panel
                Constraint::Length(1),  // Controls
            ]
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(size);

        // Header
        let (world_tile, local_offset) = self.cursor.to_hierarchical();
        let biome = self.world.biomes.get(world_tile.x, world_tile.y);
        let header_text = if self.sim_mode {
            format!(
                "LOCAL VIEW - {} - Tile ({},{}) Local ({},{})  [M] Minimap  [W] World View  [?] Help",
                biome.display_name(),
                world_tile.x, world_tile.y,
                local_offset.x, local_offset.y,
            )
        } else {
            format!(
                "LOCAL VIEW - {} - Tile ({},{}) Local ({},{}) - Seed: {}  [M] Minimap  [W] World View  [?] Help",
                biome.display_name(),
                world_tile.x, world_tile.y,
                local_offset.x, local_offset.y,
                self.world.seed,
            )
        };
        let header = Paragraph::new(header_text)
            .style(Style::default().fg(Color::Green));
        frame.render_widget(header, chunks[0]);

        // Chunk indices depend on whether simulation bar and combat log are shown
        let (map_chunk, combat_log_chunk, info_chunk, controls_chunk) = if self.sim_mode && self.show_combat_log {
            // Render simulation status bar
            if let Some(ref sim) = self.sim_state {
                let year = sim.current_tick.year();
                let season = format!("{:?}", sim.current_tick.season());
                let living_tribes = sim.tribes.values().filter(|t| t.is_alive).count();
                let total_pop: u32 = sim.tribes.values().filter(|t| t.is_alive).map(|t| t.population.total()).sum();
                let monster_count = sim.monsters.living_count();

                let sim_bar = Paragraph::new(format!(
                    "Year {} {} | Tribes: {} | Pop: {} | Monsters: {} | Speed: {} | [Space] Step [+/-] Speed",
                    year, season, living_tribes, total_pop, monster_count, self.sim_speed.name(),
                ))
                .style(Style::default().fg(Color::Yellow).bg(Color::DarkGray));
                frame.render_widget(sim_bar, chunks[1]);
            }
            (2, Some(3), 4, 5)
        } else if self.sim_mode {
            if let Some(ref sim) = self.sim_state {
                let year = sim.current_tick.year();
                let season = format!("{:?}", sim.current_tick.season());
                let living_tribes = sim.tribes.values().filter(|t| t.is_alive).count();
                let total_pop: u32 = sim.tribes.values().filter(|t| t.is_alive).map(|t| t.population.total()).sum();
                let monster_count = sim.monsters.living_count();

                let sim_bar = Paragraph::new(format!(
                    "Year {} {} | Tribes: {} | Pop: {} | Monsters: {} | Speed: {} | [Space] Step [+/-] Speed",
                    year, season, living_tribes, total_pop, monster_count, self.sim_speed.name(),
                ))
                .style(Style::default().fg(Color::Yellow).bg(Color::DarkGray));
                frame.render_widget(sim_bar, chunks[1]);
            }
            (2, None, 3, 4)
        } else {
            (1, None, 2, 3)
        };

        // Render combat log if enabled
        if let Some(combat_chunk) = combat_log_chunk {
            self.render_combat_log(frame, chunks[combat_chunk]);
        }

        // Map area with border
        let map_block = Block::default()
            .borders(Borders::ALL)
            .title(" Local Map ");
        let map_inner = map_block.inner(chunks[map_chunk]);
        frame.render_widget(map_block, chunks[map_chunk]);

        // Render local map tiles (spans multiple world tiles seamlessly)
        let map_width = map_inner.width as usize;
        let map_height = map_inner.height as usize;
        let half_w = map_width / 2;
        let half_h = map_height / 2;

        // Calculate total world size in local coordinates
        let total_local_width = self.world.width as u32 * LOCAL_MAP_SIZE;
        let total_local_height = self.world.height as u32 * LOCAL_MAP_SIZE;

        for vy in 0..map_height {
            for vx in 0..map_width {
                // Calculate global local coordinate for this screen position
                let offset_x = vx as i32 - half_w as i32;
                let offset_y = vy as i32 - half_h as i32;

                let global_x = ((self.camera.x as i32 + offset_x).rem_euclid(total_local_width as i32)) as u32;
                let global_y = (self.camera.y as i32 + offset_y).clamp(0, total_local_height as i32 - 1) as u32;

                let coord = GlobalLocalCoord::new(global_x, global_y);
                let is_cursor = global_x == self.cursor.x && global_y == self.cursor.y;

                // Get the tile from the cache
                let tile = self.local_cache.get_tile(&self.world, coord);

                let (ch, fg, bg) = if let Some(tile) = tile {
                    // Get elevation brightness
                    let brightness = tile.elevation_brightness();

                    // Background = terrain color with elevation shading
                    let (tr, tg, tb) = tile.terrain.color();
                    let bg = Color::Rgb(
                        ((tr as f32 * brightness * 0.6).min(255.0)) as u8,
                        ((tg as f32 * brightness * 0.6).min(255.0)) as u8,
                        ((tb as f32 * brightness * 0.6).min(255.0)) as u8,
                    );

                    // Foreground = feature color if present, else brightened terrain
                    let (ch, fg) = if let Some((fr, fg_g, fb)) = tile.feature_color() {
                        let fg = Color::Rgb(
                            ((fr as f32 * brightness * 1.2).min(255.0)) as u8,
                            ((fg_g as f32 * brightness * 1.2).min(255.0)) as u8,
                            ((fb as f32 * brightness * 1.2).min(255.0)) as u8,
                        );
                        (tile.ascii_char(), fg)
                    } else {
                        let fg = Color::Rgb(
                            ((tr as f32 * brightness * 1.4).min(255.0)) as u8,
                            ((tg as f32 * brightness * 1.4).min(255.0)) as u8,
                            ((tb as f32 * brightness * 1.4).min(255.0)) as u8,
                        );
                        // Add subtle elevation hint
                        let ch = if tile.elevation_offset > 0.3 {
                            '\''
                        } else if tile.elevation_offset < -0.3 {
                            '_'
                        } else {
                            tile.terrain.ascii_char()
                        };
                        (ch, fg)
                    };

                    (ch, fg, bg)
                } else {
                    // Out of bounds - show as empty
                    (' ', Color::Black, Color::Black)
                };

                // Apply cursor highlighting
                let (final_ch, final_fg, final_bg) = if is_cursor {
                    (ch, Color::Black, Color::Yellow)
                } else {
                    (ch, fg, bg)
                };

                let x = map_inner.x + vx as u16;
                let y = map_inner.y + vy as u16;

                if x < map_inner.x + map_inner.width && y < map_inner.y + map_inner.height {
                    frame.buffer_mut().set_string(
                        x, y,
                        final_ch.to_string(),
                        Style::default().fg(final_fg).bg(final_bg),
                    );
                }
            }
        }

        // Render entities (monsters, colonists) on the local map
        self.render_local_entities(frame, map_inner, half_w, half_h, total_local_width, total_local_height);

        // Render minimap overlay if enabled
        if self.show_minimap {
            self.render_minimap(frame, map_inner);
        }

        // Info panel
        let (cursor_world_tile, cursor_local_offset) = self.cursor.to_hierarchical();
        let cursor_biome = self.world.biomes.get(cursor_world_tile.x, cursor_world_tile.y);
        let tile_info = self.world.get_tile_info(cursor_world_tile.x, cursor_world_tile.y);

        let info_text = vec![
            Line::from(vec![
                Span::raw("Position: "),
                Span::styled(format!("Global ({}, {})", self.cursor.x, self.cursor.y), Style::default().fg(Color::White)),
                Span::raw("  World Tile: "),
                Span::styled(format!("({}, {})", cursor_world_tile.x, cursor_world_tile.y), Style::default().fg(Color::Cyan)),
                Span::raw("  Local: "),
                Span::styled(format!("({}, {})", cursor_local_offset.x, cursor_local_offset.y), Style::default().fg(Color::Yellow)),
            ]),
            Line::from(vec![
                Span::raw("Biome: "),
                Span::styled(cursor_biome.display_name().to_string(), Style::default().fg(Color::Green)),
                Span::raw("  Elev: "),
                Span::styled(tile_info.elevation_str(), Style::default().fg(Color::Yellow)),
                Span::raw("  Temp: "),
                Span::styled(tile_info.temperature_str(), Style::default().fg(Color::Red)),
            ]),
            Line::from(vec![
                Span::raw("Cache: "),
                Span::styled(format!("{} tiles", self.local_cache.stats().cached_count), Style::default().fg(Color::Magenta)),
            ]),
        ];

        let info_panel = Paragraph::new(info_text)
            .block(Block::default().borders(Borders::ALL).title(" Local Info "));
        frame.render_widget(info_panel, chunks[info_chunk]);

        // Controls
        let controls_text = if self.sim_mode {
            "[←↑↓→] Move  [M] Minimap  [W] World View  [Space] Step  [+/-] Speed  [S] Stop Sim  [?] Help  [Q] Quit"
        } else {
            "[←↑↓→/WASD] Move  [M] Minimap  [W] World View  [S] Start Sim  [N] New  [?] Help  [Q] Quit"
        };
        let controls = Paragraph::new(controls_text)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(controls, chunks[controls_chunk]);

        // Help overlay
        if self.show_help {
            self.render_local_primary_help(frame);
        }
    }

    /// Render entities (monsters, colonists) on the local map
    fn render_local_entities(&self, frame: &mut Frame, map_area: Rect, half_w: usize, half_h: usize, total_local_width: u32, total_local_height: u32) {
        if !self.sim_mode {
            return;
        }

        let sim = match &self.sim_state {
            Some(s) => s,
            None => return,
        };

        // Render monsters
        for monster in sim.monsters.monsters.values() {
            if monster.is_dead() {
                continue;
            }

            // Check if monster is within viewport
            let offset_x = monster.local_position.x as i32 - self.camera.x as i32;
            let offset_y = monster.local_position.y as i32 - self.camera.y as i32;

            // Handle horizontal wrapping
            let half_total = (total_local_width / 2) as i32;
            let offset_x = if offset_x > half_total {
                offset_x - total_local_width as i32
            } else if offset_x < -half_total {
                offset_x + total_local_width as i32
            } else {
                offset_x
            };

            let screen_x = half_w as i32 + offset_x;
            let screen_y = half_h as i32 + offset_y;

            if screen_x >= 0 && screen_x < map_area.width as i32 && screen_y >= 0 && screen_y < map_area.height as i32 {
                let ch = monster.species.map_char();
                let (r, g, b) = monster.species.color();
                let fg = Color::Rgb(r, g, b);
                let bg = Color::Rgb(100, 30, 30); // Red tint background

                let x = map_area.x + screen_x as u16;
                let y = map_area.y + screen_y as u16;

                frame.buffer_mut().set_string(
                    x, y,
                    ch.to_string(),
                    Style::default().fg(fg).bg(bg),
                );
            }
        }

        // Render colonists
        for tribe in sim.tribes.values() {
            if !tribe.is_alive {
                continue;
            }

            for colonist in tribe.notable_colonists.colonists.values() {
                if !colonist.is_alive {
                    continue;
                }

                let offset_x = colonist.local_position.x as i32 - self.camera.x as i32;
                let offset_y = colonist.local_position.y as i32 - self.camera.y as i32;

                // Handle horizontal wrapping
                let half_total = (total_local_width / 2) as i32;
                let offset_x = if offset_x > half_total {
                    offset_x - total_local_width as i32
                } else if offset_x < -half_total {
                    offset_x + total_local_width as i32
                } else {
                    offset_x
                };

                let screen_x = half_w as i32 + offset_x;
                let screen_y = half_h as i32 + offset_y;

                if screen_x >= 0 && screen_x < map_area.width as i32 && screen_y >= 0 && screen_y < map_area.height as i32 {
                    let ch = colonist.map_char();
                    let (r, g, b) = colonist.color();
                    let fg = Color::Rgb(r, g, b);
                    let (tr, tg, tb) = Self::tribe_color(tribe.id);
                    let bg = Color::Rgb(tr / 2, tg / 2, tb / 2);

                    let x = map_area.x + screen_x as u16;
                    let y = map_area.y + screen_y as u16;

                    frame.buffer_mut().set_string(
                        x, y,
                        ch.to_string(),
                        Style::default().fg(fg).bg(bg),
                    );
                }
            }
        }
    }

    /// Render the minimap overlay
    fn render_minimap(&self, frame: &mut Frame, map_area: Rect) {
        let minimap_w = self.minimap_width.min(map_area.width as usize - 4) as u16;
        let minimap_h = self.minimap_height.min(map_area.height as usize - 2) as u16;

        // Position in bottom-right corner of map area
        let minimap_x = map_area.x + map_area.width - minimap_w - 2;
        let minimap_y = map_area.y + map_area.height - minimap_h - 1;

        let minimap_area = Rect::new(minimap_x, minimap_y, minimap_w, minimap_h);

        // Draw border
        let minimap_block = Block::default()
            .borders(Borders::ALL)
            .title(" World ")
            .border_style(Style::default().fg(Color::DarkGray));
        let minimap_inner = minimap_block.inner(minimap_area);
        frame.render_widget(minimap_block, minimap_area);

        // Calculate what portion of the world to show
        let camera_tile = self.camera.world_tile();
        let half_minimap_w = minimap_inner.width as usize / 2;
        let half_minimap_h = minimap_inner.height as usize / 2;

        for vy in 0..minimap_inner.height as usize {
            for vx in 0..minimap_inner.width as usize {
                let world_x = ((camera_tile.x as i32 + vx as i32 - half_minimap_w as i32)
                    .rem_euclid(self.world.width as i32)) as usize;
                let world_y = (camera_tile.y as i32 + vy as i32 - half_minimap_h as i32)
                    .clamp(0, self.world.height as i32 - 1) as usize;

                let biome = self.world.biomes.get(world_x, world_y);
                let (r, g, b) = biome.color();

                // Determine if this is the current camera tile
                let is_camera_tile = world_x == camera_tile.x && world_y == camera_tile.y;

                let (fg, bg, ch) = if is_camera_tile {
                    (Color::Black, Color::Yellow, '@')
                } else {
                    // Show territory colors if in sim mode
                    let coord = TileCoord::new(world_x, world_y);
                    if self.sim_mode && self.sim_show_territories {
                        if let Some(tribe_id) = self.tribe_at_coord(world_x, world_y) {
                            let (tr, tg, tb) = Self::tribe_color(tribe_id);
                            (Color::Rgb(tr, tg, tb), Color::Rgb(r / 3, g / 3, b / 3), '.')
                        } else {
                            (Color::Rgb(r / 2, g / 2, b / 2), Color::Rgb(r / 4, g / 4, b / 4), '.')
                        }
                    } else {
                        (Color::Rgb(r / 2, g / 2, b / 2), Color::Rgb(r / 4, g / 4, b / 4), '.')
                    }
                };

                let x = minimap_inner.x + vx as u16;
                let y = minimap_inner.y + vy as u16;

                frame.buffer_mut().set_string(
                    x, y,
                    ch.to_string(),
                    Style::default().fg(fg).bg(bg),
                );
            }
        }
    }

    /// Render help for local primary view
    fn render_local_primary_help(&self, frame: &mut Frame) {
        let area = frame.area();

        let popup_width = 54;
        let popup_height = 22;
        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

        frame.render_widget(Clear, popup_area);

        let help_text = vec![
            Line::from("Local Map Help").style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Green)),
            Line::from(""),
            Line::from("Navigation:").style(Style::default().add_modifier(Modifier::BOLD)),
            Line::from("  Arrow keys / WASD / HJKL - Move cursor"),
            Line::from("  PgUp/PgDn - Fast vertical movement"),
            Line::from("  Home/End  - Fast horizontal movement"),
            Line::from("  C - Center camera on cursor"),
            Line::from("  Movement seamlessly crosses tile boundaries"),
            Line::from(""),
            Line::from("View:").style(Style::default().add_modifier(Modifier::BOLD)),
            Line::from("  M - Toggle minimap overlay"),
            Line::from("  W - Switch to world map view"),
            Line::from(""),
            Line::from("Simulation:").style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan)),
            Line::from("  Shift+S - Start/stop simulation"),
            Line::from("  Space   - Step (paused) / Pause (running)"),
            Line::from("  +/-     - Change simulation speed"),
            Line::from("  T       - Toggle territory overlay"),
            Line::from(""),
            Line::from("  ? - Toggle this help    Q/Esc - Quit"),
        ];

        let help_popup = Paragraph::new(help_text)
            .block(Block::default()
                .borders(Borders::ALL)
                .title(" Local Map Help ")
                .border_style(Style::default().fg(Color::Green))
                .style(Style::default().bg(Color::DarkGray)))
            .style(Style::default().fg(Color::White).bg(Color::DarkGray));

        frame.render_widget(help_popup, popup_area);
    }

    fn render_world(&self, frame: &mut Frame) {
        let size = frame.area();

        // Layout: header, simulation bar (if active), map, combat log (if active), info panel, controls
        let constraints = if self.sim_mode && self.show_combat_log {
            vec![
                Constraint::Length(1),  // Header
                Constraint::Length(1),  // Simulation status bar
                Constraint::Min(10),    // Map
                Constraint::Length(6),  // Combat log panel
                Constraint::Length(6),  // Info panel (expanded for reputation)
                Constraint::Length(1),  // Controls
            ]
        } else if self.sim_mode {
            vec![
                Constraint::Length(1),  // Header
                Constraint::Length(1),  // Simulation status bar
                Constraint::Min(10),    // Map
                Constraint::Length(6),  // Info panel (expanded for reputation)
                Constraint::Length(1),  // Controls
            ]
        } else {
            vec![
                Constraint::Length(1),  // Header
                Constraint::Min(10),    // Map
                Constraint::Length(5),  // Info panel
                Constraint::Length(1),  // Controls
            ]
        };

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(size);

        // Header
        let header_text = if self.sim_mode {
            format!(
                "PLANET EXPLORER - Seed: {} - SIMULATION MODE  [?] Help  [Q] Quit",
                self.world.seed,
            )
        } else {
            format!(
                "PLANET EXPLORER - Seed: {}  Size: {}x{} ({:.0} x {:.0} km)  [?] Help  [Q] Quit",
                self.world.seed,
                self.world.width,
                self.world.height,
                self.world.map_size_km().0,
                self.world.map_size_km().1,
            )
        };
        let header = Paragraph::new(header_text)
            .style(Style::default().fg(if self.sim_mode { Color::Green } else { Color::Cyan }));
        frame.render_widget(header, chunks[0]);

        // Chunk indices depend on whether simulation bar and combat log are shown
        let (map_chunk, combat_log_chunk, info_chunk, controls_chunk) = if self.sim_mode && self.show_combat_log {
            // Render simulation status bar
            if let Some(ref sim) = self.sim_state {
                let year = sim.current_tick.year();
                let season = format!("{:?}", sim.current_tick.season());
                let living_tribes = sim.tribes.values().filter(|t| t.is_alive).count();
                let total_pop: u32 = sim.tribes.values().filter(|t| t.is_alive).map(|t| t.population.total()).sum();
                let monster_count = sim.monsters.living_count();
                let combat_count = sim.combat_log.encounter_count();

                let sim_bar = Paragraph::new(format!(
                    "Year {} {} | Tribes: {} | Pop: {} | Monsters: {} | Combats: {} | Speed: {} | [+/-] Speed",
                    year, season, living_tribes, total_pop, monster_count, combat_count, self.sim_speed.name(),
                ))
                .style(Style::default().fg(Color::Yellow).bg(Color::DarkGray));
                frame.render_widget(sim_bar, chunks[1]);
            }
            (2, Some(3), 4, 5)
        } else if self.sim_mode {
            // Render simulation status bar
            if let Some(ref sim) = self.sim_state {
                let year = sim.current_tick.year();
                let season = format!("{:?}", sim.current_tick.season());
                let living_tribes = sim.tribes.values().filter(|t| t.is_alive).count();
                let total_pop: u32 = sim.tribes.values().filter(|t| t.is_alive).map(|t| t.population.total()).sum();
                let monster_count = sim.monsters.living_count();

                let sim_bar = Paragraph::new(format!(
                    "Year {} {} | Tribes: {} | Pop: {} | Monsters: {} | Speed: {} | [Space] Step [+/-] Speed",
                    year, season, living_tribes, total_pop, monster_count, self.sim_speed.name(),
                ))
                .style(Style::default().fg(Color::Yellow).bg(Color::DarkGray));
                frame.render_widget(sim_bar, chunks[1]);
            }
            (2, None, 3, 4)
        } else {
            (1, None, 2, 3)
        };

        // Render combat log panel if enabled
        if let Some(combat_chunk) = combat_log_chunk {
            self.render_combat_log(frame, chunks[combat_chunk]);
        }

        // Map area with border
        let map_title = if self.sim_mode && self.sim_show_territories {
            format!(" Map - {} + Territories ", self.view_mode.name())
        } else {
            format!(" Map - {} ", self.view_mode.name())
        };
        let map_block = Block::default()
            .borders(Borders::ALL)
            .title(map_title);
        let map_inner = map_block.inner(chunks[map_chunk]);
        frame.render_widget(map_block, chunks[map_chunk]);

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

                let coord = TileCoord::new(world_x, world_y);
                let biome = *self.world.biomes.get(world_x, world_y);
                let mut ch = biome_char(&biome);
                let is_cursor = world_x == self.cursor_x && world_y == self.cursor_y;

                // Get base colors from biome/view mode
                let (mut fg, mut bg) = self.get_tile_colors(world_x, world_y);

                // Layer 1: Territory overlay (background blend)
                if self.sim_mode && self.sim_show_territories {
                    if let Some(tribe_id) = self.tribe_at_coord(world_x, world_y) {
                        let (tr, tg, tb) = Self::tribe_color(tribe_id);
                        let is_selected = self.selected_tribe == Some(tribe_id);
                        let blend = if is_selected { 0.7 } else { 0.4 };

                        let (br, bg_r, bb) = match bg {
                            Color::Rgb(r, g, b) => (r, g, b),
                            _ => (0, 0, 0),
                        };

                        bg = Color::Rgb(
                            ((br as f32 * (1.0 - blend) + tr as f32 * blend) as u8).min(255),
                            ((bg_r as f32 * (1.0 - blend) + tg as f32 * blend) as u8).min(255),
                            ((bb as f32 * (1.0 - blend) + tb as f32 * blend) as u8).min(255),
                        );

                        if is_selected {
                            fg = Color::Rgb(tr, tg, tb);
                        }
                    }
                }

                // Layer 2: Road overlay (change character only if no higher layer)
                if self.sim_mode {
                    if let Some(ref sim) = self.sim_state {
                        if sim.road_network.has_road(&coord) {
                            ch = sim.road_network.get_road_char(&coord);
                            let (r, g, b) = sim.road_network.get_road_color(&coord);
                            fg = Color::Rgb(r, g, b);
                        }
                    }
                }

                // Layer 3: Structure overlay
                if self.sim_mode {
                    if let Some(ref sim) = self.sim_state {
                        if let Some(structure) = sim.get_structure_at(&coord) {
                            ch = structure.structure_type.map_char();
                            let (r, g, b) = structure.structure_type.color();
                            fg = Color::Rgb(r, g, b);
                        }
                    }
                }

                // Layer 3.5: Colonist overlay
                if self.sim_mode {
                    if let Some(ref sim) = self.sim_state {
                        if let Some((colonist, tribe_id)) = sim.get_colonist_at(&coord) {
                            ch = colonist.map_char();
                            let (r, g, b) = colonist.color();
                            fg = Color::Rgb(r, g, b);
                            // Blend background with tribe color for visibility
                            let (tr, tg, tb) = Self::tribe_color(tribe_id);
                            let (br, bg_r, bb) = match bg {
                                Color::Rgb(r, g, b) => (r, g, b),
                                _ => (0, 0, 0),
                            };
                            bg = Color::Rgb(
                                ((br as f32 * 0.6 + tr as f32 * 0.4) as u8).min(255),
                                ((bg_r as f32 * 0.6 + tg as f32 * 0.4) as u8).min(255),
                                ((bb as f32 * 0.6 + tb as f32 * 0.4) as u8).min(255),
                            );
                        }
                    }
                }

                // Layer 4: Monster overlay (highest priority)
                if self.sim_mode {
                    if let Some(ref sim) = self.sim_state {
                        if let Some(monster) = sim.get_monster_at(&coord) {
                            ch = monster.species.map_char();
                            let (r, g, b) = monster.species.color();
                            fg = Color::Rgb(r, g, b);
                            // Tint background red for danger
                            let (br, bg_r, bb) = match bg {
                                Color::Rgb(r, g, b) => (r, g, b),
                                _ => (0, 0, 0),
                            };
                            bg = Color::Rgb(
                                ((br as f32 * 0.7 + 100.0 * 0.3) as u8).min(255),
                                (bg_r as f32 * 0.7) as u8,
                                (bb as f32 * 0.7) as u8,
                            );
                        }
                    }
                }

                // Layer 5: Cursor overlay (absolute highest priority)
                if is_cursor {
                    fg = Color::Black;
                    bg = Color::Yellow;
                }

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

        let (br, bg_col, bb) = tile.biome.color();
        let biome_style = Style::default()
            .fg(Color::Rgb(br, bg_col, bb))
            .add_modifier(Modifier::BOLD);

        // Water body color based on type
        let water_color = match tile.water_body_type {
            crate::water_bodies::WaterBodyType::Ocean => Color::Blue,
            crate::water_bodies::WaterBodyType::Lake => Color::Cyan,
            crate::water_bodies::WaterBodyType::River => Color::LightBlue,
            crate::water_bodies::WaterBodyType::None => Color::DarkGray,
        };

        // Build info text - different content based on simulation mode
        let info_text = if self.sim_mode {
            let cursor_coord = TileCoord::new(self.cursor_x, self.cursor_y);

            // First check for monster at cursor
            let monster_info = self.sim_state.as_ref().and_then(|sim| {
                sim.get_monster_at(&cursor_coord).map(|monster| {
                    let (mr, mg, mb) = monster.species.color();
                    let monster_color = Color::Rgb(mr, mg, mb);
                    let health_pct = (monster.health / monster.max_health * 100.0) as u32;
                    let health_color = if health_pct > 60 {
                        Color::Green
                    } else if health_pct > 30 {
                        Color::Yellow
                    } else {
                        Color::Red
                    };
                    let state_str = match monster.state {
                        crate::simulation::monsters::MonsterState::Idle => "Idle",
                        crate::simulation::monsters::MonsterState::Roaming => "Roaming",
                        crate::simulation::monsters::MonsterState::Hunting => "Hunting",
                        crate::simulation::monsters::MonsterState::Attacking(_) => "Attacking",
                        crate::simulation::monsters::MonsterState::Fleeing => "Fleeing",
                        crate::simulation::monsters::MonsterState::Dead => "Dead",
                    };
                    vec![
                        Line::from(vec![
                            Span::raw("Monster: "),
                            Span::styled(monster.species.name(), Style::default().fg(monster_color).add_modifier(Modifier::BOLD)),
                            Span::raw(format!(" ({})", monster.species.map_char())),
                        ]),
                        Line::from(vec![
                            Span::raw("Health: "),
                            Span::styled(format!("{:.0}/{:.0} ({:.0}%)", monster.health, monster.max_health, health_pct), Style::default().fg(health_color)),
                            Span::raw("  Strength: "),
                            Span::styled(format!("{:.1}", monster.strength), Style::default().fg(Color::Red)),
                        ]),
                        Line::from(vec![
                            Span::raw("State: "),
                            Span::styled(state_str, Style::default().fg(Color::Cyan)),
                            Span::raw("  Kills: "),
                            Span::styled(format!("{}", monster.kills), Style::default().fg(Color::Magenta)),
                        ]),
                    ]
                })
            });

            if let Some(info) = monster_info {
                info
            } else {
                // Check for colonist at cursor
                let colonist_info = self.sim_state.as_ref().and_then(|sim| {
                    sim.get_colonist_at(&cursor_coord).map(|(colonist, tribe_id)| {
                        let (cr, cg, cb) = colonist.color();
                        let colonist_color = Color::Rgb(cr, cg, cb);
                        let health_pct = (colonist.health * 100.0) as u32;
                        let health_color = if health_pct > 60 {
                            Color::Green
                        } else if health_pct > 30 {
                            Color::Yellow
                        } else {
                            Color::Red
                        };
                        let state_str = match colonist.activity_state {
                            crate::simulation::colonists::ColonistActivityState::Idle => "Idle",
                            crate::simulation::colonists::ColonistActivityState::Traveling => "Traveling",
                            crate::simulation::colonists::ColonistActivityState::Working => "Working",
                            crate::simulation::colonists::ColonistActivityState::Returning => "Returning",
                            crate::simulation::colonists::ColonistActivityState::Fleeing => "Fleeing",
                            crate::simulation::colonists::ColonistActivityState::Socializing => "Socializing",
                            crate::simulation::colonists::ColonistActivityState::Patrolling => "Patrolling",
                            crate::simulation::colonists::ColonistActivityState::Scouting => "Scouting",
                        };
                        let (tr, tg, tb) = Self::tribe_color(tribe_id);
                        let tribe_color = Color::Rgb(tr, tg, tb);
                        let job_str = colonist.current_job.map_or("None".to_string(), |j| format!("{:?}", j));
                        let role_str = format!("{:?}", colonist.role);

                        vec![
                            Line::from(vec![
                                Span::raw("Colonist: "),
                                Span::styled(&colonist.name, Style::default().fg(colonist_color).add_modifier(Modifier::BOLD)),
                                Span::raw(format!(" ({}) ", colonist.map_char())),
                                Span::styled(role_str, Style::default().fg(Color::Yellow)),
                            ]),
                            Line::from(vec![
                                Span::raw("Age: "),
                                Span::styled(format!("{}", colonist.age), Style::default().fg(Color::White)),
                                Span::raw("  Health: "),
                                Span::styled(format!("{:.0}%", health_pct), Style::default().fg(health_color)),
                                Span::raw("  Job: "),
                                Span::styled(job_str, Style::default().fg(Color::Cyan)),
                            ]),
                            Line::from(vec![
                                Span::raw("State: "),
                                Span::styled(state_str, Style::default().fg(Color::Magenta)),
                                Span::raw("  Mood: "),
                                Span::styled(format!("{:.0}", colonist.mood.current_mood), Style::default().fg(Color::Yellow)),
                                Span::raw("  Tribe: "),
                                Span::styled(format!("{}", tribe_id.0), Style::default().fg(tribe_color)),
                            ]),
                        ]
                    })
                });

                if let Some(info) = colonist_info {
                    info
                } else {
                // Check for tribe info
                let tribe_info = self.selected_tribe.and_then(|tribe_id| {
                    self.sim_state.as_ref().and_then(|sim| {
                        sim.tribes.get(&tribe_id).map(|tribe| {
                            let (tr, tg, tb) = Self::tribe_color(tribe_id);
                            let tribe_color = Color::Rgb(tr, tg, tb);

                            // Notable colonists summary
                            let notable_count = tribe.notable_colonists.count();

                            // Job summary - count workers assigned
                            let active_jobs = tribe.jobs.jobs.values()
                                .map(|j| j.total_workers())
                                .sum::<u32>();

                            // Get species reputations for this tribe
                            let reputations = sim.reputation.get_tribe_reputations(tribe_id);
                            let mut rep_spans: Vec<Span> = vec![Span::raw("Species: ")];
                            let mut rep_count = 0;
                            for (species, rep) in reputations.iter().take(4) {
                                if rep_count > 0 {
                                    rep_spans.push(Span::raw("  "));
                                }
                                let rep_color = if rep.is_vengeful() {
                                    Color::Red
                                } else if rep.is_hostile() {
                                    Color::LightRed
                                } else if rep.is_fearful() {
                                    Color::Green
                                } else if rep.is_tolerant() {
                                    Color::Yellow
                                } else {
                                    Color::DarkGray
                                };
                                rep_spans.push(Span::styled(
                                    format!("{}: {} ({})", species.name(), rep.status_label(), rep.current),
                                    Style::default().fg(rep_color)
                                ));
                                rep_count += 1;
                            }
                            if reputations.is_empty() {
                                rep_spans.push(Span::styled("None tracked", Style::default().fg(Color::DarkGray)));
                            }

                            vec![
                                Line::from(vec![
                                    Span::raw("Tribe: "),
                                    Span::styled(tribe.name.clone(), Style::default().fg(tribe_color).add_modifier(Modifier::BOLD)),
                                    Span::raw("  Society: "),
                                    Span::styled(format!("{:?}", tribe.society_state.society_type), Style::default().fg(Color::Cyan)),
                                    Span::raw("  Leader: "),
                                    Span::styled(tribe.society_state.leader_name.clone(), Style::default().fg(Color::Yellow)),
                                ]),
                                Line::from(vec![
                                    Span::raw("Pop: "),
                                    Span::styled(format!("{}", tribe.population.total()), Style::default().fg(Color::White)),
                                    Span::raw("  Notable: "),
                                    Span::styled(format!("{}", notable_count), Style::default().fg(Color::Magenta)),
                                    Span::raw("  Jobs: "),
                                    Span::styled(format!("{}", active_jobs), Style::default().fg(Color::Green)),
                                    Span::raw("  Age: "),
                                    Span::styled(format!("{:?}", tribe.tech_state.current_age()), Style::default().fg(Color::Yellow)),
                                ]),
                                Line::from(vec![
                                    Span::raw("Morale: "),
                                    Span::styled(format!("{:.0}%", tribe.needs.morale.satisfaction * 100.0), Style::default().fg(Color::Cyan)),
                                    Span::raw("  Food: "),
                                    Span::styled(format!("{:.0}%", tribe.needs.food.satisfaction * 100.0), Style::default().fg(Color::Green)),
                                    Span::raw("  Strength: "),
                                    Span::styled(format!("{:.1}", tribe.military_strength()), Style::default().fg(Color::Red)),
                                    Span::raw("  Culture: "),
                                    Span::styled(tribe.culture.lens.culture_name(), Style::default().fg(Color::Magenta)),
                                ]),
                                Line::from(rep_spans),
                            ]
                        })
                    })
                });

                tribe_info.unwrap_or_else(|| {
                    // No tribe, monster, or colonist at cursor - show biome info
                    vec![
                        Line::from(vec![
                            Span::raw("Cursor: "),
                            Span::styled(format!("({}, {})", self.cursor_x, self.cursor_y), Style::default().fg(Color::White)),
                            Span::raw("  Biome: "),
                            Span::styled(format!("{}", tile.biome.display_name()), biome_style),
                        ]),
                        Line::from(vec![
                            Span::raw("No tribe, monster, or colonist at this location"),
                        ]),
                        Line::from(vec![
                            Span::raw("Move cursor over territory to view info"),
                        ]),
                    ]
                })
                }
            }
        } else {
            // Normal mode: show tile info
            vec![
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
            ]
        };

        let info_panel_title = if self.sim_mode {
            // Determine title based on what's at cursor
            let cursor_coord = TileCoord::new(self.cursor_x, self.cursor_y);
            if let Some(ref sim) = self.sim_state {
                if sim.get_monster_at(&cursor_coord).is_some() {
                    " Monster Info "
                } else if sim.get_colonist_at(&cursor_coord).is_some() {
                    " Colonist Info "
                } else if self.selected_tribe.is_some() {
                    " Tribe Info "
                } else {
                    " Simulation Info "
                }
            } else {
                " Simulation Info "
            }
        } else {
            " Tile Info "
        };
        let info_panel = Paragraph::new(info_text)
            .block(Block::default().borders(Borders::ALL).title(info_panel_title));
        frame.render_widget(info_panel, chunks[info_chunk]);

        // Controls - different for simulation mode
        let controls_text = if self.sim_mode {
            if self.show_combat_log {
                "[←↑↓→] Move  [Space] Step/Pause  [+/-] Speed  [T] Territories  [Shift+L] Hide Log  [S] Stop  [?] Help"
            } else {
                "[←↑↓→] Move  [Space] Step/Pause  [+/-] Speed  [T] Territories  [Shift+L] Show Log  [S] Stop  [?] Help"
            }
        } else {
            "[←↑↓→/WASD] Move  [Enter] Local Map  [S] Start Sim  [V] View  [N] New  [?] Help  [Q] Quit"
        };
        let controls = Paragraph::new(controls_text)
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(controls, chunks[controls_chunk]);

        // Help overlay
        if self.show_help {
            self.render_help(frame);
        }
    }

    /// Render the combat log panel
    fn render_combat_log(&self, frame: &mut Frame, area: Rect) {
        let sim = match &self.sim_state {
            Some(s) => s,
            None => return,
        };

        // Get recent combat entries
        let recent_entries = sim.combat_log.recent_entries(4);
        let stats = sim.combat_log.stats();

        // Build combat log lines
        let mut lines: Vec<Line> = Vec::new();

        if recent_entries.is_empty() {
            lines.push(Line::from(vec![
                Span::styled("No combat events yet. ", Style::default().fg(Color::DarkGray)),
                Span::raw("Combat logs will appear here when tribes or monsters fight."),
            ]));
        } else {
            for entry in recent_entries.iter().rev() {
                let result_color = match &entry.result {
                    crate::simulation::combat::CombatResult::Kill { .. } => Color::Red,
                    crate::simulation::combat::CombatResult::Wound => Color::Yellow,
                    crate::simulation::combat::CombatResult::Hit => Color::Green,
                    crate::simulation::combat::CombatResult::Miss => Color::DarkGray,
                    _ => Color::White,
                };

                // Shorten narrative if too long
                let narrative = if entry.narrative.len() > 80 {
                    format!("{}...", &entry.narrative[..77])
                } else {
                    entry.narrative.clone()
                };

                lines.push(Line::from(vec![
                    Span::styled(
                        format!("[T{}] ", entry.tick),
                        Style::default().fg(Color::Cyan),
                    ),
                    Span::styled(narrative, Style::default().fg(result_color)),
                ]));
            }
        }

        // Add stats line
        lines.push(Line::from(vec![
            Span::styled(
                format!(
                    "Stats: {} encounters, {} attacks, {} kills, {} wounds",
                    stats.total_encounters, stats.total_attacks, stats.total_kills, stats.total_wounds
                ),
                Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC),
            ),
        ]));

        let combat_log = Paragraph::new(lines)
            .block(Block::default()
                .borders(Borders::ALL)
                .title(" Combat Log ")
                .border_style(Style::default().fg(Color::Red)));
        frame.render_widget(combat_log, area);
    }

    fn render_local(&self, frame: &mut Frame) {
        let local_map = match &self.current_local_map {
            Some(map) => map,
            None => return,
        };

        let size = frame.area();

        // Layout: header, map, info panel, controls
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),  // Header
                Constraint::Min(10),    // Map
                Constraint::Length(4),  // Info panel
                Constraint::Length(1),  // Controls
            ])
            .split(size);

        // Get world biome for header
        let world_biome = self.world.biomes.get(self.cursor_x, self.cursor_y);

        // Header
        let header = Paragraph::new(format!(
            "LOCAL VIEW - Tile ({}, {}) - {} - 64x64  [Enter/Esc] Return to World  [?] Help",
            self.cursor_x,
            self.cursor_y,
            world_biome.display_name(),
        ))
        .style(Style::default().fg(Color::Green));
        frame.render_widget(header, chunks[0]);

        // Map area with border
        let map_block = Block::default()
            .borders(Borders::ALL)
            .title(" Local Map ");
        let map_inner = map_block.inner(chunks[1]);
        frame.render_widget(map_block, chunks[1]);

        // Render local map tiles
        let map_width = map_inner.width as usize;
        let map_height = map_inner.height as usize;

        for vy in 0..map_height.min(self.local_viewport.height) {
            let local_y = self.local_viewport.y + vy;
            if local_y >= local_map.height {
                break;
            }

            for vx in 0..map_width.min(self.local_viewport.width) {
                let local_x = self.local_viewport.x + vx;
                if local_x >= local_map.width {
                    break;
                }

                let tile = local_map.get(local_x, local_y);
                let is_cursor = local_x == self.local_cursor_x && local_y == self.local_cursor_y;

                // Choose character: feature if present, else terrain with elevation hint
                let ch = if tile.feature.is_some() {
                    tile.ascii_char()
                } else {
                    // Add subtle elevation hint to terrain
                    let elev = tile.elevation_offset;
                    if elev > 0.3 {
                        '\'' // High terrain
                    } else if elev < -0.3 {
                        '_' // Low terrain
                    } else {
                        tile.terrain.ascii_char()
                    }
                };

                let (fg, bg) = if is_cursor {
                    (Color::Black, Color::Yellow)
                } else {
                    // Get elevation brightness
                    let brightness = tile.elevation_brightness();

                    // Background = terrain color with elevation shading
                    let (tr, tg, tb) = tile.terrain.color();
                    let bg = Color::Rgb(
                        ((tr as f32 * brightness * 0.6).min(255.0)) as u8,
                        ((tg as f32 * brightness * 0.6).min(255.0)) as u8,
                        ((tb as f32 * brightness * 0.6).min(255.0)) as u8,
                    );

                    // Foreground = feature color if present, else brightened terrain
                    let fg = if let Some((fr, fg_g, fb)) = tile.feature_color() {
                        Color::Rgb(
                            ((fr as f32 * brightness * 1.2).min(255.0)) as u8,
                            ((fg_g as f32 * brightness * 1.2).min(255.0)) as u8,
                            ((fb as f32 * brightness * 1.2).min(255.0)) as u8,
                        )
                    } else {
                        // Brighten terrain color for foreground
                        Color::Rgb(
                            ((tr as f32 * brightness * 1.4).min(255.0)) as u8,
                            ((tg as f32 * brightness * 1.4).min(255.0)) as u8,
                            ((tb as f32 * brightness * 1.4).min(255.0)) as u8,
                        )
                    };
                    (fg, bg)
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
        let tile = local_map.get(self.local_cursor_x, self.local_cursor_y);
        let terrain_name = format!("{:?}", tile.terrain);
        let feature_name = tile.feature.map_or("None".to_string(), |f| format!("{:?}", f));
        let walkable_str = if tile.walkable { "Yes" } else { "No" };
        let cost_str = if tile.movement_cost.is_finite() {
            format!("{:.1}", tile.movement_cost)
        } else {
            "Impassable".to_string()
        };
        let elevation_str = format!("{:+.2}", tile.elevation_offset);
        let elevation_color = if tile.elevation_offset > 0.2 {
            Color::Rgb(200, 220, 255) // High = light blue
        } else if tile.elevation_offset < -0.2 {
            Color::Rgb(100, 80, 60) // Low = brown
        } else {
            Color::Gray
        };

        let (tr, tg, tb) = tile.terrain.color();

        let info_text = vec![
            Line::from(vec![
                Span::raw("Position: "),
                Span::styled(format!("({}, {})", self.local_cursor_x, self.local_cursor_y), Style::default().fg(Color::White)),
                Span::raw("  Terrain: "),
                Span::styled(terrain_name, Style::default().fg(Color::Rgb(tr, tg, tb))),
                Span::raw("  Elev: "),
                Span::styled(elevation_str, Style::default().fg(elevation_color)),
            ]),
            Line::from(vec![
                Span::raw("Feature: "),
                Span::styled(feature_name, Style::default().fg(Color::Magenta)),
                Span::raw("  Walkable: "),
                Span::styled(walkable_str, Style::default().fg(if tile.walkable { Color::Green } else { Color::Red })),
                Span::raw("  Cost: "),
                Span::styled(cost_str, Style::default().fg(Color::Yellow)),
            ]),
        ];

        let info_panel = Paragraph::new(info_text)
            .block(Block::default().borders(Borders::ALL).title(" Tile Info "));
        frame.render_widget(info_panel, chunks[2]);

        // Controls
        let controls = Paragraph::new("[←↑↓→/WASD] Move  [Enter/Esc] Return to World  [C] Center  [?] Help")
            .style(Style::default().fg(Color::DarkGray));
        frame.render_widget(controls, chunks[3]);

        // Help overlay
        if self.show_help {
            self.render_local_help(frame);
        }
    }

    fn render_local_help(&self, frame: &mut Frame) {
        let area = frame.area();

        let popup_width = 48;
        let popup_height = 16;
        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

        frame.render_widget(Clear, popup_area);

        let help_text = vec![
            Line::from("Local Map Help").style(Style::default().add_modifier(Modifier::BOLD)),
            Line::from(""),
            Line::from("Navigation:").style(Style::default().add_modifier(Modifier::BOLD)),
            Line::from("  Arrow keys / WASD / HJKL - Move cursor"),
            Line::from("  PgUp/PgDn - Fast vertical movement"),
            Line::from("  Home/End  - Fast horizontal movement"),
            Line::from("  C - Center viewport on cursor"),
            Line::from("  0 - Jump to map center"),
            Line::from(""),
            Line::from("Other:").style(Style::default().add_modifier(Modifier::BOLD)),
            Line::from("  Enter/Esc - Return to world view"),
            Line::from("  ? - Toggle this help"),
            Line::from(""),
            Line::from("Local maps show detailed terrain for each"),
            Line::from("world tile. Edge tiles blend with neighbors."),
        ];

        let help_popup = Paragraph::new(help_text)
            .block(Block::default()
                .borders(Borders::ALL)
                .title(" Local Map Help ")
                .style(Style::default().bg(Color::DarkGray)))
            .style(Style::default().fg(Color::White).bg(Color::DarkGray));

        frame.render_widget(help_popup, popup_area);
    }

    fn render_help(&self, frame: &mut Frame) {
        let area = frame.area();

        // Simulation mode has a different help screen
        if self.sim_mode {
            self.render_simulation_help(frame);
            return;
        }

        // Center the help popup
        let popup_width = 50;
        let popup_height = 24;
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
            Line::from("  Enter - View local map (64x64 detail)"),
            Line::from(""),
            Line::from("Mouse:").style(Style::default().add_modifier(Modifier::BOLD)),
            Line::from("  Left click  - Move cursor to tile"),
            Line::from("  Right click - Center on tile"),
            Line::from("  Scroll      - Pan the viewport"),
            Line::from(""),
            Line::from("Views (V to cycle):").style(Style::default().add_modifier(Modifier::BOLD)),
            Line::from("  Biome / Height / Temp / Moisture / Stress"),
            Line::from(""),
            Line::from("Simulation:").style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Green)),
            Line::from("  Shift+S - Start civilization simulation"),
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

    fn render_simulation_help(&self, frame: &mut Frame) {
        let area = frame.area();

        let popup_width = 54;
        let popup_height = 26;
        let popup_x = (area.width.saturating_sub(popup_width)) / 2;
        let popup_y = (area.height.saturating_sub(popup_height)) / 2;

        let popup_area = Rect::new(popup_x, popup_y, popup_width, popup_height);

        frame.render_widget(Clear, popup_area);

        let help_text = vec![
            Line::from("Simulation Controls:").style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Green)),
            Line::from("  Space     - Step (when paused) / Pause (running)"),
            Line::from("  +/=       - Increase simulation speed"),
            Line::from("  -/_       - Decrease simulation speed"),
            Line::from("  T         - Toggle territory overlay"),
            Line::from("  Shift+L   - Toggle combat log panel"),
            Line::from("  Shift+S   - Stop simulation and return to explore"),
            Line::from(""),
            Line::from("Navigation:").style(Style::default().add_modifier(Modifier::BOLD)),
            Line::from("  Arrow keys / WASD / HJKL - Move cursor"),
            Line::from("  PgUp/PgDn - Fast vertical movement"),
            Line::from("  C - Center viewport on cursor"),
            Line::from(""),
            Line::from("Colony System:").style(Style::default().add_modifier(Modifier::BOLD).fg(Color::Cyan)),
            Line::from("  Each tribe has a society type (Monarchy,"),
            Line::from("  Theocracy, Democracy, etc.) affecting bonuses."),
            Line::from("  Notable colonists have skills and jobs."),
            Line::from(""),
            Line::from("Move cursor over territory to see tribe details"),
            Line::from("including society, leader, colonists, and jobs."),
            Line::from(""),
            Line::from("  ? - Toggle this help    Q/Esc - Quit"),
        ];

        let help_popup = Paragraph::new(help_text)
            .block(Block::default()
                .borders(Borders::ALL)
                .title(" Simulation Help ")
                .border_style(Style::default().fg(Color::Green))
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
