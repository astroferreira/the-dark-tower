//! Debug images of the site layer: the landform classes by family, the surface rock, the places
//! as marks. `--zoom X,Y` writes them beside `zoom_<seed>_<x>_<y>.png` so a landform or site
//! pass can be checked by eye without the survey tool (ROADMAP Stage 1 adds the real survey).

use std::error::Error;
use std::path::Path;

use image::{Rgb, RgbImage};

use crate::erosion::materials::RockType;
use crate::region::plan::{Family, Landform};
use crate::region::zoom::ZoomRegion;

/// A colour per landform family, shaded by the landform's index within it so neighbours differ.
pub fn landform_color(l: Landform) -> [u8; 3] {
    let base: [u8; 3] = match l.family() {
        Family::Plain => [222, 214, 190],
        Family::Rock => [120, 110, 100],
        Family::Karst => [200, 190, 150],
        Family::Ice => [190, 220, 240],
        Family::Fire => [160, 60, 40],
        Family::Wind => [230, 200, 120],
        Family::River => [90, 150, 200],
        Family::Coast => [240, 220, 160],
        Family::Wet => [110, 150, 110],
    };
    let i = Landform::ALL.iter().position(|&x| x == l).unwrap_or(0) as i32;
    let shade = (i % 5) * 14 - 28;
    [(base[0] as i32 + shade).clamp(0, 255) as u8, (base[1] as i32 + shade).clamp(0, 255) as u8, (base[2] as i32 + shade).clamp(0, 255) as u8]
}

pub fn rock_color(r: RockType) -> [u8; 3] {
    match r {
        RockType::Basalt => [70, 70, 80],
        RockType::Granite => [190, 160, 160],
        RockType::Sandstone => [210, 170, 110],
        RockType::Limestone => [235, 225, 200],
        RockType::Shale => [140, 150, 165],
        RockType::Sediment => [200, 190, 150],
        RockType::Ice => [230, 240, 250],
    }
}

/// `<prefix>_landforms.png`: classes by family over a faint hillshade, water blue, places as red
/// marks (a 3-cell cross); `<prefix>_rock.png`: the surface rock.
pub fn save(region: &ZoomRegion, prefix: &Path) -> Result<(), Box<dyn Error>> {
    let (w, h) = (region.width, region.height);
    let mut land = RgbImage::new(w as u32, h as u32);
    let mut rock = RgbImage::new(w as u32, h as u32);
    for y in 0..h {
        for x in 0..w {
            let k = y * w + x;
            let e = region.elevation_m[k];
            let water = e <= 0.0 || region.lake_depth_m[k] > 0.0 || region.river_width_m[k] > 0.0;
            // A faint hillshade from the west so the classes read against the relief.
            let el = if x > 0 { region.elevation_m[k - 1] } else { e };
            let shade = ((e - el) * 0.4).clamp(-30.0, 30.0) as i32;
            let c = if water { [70, 110, 160] } else {
                let c = landform_color(region.landform[k]);
                [(c[0] as i32 + shade).clamp(0, 255) as u8, (c[1] as i32 + shade).clamp(0, 255) as u8, (c[2] as i32 + shade).clamp(0, 255) as u8]
            };
            land.put_pixel(x as u32, y as u32, Rgb(c));
            rock.put_pixel(x as u32, y as u32, Rgb(if water { [70, 110, 160] } else { rock_color(region.rock[k]) }));
        }
    }
    let s = (region.params.cells_per_tile.max(8) & !1) as i64;
    for p in &region.places {
        let (px, py) = ((p.x - (region.world_x0 * s) as f64).round() as i64, (p.y - (region.world_y0 * s) as f64).round() as i64);
        for (dx, dy) in [(0, 0), (1, 0), (-1, 0), (0, 1), (0, -1), (2, 0), (-2, 0), (0, 2), (0, -2)] {
            let (x, y) = (px + dx, py + dy);
            if x >= 0 && y >= 0 && x < w as i64 && y < h as i64 {
                land.put_pixel(x as u32, y as u32, Rgb([220, 30, 30]));
            }
        }
    }
    let name = |suffix: &str| {
        let mut p = prefix.as_os_str().to_os_string();
        p.push(suffix);
        std::path::PathBuf::from(p)
    };
    land.save(name("_landforms.png"))?;
    rock.save(name("_rock.png"))?;
    Ok(())
}

/// The share of cells per landform (name, count), most common first; plain ground included.
pub fn landform_mix(region: &ZoomRegion) -> Vec<(&'static str, usize)> {
    let mut counts = vec![0usize; Landform::ALL.len()];
    for &l in &region.landform {
        counts[Landform::ALL.iter().position(|&x| x == l).unwrap_or(0)] += 1;
    }
    let mut out: Vec<(&'static str, usize)> = Landform::ALL.iter().zip(counts).filter(|(_, c)| *c > 0).map(|(l, c)| (l.name(), c)).collect();
    out.sort_by(|a, b| b.1.cmp(&a.1));
    out
}
