//! Beach post-pass: a sand belt between pond water and grass.
//!
//! Runs after the interior pass, on the cell tier — sand near water is a
//! change of cell *type* (soil → sand), and cell type drives mining,
//! connection matching, and army traversal, so it cannot be faked in render.
//!
//! Rules (rendering.md, "BEACHES"):
//! - Only soil surface cells (grass biomes) can flip; the belt hugs water.
//! - Flip probability drops sharply with distance from water; two cells is
//!   the widest the belt gets (wider reads as desert).
//! - The one-cell edge ring never flips — border-strip purity is an
//!   invariant compatible connections depend on.
//! - Small ponds get no beach (they look wrong); only ponds of
//!   `MIN_POND_CELLS`+ grow one.
//! - Cells carrying relief keep their grass — a sand column under a green
//!   hill reads as a bug, not a beach.

use voxel_core::{BiomeCoord, BiomeType, CellGrid, CellType, Seed, CELLS_XY, SURFACE_Z};

const MAX: u8 = CELLS_XY as u8;
const EDGE: u8 = MAX - 1;

/// Ponds smaller than this stay beachless.
const MIN_POND_CELLS: usize = 6;
/// Flip probability by distance-from-water (index 0 unused — water itself).
const FLIP_PROB: [f32; 3] = [0.0, 0.92, 0.35];

/// Rule discriminator for the determinism hash (see rendering.md,
/// "Determinism seed formula").
const RULE_BEACH: u64 = 6;

/// Deterministic hash → [0, 1) on world **cell** coordinates. Same mixer as
/// render's `voxel_hash01`, kept local so mapgen stays engine-free.
fn cell_hash01(seed: Seed, rule: u64, wx: i64, wy: i64, wz: i64) -> f32 {
    let mut h = seed
        ^ rule.wrapping_mul(0xd6e8feb86659fd93)
        ^ (wx as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (wy as u64).wrapping_mul(0xc2b2ae3d27d4eb4f)
        ^ (wz as u64).wrapping_mul(0x165667b19e3779f9);
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51afd7ed558ccd);
    h ^= h >> 33;
    (h >> 40) as f32 / (1u64 << 24) as f32
}

/// Flip soil surface cells near large ponds to sand. Grass biomes only —
/// sand biomes already meet their ponds in sand, water biomes have no soil
/// surface to flip, and winter ponds keep their snowy shores (a sand beach
/// in a snowfield reads as a bug).
pub fn apply_beaches(seed: Seed, coord: BiomeCoord, bt: BiomeType, grid: &mut CellGrid) {
    if bt != BiomeType::Grass {
        return;
    }

    let dist = water_distance(grid);
    let ox = coord.col as i64 * CELLS_XY as i64;
    let oy = coord.row as i64 * CELLS_XY as i64;

    for y in 1..EDGE {
        for x in 1..EDGE {
            let d = dist[y as usize][x as usize];
            if !(1..FLIP_PROB.len()).contains(&(d as usize)) {
                continue;
            }
            if grid.get(x, y, SURFACE_Z) != CellType::Soil {
                continue;
            }
            // Relief above keeps its grass footing.
            if grid.get(x, y, SURFACE_Z + 1) != CellType::Air {
                continue;
            }
            let h = cell_hash01(
                seed,
                RULE_BEACH,
                ox + x as i64,
                oy + y as i64,
                SURFACE_Z as i64,
            );
            if h < FLIP_PROB[d as usize] {
                grid.set(x, y, SURFACE_Z, CellType::Sand);
            }
        }
    }
}

/// BFS distance from every surface cell to the nearest **large** pond
/// (u8::MAX = unreachable / pond too small). Ponds are 4-connected water
/// components at the surface layer.
fn water_distance(grid: &CellGrid) -> [[u8; CELLS_XY]; CELLS_XY] {
    let mut dist = [[u8::MAX; CELLS_XY]; CELLS_XY];
    let mut queue: Vec<(u8, u8)> = Vec::new();

    // Seed the BFS with cells of large ponds only.
    let mut visited = [[false; CELLS_XY]; CELLS_XY];
    for y in 0..MAX {
        for x in 0..MAX {
            if visited[y as usize][x as usize] || grid.get(x, y, SURFACE_Z) != CellType::Water {
                continue;
            }
            let pond = flood_pond(grid, &mut visited, x, y);
            if pond.len() >= MIN_POND_CELLS {
                for &(px, py) in &pond {
                    dist[py as usize][px as usize] = 0;
                    queue.push((px, py));
                }
            }
        }
    }

    let mut head = 0;
    while head < queue.len() {
        let (x, y) = queue[head];
        head += 1;
        let d = dist[y as usize][x as usize];
        for (nx, ny) in neighbors4(x, y) {
            let slot = &mut dist[ny as usize][nx as usize];
            if *slot == u8::MAX {
                *slot = d + 1;
                queue.push((nx, ny));
            }
        }
    }

    dist
}

/// Collect one 4-connected water component starting at (x, y).
fn flood_pond(
    grid: &CellGrid,
    visited: &mut [[bool; CELLS_XY]; CELLS_XY],
    x: u8,
    y: u8,
) -> Vec<(u8, u8)> {
    let mut pond = vec![(x, y)];
    visited[y as usize][x as usize] = true;
    let mut head = 0;
    while head < pond.len() {
        let (cx, cy) = pond[head];
        head += 1;
        for (nx, ny) in neighbors4(cx, cy) {
            if !visited[ny as usize][nx as usize] && grid.get(nx, ny, SURFACE_Z) == CellType::Water
            {
                visited[ny as usize][nx as usize] = true;
                pond.push((nx, ny));
            }
        }
    }
    pond
}

fn neighbors4(x: u8, y: u8) -> impl Iterator<Item = (u8, u8)> {
    [(1i8, 0i8), (-1, 0), (0, 1), (0, -1)]
        .into_iter()
        .filter_map(move |(dx, dy)| {
            let nx = x as i8 + dx;
            let ny = y as i8 + dy;
            (nx >= 0 && ny >= 0 && nx < MAX as i8 && ny < MAX as i8).then_some((nx as u8, ny as u8))
        })
}
