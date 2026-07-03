//! Interior pass: fills one biome's 8×8×12 cell grid.
//!
//! All noise is sampled in world cell coordinates (`col * 8 + x`) so fields
//! are continuous across biome borders, and every noise source is derived
//! from the world seed — biome generation order can never affect results.

use noise::{NoiseFn, Perlin};
use voxel_core::{BiomeCoord, BiomeType, CellGrid, CellType, Seed, CELLS_XY, SURFACE_Z};

const MAX: u8 = CELLS_XY as u8; // 8
const EDGE: u8 = MAX - 1; // 7

fn derive_u32(seed: Seed, offset: u64) -> u32 {
    let mixed = seed
        .wrapping_add(offset.wrapping_mul(0x9e3779b97f4a7c15))
        .wrapping_mul(0x517cc1b727220a95);
    (mixed ^ (mixed >> 32)) as u32
}

struct Noises {
    stone: Perlin,
    iron: Perlin,
    gold: Perlin,
    pond: Perlin,
    height: Perlin,
    mountain: Perlin,
    island: Perlin,
}

impl Noises {
    fn new(seed: Seed) -> Self {
        Self {
            // Offset 1 was the retired funnel-jitter noise; it stays reserved
            // so the other fields keep their historical derivations.
            stone: Perlin::new(derive_u32(seed, 2)),
            iron: Perlin::new(derive_u32(seed, 3)),
            gold: Perlin::new(derive_u32(seed, 4)),
            pond: Perlin::new(derive_u32(seed, 5)),
            height: Perlin::new(derive_u32(seed, 6)),
            mountain: Perlin::new(derive_u32(seed, 7)),
            island: Perlin::new(derive_u32(seed, 8)),
        }
    }
}

pub fn generate_biome(seed: Seed, coord: BiomeCoord, biome_type: BiomeType) -> CellGrid {
    let n = Noises::new(seed);
    let mut grid = CellGrid::new();

    let ox = coord.col as f64 * CELLS_XY as f64;
    let oy = coord.row as f64 * CELLS_XY as f64;

    for y in 0..MAX {
        for x in 0..MAX {
            let wx = ox + x as f64;
            let wy = oy + y as f64;

            fill_underground(&mut grid, &n, x, y, wx, wy);
            fill_surface(&mut grid, &n, biome_type, x, y, wx, wy);
            fill_relief(&mut grid, &n, biome_type, x, y, wx, wy);
        }
    }

    grid
}

// ── Underground (z = 0..=4, cell layers 1–5) ────────────────────────────────
//
// The biome is a floating island: below the full 8×8 surface, each layer is a
// fixed centered rectangle, stepping down 6×6 → 5×4 → 4×3 → 3×2 → 2×1 toward
// the bottom tip. The rectangles nest, so underground mass always hangs from
// the layer above by construction. Cells inside the funnel are soil salted
// with resource deposits.

/// Funnel footprint (width, depth) per underground z, deepest first.
const FUNNEL: [(u8, u8); SURFACE_Z as usize] = [(2, 1), (3, 2), (4, 3), (5, 4), (6, 6)];

/// Does the funnel rectangle at underground layer `z` contain (x, y)?
fn in_funnel(x: u8, y: u8, z: u8) -> bool {
    let (w, h) = FUNNEL[z as usize];
    let x0 = (MAX - w) / 2;
    let y0 = (MAX - h) / 2;
    (x0..x0 + w).contains(&x) && (y0..y0 + h).contains(&y)
}

fn fill_underground(grid: &mut CellGrid, n: &Noises, x: u8, y: u8, wx: f64, wy: f64) {
    for z in 0..SURFACE_Z {
        if in_funnel(x, y, z) {
            grid.set(x, y, z, pick_deposit(n, wx, wy, z));
        }
    }
}

/// Resource deposits by rarity (spec: stone ≥30%, iron ~10%, gold ~5%).
/// Rarer deposits sit deeper; stone gets denser toward the bottom so the
/// island underside reads as rock rubble.
fn pick_deposit(n: &Noises, wx: f64, wy: f64, z: u8) -> CellType {
    let zf = z as f64;
    if z <= 2 && n.gold.get([wx * 0.45, wy * 0.45, zf * 0.8]) > 0.34 {
        return CellType::Gold;
    }
    if z <= 3 && n.iron.get([wx * 0.35, wy * 0.35, zf * 0.7]) > 0.30 {
        return CellType::Iron;
    }
    // Denser stone deeper: threshold rises with height above the bottom.
    let stone_threshold = -0.35 + 0.13 * zf;
    if n.stone.get([wx * 0.22, wy * 0.22, zf * 0.5]) > stone_threshold {
        return CellType::Stone;
    }
    CellType::Soil
}

// ── Surface (z = 5, cell layer 6) ────────────────────────────────────────────
//
// Always solid, typed by the biome — the widest part of the island (8×8).
// Grass, winter, and sand biomes get interior ponds carved by noise; water
// biomes get occasional sand islands. The one-cell edge ring always stays the
// biome's own type so compatible borders match cell-for-cell.

fn fill_surface(grid: &mut CellGrid, n: &Noises, bt: BiomeType, x: u8, y: u8, wx: f64, wy: f64) {
    let mut cell = bt.surface_cell();

    let interior = x > 0 && x < EDGE && y > 0 && y < EDGE;
    let ponds_allowed = matches!(bt, BiomeType::Grass | BiomeType::Sand | BiomeType::Winter);
    if interior && ponds_allowed && n.pond.get([wx * 0.16, wy * 0.16]) > 0.48 {
        cell = CellType::Water;
    }
    // Water biomes: sparse sand islets where the island noise spikes. High
    // threshold + higher frequency than ponds → small clusters, and many
    // water biomes stay open sea.
    if interior && bt == BiomeType::Water && n.island.get([wx * 0.24, wy * 0.24]) > 0.52 {
        cell = CellType::Sand;
    }

    grid.set(x, y, SURFACE_Z, cell);
}

// ── Relief (z = 6..=11, cell layers 7–12) ────────────────────────────────────
//
// Column heights come from a rolling-hills field plus a sparse mountain mask.
// Heights fade to zero over the two cells nearest a biome edge so border
// strips stay flat and walkable. Water biomes (islands included) and pond
// columns stay flat.

fn fill_relief(grid: &mut CellGrid, n: &Noises, bt: BiomeType, x: u8, y: u8, wx: f64, wy: f64) {
    if bt == BiomeType::Water || grid.get(x, y, SURFACE_Z) == CellType::Water {
        return;
    }

    // 0 on the edge ring → 1 two cells in (the 8×8 footprint leaves a 4×4
    // full-height core; the old 3-cell fade would squeeze it to 2×2).
    let edge_dist = x.min(y).min(EDGE - x).min(EDGE - y);
    let fade = (edge_dist.min(2) as f64) / 2.0;
    if fade == 0.0 {
        return;
    }

    let hills01 = (n.height.get([wx * 0.055, wy * 0.055]) + 1.0) / 2.0;
    // Higher frequency than the hills: with only 6 relief layers, peaks must
    // stay a few cells wide or they clip into flat-topped mesas.
    let mountain01 = (n.mountain.get([wx * 0.11, wy * 0.11]) + 1.0) / 2.0;

    // Peaks are added on top of the rolling hills so mountains rise out of
    // the terrain in cones instead of clipping into flat-topped towers.
    let (hill_height, peak_height) = match bt {
        BiomeType::Grass | BiomeType::Winter => (
            hills01.powf(1.4) * 3.0,
            mountain_peak(mountain01, 0.62, 6.5),
        ),
        BiomeType::Sand => (hills01.powf(2.0) * 2.2, 0.0),
        BiomeType::Water => unreachable!(),
    };

    let height = ((hill_height + peak_height) * fade).round().min(6.0) as u8;
    if height == 0 {
        return;
    }

    // Tall columns are bare mountain rock; low relief keeps the biome's
    // surface material.
    let material = if height >= 4 {
        CellType::Stone
    } else {
        bt.surface_cell()
    };

    for dz in 0..height {
        grid.set(x, y, SURFACE_Z + 1 + dz, material);
    }
}

/// Sharp peak profile: zero below `threshold`, then a steep power ramp.
fn mountain_peak(mask01: f64, threshold: f64, amplitude: f64) -> f64 {
    if mask01 <= threshold {
        return 0.0;
    }
    let t = (mask01 - threshold) / (1.0 - threshold);
    t.powf(1.3) * amplitude
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn funnel_rectangles_nest() {
        // Every funnel layer must sit inside the layer above (the surface is
        // the full 8×8), so underground mass always hangs from above.
        for z in 0..SURFACE_Z {
            for y in 0..MAX {
                for x in 0..MAX {
                    if in_funnel(x, y, z) {
                        let above_ok = z + 1 == SURFACE_Z || in_funnel(x, y, z + 1);
                        assert!(above_ok, "funnel cell ({x},{y},{z}) has no support above");
                    }
                }
            }
        }
    }

    #[test]
    fn funnel_footprint_sizes() {
        for (z, &(w, h)) in FUNNEL.iter().enumerate() {
            let count = (0..MAX)
                .flat_map(|y| (0..MAX).map(move |x| (x, y)))
                .filter(|&(x, y)| in_funnel(x, y, z as u8))
                .count();
            assert_eq!(count, w as usize * h as usize);
        }
    }
}
