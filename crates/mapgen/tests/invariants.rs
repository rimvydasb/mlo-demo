//! Cell-tier invariants for `mapgen::generate`. Runs headless — no GPU.

use voxel_core::{BiomeCoord, BiomeType, CellType, EdgeDir, CELLS, SURFACE_Z};
use voxel_mapgen::generate;

const MAX: u8 = CELLS as u8;
const EDGE: u8 = MAX - 1;

// ── Determinism ──────────────────────────────────────────────────────────────

#[test]
fn same_seed_deterministic() {
    let a = generate(42);
    let b = generate(42);
    for row in 0..6u8 {
        for col in 0..6u8 {
            let coord = BiomeCoord::new(row, col);
            assert_eq!(
                a.biome(coord),
                b.biome(coord),
                "biome ({row},{col}) differs between runs with seed 42"
            );
        }
    }
}

#[test]
fn different_seeds_differ() {
    let a = generate(0);
    let b = generate(1);
    let any_diff = (0..6u8)
        .flat_map(|r| (0..6u8).map(move |c| BiomeCoord::new(r, c)))
        .any(|coord| a.biome(coord) != b.biome(coord));
    assert!(any_diff, "seeds 0 and 1 produced identical maps");
}

// ── Invariant helpers ─────────────────────────────────────────────────────────

fn check_biome_invariants(map: &voxel_mapgen::WorldMap, coord: BiomeCoord) {
    let grid = map.biome(coord);
    let bt = map.biome_type(coord);
    let label = format!("biome ({},{})", coord.row, coord.col);

    for y in 0..MAX {
        for x in 0..MAX {
            let is_edge = x == 0 || x == EDGE || y == 0 || y == EDGE;

            // Underground (z = 0..5): funnel of deposits hanging from the
            // surface. Gold/iron never appear elsewhere.
            for z in 0..SURFACE_Z {
                let c = grid.get(x, y, z);
                assert!(
                    matches!(
                        c,
                        CellType::Air
                            | CellType::Soil
                            | CellType::Stone
                            | CellType::Gold
                            | CellType::Iron
                    ),
                    "{label} ({x},{y},{z}) unexpected underground cell {c:?}"
                );
                // Hangs from above: solid here ⇒ solid one layer up.
                if c != CellType::Air {
                    assert_ne!(
                        grid.get(x, y, z + 1),
                        CellType::Air,
                        "{label} ({x},{y},{z}) underground cell has no support above"
                    );
                }
            }

            // Surface (z = 5): always solid, typed by the biome; ponds only
            // in grass/sand interiors, and the edge ring is always pure.
            {
                let c = grid.get(x, y, SURFACE_Z);
                assert_ne!(
                    c,
                    CellType::Air,
                    "{label} ({x},{y},5) surface must be solid"
                );
                if is_edge {
                    assert_eq!(
                        c,
                        bt.surface_cell(),
                        "{label} ({x},{y},5) edge ring must match biome surface type"
                    );
                } else {
                    let pond_ok =
                        c == CellType::Water && matches!(bt, BiomeType::Grass | BiomeType::Sand);
                    // Beach pass: grass-biome interiors may flip soil → sand
                    // near large ponds.
                    let beach_ok = c == CellType::Sand && bt == BiomeType::Grass;
                    assert!(
                        c == bt.surface_cell() || pond_ok || beach_ok,
                        "{label} ({x},{y},5) unexpected surface cell {c:?}"
                    );
                }
            }

            // Relief (z = 6..12): soil/sand/stone columns, no resources, no
            // floating cells, flat on the edge ring and over water.
            for z in SURFACE_Z + 1..MAX {
                let c = grid.get(x, y, z);
                assert!(
                    matches!(
                        c,
                        CellType::Air | CellType::Soil | CellType::Sand | CellType::Stone
                    ),
                    "{label} ({x},{y},{z}) unexpected relief cell {c:?}"
                );
                if c != CellType::Air {
                    assert!(!is_edge, "{label} ({x},{y},{z}) relief on the edge ring");
                    assert_ne!(
                        grid.get(x, y, SURFACE_Z),
                        CellType::Water,
                        "{label} ({x},{y},{z}) relief above a water surface"
                    );
                    assert_ne!(
                        grid.get(x, y, z - 1),
                        CellType::Air,
                        "{label} ({x},{y},{z}) floating relief cell"
                    );
                }
            }
        }
    }
}

fn check_edge_continuity(map: &voxel_mapgen::WorldMap) {
    // Compatible east-west borders: the layer-6 strips must match cell-for-cell.
    for row in 0..6u8 {
        for col in 0..5u8 {
            let left = BiomeCoord::new(row, col);
            let right = BiomeCoord::new(row, col + 1);
            let conn = map.connection(left, EdgeDir::East);
            if !conn.map(|c| c.compatible).unwrap_or(false) {
                continue;
            }

            let lg = map.biome(left);
            let rg = map.biome(right);
            for y in 0..MAX {
                assert_eq!(
                    lg.get(EDGE, y, SURFACE_Z),
                    rg.get(0, y, SURFACE_Z),
                    "edge continuity fail H ({row},{col})|({row},{}) y={y}",
                    col + 1
                );
            }
        }
    }

    // Compatible north-south borders.
    for row in 0..5u8 {
        for col in 0..6u8 {
            let top = BiomeCoord::new(row, col);
            let bottom = BiomeCoord::new(row + 1, col);
            let conn = map.connection(top, EdgeDir::South);
            if !conn.map(|c| c.compatible).unwrap_or(false) {
                continue;
            }

            let tg = map.biome(top);
            let bg = map.biome(bottom);
            for x in 0..MAX {
                assert_eq!(
                    tg.get(x, EDGE, SURFACE_Z),
                    bg.get(x, 0, SURFACE_Z),
                    "edge continuity fail V ({row},{col})|({},{col}) x={x}",
                    row + 1
                );
            }
        }
    }
}

fn check_resource_rarity(map: &voxel_mapgen::WorldMap) {
    // Spec: of solid underground cells — stone ≥ 30%, iron ~10%, gold ~5%.
    // Bounds are loose: a single map is a small sample.
    let mut solid = 0u32;
    let mut stone = 0u32;
    let mut iron = 0u32;
    let mut gold = 0u32;
    for row in 0..6u8 {
        for col in 0..6u8 {
            let grid = map.biome(BiomeCoord::new(row, col));
            for z in 0..SURFACE_Z {
                for y in 0..MAX {
                    for x in 0..MAX {
                        match grid.get(x, y, z) {
                            CellType::Air => {}
                            c => {
                                solid += 1;
                                match c {
                                    CellType::Stone => stone += 1,
                                    CellType::Iron => iron += 1,
                                    CellType::Gold => gold += 1,
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    let pct = |n: u32| n as f64 / solid as f64 * 100.0;
    assert!(pct(stone) >= 30.0, "stone {:.1}% < 30%", pct(stone));
    assert!(
        (4.0..=20.0).contains(&pct(iron)),
        "iron {:.1}% out of range",
        pct(iron)
    );
    assert!(
        (1.5..=12.0).contains(&pct(gold)),
        "gold {:.1}% out of range",
        pct(gold)
    );
}

/// Beach cells hug water: every sand surface cell in a grass biome sits
/// within two 4-connected steps of surface water, never on the edge ring
/// (the edge-ring assertion in `check_biome_invariants` covers the latter).
fn check_beach_belt(map: &voxel_mapgen::WorldMap) {
    for row in 0..6u8 {
        for col in 0..6u8 {
            let coord = BiomeCoord::new(row, col);
            if map.biome_type(coord) != BiomeType::Grass {
                continue;
            }
            let grid = map.biome(coord);
            for y in 0..MAX {
                for x in 0..MAX {
                    if grid.get(x, y, SURFACE_Z) != CellType::Sand {
                        continue;
                    }
                    let near_water = (-2i32..=2).any(|dy| {
                        (-2i32..=2).any(|dx| {
                            (dx.abs() + dy.abs()) <= 2
                                && grid.get_or_air(x as i32 + dx, y as i32 + dy, SURFACE_Z as i32)
                                    == CellType::Water
                        })
                    });
                    assert!(
                        near_water,
                        "beach cell ({row},{col})/({x},{y}) has no water within 2 steps"
                    );
                }
            }
        }
    }
}

// ── Deterministic tests ───────────────────────────────────────────────────────

#[test]
fn biome_invariants_seed42() {
    let map = generate(42);
    for row in 0..6u8 {
        for col in 0..6u8 {
            check_biome_invariants(&map, BiomeCoord::new(row, col));
        }
    }
}

#[test]
fn edge_continuity_seed42() {
    check_edge_continuity(&generate(42));
}

#[test]
fn edge_continuity_seed0() {
    check_edge_continuity(&generate(0));
}

#[test]
fn resource_rarity_seed42() {
    check_resource_rarity(&generate(42));
}

#[test]
fn beach_belt_seed42() {
    check_beach_belt(&generate(42));
}

// ── Property tests (proptest) ─────────────────────────────────────────────────

use proptest::prelude::*;

proptest! {
    #![proptest_config(proptest::test_runner::Config {
        cases: 32,
        ..Default::default()
    })]

    #[test]
    fn prop_biome_invariants(seed: u64) {
        let map = generate(seed);
        for row in 0..6u8 {
            for col in 0..6u8 {
                check_biome_invariants(&map, BiomeCoord::new(row, col));
            }
        }
    }

    #[test]
    fn prop_edge_continuity(seed: u64) {
        check_edge_continuity(&generate(seed));
    }

    #[test]
    fn prop_beach_belt(seed: u64) {
        check_beach_belt(&generate(seed));
    }

    #[test]
    fn prop_determinism(seed: u64) {
        let a = generate(seed);
        let b = generate(seed);
        for row in 0..6u8 {
            for col in 0..6u8 {
                let coord = BiomeCoord::new(row, col);
                prop_assert_eq!(a.biome(coord), b.biome(coord));
            }
        }
    }
}

// ── Snapshot tests ────────────────────────────────────────────────────────────

#[test]
fn snapshot_ascii_biome() {
    let map = generate(42);
    insta::assert_snapshot!(
        "ascii_biome_seed42_0_0",
        voxel_mapgen::ascii_dump(&map, BiomeCoord::new(0, 0))
    );
}

#[test]
fn snapshot_ascii_macro() {
    let map = generate(42);
    insta::assert_snapshot!("ascii_macro_seed42", voxel_mapgen::ascii_macro(&map));
}
