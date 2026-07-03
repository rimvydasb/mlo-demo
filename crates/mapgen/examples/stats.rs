//! Prints cell-type distribution across seeds — used to tune deposit
//! thresholds against the spec rarities (stone ≥30%, iron ~10%, gold ~5%).
//! Run: cargo run -p voxel-mapgen --example stats

use voxel_core::{BiomeCoord, CellType, CELLS_XY, CELLS_Z, SURFACE_Z};

fn main() {
    let mut counts = std::collections::BTreeMap::<&str, u64>::new();
    let mut solid = 0u64;
    let mut relief_max = 0usize;

    for seed in 0..8u64 {
        let map = voxel_mapgen::generate(seed);
        for row in 0..6u8 {
            for col in 0..6u8 {
                let grid = map.biome(BiomeCoord::new(row, col));
                for z in 0..SURFACE_Z {
                    for y in 0..CELLS_XY as u8 {
                        for x in 0..CELLS_XY as u8 {
                            let c = grid.get(x, y, z);
                            if c != CellType::Air {
                                solid += 1;
                                *counts.entry(c.label()).or_default() += 1;
                            }
                        }
                    }
                }
                for y in 0..CELLS_XY as u8 {
                    for x in 0..CELLS_XY as u8 {
                        let h = (SURFACE_Z + 1..CELLS_Z as u8)
                            .take_while(|&z| grid.get(x, y, z) != CellType::Air)
                            .count();
                        relief_max = relief_max.max(h);
                    }
                }
            }
        }
    }

    println!("underground solid cells: {solid}");
    for (label, n) in &counts {
        println!("  {label:>6}: {:5.1}%", *n as f64 / solid as f64 * 100.0);
    }
    println!("max relief height seen: {relief_max}");
}
