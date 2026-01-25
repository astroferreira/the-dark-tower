//! Pre-generation menu for configuring world parameters

use std::error::Error;
use std::io::stdout;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode},
    execute,
    terminal::{self, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Paragraph},
};

use crate::climate::{ClimateConfig, ClimateMode, RainfallLevel};
use crate::erosion::params::ErosionPreset;
use crate::plates::WorldStyle;

/// Result of running the menu
pub enum MenuResult {
    /// User chose to generate with these settings
    Generate(WorldConfig),
    /// User quit
    Quit,
}

/// World generation configuration
#[derive(Clone)]
pub struct WorldConfig {
    pub width: usize,
    pub height: usize,
    pub seed: Option<u64>,
    pub plates: Option<usize>,
    pub world_style: WorldStyle,
    pub erosion_preset: ErosionPreset,
    pub climate_mode: ClimateMode,
    pub rainfall: RainfallLevel,
}

impl Default for WorldConfig {
    fn default() -> Self {
        Self {
            width: 512,
            height: 256,
            seed: None,
            plates: None,
            world_style: WorldStyle::Earthlike,
            erosion_preset: ErosionPreset::Normal,
            climate_mode: ClimateMode::Globe,
            rainfall: RainfallLevel::Normal,
        }
    }
}

/// Currently selected field
#[derive(Clone, Copy, PartialEq, Eq)]
enum MenuField {
    Width,
    Height,
    Seed,
    Plates,
    WorldStyle,
    Erosion,
    Climate,
    Rainfall,
    Generate,
    Quit,
}

impl MenuField {
    fn next(&self) -> MenuField {
        match self {
            MenuField::Width => MenuField::Height,
            MenuField::Height => MenuField::Seed,
            MenuField::Seed => MenuField::Plates,
            MenuField::Plates => MenuField::WorldStyle,
            MenuField::WorldStyle => MenuField::Erosion,
            MenuField::Erosion => MenuField::Climate,
            MenuField::Climate => MenuField::Rainfall,
            MenuField::Rainfall => MenuField::Generate,
            MenuField::Generate => MenuField::Quit,
            MenuField::Quit => MenuField::Width,
        }
    }

    fn prev(&self) -> MenuField {
        match self {
            MenuField::Width => MenuField::Quit,
            MenuField::Height => MenuField::Width,
            MenuField::Seed => MenuField::Height,
            MenuField::Plates => MenuField::Seed,
            MenuField::WorldStyle => MenuField::Plates,
            MenuField::Erosion => MenuField::WorldStyle,
            MenuField::Climate => MenuField::Erosion,
            MenuField::Rainfall => MenuField::Climate,
            MenuField::Generate => MenuField::Rainfall,
            MenuField::Quit => MenuField::Generate,
        }
    }

    fn is_numeric(&self) -> bool {
        matches!(self, MenuField::Width | MenuField::Height | MenuField::Seed | MenuField::Plates)
    }

    fn is_cyclable(&self) -> bool {
        matches!(self, MenuField::WorldStyle | MenuField::Erosion | MenuField::Climate | MenuField::Rainfall)
    }
}

/// Menu state
struct Menu {
    config: WorldConfig,
    selected: MenuField,
    editing: bool,
    input_buffer: String,
}

impl Menu {
    fn new(config: WorldConfig) -> Self {
        Self {
            config,
            selected: MenuField::Width,
            editing: false,
            input_buffer: String::new(),
        }
    }

    fn cycle_world_style(&mut self, forward: bool) {
        let styles = WorldStyle::all();
        let current_idx = styles.iter().position(|&s| s == self.config.world_style).unwrap_or(0);
        let new_idx = if forward {
            (current_idx + 1) % styles.len()
        } else {
            (current_idx + styles.len() - 1) % styles.len()
        };
        self.config.world_style = styles[new_idx];
    }

    fn cycle_erosion(&mut self, forward: bool) {
        let presets = ErosionPreset::all();
        let current_idx = presets.iter().position(|&p| p == self.config.erosion_preset).unwrap_or(0);
        let new_idx = if forward {
            (current_idx + 1) % presets.len()
        } else {
            (current_idx + presets.len() - 1) % presets.len()
        };
        self.config.erosion_preset = presets[new_idx];
    }

    fn cycle_climate(&mut self, forward: bool) {
        let modes = ClimateMode::all();
        let current_idx = modes.iter().position(|&m| m == self.config.climate_mode).unwrap_or(0);
        let new_idx = if forward {
            (current_idx + 1) % modes.len()
        } else {
            (current_idx + modes.len() - 1) % modes.len()
        };
        self.config.climate_mode = modes[new_idx];
    }

    fn cycle_rainfall(&mut self, forward: bool) {
        let levels = RainfallLevel::all();
        let current_idx = levels.iter().position(|&l| l == self.config.rainfall).unwrap_or(0);
        let new_idx = if forward {
            (current_idx + 1) % levels.len()
        } else {
            (current_idx + levels.len() - 1) % levels.len()
        };
        self.config.rainfall = levels[new_idx];
    }

    fn cycle_selected(&mut self, forward: bool) {
        match self.selected {
            MenuField::WorldStyle => self.cycle_world_style(forward),
            MenuField::Erosion => self.cycle_erosion(forward),
            MenuField::Climate => self.cycle_climate(forward),
            MenuField::Rainfall => self.cycle_rainfall(forward),
            _ => {}
        }
    }

    fn start_editing(&mut self) {
        if self.selected.is_numeric() {
            self.editing = true;
            self.input_buffer = match self.selected {
                MenuField::Width => self.config.width.to_string(),
                MenuField::Height => self.config.height.to_string(),
                MenuField::Seed => self.config.seed.map(|s| s.to_string()).unwrap_or_default(),
                MenuField::Plates => self.config.plates.map(|p| p.to_string()).unwrap_or_default(),
                _ => String::new(),
            };
        }
    }

    fn confirm_edit(&mut self) {
        if !self.editing {
            return;
        }

        match self.selected {
            MenuField::Width => {
                if let Ok(val) = self.input_buffer.parse::<usize>() {
                    self.config.width = val.clamp(64, 4096);
                }
            }
            MenuField::Height => {
                if let Ok(val) = self.input_buffer.parse::<usize>() {
                    self.config.height = val.clamp(32, 2048);
                }
            }
            MenuField::Seed => {
                if self.input_buffer.is_empty() {
                    self.config.seed = None;
                } else if let Ok(val) = self.input_buffer.parse::<u64>() {
                    self.config.seed = Some(val);
                }
            }
            MenuField::Plates => {
                if self.input_buffer.is_empty() {
                    self.config.plates = None;
                } else if let Ok(val) = self.input_buffer.parse::<usize>() {
                    self.config.plates = Some(val.clamp(3, 30));
                }
            }
            _ => {}
        }

        self.editing = false;
        self.input_buffer.clear();
    }

    fn cancel_edit(&mut self) {
        self.editing = false;
        self.input_buffer.clear();
    }

    fn render(&self, frame: &mut Frame) {
        let area = frame.area();

        // Clear background
        frame.render_widget(
            Block::default().style(Style::default().bg(Color::Black)),
            area,
        );

        // Calculate centered box
        let box_width: u16 = 56;
        let box_height: u16 = 22;
        let box_x = (area.width.saturating_sub(box_width)) / 2;
        let box_y = (area.height.saturating_sub(box_height)) / 2;

        let box_area = Rect::new(box_x, box_y, box_width, box_height);

        // Main box
        let block = Block::default()
            .title(" Planet Generator - Setup ")
            .title_alignment(Alignment::Center)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Cyan));

        let inner = block.inner(box_area);
        frame.render_widget(block, box_area);

        // Content
        let mut y = inner.y;

        // === Map Settings ===
        self.render_section_header(frame, inner.x + 2, y, "Map Settings");
        y += 1;

        self.render_field(frame, inner.x + 3, y, "Width:", self.format_width(), MenuField::Width);
        y += 1;

        self.render_field(frame, inner.x + 3, y, "Height:", self.format_height(), MenuField::Height);
        y += 1;

        self.render_field(frame, inner.x + 3, y, "Seed:", self.format_seed(), MenuField::Seed);
        y += 1;

        self.render_field(frame, inner.x + 3, y, "Plates:", self.format_plates(), MenuField::Plates);
        y += 1;

        self.render_cycle_field(frame, inner.x + 3, y, "World Style:", &format!("{}", self.config.world_style), MenuField::WorldStyle);
        y += 2;

        // === Simulation Settings ===
        self.render_section_header(frame, inner.x + 2, y, "Simulation");
        y += 1;

        self.render_cycle_field(frame, inner.x + 3, y, "Erosion:", &format!("{}", self.config.erosion_preset), MenuField::Erosion);
        y += 1;

        self.render_cycle_field(frame, inner.x + 3, y, "Climate:", &format!("{}", self.config.climate_mode), MenuField::Climate);
        y += 1;

        self.render_cycle_field(frame, inner.x + 3, y, "Rainfall:", &format!("{}", self.config.rainfall), MenuField::Rainfall);
        y += 2;

        // Description of currently selected cyclable field
        let desc = self.get_selected_description();
        if !desc.is_empty() {
            let desc_text = Paragraph::new(desc)
                .style(Style::default().fg(Color::DarkGray).add_modifier(Modifier::ITALIC))
                .alignment(Alignment::Center);
            frame.render_widget(desc_text, Rect::new(inner.x, y, inner.width, 1));
        }
        y += 2;

        // Separator line
        let sep_y = y;
        for x in box_area.x + 1..box_area.x + box_width - 1 {
            frame.buffer_mut()[(x, sep_y)].set_char('─').set_fg(Color::Cyan);
        }
        y += 1;

        // Buttons
        self.render_buttons(frame, inner.x, y, inner.width);

        // Help text at bottom
        let help_y = box_area.y + box_height;
        if help_y < area.height {
            let help = if self.editing {
                "Type value, Enter: Confirm, Esc: Cancel"
            } else {
                "↑↓/jk: Navigate  Enter: Edit  ←→/hl: Cycle  q: Quit"
            };
            let help_text = Paragraph::new(help)
                .style(Style::default().fg(Color::DarkGray))
                .alignment(Alignment::Center);
            frame.render_widget(help_text, Rect::new(box_x, help_y, box_width, 1));
        }
    }

    fn render_section_header(&self, frame: &mut Frame, x: u16, y: u16, title: &str) {
        let style = Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD);
        frame.render_widget(
            Paragraph::new(format!("─ {} ─", title)).style(style),
            Rect::new(x, y, title.len() as u16 + 6, 1),
        );
    }

    fn get_selected_description(&self) -> &'static str {
        match self.selected {
            MenuField::WorldStyle => self.config.world_style.description(),
            MenuField::Erosion => self.config.erosion_preset.description(),
            MenuField::Climate => self.config.climate_mode.description(),
            MenuField::Rainfall => self.config.rainfall.description(),
            _ => "",
        }
    }

    fn render_field(&self, frame: &mut Frame, x: u16, y: u16, label: &str, value: String, field: MenuField) {
        let is_selected = self.selected == field;
        let is_editing = self.editing && is_selected;

        // Label
        let label_style = if is_selected {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        };
        frame.render_widget(
            Paragraph::new(format!("{:<14}", label)).style(label_style),
            Rect::new(x, y, 14, 1),
        );

        // Value box
        let display_value = if is_editing {
            format!("{}_", self.input_buffer)
        } else {
            value
        };

        let value_style = if is_selected {
            Style::default().fg(Color::Black).bg(Color::Yellow)
        } else {
            Style::default().fg(Color::White).bg(Color::DarkGray)
        };

        frame.render_widget(
            Paragraph::new(format!(" {:<14}", display_value)).style(value_style),
            Rect::new(x + 14, y, 16, 1),
        );
    }

    fn render_cycle_field(&self, frame: &mut Frame, x: u16, y: u16, label: &str, value: &str, field: MenuField) {
        let is_selected = self.selected == field;

        // Label
        let label_style = if is_selected {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::White)
        };
        frame.render_widget(
            Paragraph::new(format!("{:<14}", label)).style(label_style),
            Rect::new(x, y, 14, 1),
        );

        // Left arrow
        let arrow_style = if is_selected {
            Style::default().fg(Color::Yellow)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        frame.render_widget(
            Paragraph::new("< ").style(arrow_style),
            Rect::new(x + 14, y, 2, 1),
        );

        // Value
        let value_style = if is_selected {
            Style::default().fg(Color::Black).bg(Color::Yellow)
        } else {
            Style::default().fg(Color::White).bg(Color::DarkGray)
        };
        frame.render_widget(
            Paragraph::new(format!(" {:<12}", value)).style(value_style),
            Rect::new(x + 16, y, 14, 1),
        );

        // Right arrow
        frame.render_widget(
            Paragraph::new(" >").style(arrow_style),
            Rect::new(x + 30, y, 2, 1),
        );
    }

    fn render_buttons(&self, frame: &mut Frame, x: u16, y: u16, width: u16) {
        let gen_selected = self.selected == MenuField::Generate;
        let quit_selected = self.selected == MenuField::Quit;

        // Calculate button positions for centering
        let gen_text = "[ Generate ]";
        let quit_text = "[ Quit ]";
        let total_width = gen_text.len() + 8 + quit_text.len();
        let start_x = x + (width.saturating_sub(total_width as u16)) / 2;

        // Generate button
        let gen_style = if gen_selected {
            Style::default().fg(Color::Black).bg(Color::Green)
        } else {
            Style::default().fg(Color::Green)
        };
        frame.render_widget(
            Paragraph::new(gen_text).style(gen_style),
            Rect::new(start_x, y, gen_text.len() as u16, 1),
        );

        // Quit button
        let quit_style = if quit_selected {
            Style::default().fg(Color::Black).bg(Color::Red)
        } else {
            Style::default().fg(Color::Red)
        };
        frame.render_widget(
            Paragraph::new(quit_text).style(quit_style),
            Rect::new(start_x + gen_text.len() as u16 + 8, y, quit_text.len() as u16, 1),
        );
    }

    fn format_width(&self) -> String {
        self.config.width.to_string()
    }

    fn format_height(&self) -> String {
        self.config.height.to_string()
    }

    fn format_seed(&self) -> String {
        self.config.seed.map(|s| s.to_string()).unwrap_or_else(|| "random".to_string())
    }

    fn format_plates(&self) -> String {
        self.config.plates.map(|p| p.to_string()).unwrap_or_else(|| "auto".to_string())
    }
}

/// Run the pre-generation menu
pub fn run_menu(initial: WorldConfig) -> Result<MenuResult, Box<dyn Error>> {
    // Setup terminal
    terminal::enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut menu = Menu::new(initial);
    let result;

    loop {
        // Render
        terminal.draw(|f| menu.render(f))?;

        // Handle input
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if menu.editing {
                    // Input mode
                    match key.code {
                        KeyCode::Enter => menu.confirm_edit(),
                        KeyCode::Esc => menu.cancel_edit(),
                        KeyCode::Backspace => {
                            menu.input_buffer.pop();
                        }
                        KeyCode::Char(c) if c.is_ascii_digit() => {
                            menu.input_buffer.push(c);
                        }
                        _ => {}
                    }
                } else {
                    // Navigation mode
                    match key.code {
                        KeyCode::Char('q') | KeyCode::Esc => {
                            result = MenuResult::Quit;
                            break;
                        }
                        KeyCode::Up | KeyCode::Char('k') => {
                            menu.selected = menu.selected.prev();
                        }
                        KeyCode::Down | KeyCode::Char('j') => {
                            menu.selected = menu.selected.next();
                        }
                        KeyCode::Left | KeyCode::Char('h') => {
                            if menu.selected.is_cyclable() {
                                menu.cycle_selected(false);
                            }
                        }
                        KeyCode::Right | KeyCode::Char('l') => {
                            if menu.selected.is_cyclable() {
                                menu.cycle_selected(true);
                            }
                        }
                        KeyCode::Enter => {
                            match menu.selected {
                                MenuField::Generate => {
                                    result = MenuResult::Generate(menu.config.clone());
                                    break;
                                }
                                MenuField::Quit => {
                                    result = MenuResult::Quit;
                                    break;
                                }
                                _ if menu.selected.is_cyclable() => {
                                    menu.cycle_selected(true);
                                }
                                _ => {
                                    menu.start_editing();
                                }
                            }
                        }
                        KeyCode::Tab => {
                            menu.selected = menu.selected.next();
                        }
                        KeyCode::BackTab => {
                            menu.selected = menu.selected.prev();
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    // Cleanup
    terminal::disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    Ok(result)
}
