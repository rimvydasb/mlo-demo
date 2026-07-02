//! Terrain beautification passes (render tier, cosmetic only).
//!
//! These run on the voxel volume after the base cell → voxel expansion and
//! before the underside erosion, in the fixed order from rendering.md:
//!
//! 1. SLOPES — chamfer the top edge of a cell toward a lower lateral
//!    neighbor, so stacked-cell hills stop reading as staircases.
//! 2. CLIFF FRACTURES — knock the top voxels off one or two columns of an
//!    exposed side face, so cliffs read as fractured rock instead of cubes.
//! 3. MICROHEIGHT — (optional, `BeautifyOptions::microheight`) drop sparse
//!    top voxels on wide flat fields.
//! 4. RETOP — regrow grass tops / snow caps on whatever the subtractive
//!    passes left as the new top, exactly like peeled terrain does.
//! 5. GRASS OVERHANG — the grass top of a soil cell drapes one voxel down
//!    every exposed side, the thin green rim from the reference art.
//!
//! FLAT TOPS contract: nothing ever writes above a cell's own top plane —
//! interior grass fields stay flat so fauna & flora props can stand on them.
//! (The old GRASS TUFTS rule added single voxels above grass tops; they read
//! as pimples and were replaced by real grass/flower props — see
//! docs/rendering-fauna-flora.md.)
//!
//! Subtract first, then repaint, then add: overhang must see the
//! already-carved top or it would decorate voxels that no longer exist.
//!
//! Every random choice is `rule_hash01` on world voxel coordinates with a
//! per-rule discriminator — same seed, same terrain, on every platform.

use voxel_core::{CellGrid, CellType, CELLS, SNOW_Z};

use crate::expansion::{voxel_hash01, VoxelKind, VoxelVolume, SUB, VOX};

/// Toggles for the optional beautification rules. Lives here (render) but is
/// registered as a Bevy resource by `RenderPlugin` so the CLI can override it.
#[derive(bevy::prelude::Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BeautifyOptions {
    /// MICROHEIGHT is off by default: the per-voxel color jitter already
    /// carries most flat-field variation. A/B it via `--microheight`.
    pub microheight: bool,
}

// Rule discriminators for the determinism hash. Without these, two rules
// would draw correlated numbers at the same voxel and produce artifacts.
const RULE_CLIFF: u64 = 1;
const RULE_SLOPE: u64 = 2;
const RULE_MICRO: u64 = 3;

/// `voxel_hash01` with a rule discriminator folded into the seed — the
/// "determinism seed formula" from rendering.md. Mirrors mapgen's
/// `cell_hash01` mixer.
pub fn rule_hash01(seed: u64, rule: u64, wx: i64, wy: i64, wz: i64) -> f32 {
    voxel_hash01(seed ^ rule.wrapping_mul(0xd6e8feb86659fd93), wx, wy, wz)
}

/// The four lateral face directions as (dx, dy).
const DIRS: [(i32, i32); 4] = [(-1, 0), (1, 0), (0, -1), (0, 1)];

pub(crate) struct BeautifyCtx<'a> {
    grid: &'a CellGrid,
    cutoff: u8,
    /// World voxel offset of this biome (for the determinism hash).
    ox: i64,
    oy: i64,
    seed: u64,
}

impl<'a> BeautifyCtx<'a> {
    pub(crate) fn new(grid: &'a CellGrid, cutoff: u8, ox: i64, oy: i64, seed: u64) -> Self {
        Self {
            grid,
            cutoff,
            ox,
            oy,
            seed,
        }
    }

    /// Cell with the layer peel applied: peeled (z ≥ cutoff) reads as Air,
    /// same as the base expansion sees it.
    fn cell(&self, x: i32, y: i32, z: i32) -> CellType {
        if z >= self.cutoff as i32 {
            CellType::Air
        } else {
            self.grid.get_or_air(x, y, z)
        }
    }

    fn top_air(&self, x: u8, y: u8, z: u8) -> bool {
        self.cell(x as i32, y as i32, z as i32 + 1) == CellType::Air
    }

    fn hash(&self, rule: u64, vx: usize, vy: usize, vz: usize) -> f32 {
        rule_hash01(
            self.seed,
            rule,
            self.ox + vx as i64,
            self.oy + vy as i64,
            vz as i64,
        )
    }
}

/// Run all beautification passes in spec order.
pub(crate) fn apply(vol: &mut VoxelVolume, ctx: &BeautifyCtx, opts: BeautifyOptions) {
    slopes(vol, ctx);
    cliff_fractures(vol, ctx);
    if opts.microheight {
        microheight(vol, ctx);
    }
    retop(vol, ctx);
    grass_overhang(vol, ctx);
}

// ── Voxel column helpers ──────────────────────────────────────────────────────

/// Topmost non-air voxel z in the full-height column, or None.
fn column_top(vol: &VoxelVolume, vx: i32, vy: i32) -> Option<usize> {
    if vx < 0 || vy < 0 || vx >= VOX as i32 || vy >= VOX as i32 {
        return None;
    }
    (0..VOX)
        .rev()
        .find(|&z| vol.get(vx as usize, vy as usize, z) != VoxelKind::Air)
}

/// Topmost non-air voxel z within one cell's 4-voxel z range, or None.
fn cell_col_top(vol: &VoxelVolume, vx: usize, vy: usize, cz: u8) -> Option<usize> {
    let lo = cz as usize * SUB;
    (lo..lo + SUB)
        .rev()
        .find(|&z| vol.get(vx, vy, z) != VoxelKind::Air)
}

/// The 4 voxel positions of a cell's side-face line for direction `d`,
/// `inset` voxel steps in from the face (0 = the face itself).
fn face_line(cx: u8, cy: u8, d: (i32, i32), inset: usize) -> [(usize, usize); 4] {
    let bx = cx as usize * SUB;
    let by = cy as usize * SUB;
    std::array::from_fn(|i| match d {
        (-1, 0) => (bx + inset, by + i),
        (1, 0) => (bx + SUB - 1 - inset, by + i),
        (0, -1) => (bx + i, by + inset),
        _ => (bx + i, by + SUB - 1 - inset),
    })
}

/// Remove the top voxel of a cell-column, but only if nothing rests on it
/// (never leave a floating voxel) and at least one voxel stays behind.
fn shave_top(vol: &mut VoxelVolume, vx: usize, vy: usize, cz: u8) {
    let Some(t) = cell_col_top(vol, vx, vy, cz) else {
        return;
    };
    if t > cz as usize * SUB && vol.get_or_air(vx as i32, vy as i32, t as i32 + 1) == VoxelKind::Air
    {
        vol.set(vx, vy, t, VoxelKind::Air);
    }
}

// ── SLOPES ────────────────────────────────────────────────────────────────────

/// Chamfer the top edge of a top-exposed cell toward each lower lateral
/// neighbor: 1 voxel for small steps, a 2-voxel two-line chamfer for big
/// ones. Applies to soil, sand, stone; water surfaces stay flat by contract.
fn slopes(vol: &mut VoxelVolume, ctx: &BeautifyCtx) {
    for_top_cells(
        ctx,
        &[CellType::Soil, CellType::Sand, CellType::Stone],
        |x, y, z| {
            for d in DIRS {
                let outer = face_line(x, y, d, 0);
                // Face-level decision, hashed at the face's first voxel.
                let h = ctx.hash(RULE_SLOPE, outer[0].0, outer[0].1, z as usize * SUB);

                // Per-column so the chamfer follows tops already carved by the
                // other faces of this cell (corners).
                let mut any_big = false;
                for &(vx, vy) in &outer {
                    let Some(t) = cell_col_top(vol, vx, vy, z) else {
                        continue;
                    };
                    let neighbor_top = column_top(vol, vx as i32 + d.0, vy as i32 + d.1);
                    let diff = match neighbor_top {
                        Some(nt) if nt >= t => continue, // neighbor as high or higher
                        Some(nt) => t - nt,
                        None => SUB, // island edge: big drop
                    };
                    let big = diff >= 3 && h < 0.75;
                    shave_top(vol, vx, vy, z);
                    if big {
                        shave_top(vol, vx, vy, z);
                        any_big = true;
                    }
                }
                // Big steps also lower the second line by one, completing the
                // two-step chamfer.
                if any_big {
                    for (vx, vy) in face_line(x, y, d, 1) {
                        shave_top(vol, vx, vy, z);
                    }
                }
            }
        },
    );
}

// ── CLIFF FRACTURES ───────────────────────────────────────────────────────────

/// Break the straight vertical seam of exposed side faces: on a face whose
/// lateral neighbor cell is air, knock the top 1–3 voxels off up to two of
/// its four columns. Soil, stone, gold, iron only — fractured sand looks
/// wrong on a beach.
fn cliff_fractures(vol: &mut VoxelVolume, ctx: &BeautifyCtx) {
    let kinds = [
        CellType::Soil,
        CellType::Stone,
        CellType::Gold,
        CellType::Iron,
    ];
    for z in 0..(CELLS as u8).min(ctx.cutoff) {
        for y in 0..CELLS as u8 {
            for x in 0..CELLS as u8 {
                let cell = ctx.cell(x as i32, y as i32, z as i32);
                if !kinds.contains(&cell) {
                    continue;
                }
                for d in DIRS {
                    if ctx.cell(x as i32 + d.0, y as i32 + d.1, z as i32) != CellType::Air {
                        continue; // face not exposed
                    }
                    let mut fractured = 0;
                    for (vx, vy) in face_line(x, y, d, 0) {
                        if fractured == 2 {
                            break; // cap: never more than half the face
                        }
                        let h = ctx.hash(RULE_CLIFF, vx, vy, z as usize * SUB);
                        if h < 0.6 {
                            continue;
                        }
                        let depth = 1 + (((h - 0.6) / 0.4) * 3.0) as usize;
                        for _ in 0..depth.min(3) {
                            shave_top(vol, vx, vy, z);
                        }
                        fractured += 1;
                    }
                }
            }
        }
    }
}

// ── MICROHEIGHT ───────────────────────────────────────────────────────────────

/// Drop ~10% of top voxels on exposed soil/sand/stone cells. Optional: the
/// color jitter already carries most flat-field variation.
fn microheight(vol: &mut VoxelVolume, ctx: &BeautifyCtx) {
    for_top_cells(
        ctx,
        &[CellType::Soil, CellType::Sand, CellType::Stone],
        |x, y, z| {
            for dy in 0..SUB {
                for dx in 0..SUB {
                    let (vx, vy) = (x as usize * SUB + dx, y as usize * SUB + dy);
                    if ctx.hash(RULE_MICRO, vx, vy, z as usize * SUB) < 0.10 {
                        shave_top(vol, vx, vy, z);
                    }
                }
            }
        },
    );
}

// ── RETOP ─────────────────────────────────────────────────────────────────────

/// Regrow the cosmetic top on whatever the subtractive passes exposed:
/// carved soil regrows grass, carved high-band stone regrows its snow cap —
/// the same rule natural and peeled terrain follow.
fn retop(vol: &mut VoxelVolume, ctx: &BeautifyCtx) {
    for_top_cells(ctx, &[CellType::Soil, CellType::Stone], |x, y, z| {
        let snow = ctx.cell(x as i32, y as i32, z as i32) == CellType::Stone && z >= SNOW_Z;
        for dy in 0..SUB {
            for dx in 0..SUB {
                let (vx, vy) = (x as usize * SUB + dx, y as usize * SUB + dy);
                let Some(t) = cell_col_top(vol, vx, vy, z) else {
                    continue;
                };
                match vol.get(vx, vy, t) {
                    VoxelKind::Dirt => vol.set(vx, vy, t, VoxelKind::Grass),
                    VoxelKind::Stone if snow => vol.set(vx, vy, t, VoxelKind::Snow),
                    _ => {}
                }
            }
        }
    });
}

// ── GRASS OVERHANG ────────────────────────────────────────────────────────────

/// The grass top of a soil cell drapes one voxel down every side face whose
/// lateral voxel column is lower — the thin green rim over brown dirt seen
/// on every cliff of the reference art. A face-paint: writes only into this
/// cell's own voxel slots.
fn grass_overhang(vol: &mut VoxelVolume, ctx: &BeautifyCtx) {
    for_top_cells(ctx, &[CellType::Soil], |x, y, z| {
        for d in DIRS {
            for (vx, vy) in face_line(x, y, d, 0) {
                let Some(t) = cell_col_top(vol, vx, vy, z) else {
                    continue;
                };
                let lower_neighbor = match column_top(vol, vx as i32 + d.0, vy as i32 + d.1) {
                    Some(nt) => nt < t,
                    None => true,
                };
                if lower_neighbor
                    && t > z as usize * SUB
                    && vol.get(vx, vy, t - 1) == VoxelKind::Dirt
                {
                    vol.set(vx, vy, t - 1, VoxelKind::Grass);
                }
            }
        }
    });
}

// ── Iteration helper ──────────────────────────────────────────────────────────

/// Visit every top-exposed cell (air above, peel-aware) of the given types.
fn for_top_cells(ctx: &BeautifyCtx, kinds: &[CellType], mut f: impl FnMut(u8, u8, u8)) {
    for z in 0..(CELLS as u8).min(ctx.cutoff) {
        for y in 0..CELLS as u8 {
            for x in 0..CELLS as u8 {
                let cell = ctx.cell(x as i32, y as i32, z as i32);
                if kinds.contains(&cell) && ctx.top_air(x, y, z) {
                    f(x, y, z);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::expansion::expand;
    use voxel_core::{BiomeCoord, SURFACE_Z};

    const OPTS: BeautifyOptions = BeautifyOptions { microheight: false };

    fn flat_soil_grid() -> CellGrid {
        let mut g = CellGrid::new();
        for y in 0..CELLS as u8 {
            for x in 0..CELLS as u8 {
                for z in 0..=SURFACE_Z {
                    g.set(x, y, z, CellType::Soil);
                }
            }
        }
        g
    }

    fn top_of(vol: &VoxelVolume, x: usize, y: usize) -> Option<usize> {
        (0..VOX).rev().find(|&z| vol.get(x, y, z) != VoxelKind::Air)
    }

    #[test]
    fn slopes_chamfer_relief_steps() {
        // One relief cell on a flat field: every perimeter voxel column of
        // its exposed top must lose at least one voxel to the chamfer, and
        // retop must regrow grass on whatever is left.
        let mut g = flat_soil_grid();
        g.set(6, 6, SURFACE_Z + 1, CellType::Soil);
        let vol = expand(&g, 12, BiomeCoord::new(0, 0), 42, OPTS);

        let base_top = (SURFACE_Z as usize + 1) * SUB + SUB - 1;
        for d in DIRS {
            for (vx, vy) in face_line(6, 6, d, 0) {
                let t = top_of(&vol, vx, vy).unwrap();
                assert!(t < base_top, "({vx},{vy}) rim column not chamfered");
                assert_eq!(
                    vol.get(vx, vy, t),
                    VoxelKind::Grass,
                    "({vx},{vy}) not regrassed"
                );
            }
        }
    }

    #[test]
    fn grass_overhang_drapes_the_island_rim() {
        // On the biome rim the lateral neighbor is world air, so grass must
        // drape a voxel down the face on a healthy share of columns.
        let vol = expand(&flat_soil_grid(), 12, BiomeCoord::new(0, 0), 42, OPTS);
        let mut draped = 0;
        for vy in 0..VOX {
            let t = top_of(&vol, 0, vy).unwrap();
            assert_eq!(vol.get(0, vy, t), VoxelKind::Grass);
            if t > SURFACE_Z as usize * SUB && vol.get(0, vy, t - 1) == VoxelKind::Grass {
                draped += 1;
            }
        }
        assert!(draped > VOX / 2, "only {draped}/{VOX} rim columns draped");
    }

    #[test]
    fn grass_tops_are_flat() {
        // FLAT TOPS contract: fauna & flora props stand on grass, so nothing
        // may poke above a flat field's top plane (the old GRASS TUFTS rule
        // left single-voxel pimples here).
        let vol = expand(&flat_soil_grid(), 12, BiomeCoord::new(0, 0), 42, OPTS);
        let above = SURFACE_Z as usize * SUB + SUB; // one above the grass top
        for vy in 0..VOX {
            for vx in 0..VOX {
                assert_eq!(
                    vol.get(vx, vy, above),
                    VoxelKind::Air,
                    "pimple above flat grass at ({vx},{vy})"
                );
            }
        }
    }

    #[test]
    fn microheight_only_fires_when_enabled() {
        let g = flat_soil_grid();
        let base = expand(&g, 12, BiomeCoord::new(0, 0), 42, OPTS);
        let micro = expand(
            &g,
            12,
            BiomeCoord::new(0, 0),
            42,
            BeautifyOptions { microheight: true },
        );
        let differs = (0..VOX)
            .any(|z| (0..VOX).any(|y| (0..VOX).any(|x| base.get(x, y, z) != micro.get(x, y, z))));
        assert!(differs, "--microheight had no effect");
    }

    #[test]
    fn no_floating_voxels_above_the_erosion_band() {
        // Slopes/cliffs/microheight only shave tops and tufts sit on grass,
        // so above the underside-erosion band every solid voxel must rest on
        // another. Checked over a real generated world for coverage.
        let map = voxel_mapgen::generate(42);
        for row in 0..2u8 {
            for col in 0..2u8 {
                let coord = BiomeCoord::new(row, col);
                let vol = expand(
                    map.biome(coord),
                    12,
                    coord,
                    42,
                    BeautifyOptions { microheight: true },
                );
                let erosion_top = SURFACE_Z as usize * SUB;
                for z in erosion_top + 1..VOX {
                    for y in 0..VOX {
                        for x in 0..VOX {
                            if vol.get(x, y, z) != VoxelKind::Air {
                                assert_ne!(
                                    vol.get(x, y, z - 1),
                                    VoxelKind::Air,
                                    "floating voxel at ({x},{y},{z}) in biome ({row},{col})"
                                );
                            }
                        }
                    }
                }
            }
        }
    }
}
