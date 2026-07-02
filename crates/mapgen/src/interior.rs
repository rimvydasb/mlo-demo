//! Interior pass: fills one biome's 12³ cell grid.
//!
//! All noise is sampled in world cell coordinates (`col * 12 + x`) so fields
//! are continuous across biome borders, and every noise source is derived
//! from the world seed — biome generation order can never affect results.

use noise::{NoiseFn, Perlin};
use voxel_core::{BiomeCoord, BiomeType, CellGrid, CellType, Seed, CELLS, SURFACE_Z};

const MAX: u8 = CELLS as u8; // 12
const EDGE: u8 = MAX - 1; // 11

fn derive_u32(seed: Seed, offset: u64) -> u32 {
    let mixed = seed
        .wrapping_add(offset.wrapping_mul(0x9e3779b97f4a7c15))
        .wrapping_mul(0x517cc1b727220a95);
    (mixed ^ (mixed >> 32)) as u32
}

struct Noises {
    funnel: Perlin,
    stone: Perlin,
    iron: Perlin,
    gold: Perlin,
    pond: Perlin,
    height: Perlin,
    mountain: Perlin,
}

impl Noises {
    fn new(seed: Seed) -> Self {
        Self {
            funnel: Perlin::new(derive_u32(seed, 1)),
            stone: Perlin::new(derive_u32(seed, 2)),
            iron: Perlin::new(derive_u32(seed, 3)),
            gold: Perlin::new(derive_u32(seed, 4)),
            pond: Perlin::new(derive_u32(seed, 5)),
            height: Perlin::new(derive_u32(seed, 6)),
            mountain: Perlin::new(derive_u32(seed, 7)),
        }
    }
}

pub fn generate_biome(seed: Seed, coord: BiomeCoord, biome_type: BiomeType) -> CellGrid {
    let n = Noises::new(seed);
    let mut grid = CellGrid::new();

    let ox = coord.col as f64 * CELLS as f64;
    let oy = coord.row as f64 * CELLS as f64;

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
// The biome is a floating island: the underground mass tapers toward the
// bottom (a "funnel"), so deeper layers cover a shrinking noise-perturbed
// footprint. Cells inside the funnel are soil salted with resource deposits.

fn fill_underground(grid: &mut CellGrid, n: &Noises, x: u8, y: u8, wx: f64, wy: f64) {
    // Distance from biome center, 0 at center → 1 at the rim. Blends square
    // and round metrics so the taper steps like the reference art but does
    // not read as a perfect pyramid.
    let (dx, dy) = (x as f64 - 5.5, y as f64 - 5.5);
    let cheb = dx.abs().max(dy.abs()) / 5.5;
    let eucl = (dx * dx + dy * dy).sqrt() / 7.78; // 7.78 ≈ corner distance
    let dist = 0.6 * cheb + 0.4 * eucl;

    // Walk top-down and stop at the first cut, so underground mass always
    // hangs from the layer above (and ultimately from the full surface).
    for z in (0..SURFACE_Z).rev() {
        // Radius of the solid footprint at this depth. The layer directly
        // under the surface is always full so the surface has support;
        // deeper layers keep a shrinking core.
        if z < SURFACE_Z - 1 {
            let radius = 0.45 + 0.16 * z as f64;
            let jitter = n.funnel.get([wx * 0.35, wy * 0.35, z as f64 * 0.9]) * 0.10;
            if dist > radius + jitter {
                break; // this cell and everything below stays Air
            }
        }

        grid.set(x, y, z, pick_deposit(n, wx, wy, z));
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
// Always solid, typed by the biome. Grass and sand biomes get interior ponds
// carved by noise; the one-cell edge ring always stays the biome's own type
// so compatible borders match cell-for-cell.

fn fill_surface(grid: &mut CellGrid, n: &Noises, bt: BiomeType, x: u8, y: u8, wx: f64, wy: f64) {
    let mut cell = bt.surface_cell();

    let interior = x > 0 && x < EDGE && y > 0 && y < EDGE;
    let ponds_allowed = matches!(bt, BiomeType::Grass | BiomeType::Sand);
    if interior && ponds_allowed && n.pond.get([wx * 0.16, wy * 0.16]) > 0.48 {
        cell = CellType::Water;
    }

    grid.set(x, y, SURFACE_Z, cell);
}

// ── Relief (z = 6..=11, cell layers 7–12) ────────────────────────────────────
//
// Column heights come from a rolling-hills field plus a sparse mountain mask.
// Heights fade to zero over the three cells nearest a biome edge so border
// strips stay flat and walkable. Water biomes and pond cells stay flat.

fn fill_relief(grid: &mut CellGrid, n: &Noises, bt: BiomeType, x: u8, y: u8, wx: f64, wy: f64) {
    if bt == BiomeType::Water || grid.get(x, y, SURFACE_Z) == CellType::Water {
        return;
    }

    // 0 on the edge ring → 1 three cells in.
    let edge_dist = x.min(y).min(EDGE - x).min(EDGE - y);
    let fade = (edge_dist.min(3) as f64) / 3.0;
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
        BiomeType::Grass => (
            hills01.powf(1.4) * 3.0,
            mountain_peak(mountain01, 0.62, 6.5),
        ),
        BiomeType::Rock => (hills01 * 3.5, mountain_peak(mountain01, 0.55, 6.5)),
        BiomeType::Sand => (hills01.powf(2.0) * 2.2, 0.0),
        BiomeType::Water => unreachable!(),
    };

    let height = ((hill_height + peak_height) * fade).round().min(6.0) as u8;
    if height == 0 {
        return;
    }

    // Tall columns are bare mountain rock; low relief keeps the biome's
    // surface material. Rock biomes are stone throughout.
    let material = if bt == BiomeType::Rock || height >= 4 {
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
