//! Cosmetic cell → 4×4×4 voxel expansion.
//!
//! This is the only place the visual voxel tier exists. `mapgen` and future
//! `sim` code never see it. The expansion is a pure function of the cell
//! grid, the biome type, the layer cutoff, and the biome's world position +
//! seed (for the deterministic underside erosion), so screenshots are stable.

use voxel_core::{BiomeCoord, BiomeType, CellGrid, CellType, CELLS_XY, CELLS_Z, SNOW_Z, SURFACE_Z};

use crate::beautify::BeautifyOptions;

/// Voxels per cell edge.
pub const SUB: usize = 4;
/// Voxels per biome edge in the horizontal plane.
pub const VOX_XY: usize = CELLS_XY * SUB; // 32
/// Voxels per biome column (vertical).
pub const VOX_Z: usize = CELLS_Z * SUB; // 48

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

/// 32×32×48 voxel volume for one biome.
pub struct VoxelVolume {
    data: Box<[VoxelKind]>,
}

impl VoxelVolume {
    fn new() -> Self {
        Self {
            data: vec![VoxelKind::Air; VOX_XY * VOX_XY * VOX_Z].into_boxed_slice(),
        }
    }

    #[inline]
    fn idx(x: usize, y: usize, z: usize) -> usize {
        x + VOX_XY * y + VOX_XY * VOX_XY * z
    }

    #[inline]
    pub fn get(&self, x: usize, y: usize, z: usize) -> VoxelKind {
        self.data[Self::idx(x, y, z)]
    }

    #[inline]
    pub(crate) fn set(&mut self, x: usize, y: usize, z: usize, k: VoxelKind) {
        self.data[Self::idx(x, y, z)] = k;
    }

    /// Out-of-bounds reads as Air.
    #[inline]
    pub fn get_or_air(&self, x: i32, y: i32, z: i32) -> VoxelKind {
        let max_xy = VOX_XY as i32;
        let max_z = VOX_Z as i32;
        if x < 0 || y < 0 || z < 0 || x >= max_xy || y >= max_xy || z >= max_z {
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
/// like natural terrain. `bt` drives the winter dressing: in winter biomes
/// every exposed soil or stone top wears snow instead of grass.
///
/// Pipeline: base cell → voxel patterns, then the beautification passes
/// (slopes, cliff fractures, optional microheight, retop, grass overhang —
/// see `beautify`), then the underground passes: cave carving, underside
/// erosion, and the pinhole seal (no single-voxel air holes underground).
pub fn expand(
    grid: &CellGrid,
    bt: BiomeType,
    cutoff: u8,
    coord: BiomeCoord,
    seed: u64,
    opts: BeautifyOptions,
) -> VoxelVolume {
    let mut vol = VoxelVolume::new();
    let winter = bt == BiomeType::Winter;

    for z in 0..(CELLS_Z as u8).min(cutoff) {
        for y in 0..CELLS_XY as u8 {
            for x in 0..CELLS_XY as u8 {
                let cell = grid.get(x, y, z);
                if cell == CellType::Air {
                    continue;
                }
                let above = if z + 1 >= cutoff {
                    CellType::Air
                } else {
                    grid.get_or_air(x as i32, y as i32, z as i32 + 1)
                };
                expand_cell(&mut vol, x, y, z, cell, above == CellType::Air, winter);
            }
        }
    }

    let ox = coord.col as i64 * VOX_XY as i64;
    let oy = coord.row as i64 * VOX_XY as i64;
    let ctx = crate::beautify::BeautifyCtx::new(grid, cutoff, ox, oy, seed, winter);
    crate::beautify::apply(&mut vol, &ctx, opts);

    carve_caves(&mut vol, coord, seed);
    erode_underside(&mut vol, coord, seed);
    seal_pinholes(&mut vol);
    vol
}

/// Voxel sub-layers (dz = 0 bottom .. 3 top) for one cell — the pure
/// cell → voxel pattern from the spec table in rendering.md. Also drives the
/// inspector's expansion-preview panel. `winter` swaps the exposed-top
/// dressing: soil tops wear snow instead of grass, and stone tops are snowy
/// at any height, not just in the SNOW_Z band.
pub fn cell_column(cell: CellType, top_air: bool, cz: u8, winter: bool) -> [VoxelKind; SUB] {
    std::array::from_fn(|dz| {
        let top = dz == SUB - 1;
        match cell {
            CellType::Air => VoxelKind::Air,
            CellType::Soil if top_air && top && winter => VoxelKind::Snow,
            CellType::Soil if top_air && top => VoxelKind::Grass,
            CellType::Soil => VoxelKind::Dirt,
            CellType::Sand if top_air && top => VoxelKind::Air, // sand sits slightly sunken
            CellType::Sand => VoxelKind::Sand,
            CellType::Water if top_air && dz >= SUB / 2 => VoxelKind::Air, // sunken water surface
            CellType::Water => VoxelKind::Water,
            CellType::Stone if top_air && top && (winter || cz >= SNOW_Z) => VoxelKind::Snow,
            CellType::Stone => VoxelKind::Stone,
            CellType::Gold => VoxelKind::Gold,
            CellType::Iron => VoxelKind::Iron,
        }
    })
}

/// One cell → its 4×4×4 voxel block.
fn expand_cell(
    vol: &mut VoxelVolume,
    cx: u8,
    cy: u8,
    cz: u8,
    cell: CellType,
    top_air: bool,
    winter: bool,
) {
    let column = cell_column(cell, top_air, cz, winter);
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

/// Top of the underground voxel band (voxel z below this is underground).
const UNDERGROUND_TOP: usize = SURFACE_Z as usize * SUB;

/// Rule discriminator for cave parameters (see `beautify::rule_hash01`; the
/// beautify passes use 1–5, mapgen's beach pass uses 6).
const RULE_CAVE: u64 = 8;

/// Carve 0–2 deterministic ellipsoid cave pockets per biome into the
/// underground mass. Caves are interior voids — invisible from outside until
/// the layer peel (or a lucky erosion breach) exposes them, which is exactly
/// the "mystery" they are for. They stay below the cell band directly under
/// the surface (voxel z < 16), so the surface never loses its visual support,
/// and they are voxel-tier only: the cell grid (mining, invariants) never
/// changes.
fn carve_caves(vol: &mut VoxelVolume, coord: BiomeCoord, seed: u64) {
    let ox = coord.col as i64 * VOX_XY as i64;
    let oy = coord.row as i64 * VOX_XY as i64;
    // Per-biome, per-cave parameter hash: world offset + cave/param ids.
    let param = |cave: u64, p: u64| {
        crate::beautify::rule_hash01(seed, RULE_CAVE + cave * 16 + p, ox, oy, 0)
    };

    let count = (param(0, 0) * 3.0) as u64; // 0, 1, or 2 caves
    for cave in 1..=count {
        let rx = 2.5 + param(cave, 1) * 2.5; // lateral radii 2.5–5 voxels
        let ry = 2.5 + param(cave, 2) * 2.5;
        let rz = 2.0 + param(cave, 3) * 1.5; // flatter than wide, like real pockets
                                             // Center: laterally well inside the biome, vertically inside the
                                             // funnel mass but below the surface-support band (cap + margin ≤ 16).
        let margin = 2.0;
        let cx = rx + margin + param(cave, 4) * (VOX_XY as f32 - 2.0 * (rx + margin));
        let cy = ry + margin + param(cave, 5) * (VOX_XY as f32 - 2.0 * (ry + margin));
        let z_top = (4 * SUB) as f32 - rz; // cell z=4 stays untouched
        let cz = rz + margin + param(cave, 6) * (z_top - rz - margin).max(0.0);

        for z in 0..4 * SUB {
            for y in 0..VOX_XY {
                for x in 0..VOX_XY {
                    let dx = (x as f32 + 0.5 - cx) / rx;
                    let dy = (y as f32 + 0.5 - cy) / ry;
                    let dz = (z as f32 + 0.5 - cz) / rz;
                    if dx * dx + dy * dy + dz * dz < 1.0 {
                        vol.set(x, y, z, VoxelKind::Air);
                    }
                }
            }
        }
    }
}

/// Fill single-voxel air pockets in the underground band until none remain:
/// any air voxel with ≥5 solid face-neighbours (out-of-bounds counts as air)
/// is refilled with its most common neighbour kind. This is the "no voxel
/// holes" rule — the underside erosion nibbles voxel-by-voxel, and isolated
/// one-voxel pits read as termite damage instead of weathering. Larger
/// openings (erosion clusters, cave mouths, caves themselves) survive
/// untouched. Runs to a fixpoint, so sealing one pinhole never leaves a new
/// one behind.
fn seal_pinholes(vol: &mut VoxelVolume) {
    loop {
        let mut fills: Vec<(usize, usize, usize, VoxelKind)> = Vec::new();
        for z in 0..UNDERGROUND_TOP {
            for y in 0..VOX_XY {
                for x in 0..VOX_XY {
                    if vol.get(x, y, z) != VoxelKind::Air {
                        continue;
                    }
                    let (xi, yi, zi) = (x as i32, y as i32, z as i32);
                    let neighbors = [
                        vol.get_or_air(xi + 1, yi, zi),
                        vol.get_or_air(xi - 1, yi, zi),
                        vol.get_or_air(xi, yi + 1, zi),
                        vol.get_or_air(xi, yi - 1, zi),
                        vol.get_or_air(xi, yi, zi + 1),
                        vol.get_or_air(xi, yi, zi - 1),
                    ];
                    let solid = neighbors.iter().filter(|k| k.is_opaque()).count();
                    if solid >= 5 {
                        fills.push((x, y, z, dominant_kind(&neighbors)));
                    }
                }
            }
        }
        if fills.is_empty() {
            break;
        }
        for (x, y, z, kind) in fills {
            vol.set(x, y, z, kind);
        }
    }
}

/// Most common opaque kind among the given neighbours; ties break toward
/// plain terrain (dirt, then stone) so sealing never mints extra ore voxels.
fn dominant_kind(neighbors: &[VoxelKind; 6]) -> VoxelKind {
    // `max_by_key` keeps the *last* maximum, so list preferred kinds last.
    const ORDER: [VoxelKind; 7] = [
        VoxelKind::Gold,
        VoxelKind::Iron,
        VoxelKind::Snow,
        VoxelKind::Grass,
        VoxelKind::Sand,
        VoxelKind::Stone,
        VoxelKind::Dirt,
    ];
    let count = |k| neighbors.iter().filter(|&&n| n == k).count();
    ORDER
        .into_iter()
        .max_by_key(|&k| count(k))
        .unwrap_or(VoxelKind::Dirt)
}

/// Nibble voxels off the exposed underground boundary so the floating island
/// ends in ragged rubble instead of clean cell-sized steps (per the
/// reference art). Only side/bottom exposure counts — peeled top layers stay
/// crisp. Two passes deepen the erosion near the bottom.
fn erode_underside(vol: &mut VoxelVolume, coord: BiomeCoord, seed: u64) {
    let underground_top = SURFACE_Z as usize * SUB; // voxel z below this is underground
    let ox = coord.col as i64 * VOX_XY as i64;
    let oy = coord.row as i64 * VOX_XY as i64;

    for pass in 0..2u64 {
        let mut removals: Vec<(usize, usize, usize)> = Vec::new();
        for z in 0..underground_top {
            // Erosion gets more aggressive toward the island's bottom tip.
            let depth01 = 1.0 - z as f32 / underground_top as f32;
            let threshold = 0.25 + 0.35 * depth01;
            for y in 0..VOX_XY {
                for x in 0..VOX_XY {
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
        for y in 0..CELLS_XY as u8 {
            for x in 0..CELLS_XY as u8 {
                for z in 0..=SURFACE_Z {
                    g.set(x, y, z, CellType::Soil);
                }
            }
        }
        g
    }

    fn top_of(vol: &VoxelVolume, x: usize, y: usize) -> Option<(usize, VoxelKind)> {
        (0..VOX_Z)
            .rev()
            .find(|&z| vol.get(x, y, z) != VoxelKind::Air)
            .map(|z| (z, vol.get(x, y, z)))
    }

    #[test]
    fn soil_grows_grass_top_when_exposed() {
        let vol = expand(
            &flat_soil_grid(),
            BiomeType::Grass,
            CELLS_Z as u8,
            BiomeCoord::new(0, 0),
            1,
            BeautifyOptions::default(),
        );
        let top = SURFACE_Z as usize * SUB + SUB - 1;
        assert_eq!(vol.get(16, 16, top), VoxelKind::Grass);
        assert_eq!(vol.get(16, 16, top - 1), VoxelKind::Dirt);
    }

    #[test]
    fn winter_soil_grows_snow_top_when_exposed() {
        let vol = expand(
            &flat_soil_grid(),
            BiomeType::Winter,
            CELLS_Z as u8,
            BiomeCoord::new(0, 0),
            1,
            BeautifyOptions::default(),
        );
        let top = SURFACE_Z as usize * SUB + SUB - 1;
        assert_eq!(vol.get(16, 16, top), VoxelKind::Snow);
        assert_eq!(vol.get(16, 16, top - 1), VoxelKind::Dirt);
    }

    #[test]
    fn winter_stone_is_snow_capped_at_any_height() {
        // In winter, exposed stone wears snow well below the SNOW_Z band.
        let col = cell_column(CellType::Stone, true, SURFACE_Z, true);
        assert_eq!(col[SUB - 1], VoxelKind::Snow);
        // Outside winter the low stone top stays bare.
        let col = cell_column(CellType::Stone, true, SURFACE_Z, false);
        assert_eq!(col[SUB - 1], VoxelKind::Stone);
    }

    #[test]
    fn peeling_exposes_new_grass() {
        // Cut at cell layer 3: the top of cell z=2 becomes exposed soil → grass.
        let vol = expand(
            &flat_soil_grid(),
            BiomeType::Grass,
            3,
            BiomeCoord::new(0, 0),
            1,
            BeautifyOptions::default(),
        );
        let top = 2 * SUB + SUB - 1;
        assert_eq!(vol.get(16, 16, top), VoxelKind::Grass);
        // Nothing above the cutoff — grass tops stay flat (FLAT TOPS).
        assert_eq!(vol.get(16, 16, 3 * SUB), VoxelKind::Air);
        assert_eq!(vol.get(16, 16, 3 * SUB + 1), VoxelKind::Air);
    }

    #[test]
    fn exposed_water_is_sunken() {
        let mut g = flat_soil_grid();
        g.set(4, 4, SURFACE_Z, CellType::Water);
        let vol = expand(
            &g,
            BiomeType::Grass,
            CELLS_Z as u8,
            BiomeCoord::new(0, 0),
            1,
            BeautifyOptions::default(),
        );
        let base = SURFACE_Z as usize * SUB;
        assert_eq!(vol.get(17, 17, base), VoxelKind::Water);
        assert_eq!(vol.get(17, 17, base + 1), VoxelKind::Water);
        assert_eq!(vol.get(17, 17, base + 2), VoxelKind::Air);
        assert_eq!(vol.get(17, 17, base + 3), VoxelKind::Air);
    }

    #[test]
    fn snow_caps_tall_stone_only() {
        let mut g = flat_soil_grid();
        for z in SURFACE_Z + 1..CELLS_Z as u8 {
            g.set(4, 4, z, CellType::Stone);
        }
        let vol = expand(
            &g,
            BiomeType::Grass,
            CELLS_Z as u8,
            BiomeCoord::new(0, 0),
            1,
            BeautifyOptions::default(),
        );
        // The peak column may be chamfered by SLOPES/CLIFF FRACTURES, but
        // whatever remains on top in the snow band must be snow (retop).
        let (peak_z, peak_kind) = top_of(&vol, 17, 17).unwrap();
        assert!(
            peak_z >= SNOW_Z as usize * SUB,
            "peak carved below snow band"
        );
        assert_eq!(peak_kind, VoxelKind::Snow);
        // Below the snow band it stays bare stone even where locally exposed.
        let low_top = 8 * SUB + SUB - 1;
        assert_eq!(vol.get(17, 17, low_top), VoxelKind::Stone);
    }

    #[test]
    fn no_single_voxel_air_holes_underground() {
        // The "no voxel holes" rule: after erosion + sealing, no air voxel in
        // the underground band may be a pinhole (≥5 solid face-neighbours).
        // Checked over real generated biomes for coverage.
        let map = voxel_mapgen::generate(42);
        for row in 0..3u8 {
            for col in 0..3u8 {
                let coord = BiomeCoord::new(row, col);
                let vol = expand(
                    map.biome(coord),
                    map.biome_type(coord),
                    CELLS_Z as u8,
                    coord,
                    42,
                    BeautifyOptions::default(),
                );
                for z in 0..UNDERGROUND_TOP {
                    for y in 0..VOX_XY {
                        for x in 0..VOX_XY {
                            if vol.get(x, y, z) != VoxelKind::Air {
                                continue;
                            }
                            let (xi, yi, zi) = (x as i32, y as i32, z as i32);
                            let solid = [
                                vol.get_or_air(xi + 1, yi, zi),
                                vol.get_or_air(xi - 1, yi, zi),
                                vol.get_or_air(xi, yi + 1, zi),
                                vol.get_or_air(xi, yi - 1, zi),
                                vol.get_or_air(xi, yi, zi + 1),
                                vol.get_or_air(xi, yi, zi - 1),
                            ]
                            .iter()
                            .filter(|k| k.is_opaque())
                            .count();
                            assert!(solid < 5, "pinhole at ({x},{y},{z}) in biome ({row},{col})");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn caves_stay_below_the_surface_support_band() {
        // Caves may only remove voxels below cell z=4 (voxel z<16): the band
        // directly under the surface keeps its visual support everywhere.
        let mut solid = VoxelVolume::new();
        for z in 0..VOX_Z {
            for y in 0..VOX_XY {
                for x in 0..VOX_XY {
                    solid.set(x, y, z, VoxelKind::Dirt);
                }
            }
        }
        let mut carved_any = false;
        for row in 0..6u8 {
            for col in 0..6u8 {
                let mut vol = VoxelVolume::new();
                vol.data.copy_from_slice(&solid.data);
                carve_caves(&mut vol, BiomeCoord::new(row, col), 42);
                for z in 0..VOX_Z {
                    for y in 0..VOX_XY {
                        for x in 0..VOX_XY {
                            let is_air = vol.get(x, y, z) == VoxelKind::Air;
                            carved_any |= is_air;
                            assert!(
                                !(is_air && z >= 4 * SUB),
                                "cave breached support band at ({x},{y},{z}) biome ({row},{col})"
                            );
                        }
                    }
                }
            }
        }
        assert!(carved_any, "no biome of seed 42 carved any cave");
    }

    #[test]
    fn expansion_is_deterministic() {
        let g = flat_soil_grid();
        let opts = BeautifyOptions { microheight: true };
        let a = expand(
            &g,
            BiomeType::Grass,
            CELLS_Z as u8,
            BiomeCoord::new(2, 3),
            42,
            opts,
        );
        let b = expand(
            &g,
            BiomeType::Grass,
            CELLS_Z as u8,
            BiomeCoord::new(2, 3),
            42,
            opts,
        );
        for z in 0..VOX_Z {
            for y in 0..VOX_XY {
                for x in 0..VOX_XY {
                    assert_eq!(a.get(x, y, z), b.get(x, y, z));
                }
            }
        }
    }
}
