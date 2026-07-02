//! Decoration planning: deterministic fauna & flora placement on the cell
//! grid (see docs/rendering-fauna-flora.md).
//!
//! This module is the *planner* only — a pure function of the cell grid, the
//! layer cutoff, the biome coordinate, and the seed. It decides **what**
//! stands **where** (kind, variant, transform, animation phase) and nothing
//! else. Loading the GLB assets and spawning/animating entities is the app
//! tier's job (`voxel-app::{assets, decor}`); this keeps placement headless
//! and unit-testable, exactly like the mapgen passes.
//!
//! Placement is cell-tier: decorations anchor to the top cell of a column,
//! never to individual voxels. All decorations are cosmetic — the cell grid
//! (mining, connections, invariants) never changes.
//!
//! Every random choice is `rule_hash01` on world *cell* coordinates with a
//! per-kind rule id, so plans are deterministic per seed and independent of
//! biome generation order — the same contract as the beautify passes.

use bevy::math::Vec3;
use std::f32::consts::TAU;
use voxel_core::{BiomeCoord, CellGrid, CellType, CELLS};

use crate::beautify::rule_hash01;
use crate::mesh::VOXEL_SIZE;

// ── Catalog ───────────────────────────────────────────────────────────────────

/// One loadable model variant. `path` is relative to the workspace `assets/`
/// root; `scale` is the base uniform scale that brings the model's measured
/// GLB bounds to its target world size (animals fit a 0.5-unit box = 2×2
/// voxels; trees ≈ 1.7 world units tall; flowers ≈ 0.2–0.4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecorModel {
    pub path: &'static str,
    pub scale: f32,
}

const fn model(path: &'static str, scale: f32) -> DecorModel {
    DecorModel { path, scale }
}

/// Trees stand on soil (grass) tops.
pub const TREES: &[DecorModel] = &[
    model("models/flora/tree_default.glb", 1.00), // native h 1.71
    model("models/flora/tree_oak.glb", 1.40),     // native h 1.23
    model("models/flora/tree_pineDefaultA.glb", 1.15), // native h 1.55
    model("models/flora/tree_simple.glb", 1.10),  // native h 1.52
];

/// Palms stand on sand tops only.
pub const PALMS: &[DecorModel] = &[
    model("models/flora/tree_palm.glb", 1.15), // native h 1.52
    model("models/flora/tree_palmShort.glb", 1.25), // native h 1.06
    model("models/flora/tree_palmTall.glb", 1.30), // native h 1.36
    model("models/flora/tree_palmBend.glb", 1.20), // native h 1.38
];

/// Flowers stand on grass (exposed soil) tops.
pub const FLOWERS: &[DecorModel] = &[
    model("models/flora/flower_purpleA.glb", 1.3),
    model("models/flora/flower_redA.glb", 1.3),
    model("models/flora/flower_yellowA.glb", 1.3),
    model("models/flora/flower_purpleC.glb", 1.3),
    model("models/flora/flower_redC.glb", 1.3),
    model("models/flora/flower_yellowC.glb", 1.3),
];

/// Grass/bush props on grass tops — the prop-tier replacement for the
/// removed single-voxel GRASS TUFTS rule.
pub const GRASS_PROPS: &[DecorModel] = &[
    model("models/flora/grass.glb", 1.1),
    model("models/flora/grass_large.glb", 1.1),
    model("models/flora/plant_bushSmall.glb", 1.3),
];

/// Land animals, grouped by habitat via the `ANIMAL_*` ranges below.
/// Scales fit each model's measured max dimension into a 0.5-unit box.
pub const ANIMALS: &[DecorModel] = &[
    // Meadow (soil/grass tops)
    model("models/fauna/animal-bunny.glb", 0.25),
    model("models/fauna/animal-fox.glb", 0.22),
    model("models/fauna/animal-deer.glb", 0.25),
    model("models/fauna/animal-pig.glb", 0.32),
    model("models/fauna/animal-chick.glb", 0.23),
    model("models/fauna/animal-cow.glb", 0.31),
    // Beach (sand tops)
    model("models/fauna/animal-crab.glb", 0.21),
    model("models/fauna/animal-parrot.glb", 0.23),
    // Mountain (stone tops)
    model("models/fauna/animal-penguin.glb", 0.23),
    model("models/fauna/animal-polar.glb", 0.33),
];
pub const ANIMAL_MEADOW: std::ops::Range<usize> = 0..6;
pub const ANIMAL_BEACH: std::ops::Range<usize> = 6..8;
pub const ANIMAL_MOUNTAIN: std::ops::Range<usize> = 8..10;

/// Fish swim inside surface water cells. Scale keeps the body (max dim
/// 1.88 native) well below the sunken water surface — at larger scales the
/// fish fills the whole 0.5-unit water depth and reads as standing *on* it.
pub const FISH: &[DecorModel] = &[model("models/fauna/animal-fish.glb", 0.15)];

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum DecorKind {
    Tree,
    Palm,
    Flower,
    GrassProp,
    Animal,
    Fish,
}

impl DecorKind {
    pub const ALL: [DecorKind; 6] = [
        DecorKind::Tree,
        DecorKind::Palm,
        DecorKind::Flower,
        DecorKind::GrassProp,
        DecorKind::Animal,
        DecorKind::Fish,
    ];

    pub fn models(self) -> &'static [DecorModel] {
        match self {
            DecorKind::Tree => TREES,
            DecorKind::Palm => PALMS,
            DecorKind::Flower => FLOWERS,
            DecorKind::GrassProp => GRASS_PROPS,
            DecorKind::Animal => ANIMALS,
            DecorKind::Fish => FISH,
        }
    }

    /// Stable index for kind-keyed storage (asset handle tables).
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|&k| k == self).unwrap()
    }
}

// ── Plan output ───────────────────────────────────────────────────────────────

/// One planned decoration. `translation` is in biome-local Bevy world units
/// (X east, Y up, Z south — same space the terrain mesh is emitted in);
/// `scale` is final (catalog base × per-instance jitter); `phase` in [0, 1)
/// de-synchronizes animations between neighbours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DecorInstance {
    pub kind: DecorKind,
    pub variant: usize,
    pub translation: Vec3,
    pub yaw: f32,
    pub scale: f32,
    pub phase: f32,
}

// ── Densities (probability per eligible top cell) ─────────────────────────────

const TREE_P: f32 = 0.06;
const PALM_P: f32 = 0.06;
const FLOWER_P: f32 = 0.10;
const GRASS_PROP_P: f32 = 0.12;
const FISH_P: f32 = 0.10;
const ANIMAL_MEADOW_P: f32 = 0.05;
/// Extra meadow-animal probability when a tree stands within 2 cells —
/// "higher density in areas with grass and trees".
const ANIMAL_NEAR_TREE_BONUS: f32 = 0.05;
const ANIMAL_BEACH_P: f32 = 0.03;
const ANIMAL_MOUNTAIN_P: f32 = 0.02;

// ── Determinism rule ids ──────────────────────────────────────────────────────
// Each kind owns a block of 8 ids: +0 presence, +1 variant, +2 yaw,
// +3/+4 lateral jitter, +5 scale, +6 phase. The 64+ range stays clear of the
// beautify rules (1–5), the beach pass (6), and the cave block (8–46).

const RULE_TREE: u64 = 64;
const RULE_PALM: u64 = 72;
const RULE_FLOWER: u64 = 80;
const RULE_GRASS_PROP: u64 = 88;
const RULE_ANIMAL: u64 = 96;
const RULE_FISH: u64 = 104;

// ── Planner ───────────────────────────────────────────────────────────────────

/// Plan all decorations for one biome. Pure and deterministic: same inputs,
/// same plan, on every platform. Cutoff-aware like the expansion — peeling
/// layers re-plans decor on the freshly exposed surface, exactly as peeled
/// soil regrows a grass top.
pub fn plan_decor(grid: &CellGrid, cutoff: u8, coord: BiomeCoord, seed: u64) -> Vec<DecorInstance> {
    let mut out = Vec::new();
    let tops: Vec<Option<(u8, CellType)>> = (0..CELLS * CELLS)
        .map(|i| column_top(grid, cutoff, (i % CELLS) as u8, (i / CELLS) as u8))
        .collect();
    let top = |x: i32, y: i32| -> Option<(u8, CellType)> {
        if x < 0 || y < 0 || x >= CELLS as i32 || y >= CELLS as i32 {
            None
        } else {
            tops[x as usize + CELLS * y as usize]
        }
    };

    let mut ctx = Ctx {
        coord,
        seed,
        out: &mut out,
    };
    let mut occupied = [false; CELLS * CELLS];
    let mut tree_at = [false; CELLS * CELLS];

    // Pass A: flora and fish. At most one decoration per cell by
    // construction (first matching rule wins).
    for y in 0..CELLS as u8 {
        for x in 0..CELLS as u8 {
            let Some((z, cell)) = top(x as i32, y as i32) else {
                continue;
            };
            let i = x as usize + CELLS * y as usize;
            match cell {
                CellType::Water => {
                    if ctx.roll(RULE_FISH, x, y, z) < FISH_P {
                        ctx.place(DecorKind::Fish, RULE_FISH, x, y, z, cell, 0..1, 0.25);
                        occupied[i] = true;
                    }
                }
                CellType::Soil => {
                    let canopy_fits = interior(x, y) && local_flat(&top, x, y, z);
                    if canopy_fits && ctx.roll(RULE_TREE, x, y, z) < TREE_P {
                        ctx.place(
                            DecorKind::Tree,
                            RULE_TREE,
                            x,
                            y,
                            z,
                            cell,
                            0..TREES.len(),
                            0.15,
                        );
                        occupied[i] = true;
                        tree_at[i] = true;
                    } else if ctx.roll(RULE_FLOWER, x, y, z) < FLOWER_P {
                        ctx.place(
                            DecorKind::Flower,
                            RULE_FLOWER,
                            x,
                            y,
                            z,
                            cell,
                            0..FLOWERS.len(),
                            0.2,
                        );
                        occupied[i] = true;
                    } else if ctx.roll(RULE_GRASS_PROP, x, y, z) < GRASS_PROP_P {
                        let n = GRASS_PROPS.len();
                        ctx.place(
                            DecorKind::GrassProp,
                            RULE_GRASS_PROP,
                            x,
                            y,
                            z,
                            cell,
                            0..n,
                            0.2,
                        );
                        occupied[i] = true;
                    }
                }
                CellType::Sand => {
                    let canopy_fits = interior(x, y) && local_flat(&top, x, y, z);
                    if canopy_fits && ctx.roll(RULE_PALM, x, y, z) < PALM_P {
                        ctx.place(
                            DecorKind::Palm,
                            RULE_PALM,
                            x,
                            y,
                            z,
                            cell,
                            0..PALMS.len(),
                            0.15,
                        );
                        occupied[i] = true;
                    }
                }
                _ => {}
            }
        }
    }

    // Pass B: animals on any solid, non-water top cell that is still free.
    for y in 0..CELLS as u8 {
        for x in 0..CELLS as u8 {
            let i = x as usize + CELLS * y as usize;
            if occupied[i] {
                continue;
            }
            let Some((z, cell)) = top(x as i32, y as i32) else {
                continue;
            };
            // An animal on a raised ledge that drops straight into water
            // reads as "floating on the pond" from the isometric camera —
            // keep animals off pond-edge relief (same-level shores are fine).
            let over_water = [(-1i32, 0i32), (1, 0), (0, -1), (0, 1)]
                .iter()
                .any(|&(dx, dy)| {
                    matches!(top(x as i32 + dx, y as i32 + dy),
                    Some((nz, CellType::Water)) if nz < z)
                });
            if over_water {
                continue;
            }
            let (p, group) = match cell {
                CellType::Soil => {
                    let near_tree = chebyshev_2(&tree_at, x, y);
                    let bonus = if near_tree {
                        ANIMAL_NEAR_TREE_BONUS
                    } else {
                        0.0
                    };
                    (ANIMAL_MEADOW_P + bonus, ANIMAL_MEADOW)
                }
                CellType::Sand => (ANIMAL_BEACH_P, ANIMAL_BEACH),
                CellType::Stone | CellType::Gold | CellType::Iron => {
                    (ANIMAL_MOUNTAIN_P, ANIMAL_MOUNTAIN)
                }
                _ => continue,
            };
            if ctx.roll(RULE_ANIMAL, x, y, z) < p {
                ctx.place(DecorKind::Animal, RULE_ANIMAL, x, y, z, cell, group, 0.2);
            }
        }
    }

    out
}

/// Top-most non-air cell of a column, with the layer peel applied (cells at
/// z ≥ cutoff read as air, same as the expansion sees them).
fn column_top(grid: &CellGrid, cutoff: u8, x: u8, y: u8) -> Option<(u8, CellType)> {
    (0..(CELLS as u8).min(cutoff)).rev().find_map(|z| {
        let c = grid.get(x, y, z);
        (c != CellType::Air).then_some((z, c))
    })
}

/// Off the one-cell edge ring — trees and palms have canopies that would
/// overhang the island rim (and the neighbouring biome's border strip).
fn interior(x: u8, y: u8) -> bool {
    let max = CELLS as u8 - 1;
    x > 0 && y > 0 && x < max && y < max
}

/// No 8-neighbour column rises above this one — keeps canopies out of
/// hillsides and naturally gathers trees on flats and crests.
fn local_flat(top: &impl Fn(i32, i32) -> Option<(u8, CellType)>, x: u8, y: u8, z: u8) -> bool {
    for dy in -1..=1i32 {
        for dx in -1..=1i32 {
            if (dx, dy) == (0, 0) {
                continue;
            }
            if let Some((nz, _)) = top(x as i32 + dx, y as i32 + dy) {
                if nz > z {
                    return false;
                }
            }
        }
    }
    true
}

/// Any planned tree within Chebyshev distance 2 of (x, y)?
fn chebyshev_2(tree_at: &[bool; CELLS * CELLS], x: u8, y: u8) -> bool {
    for dy in -2..=2i32 {
        for dx in -2..=2i32 {
            let (nx, ny) = (x as i32 + dx, y as i32 + dy);
            if nx >= 0
                && ny >= 0
                && nx < CELLS as i32
                && ny < CELLS as i32
                && tree_at[nx as usize + CELLS * ny as usize]
            {
                return true;
            }
        }
    }
    false
}

/// Anchor height above the cell's base for a decoration standing on (or
/// swimming in) a top cell — mirrors the `cell_column` visual patterns:
/// grass tops are flush with the cell top, sand sits one voxel sunken, and
/// water fills only the bottom two voxels (fish anchor near the floor so the
/// body stays submerged).
fn anchor_height(cell: CellType) -> f32 {
    match cell {
        CellType::Sand => 1.0 - VOXEL_SIZE,
        CellType::Water => 0.05,
        _ => 1.0,
    }
}

struct Ctx<'a> {
    coord: BiomeCoord,
    seed: u64,
    out: &'a mut Vec<DecorInstance>,
}

impl Ctx<'_> {
    /// Hash in [0,1) for `rule` at world cell coords of (x, y, z).
    fn hash(&self, rule: u64, x: u8, y: u8, z: u8) -> f32 {
        let wx = self.coord.col as i64 * CELLS as i64 + x as i64;
        let wy = self.coord.row as i64 * CELLS as i64 + y as i64;
        rule_hash01(self.seed, rule, wx, wy, z as i64)
    }

    fn roll(&self, rule: u64, x: u8, y: u8, z: u8) -> f32 {
        self.hash(rule, x, y, z)
    }

    /// Emit one instance on the top cell (x, y, z) of type `cell`: variant
    /// from `variants`, lateral jitter within ±`jitter` cells (small enough
    /// to stay on the flat inner 2×2 voxels that the slope/fracture passes
    /// never carve).
    #[allow(clippy::too_many_arguments)]
    fn place(
        &mut self,
        kind: DecorKind,
        rule: u64,
        x: u8,
        y: u8,
        z: u8,
        cell: CellType,
        variants: std::ops::Range<usize>,
        jitter: f32,
    ) {
        let h = |p: u64| self.hash(rule + p, x, y, z);
        let variant =
            variants.start + ((h(1) * variants.len() as f32) as usize).min(variants.len() - 1);
        let jx = (h(3) - 0.5) * 2.0 * jitter;
        let jy = (h(4) - 0.5) * 2.0 * jitter;
        let base_y = z as f32 + anchor_height(cell);
        self.out.push(DecorInstance {
            kind,
            variant,
            // Cell (x, y) center → Bevy (x+0.5, ·, y+0.5): grid-Y is Bevy-Z.
            translation: Vec3::new(x as f32 + 0.5 + jx, base_y, y as f32 + 0.5 + jy),
            yaw: h(2) * TAU,
            scale: DecorKind::models(kind)[variant].scale * (0.85 + 0.3 * h(5)),
            phase: h(6),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use voxel_core::SURFACE_Z;

    fn world_plans(seed: u64) -> Vec<(BiomeCoord, Vec<DecorInstance>)> {
        let map = voxel_mapgen::generate(seed);
        (0..6u8)
            .flat_map(|row| (0..6u8).map(move |col| BiomeCoord::new(row, col)))
            .map(|c| (c, plan_decor(map.biome(c), CELLS as u8, c, seed)))
            .collect()
    }

    #[test]
    fn planning_is_deterministic() {
        let map = voxel_mapgen::generate(42);
        let coord = BiomeCoord::new(2, 3);
        let a = plan_decor(map.biome(coord), 12, coord, 42);
        let b = plan_decor(map.biome(coord), 12, coord, 42);
        assert_eq!(a, b);
        assert_ne!(
            plan_decor(map.biome(coord), 12, coord, 43),
            a,
            "different seed should give a different plan"
        );
    }

    #[test]
    fn placement_rules_hold_on_a_real_world() {
        let map = voxel_mapgen::generate(42);
        for (coord, plan) in world_plans(42) {
            let grid = map.biome(coord);
            for inst in plan {
                // Invert the translation back to the anchor cell.
                let x = inst.translation.x.floor() as i32;
                let y = inst.translation.z.floor() as i32; // Bevy Z = grid Y
                assert!((0..CELLS as i32).contains(&x) && (0..CELLS as i32).contains(&y));
                let (z, cell) = column_top(grid, CELLS as u8, x as u8, y as u8)
                    .expect("decor on an empty column");
                match inst.kind {
                    DecorKind::Tree | DecorKind::Flower | DecorKind::GrassProp => {
                        assert_eq!(cell, CellType::Soil, "{:?} not on soil", inst.kind);
                    }
                    DecorKind::Palm => assert_eq!(cell, CellType::Sand),
                    DecorKind::Fish => {
                        assert_eq!(cell, CellType::Water);
                        // Fully submerged: anchor plus body height (native
                        // max dim 1.88 x final scale) stays below the sunken
                        // water surface at z + 0.5.
                        assert!(inst.translation.y >= z as f32);
                        assert!(inst.translation.y + 1.7 * inst.scale <= z as f32 + 0.52);
                    }
                    DecorKind::Animal => {
                        assert!(cell.is_solid(), "animal on {cell:?}");
                    }
                }
                if matches!(inst.kind, DecorKind::Tree | DecorKind::Palm) {
                    assert!(
                        interior(x as u8, y as u8),
                        "{:?} on the edge ring at ({x},{y})",
                        inst.kind
                    );
                }
                let expected_y = z as f32 + anchor_height(cell);
                assert!(
                    (inst.translation.y - expected_y).abs() < 1e-5,
                    "{:?} anchored at {} expected {}",
                    inst.kind,
                    inst.translation.y,
                    expected_y
                );
            }
        }
    }

    #[test]
    fn every_kind_appears_somewhere_in_the_world() {
        let mut counts = [0usize; DecorKind::ALL.len()];
        for (_, plan) in world_plans(42) {
            for inst in plan {
                counts[inst.kind.index()] += 1;
            }
        }
        for (kind, &n) in DecorKind::ALL.iter().zip(&counts) {
            assert!(n > 0, "no {kind:?} anywhere in seed-42 world");
        }
    }

    #[test]
    fn peeling_replans_on_the_exposed_surface() {
        // With the cutoff below the surface, decor must anchor to the peeled
        // top, never above the cutoff.
        let map = voxel_mapgen::generate(42);
        let coord = BiomeCoord::new(0, 0);
        let cutoff = SURFACE_Z; // peel down into the underground
        for inst in plan_decor(map.biome(coord), cutoff, coord, 42) {
            assert!(inst.translation.y <= cutoff as f32 + 1.0);
        }
    }

    #[test]
    fn variants_stay_in_catalog_bounds() {
        for (_, plan) in world_plans(42) {
            for inst in plan {
                assert!(inst.variant < inst.kind.models().len());
                if inst.kind == DecorKind::Animal {
                    assert!(
                        ANIMAL_MEADOW.contains(&inst.variant)
                            || ANIMAL_BEACH.contains(&inst.variant)
                            || ANIMAL_MOUNTAIN.contains(&inst.variant)
                    );
                }
            }
        }
    }
}
