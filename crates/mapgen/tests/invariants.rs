use voxel_core::{BiomeCoord, EdgeDir, Material};
use voxel_mapgen::generate;

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
    // At least one biome should differ
    let any_diff = (0..6u8).flat_map(|r| (0..6u8).map(move |c| BiomeCoord::new(r, c)))
        .any(|coord| a.biome(coord) != b.biome(coord));
    assert!(any_diff, "seeds 0 and 1 produced identical maps");
}

// ── Invariant helpers ─────────────────────────────────────────────────────────

fn check_biome_invariants(map: &voxel_mapgen::WorldMap, coord: BiomeCoord) {
    let grid = map.biome(coord);
    let label = format!("biome ({},{})", coord.row, coord.col);

    for vy in 0u8..12 {
        for vx in 0u8..12 {
            // Underground z=0..4: must be Underground material, may have element
            for vz in 0u8..5 {
                let v = grid.get(vx, vy, vz);
                assert_eq!(
                    v.material, Material::Underground,
                    "{label} ({vx},{vy},{vz}) underground must be Underground"
                );
            }

            // Surface z=5: must be Ground(…), no element
            {
                let v = grid.get(vx, vy, 5);
                assert!(
                    matches!(v.material, Material::Ground(_)),
                    "{label} ({vx},{vy},5) surface must be Ground"
                );
                assert_eq!(v.element, None, "{label} ({vx},{vy},5) surface must have no element");
            }

            // Above-ground z=6..11: must NOT have elements; must be Air or Ground
            for vz in 6u8..12 {
                let v = grid.get(vx, vy, vz);
                assert_eq!(
                    v.element, None,
                    "{label} ({vx},{vy},{vz}) above-ground must have no element"
                );
                assert!(
                    matches!(v.material, Material::Air | Material::Ground(_)),
                    "{label} ({vx},{vy},{vz}) above-ground must be Air or Ground"
                );
            }

            // No floating voxels: solid at z => solid at z-1 (down to z=6)
            for vz in 7u8..12 {
                let above = grid.get(vx, vy, vz);
                let below = grid.get(vx, vy, vz - 1);
                if above.material != Material::Air {
                    assert_ne!(
                        below.material, Material::Air,
                        "{label} ({vx},{vy},{vz}) floating voxel: solid above Air"
                    );
                }
            }
        }
    }
}

fn check_edge_continuity(map: &voxel_mapgen::WorldMap) {
    // For each compatible east-west border: layer-6 strip must match material
    for row in 0..6u8 {
        for col in 0..5u8 {
            let left  = BiomeCoord::new(row, col);
            let right = BiomeCoord::new(row, col + 1);
            let conn  = map.connection(left, EdgeDir::East);
            if !conn.map(|c| c.compatible).unwrap_or(false) { continue; }

            let lg = map.biome(left);
            let rg = map.biome(right);
            for vy in 0u8..12 {
                let lv = lg.get(11, vy, 5); // east edge of left biome
                let rv = rg.get(0,  vy, 5); // west edge of right biome
                assert_eq!(
                    lv.material, rv.material,
                    "edge continuity fail H ({row},{col})|({row},{}) vy={vy}",
                    col + 1
                );
            }
        }
    }

    // For each compatible north-south border
    for row in 0..5u8 {
        for col in 0..6u8 {
            let top    = BiomeCoord::new(row, col);
            let bottom = BiomeCoord::new(row + 1, col);
            let conn   = map.connection(top, EdgeDir::South);
            if !conn.map(|c| c.compatible).unwrap_or(false) { continue; }

            let tg = map.biome(top);
            let bg = map.biome(bottom);
            for vx in 0u8..12 {
                let tv = tg.get(vx, 11, 5); // south edge of top biome
                let bv = bg.get(vx, 0,  5); // north edge of bottom biome
                assert_eq!(
                    tv.material, bv.material,
                    "edge continuity fail V ({row},{col})|({},{col}) vx={vx}",
                    row + 1
                );
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
