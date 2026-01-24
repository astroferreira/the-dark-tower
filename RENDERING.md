# Rendering System

Comprehensive documentation of the procedural world generator's map rendering and visualization systems.

---

## Table of Contents

1. [Overview](#overview)
2. [Terminal Explorer (ratatui)](#terminal-explorer-ratatui)
3. [View Modes](#view-modes)
4. [Color Systems](#color-systems)
5. [Biome Rendering](#biome-rendering)
6. [Height/Elevation Rendering](#heightelevation-rendering)
7. [Temperature Rendering](#temperature-rendering)
8. [Moisture Rendering](#moisture-rendering)
9. [Plate Tectonics Rendering](#plate-tectonics-rendering)
10. [River Rendering](#river-rendering)
11. [Region Map Rendering](#region-map-rendering)
12. [PNG Export](#png-export)
13. [ASCII Export](#ascii-export)
14. [Module Reference](#module-reference)

---

## Overview

The rendering system provides multiple visualization methods:

| System | Output | Use Case |
|--------|--------|----------|
| **Terminal Explorer** | Live TUI (ratatui) | Interactive exploration |
| **PNG Export** | Image files | Static maps, sharing |
| **ASCII Export** | Text files | Debugging, portability |

All rendering uses consistent color mappings defined in `src/ascii.rs` and `src/biomes.rs`.

---

## Terminal Explorer (ratatui)

**File**: `src/explorer.rs`

The interactive terminal UI uses [ratatui](https://github.com/ratatui-org/ratatui) for rendering.

### Architecture

```rust
struct Explorer {
    world: WorldData,           // All world data
    cursor_x: usize,            // Current X position
    cursor_y: usize,            // Current Y position
    view_mode: ViewMode,        // Current visualization mode
    zoom: usize,                // Zoom level (1, 2, 4, 8, 16)
    show_panel: bool,           // Tile info panel visibility
    show_region_map: bool,      // Region detail panel
    show_help: bool,            // Help overlay
}
```

### Main Render Loop

```rust
pub fn run_explorer(world: WorldData) -> Result<(), Box<dyn Error>> {
    let mut terminal = Terminal::new(CrosstermBackend::new(stdout()))?;
    let mut explorer = Explorer::new(world);

    loop {
        terminal.draw(|f| {
            // Layout: content area + status bar
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Min(1), Constraint::Length(1)])
                .split(f.area());

            // Render map viewport
            explorer.render_map(chunks[0], f.buffer_mut());

            // Render status bar
            let status = format!("({},{}) | {} | ...",
                explorer.cursor_x, explorer.cursor_y,
                explorer.view_mode.name());
            f.render_widget(Paragraph::new(status), chunks[1]);

            // Optional overlays
            if explorer.show_panel {
                explorer.render_tile_panel(panel_area, f.buffer_mut());
            }
            if explorer.show_help {
                explorer.render_help(map_area, f.buffer_mut());
            }
        })?;

        // Handle input events
        if event::poll(Duration::from_millis(50))? {
            match event::read()? {
                Event::Key(key) => { /* handle navigation, mode changes */ }
                Event::Mouse(mouse) => { /* handle clicks */ }
                _ => {}
            }
        }
    }
}
```

### Viewport Calculation

The map viewport centers on the cursor and accounts for zoom:

```rust
fn render_map(&self, area: Rect, buf: &mut Buffer) {
    let zoom = self.zoom;
    let view_width = area.width as usize;
    let view_height = area.height as usize;

    // Map area visible = screen size * zoom
    let map_view_width = view_width * zoom;
    let map_view_height = view_height * zoom;

    // Center viewport on cursor
    let start_x = self.cursor_x.saturating_sub(map_view_width / 2);
    let start_y = self.cursor_y.saturating_sub(map_view_height / 2);

    for dy in 0..view_height {
        for dx in 0..view_width {
            // Sample from map at zoom intervals
            let map_x = (start_x + dx * zoom) % width;  // Horizontal wrap
            let map_y = (start_y + dy * zoom).min(height - 1);

            let (ch, fg, bg) = self.get_tile_display(map_x, map_y);

            // Highlight cursor
            let style = if is_cursor_cell {
                Style::default().fg(Color::Black).bg(Color::Yellow)
            } else {
                Style::default().fg(fg).bg(bg)
            };

            buf.get_mut(screen_x, screen_y).set_char(ch).set_style(style);
        }
    }
}
```

---

## View Modes

**File**: `src/explorer.rs`

The explorer supports 13 view modes, cycled with `V`:

```rust
enum ViewMode {
    Biome,          // Biome types with custom colors
    BaseBiome,      // Parent biome (ignores special variants)
    Height,         // Elevation gradient
    Temperature,    // Temperature gradient
    Moisture,       // Moisture/precipitation
    Plates,         // Tectonic plate colors
    Stress,         // Convergent/divergent boundaries
    Rivers,         // River network overlay
    BiomeBlend,     // Biome boundary edges
    Coastline,      // Coastal tile highlighting
    WeatherZones,   // Extreme weather risk areas
    Microclimate,   // Local temperature modifiers
    SeasonalTemp,   // Temperature with seasonal variation
}
```

### Mode-Specific Rendering

```rust
fn get_tile_display(&self, x: usize, y: usize) -> (char, Color, Color) {
    match self.view_mode {
        ViewMode::Biome => {
            let biome = *self.world.biomes.get(x, y);
            let ch = biome_char(&biome);
            let (r, g, b) = biome.color();
            (ch, make_fg_color(r, g, b), make_bg_color(r, g, b))
        }
        ViewMode::Height => {
            let h = *self.world.heightmap.get(x, y);
            let ch = if h < 0.0 { '~' } else { '.' };
            let (r, g, b) = height_color(h);
            (ch, make_fg_color(r, g, b), make_bg_color(r, g, b))
        }
        // ... other modes
    }
}
```

---

## Color Systems

**File**: `src/ascii.rs`

### Foreground/Background Contrast

Colors are split into foreground (character) and background (cell fill) for readability:

```rust
/// Darken color for background (35% brightness)
fn make_bg_color(r: u8, g: u8, b: u8) -> Color {
    let factor = 0.35;
    Color::Rgb(
        (r as f32 * factor) as u8,
        (g as f32 * factor) as u8,
        (b as f32 * factor) as u8,
    )
}

/// Brighten color for foreground (+40 per channel)
fn make_fg_color(r: u8, g: u8, b: u8) -> Color {
    Color::Rgb(
        (r as u16 + 40).min(255) as u8,
        (g as u16 + 40).min(255) as u8,
        (b as u16 + 40).min(255) as u8,
    )
}
```

### ANSI True Color

For terminal output with 24-bit color:

```rust
/// Format character with ANSI true color escape codes
pub fn ansi_colored_char(ch: char, fg: (u8, u8, u8), bg: (u8, u8, u8)) -> String {
    format!(
        "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m{}\x1b[0m",
        fg.0, fg.1, fg.2,  // Foreground RGB
        bg.0, bg.1, bg.2,  // Background RGB
        ch
    )
}
```

---

## Biome Rendering

**File**: `src/biomes.rs`, `src/ascii.rs`

### Biome Colors

Each biome has a defined RGB color:

```rust
impl ExtendedBiome {
    pub fn color(&self) -> (u8, u8, u8) {
        match self {
            // Ocean biomes
            ExtendedBiome::DeepOcean => (20, 40, 80),       // Dark blue
            ExtendedBiome::Ocean => (30, 60, 120),          // Blue
            ExtendedBiome::CoastalWater => (60, 100, 160),  // Light blue

            // Cold biomes
            ExtendedBiome::Ice => (240, 250, 255),          // White-blue
            ExtendedBiome::Tundra => (180, 190, 170),       // Gray-green
            ExtendedBiome::BorealForest => (50, 80, 50),    // Dark green

            // Temperate biomes
            ExtendedBiome::TemperateGrassland => (140, 170, 80),   // Yellow-green
            ExtendedBiome::TemperateForest => (40, 100, 40),       // Green
            ExtendedBiome::TemperateRainforest => (30, 80, 50),    // Dark green

            // Warm biomes
            ExtendedBiome::Desert => (210, 180, 120),       // Tan/sand
            ExtendedBiome::Savanna => (170, 160, 80),       // Yellow-brown
            ExtendedBiome::TropicalForest => (30, 120, 30), // Bright green
            ExtendedBiome::TropicalRainforest => (20, 90, 40), // Dark green

            // Mountain biomes
            ExtendedBiome::AlpineTundra => (140, 140, 130), // Gray
            ExtendedBiome::SnowyPeaks => (255, 255, 255),   // White

            // Fantasy biomes (100+ types with unique colors)
            ExtendedBiome::LavaLake => (255, 80, 0),        // Orange-red
            ExtendedBiome::CrystalForest => (180, 220, 255), // Light cyan
            ExtendedBiome::MushroomForest => (160, 100, 180), // Purple
            // ... many more
        }
    }
}
```

### Biome Characters

Each biome has an ASCII character for text rendering:

```rust
pub fn biome_char(biome: &ExtendedBiome) -> char {
    match biome {
        // Water
        ExtendedBiome::DeepOcean => '~',
        ExtendedBiome::Ocean => '.',
        ExtendedBiome::CoastalWater => ',',

        // Cold
        ExtendedBiome::Ice => '#',
        ExtendedBiome::Tundra => ':',
        ExtendedBiome::BorealForest => 'B',

        // Temperate
        ExtendedBiome::TemperateGrassland => '"',
        ExtendedBiome::TemperateForest => 'T',
        ExtendedBiome::TemperateRainforest => 'R',

        // Warm
        ExtendedBiome::Desert => 'd',
        ExtendedBiome::Savanna => ';',
        ExtendedBiome::TropicalForest => 't',
        ExtendedBiome::TropicalRainforest => 'r',

        // Mountain
        ExtendedBiome::AlpineTundra => '^',
        ExtendedBiome::SnowyPeaks => 'A',

        // Fantasy (Unicode for special biomes)
        ExtendedBiome::LavaLake => '@',
        ExtendedBiome::CoralReef => '⌇',
        ExtendedBiome::OceanicTrench => '▼',
        ExtendedBiome::ThermalVents => '♨',
        // ... many more
    }
}
```

---

## Height/Elevation Rendering

**File**: `src/ascii.rs`

### Height Color Gradient

Elevation maps to a color gradient from deep ocean to snow peaks:

```rust
pub fn height_color(elevation: f32) -> (u8, u8, u8) {
    // Range: -4000m to +4000m, normalized to 0.0-1.0
    let normalized = ((elevation + 4000.0) / 8000.0).clamp(0.0, 1.0);

    if normalized < 0.4 {
        // Ocean: deep blue to light blue
        let t = normalized / 0.4;
        let r = (20.0 + t * 40.0) as u8;
        let g = (40.0 + t * 60.0) as u8;
        let b = (100.0 + t * 60.0) as u8;
        (r, g, b)
    } else if normalized < 0.5 {
        // Coastal/Beach: sandy tan
        (210, 190, 140)
    } else if normalized < 0.65 {
        // Lowlands: green
        let t = (normalized - 0.5) / 0.15;
        ((80.0 - t * 30.0) as u8, (140.0 + t * 20.0) as u8, (60.0 - t * 20.0) as u8)
    } else if normalized < 0.8 {
        // Hills: brown/tan
        let t = (normalized - 0.65) / 0.15;
        ((100.0 + t * 40.0) as u8, (100.0 - t * 20.0) as u8, (70.0 - t * 20.0) as u8)
    } else if normalized < 0.92 {
        // Mountains: gray rock
        let t = (normalized - 0.8) / 0.12;
        ((120.0 + t * 40.0) as u8, (110.0 + t * 40.0) as u8, (100.0 + t * 50.0) as u8)
    } else {
        // Snow peaks: white
        let t = (normalized - 0.92) / 0.08;
        let v = (200.0 + t * 55.0) as u8;
        (v, v, v.min(255))
    }
}
```

### Height Gradient Visualization

```
Elevation Range         Color              Normalized
─────────────────────────────────────────────────────
-4000m (deep ocean)     Dark blue          0.0
-2000m (ocean)          Medium blue        0.25
   0m (sea level)       Light blue         0.5
 100m (coast)           Sandy tan          0.51
 500m (lowlands)        Green              0.56
1500m (hills)           Brown              0.69
2500m (mountains)       Gray               0.81
3500m (high peaks)      Light gray         0.94
4000m (snow)            White              1.0
```

---

## Temperature Rendering

**File**: `src/ascii.rs`

### Temperature Color Gradient

Temperature maps from cold (blue) through mild (green/yellow) to hot (red):

```rust
pub fn temperature_color(temp: f32) -> (u8, u8, u8) {
    // Range: -30°C to +30°C, normalized to 0.0-1.0
    let normalized = ((temp + 30.0) / 60.0).clamp(0.0, 1.0);

    if normalized < 0.3 {
        // Cold: deep blue to cyan
        let t = normalized / 0.3;
        ((50.0 + t * 100.0) as u8, (100.0 + t * 100.0) as u8, (200.0 + t * 55.0) as u8)
    } else if normalized < 0.5 {
        // Cool: cyan to green
        let t = (normalized - 0.3) / 0.2;
        ((150.0 - t * 50.0) as u8, (200.0 + t * 30.0) as u8, (255.0 - t * 155.0) as u8)
    } else if normalized < 0.7 {
        // Mild: green to yellow
        let t = (normalized - 0.5) / 0.2;
        ((100.0 + t * 155.0) as u8, (230.0 - t * 30.0) as u8, (100.0 - t * 50.0) as u8)
    } else {
        // Hot: yellow to red
        let t = (normalized - 0.7) / 0.3;
        (255, (200.0 - t * 150.0) as u8, (50.0 + t * 20.0) as u8)
    }
}
```

### Temperature Gradient Visualization

```
Temperature    Color           Description
────────────────────────────────────────────
-30°C          Deep blue       Polar/glacial
-15°C          Cyan            Arctic
  0°C          Green-cyan      Cold temperate
 10°C          Green           Temperate
 20°C          Yellow          Warm
 30°C          Orange/red      Hot/tropical
```

---

## Moisture Rendering

**File**: `src/ascii.rs`

### Moisture Color Gradient

Moisture maps from dry (tan) through moderate (green) to wet (blue):

```rust
pub fn moisture_color(moisture: f32) -> (u8, u8, u8) {
    // Range: 0.0 (dry) to 1.0 (wet)
    let m = moisture.clamp(0.0, 1.0);

    if m < 0.3 {
        // Dry: tan/brown
        let t = m / 0.3;
        ((210.0 - t * 50.0) as u8, (180.0 - t * 30.0) as u8, (120.0 + t * 30.0) as u8)
    } else if m < 0.6 {
        // Moderate: greenish
        let t = (m - 0.3) / 0.3;
        ((160.0 - t * 80.0) as u8, (150.0 + t * 50.0) as u8, (150.0 - t * 50.0) as u8)
    } else {
        // Wet: blue-green to blue
        let t = (m - 0.6) / 0.4;
        ((80.0 - t * 40.0) as u8, (200.0 - t * 80.0) as u8, (100.0 + t * 100.0) as u8)
    }
}
```

---

## Plate Tectonics Rendering

**File**: `src/explorer.rs`

### Plate Color Assignment

Each plate gets a unique color based on its ID and type:

```rust
ViewMode::Plates => {
    let plate_id = *self.world.plate_map.get(x, y);
    let plate_idx = plate_id.0 as usize;

    if plate_idx < self.world.plates.len() {
        // Use plate's assigned color
        let [r, g, b] = self.world.plates[plate_idx].color;
        (ch, make_fg_color(r, g, b), make_bg_color(r, g, b))
    } else {
        (ch, Color::Gray, Color::Rgb(30, 30, 30))
    }
}
```

### Stress Color Gradient

Stress shows convergent (red) vs divergent (blue) boundaries:

```rust
pub fn stress_color(stress: f32) -> (u8, u8, u8) {
    // Range: -1.0 (divergent) to +1.0 (convergent)
    let s = stress.clamp(-1.0, 1.0);

    if s < -0.3 {
        // Strong divergent: blue (rifts, spreading centers)
        (40, (80.0 + (-s - 0.3) / 0.7 * 80.0) as u8, (180.0 + (-s - 0.3) / 0.7 * 75.0) as u8)
    } else if s < 0.0 {
        // Weak divergent: cyan/neutral
        ((100.0 + s / 0.3 * 60.0) as u8, (140.0 + s / 0.3 * 60.0) as u8, (140.0 - s / 0.3 * 40.0) as u8)
    } else if s < 0.3 {
        // Weak convergent: neutral/yellow
        ((100.0 + s / 0.3 * 80.0) as u8, (140.0 + s / 0.3 * 40.0) as u8, (140.0 - s / 0.3 * 80.0) as u8)
    } else {
        // Strong convergent: orange/red (mountains, subduction)
        ((180.0 + (s - 0.3) / 0.7 * 75.0) as u8, (180.0 - (s - 0.3) / 0.7 * 120.0) as u8, (60.0 - (s - 0.3) / 0.7 * 30.0) as u8)
    }
}
```

---

## Water Detection

**File**: `src/explorer.rs`

### The `is_submerged` Helper

Water detection uses **both** elevation and water depth to correctly identify:
- **Ocean tiles**: `height < 0.0`
- **Alpine lakes**: `height > 0.0` but `water_depth > 0.5`
- **Rivers on land**: Detected via `river_network.get_width_at()`

```rust
/// Check if a tile is submerged (water body: ocean, lake, or flooded area)
/// This correctly identifies alpine lakes which have height > 0 but water_depth > 0
fn is_submerged(&self, x: usize, y: usize) -> bool {
    let height = *self.world.heightmap.get(x, y);
    let water_depth = *self.world.water_depth.get(x, y);
    // Submerged if: below sea level OR has water depth (alpine lakes)
    height < 0.0 || water_depth > 0.5  // 0.5m threshold to avoid float noise
}
```

This is used in `get_tile_display()` as a local variable:
```rust
let water_depth = *self.world.water_depth.get(x, y);
let is_water = height < 0.0 || water_depth > 0.5;
```

---

## River Rendering

**File**: `src/explorer.rs`

### River View Mode

Rivers are highlighted with special colors based on flow. The logic correctly handles:
- **Ocean and lakes** (including alpine lakes)
- **Surface rivers** flowing on land

```rust
ViewMode::Rivers => {
    if is_water {
        // Water body (ocean, lake, or alpine lake)
        // Use water_depth for color intensity
        let depth_intensity = (water_depth / 100.0).min(1.0);
        if let Some(ref river_network) = self.world.river_network {
            let width = river_network.get_width_at(x as f32, y as f32, 2.0);
            if width > 0.0 {
                // River flowing through water body
                let intensity = (width * 30.0).min(255.0) as u8;
                ('~', Color::Rgb(100, intensity.saturating_add(50), 255),
                     Color::Rgb(10, 30, intensity / 2 + 40))
            } else {
                // Still water (lake or ocean) - darker blue for deeper
                let blue = (140.0 + depth_intensity * 80.0) as u8;
                ('~', Color::Rgb(80, blue, 220), Color::Rgb(20, 40, 80))
            }
        }
    } else {
        // Land tiles - check for surface river presence
        if let Some(ref river_network) = self.world.river_network {
            let width = river_network.get_width_at(x as f32, y as f32, 1.0);
            if width > 0.0 {
                // River on land - bright cyan
                ('~', Color::Rgb(60, 220, 255), Color::Rgb(0, 60, 100))
            } else {
                // Regular land
                let (r, g, b) = height_color(height);
                ('.', make_fg_color(r, g, b), make_bg_color(r, g, b))
            }
        }
    }
}
```

---

## Region Map Rendering

**File**: `src/explorer.rs`

The region map panel shows high-detail local terrain:

```rust
fn render_region_panel(&mut self, area: Rect, buf: &mut Buffer) {
    let region = self.region_cache.get_region(&self.world, self.cursor_x, self.cursor_y);

    for dy in 0..display_height {
        for dx in 0..display_width {
            let height = region.get_height(rx, ry);
            let river = region.get_river(rx, ry);
            let vegetation = region.get_vegetation(rx, ry);
            let rocks = region.get_rocks(rx, ry);
            let spring = region.get_spring(rx, ry);
            let waterfall = region.get_waterfall(rx, ry);

            // Priority: waterfall > spring > river > terrain
            let (ch, fg, bg) = if waterfall.is_present {
                ('▼', Color::Rgb(180, 220, 255), Color::Rgb(40, 100, 140))
            } else if spring.spring_type.is_present() {
                match spring.spring_type {
                    SpringType::Thermal => ('◎', Color::Rgb(255, 180, 100), Color::Rgb(120, 60, 30)),
                    SpringType::Artesian => ('◉', Color::Rgb(100, 200, 255), Color::Rgb(30, 80, 120)),
                    SpringType::Karst => ('○', Color::Rgb(80, 180, 200), Color::Rgb(20, 60, 80)),
                    _ => ('●', Color::Rgb(120, 180, 220), Color::Rgb(30, 60, 90)),
                }
            } else if river > 0.5 {
                ('≈', Color::Rgb(80, 180, 255), Color::Rgb(15, 35, 100))
            } else if river > 0.2 {
                ('~', Color::Rgb(100, 180, 240), Color::Rgb(20, 50, 100))
            } else if rocks > 0.5 {
                ('▲', Color::Rgb(160, 150, 140), Color::Rgb(70, 65, 60))
            } else if vegetation > 0.7 {
                ('♣', biome_tinted_green, darker_biome_green)
            } else if vegetation > 0.5 {
                ('↟', biome_green, darker_green)
            } else {
                ('.', biome_color, darker_biome)
            };

            buf.get_mut(x, y).set_char(ch).set_style(Style::default().fg(fg).bg(bg));
        }
    }
}
```

---

## PNG Export

**File**: `src/explorer.rs`

### Map Export

Export the current view mode as a PNG image:

```rust
pub fn export_map_image(
    world: &WorldData,
    view_mode: ViewMode,
    filename: &str,
) -> Result<(), Box<dyn Error>> {
    let mut img = ImageBuffer::new(width as u32, height as u32);

    for y in 0..height {
        for x in 0..width {
            let (r, g, b) = match view_mode {
                ViewMode::Biome => world.biomes.get(x, y).color(),
                ViewMode::Height => height_color(*world.heightmap.get(x, y)),
                ViewMode::Temperature => temperature_color(*world.temperature.get(x, y)),
                ViewMode::Moisture => moisture_color(*world.moisture.get(x, y)),
                ViewMode::Stress => stress_color(*world.stress_map.get(x, y)),
                ViewMode::Plates => {
                    let hue = (plate_id.0 as f32 * 37.0) % 360.0;
                    hsv_to_rgb(hue, 0.7, 0.8)
                }
                // ... other modes
            };

            img.put_pixel(x as u32, y as u32, Rgb([r, g, b]));
        }
    }

    img.save(filename)?;
    Ok(())
}
```

### Top-Down Aesthetic Export

Export with hillshading for a natural look:

```rust
pub fn export_topdown_image(world: &WorldData, filename: &str) -> Result<(), Box<dyn Error>> {
    for y in 0..height {
        for x in 0..width {
            let biome = *world.biomes.get(x, y);
            let h = *world.heightmap.get(x, y);
            let (base_r, base_g, base_b) = biome.color();

            // Compute hillshading from 3x3 kernel
            let mut shade = 1.0f32;
            if x >= 2 && y >= 2 && x < width - 2 && y < height - 2 {
                let h_topleft = average_height(x-2, y-2, 3);  // NW corner
                let h_botright = average_height(x, y, 3);      // SE corner
                let slope = (h_botright - h_topleft) / 200.0;
                shade = (1.0 + slope).clamp(0.85, 1.15);
            }

            // Apply elevation brightness
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
    Ok(())
}
```

---

## ASCII Export

**File**: `src/ascii.rs`

### Monochrome ASCII

```rust
pub fn render_ascii_map(
    heightmap: &Tilemap<f32>,
    biomes: &Tilemap<ExtendedBiome>,
    mode: AsciiMode,
) -> String {
    let mut result = String::new();

    for y in 0..height {
        for x in 0..width {
            let ch = match mode {
                AsciiMode::Biome => biome_char(biomes.get(x, y)),
                AsciiMode::Height => height_char(*heightmap.get(x, y)),
                AsciiMode::Temperature => temperature_char(*temperature.get(x, y)),
                AsciiMode::Moisture => moisture_char(*moisture.get(x, y)),
                AsciiMode::Stress => stress_char(*stress_map.get(x, y)),
                AsciiMode::Plates => plate_char(*plate_map.get(x, y), plates),
            };
            result.push(ch);
        }
        result.push('\n');
    }

    result
}
```

### Colorized ASCII (ANSI)

```rust
pub fn render_colored_ascii_map(...) -> String {
    let mut result = String::new();

    for y in 0..height {
        for x in 0..width {
            let (ch, fg, bg) = match mode {
                AsciiMode::Biome => {
                    let biome = biomes.get(x, y);
                    (biome_char(biome), biome_fg_color(biome), biome_bg_color(biome))
                }
                AsciiMode::Height => {
                    let h = *heightmap.get(x, y);
                    let color = height_color(h);
                    let fg = darken(color, 40);
                    (height_char(h), fg, color)
                }
                // ... other modes
            };

            // ANSI escape codes for 24-bit color
            result.push_str(&format!(
                "\x1b[38;2;{};{};{}m\x1b[48;2;{};{};{}m{}\x1b[0m",
                fg.0, fg.1, fg.2,
                bg.0, bg.1, bg.2,
                ch
            ));
        }
        result.push_str("\x1b[0m\n");
    }

    result
}
```

### ASCII PNG Export

Render ASCII characters into a PNG image:

```rust
pub fn export_ascii_png(biomes: &Tilemap<ExtendedBiome>, path: &str) -> io::Result<()> {
    const CELL_SIZE: u32 = 8;  // Each character = 8x8 pixels
    let mut img = RgbImage::new(width * CELL_SIZE, height * CELL_SIZE);

    // Simple 5x7 bitmap font
    let font = create_bitmap_font();

    for y in 0..height {
        for x in 0..width {
            let biome = *biomes.get(x, y);
            let (r, g, b) = biome.color();
            let ch = biome_char(&biome);

            // Calculate text color for contrast
            let brightness = (r as u32 * 299 + g as u32 * 587 + b as u32 * 114) / 1000;
            let text_color = if brightness > 128 { Rgb([0,0,0]) } else { Rgb([255,255,255]) };

            // Fill cell with background
            for py in 0..CELL_SIZE {
                for px in 0..CELL_SIZE {
                    img.put_pixel(x * CELL_SIZE + px, y * CELL_SIZE + py, Rgb([r, g, b]));
                }
            }

            // Draw character glyph
            if let Some(glyph) = font.get(&ch) {
                for (row_idx, &row) in glyph.iter().enumerate() {
                    for col in 0..5 {
                        if (row >> (4 - col)) & 1 == 1 {
                            img.put_pixel(cell_x + col, cell_y + row_idx, text_color);
                        }
                    }
                }
            }
        }
    }

    img.save(path)?;
    Ok(())
}
```

---

## Module Reference

| File | Purpose |
|------|---------|
| `src/explorer.rs` | Terminal UI, view modes, viewport, input handling |
| `src/ascii.rs` | Color gradients, ASCII characters, export functions |
| `src/biomes.rs` | Biome colors and display names |
| `src/climate.rs` | Base biome colors |
| `src/region.rs` | High-detail region map generation |
| `src/erosion/materials.rs` | Rock type colors |
| `src/weather_zones.rs` | Weather risk colors |

---

## Water Network Export

**File**: `src/explorer.rs`

### Export All Water (Rivers + Lakes + Ocean)

Press `w` to export a water network image with black background:

```rust
pub fn export_water_network_image(world: &WorldData, filename: &str) -> Result<()> {
    for y in 0..height {
        for x in 0..width {
            let h = *world.heightmap.get(x, y);
            let water_depth = *world.water_depth.get(x, y);
            let river_width = river_network.get_width_at(x, y, 1.0);

            let (r, g, b) = if river_width > 0.0 {
                // River - bright cyan
                let intensity = (river_width * 40.0).min(255.0) as u8;
                (0, intensity.saturating_add(100), 255)
            } else if water_depth > 0.5 {
                // Lake - blue based on depth
                let depth_factor = (water_depth / 50.0).min(1.0);
                (30, 80 + depth_factor * 40, 150 + depth_factor * 105)
            } else if h < 0.0 {
                // Ocean - dark blue
                (10, 30, 80 + depth_factor * 80)
            } else {
                // Land - black
                (0, 0, 0)
            };
        }
    }
}
```

### Export Freshwater Only (No Ocean)

Press `W` (shift+w) to export only rivers and alpine lakes:

```rust
pub fn export_freshwater_network_image(world: &WorldData, filename: &str) -> Result<()> {
    // Only shows:
    // - Rivers on land (height >= 0)
    // - Alpine lakes (water_depth > 0 AND height >= 0)
    // Ocean tiles render as black
}
```

---

## Keyboard Controls

| Key | Action |
|-----|--------|
| `V` | Cycle view mode |
| `Arrow keys / WASD / HJKL` | Move cursor |
| `PgUp/PgDn` | Fast vertical movement |
| `Home/End` | Fast horizontal movement |
| `+/-` | Zoom in/out |
| `F` | Fit map to screen |
| `I/Tab` | Toggle info panel |
| `M` | Toggle region map panel |
| `[/]` | Previous/next season |
| `E` | Export current view as PNG |
| `T` | Export top-down view as PNG |
| `w` | Export water network (rivers+lakes+ocean) |
| `W` | Export freshwater only (no ocean) |
| `R` | Regenerate world (new seed) |
| `?` | Toggle help overlay |
| `Q/Esc` | Quit |

---

## Example: Custom View Mode

To add a new view mode:

```rust
// 1. Add to ViewMode enum
enum ViewMode {
    // ... existing modes
    MyCustomView,
}

// 2. Implement name()
fn name(&self) -> &'static str {
    match self {
        ViewMode::MyCustomView => "Custom",
        // ...
    }
}

// 3. Implement next() for cycling
fn next(&self) -> ViewMode {
    match self {
        ViewMode::SeasonalTemp => ViewMode::MyCustomView,
        ViewMode::MyCustomView => ViewMode::Biome,
        // ...
    }
}

// 4. Implement rendering in get_tile_display()
fn get_tile_display(&self, x: usize, y: usize) -> (char, Color, Color) {
    match self.view_mode {
        ViewMode::MyCustomView => {
            let value = calculate_my_value(x, y);
            let (r, g, b) = my_color_function(value);
            ('.', make_fg_color(r, g, b), make_bg_color(r, g, b))
        }
        // ...
    }
}

// 5. Implement PNG export in export_map_image()
ViewMode::MyCustomView => my_color_function(my_value),
```
