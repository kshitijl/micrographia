/// A periodic cell list: particles grouped by cell, cells at least one cutoff wide.
pub struct CellList {
    pub dims: [u32; 2],
    pub start: Vec<u32>,
    pub items: Vec<u32>,
}

impl CellList {
    /// `cell(i)` gives the (x, y) cell of particle i.
    pub fn build(n: usize, dims: [u32; 2], cell: impl Fn(usize) -> [u32; 2]) -> Self {
        let ncell = (dims[0] * dims[1]) as usize;
        let keys: Vec<u32> = (0..n)
            .map(|i| {
                let c = cell(i);
                c[1] * dims[0] + c[0]
            })
            .collect();
        let mut start = vec![0u32; ncell + 1];
        for &k in &keys {
            start[k as usize + 1] += 1;
        }
        for c in 0..ncell {
            start[c + 1] += start[c];
        }
        let mut fill = start.clone();
        let mut items = vec![0u32; n];
        for (i, &k) in keys.iter().enumerate() {
            items[fill[k as usize] as usize] = i as u32;
            fill[k as usize] += 1;
        }
        Self { dims, start, items }
    }

    /// Call `f(j)` for every particle in the 3×3 block of cells around cell `c`.
    #[inline]
    pub fn for_neighbors(&self, c: [u32; 2], mut f: impl FnMut(usize)) {
        let [nx, ny] = self.dims;
        for oy in 0..3 {
            let y = (c[1] + ny + oy - 1) % ny;
            for ox in 0..3 {
                let x = (c[0] + nx + ox - 1) % nx;
                let k = (y * nx + x) as usize;
                for &j in &self.items[self.start[k] as usize..self.start[k + 1] as usize] {
                    f(j as usize);
                }
            }
        }
    }
}

/// Cell index of a coordinate `x` in `[0, l)` with `n` cells.
#[inline]
pub fn cell_of(x: f64, l: f64, n: u32) -> u32 {
    ((x / l * n as f64) as u32).min(n - 1)
}
