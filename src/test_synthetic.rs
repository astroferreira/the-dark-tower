// Synthetic terrain test
use crate::tilemap::Tilemap;
use crate::erosion::geomorphometry;

pub fn test_synthetic_drainage() {
    // Create a simple terrain with clear drainage
    // Terrain slopes from center peak to edges
    let size = 64;
    let mut heightmap = Tilemap::new_with(size, size, 0.0f32);
    
    // Create a cone that drains to the bottom edge (ocean)
    for y in 0..size {
        for x in 0..size {
            if y == size - 1 {
                // Bottom row is ocean
                heightmap.set(x, y, -10.0);
            } else {
                // Terrain slopes down toward bottom
                let elev = 100.0 - (y as f32) * 2.0;
                heightmap.set(x, y, elev);
            }
        }
    }
    
    // Now analyze
    let results = geomorphometry::analyze(&heightmap, 5.0);
    results.print_summary();
    
    println!("Synthetic Test Realism Score: {:.1}/100", results.realism_score());
}
