/// A 2D tilemap grid with equirectangular projection (wraps horizontally).
#[derive(Clone)]
pub struct Tilemap<T> {
    pub width: usize,
    pub height: usize,
    data: Vec<T>,
}

impl<T: Clone + Default> Tilemap<T> {
    pub fn new(width: usize, height: usize) -> Self {
        Self {
            width,
            height,
            data: vec![T::default(); width * height],
        }
    }
}

impl<T: Clone> Tilemap<T> {
    pub fn new_with(width: usize, height: usize, value: T) -> Self {
        Self {
            width,
            height,
            data: vec![value; width * height],
        }
    }

    /// Get the index into the data array, handling horizontal wrapping.
    fn index(&self, x: usize, y: usize) -> usize {
        let x = x % self.width; // Wrap horizontally
        y * self.width + x
    }

    pub fn get(&self, x: usize, y: usize) -> &T {
        &self.data[self.index(x, y)]
    }

    pub fn get_mut(&mut self, x: usize, y: usize) -> &mut T {
        let idx = self.index(x, y);
        &mut self.data[idx]
    }

    pub fn set(&mut self, x: usize, y: usize, value: T) {
        let idx = self.index(x, y);
        self.data[idx] = value;
    }

    /// Get neighbors with horizontal wrapping.
    /// Returns up to 4 neighbors (up, down, left, right).
    /// Top and bottom edges don't wrap.
    pub fn neighbors(&self, x: usize, y: usize) -> Vec<(usize, usize)> {
        let mut result = Vec::with_capacity(4);

        // Left (wraps)
        let left_x = if x == 0 { self.width - 1 } else { x - 1 };
        result.push((left_x, y));

        // Right (wraps)
        let right_x = if x == self.width - 1 { 0 } else { x + 1 };
        result.push((right_x, y));

        // Up (no wrap at top)
        if y > 0 {
            result.push((x, y - 1));
        }

        // Down (no wrap at bottom)
        if y < self.height - 1 {
            result.push((x, y + 1));
        }

        result
    }

    /// Iterate over all cells with their coordinates.
    pub fn iter(&self) -> impl Iterator<Item = (usize, usize, &T)> {
        self.data.iter().enumerate().map(move |(idx, val)| {
            let x = idx % self.width;
            let y = idx / self.width;
            (x, y, val)
        })
    }

    /// Iterate mutably over all cells with their coordinates.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (usize, usize, &mut T)> {
        let width = self.width;
        self.data.iter_mut().enumerate().map(move |(idx, val)| {
            let x = idx % width;
            let y = idx / width;
            (x, y, val)
        })
    }
}
