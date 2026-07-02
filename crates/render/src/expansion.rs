//! Cosmetic cell → 4×4×4 voxel expansion.
//!
//! This is the only place the visual voxel tier exists. `mapgen` and future
//! `sim` code never see it. The expansion is a pure function of the cell
//! grid, the layer cutoff, and the biome's world position + seed (for the
//! deterministic underside erosion), so screenshots are stable.

use voxel_core::{BiomeCoord, CellGrid, CellType, CELLS, SNOW_Z, SURFACE_Z};

/// Voxels per cell edge.
pub const SUB: usize = 4;
/// Voxels per biome edge.
pub const VOX: usize = CELLS * SUB; // 48

/// Visual voxel material. Distinct from `CellType`: soil splits into
/// grass/dirt, tall stone grows snow caps, and Air means "no voxel".
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum VoxelKind {
    #[default]
    Air,
    Grass,
    Dirt,
    Sand,
    Water,
    Stone,
    Gold,
    Iron,
    Snow,
}

impl VoxelKind {
    /// Occludes neighbouring faces (water does not — it's translucent).
    pub fn is_opaque(self) -> bool {
        !matches!(self, VoxelKind::Air | VoxelKind::Water)
    }
}

/// 48³ voxel volume for one biome.
pub struct VoxelVolume {
    data: Box<[VoxelKind]>,
}

impl VoxelVolume {
    fn new() -> Self {
        Self {
            data: vec![VoxelKind::Air; VOX * VOX * VOX].into_boxed_slice(),
        }
    }

    #[inline]
    fn idx(x: usize, y: usize, z: usize) -> usize {
        x + VOX * y + VOX * VOX * z
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> VoxelKind {
        self.data[Self::idx(x, y, z)]
    }

    #[inline]
    fn set(&mut self, x: usize, y: usize, z: usize, k: VoxelKind) {
        self.data[Self::idx(x, y, z)] = k;
    }

    /// Out-of-bounds reads as Air.
    #[inline]
    pub fn get_or_air(&self, x: i32, y: i32, z: i32) -> VoxelKind {
        let max = VOX as i32;
        if x < 0 || y < 0 || z < 0 || x >= max || y >= max || z >= max {
            VoxelKind::Air
        } else {
            self.get(x as usize, y as usize, z as usize)
        }
    }
}

/// Expand a biome's cell grid into its visual voxel volume.
///
/// `cutoff` hides cell layers with z ≥ cutoff (the inspector's layer peel);
/// peeled cells count as air, so freshly exposed soil grows a grass top just
/// like natural terrain.
pub fn expand(grid: &CellGrid, cutoff: u8, coord: BiomeCoord, seed: u64) -> VoxelVolume {
    let mut vol = VoxelVolume::new();
    let max = CELLS as u8;

    for z in 0..max.min(cutoff) {
        for y in 0..max {
            for x in 0..max {
                let cell = grid.get(x, y, z);
                if cell == CellType::Air {
                    continue;
                }
                let above = if z + 1 >= cutoff {
                    CellType::Air
                } else {
                    grid.get_or_air(x as i32, y as i32, z as i32 + 1)
                };
                expand_cell(&mut vol, x, y, z, cell, above == CellType::Air);
            }
        }
    }

    erode_underside(&mut vol, coord, seed);
    vol
}

/// Voxel sub-layers (dz = 0 bottom .. 3 top) for one cell — the pure
/// cell → voxel pattern from the spec table in rendering.md. Also drives the
/// inspector's expansion-preview panel.
pub fn cell_column(cell: CellType, top_air: bool, cz: u8) -> [VoxelKind; SUB] {
    std::array::from_fn(|dz| {
        let top = dz == SUB - 1;
        match cell {
            CellType::Air => VoxelKind::Air,
            CellType::Soil if top_air && top => VoxelKind::Grass,
            CellType::Soil => VoxelKind::Dirt,
            CellType::Sand if top_air && top => VoxelKind::Air, // sand sits slightly sunken
            CellType::Sand => VoxelKind::Sand,
            CellType::Water if top_air && dz >= SUB / 2 => VoxelKind::Air, // sunken water surface
            CellType::Water => VoxelKind::Water,
            CellType::Stone if top_air && top && cz >= SNOW_Z => VoxelKind::Snow,
            CellType::Stone => VoxelKind::Stone,
            CellType::Gold => VoxelKind::Gold,
            CellType::Iron => VoxelKind::Iron,
        }
    })
}

/// One cell → its 4×4×4 voxel block.
fn expand_cell(vol: &mut VoxelVolume, cx: u8, cy: u8, cz: u8, cell: CellType, top_air: bool) {
    let column = cell_column(cell, top_air, cz);
    for (dz, &kind) in column.iter().enumerate() {
        if kind == VoxelKind::Air {
            continue;
        }
        for dy in 0..SUB {
            for dx in 0..SUB {
                vol.set(
                    cx as usize * SUB + dx,
                    cy as usize * SUB + dy,
                    cz as usize * SUB + dz,
                    kind,
                );
            }
        }
    }
}

/// Deterministic hash → [0, 1). Seeded per world voxel so erosion (and the
/// mesher's color jitter) is stable across rebuilds and camera moves.
pub fn voxel_hash01(seed: u64, wx: i64, wy: i64, wz: i64) -> f32 {
    let mut h = seed
        ^ (wx as u64).wrapping_mul(0x9e3779b97f4a7c15)
        ^ (wy as u64).wrapping_mul(0xc2b2ae3d27d4eb4f)
        ^ (wz as u64).wrapping_mul(0x165667b19e3779f9);
    h ^= h >> 33;
    h = h.wrapping_mul(0xff51afd7ed558ccd);
    h ^= h >> 33;
    (h >> 40) as f32 / (1u64 << 24) as f32
}

/// Nibble voxels off the exposed underground boundary so the floating island
/// ends in ragged rubble instead of clean cell-sized steps (per the
/// reference art). Only side/bottom exposure counts — peeled top layers stay
/// crisp. Two passes deepen the erosion near the bottom.
fn erode_underside(vol: &mut VoxelVolume, coord: BiomeCoord, seed: u64) {
    let underground_top = SURFACE_Z as usize * SUB; // voxel z below this is underground
    let ox = coord.col as i64 * VOX as i64;
    let oy = coord.row as i64 * VOX as i64;

    for pass in 0..2u64 {
        let mut removals: Vec<(usize, usize, usize)> = Vec::new();
        for z in 0..underground_top {
            // Erosion gets more aggressive toward the island's bottom tip.
            let depth01 = 1.0 - z as f32 / underground_top as f32;
            let threshold = 0.25 + 0.35 * depth01;
            for y in 0..VOX {
                for x in 0..VOX {
                    if vol.get(x, y, z) == VoxelKind::Air {
                        continue;
                    }
                    let (xi, yi, zi) = (x as i32, y as i32, z as i32);
                    let side_exposed = vol.get_or_air(xi + 1, yi, zi) == VoxelKind::Air
                        || vol.get_or_air(xi - 1, yi, zi) == VoxelKind::Air
                        || vol.get_or_air(xi, yi + 1, zi) == VoxelKind::Air
                        || vol.get_or_air(xi, yi - 1, zi) == VoxelKind::Air
                        || vol.get_or_air(xi, yi, zi - 1) == VoxelKind::Air;
                    if !side_exposed {
                        continue;
                    }
                    let h = voxel_hash01(
                        seed.wrapping_add(pass),
                        ox + x as i64,
                        oy + y as i64,
                        z as i64,
                    );
                    if h < threshold {
                        removals.push((x, y, z));
                    }
                }
            }
        }
        for (x, y, z) in removals {
            vol.set(x, y, z, VoxelKind::Air);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voxel_core::BiomeCoord;

    fn flat_soil_grid() -> CellGrid {
        let mut g = CellGrid::new();
        for y in 0..12u8 {
            for x in 0..12u8 {
                for z in 0..=SURFACE_Z {
                    g.set(x, y, z, CellType::Soil);
                }
            }
        }
        g
    }

    #[test]
    fn soil_grows_grass_top_when_exposed() {
        let vol = expand(&flat_soil_grid(), 12, BiomeCoord::new(0, 0), 1);
        let top = SURFACE_Z as usize * SUB + SUB - 1;
        assert_eq!(vol.get(20, 20, top), VoxelKind::Grass);
        assert_eq!(vol.get(20, 20, top - 1), VoxelKind::Dirt);
    }

    #[test]
    fn peeling_exposes_new_grass() {
        // Cut at cell layer 3: the top of cell z=2 becomes exposed soil → grass.
        let vol = expand(&flat_soil_grid(), 3, BiomeCoord::new(0, 0), 1);
        let top = 2 * SUB + SUB - 1;
        assert_eq!(vol.get(20, 20, top), VoxelKind::Grass);
        // Nothing above the cutoff.
        assert_eq!(vol.get(20, 20, 3 * SUB), VoxelKind::Air);
    }

    #[test]
    fn exposed_water_is_sunken() {
        let mut g = flat_soil_grid();
        g.set(6, 6, SURFACE_Z, CellType::Water);
        let vol = expand(&g, 12, BiomeCoord::new(0, 0), 1);
        let base = SURFACE_Z as usize * SUB;
        assert_eq!(vol.get(25, 25, base), VoxelKind::Water);
        assert_eq!(vol.get(25, 25, base + 1), VoxelKind::Water);
        assert_eq!(vol.get(25, 25, base + 2), VoxelKind::Air);
        assert_eq!(vol.get(25, 25, base + 3), VoxelKind::Air);
    }

    #[test]
    fn snow_caps_tall_stone_only() {
        let mut g = flat_soil_grid();
        for z in SURFACE_Z + 1..12 {
            g.set(6, 6, z, CellType::Stone);
        }
        let vol = expand(&g, 12, BiomeCoord::new(0, 0), 1);
        let peak_top = 11 * SUB + SUB - 1;
        assert_eq!(vol.get(25, 25, peak_top), VoxelKind::Snow);
        // Below the snow band it stays bare stone even where locally exposed.
        let low_top = 8 * SUB + SUB - 1;
        assert_eq!(vol.get(25, 25, low_top), VoxelKind::Stone);
    }

    #[test]
    fn erosion_is_deterministic() {
        let g = flat_soil_grid();
        let a = expand(&g, 12, BiomeCoord::new(2, 3), 42);
        let b = expand(&g, 12, BiomeCoord::new(2, 3), 42);
        for z in 0..VOX {
            for y in 0..VOX {
                for x in 0..VOX {
                    assert_eq!(a.get(x, y, z), b.get(x, y, z));
                }
            }
        }
    }
}
