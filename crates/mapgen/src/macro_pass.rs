use rand::Rng;
use rand_chacha::ChaCha8Rng;
use std::collections::BTreeMap;
use voxel_core::{BiomeCoord, BiomeType, Connection, EdgeDir};

const BIOME_VARIANTS: [BiomeType; 4] = [
    BiomeType::Grass,
    BiomeType::Sand,
    BiomeType::Water,
    BiomeType::Rock,
];
const BIOME_WEIGHTS: [u32; 4] = [40, 25, 20, 15];

pub fn assign_biome_types(rng: &mut ChaCha8Rng) -> [[BiomeType; 6]; 6] {
    let total: u32 = BIOME_WEIGHTS.iter().sum();
    let mut grid = [[BiomeType::Grass; 6]; 6];
    for row in grid.iter_mut() {
        for cell in row.iter_mut() {
            let pick: u32 = rng.gen_range(0..total);
            let mut cum = 0u32;
            for (bt, &w) in BIOME_VARIANTS.iter().zip(BIOME_WEIGHTS.iter()) {
                cum += w;
                if pick < cum {
                    *cell = *bt;
                    break;
                }
            }
        }
    }
    grid
}

pub fn compute_connections(
    types: &[[BiomeType; 6]; 6],
) -> BTreeMap<(BiomeCoord, EdgeDir), Connection> {
    let mut map = BTreeMap::new();

    for row in 0..6usize {
        for col in 0..6usize {
            let coord = BiomeCoord::new(row as u8, col as u8);
            let my_type = types[row][col];

            // East neighbour
            if col + 1 < 6 {
                let nb = types[row][col + 1];
                let compatible = my_type == nb;
                map.insert(
                    (coord, EdgeDir::East),
                    Connection {
                        compatible,
                        surface: my_type,
                    },
                );
                map.insert(
                    (BiomeCoord::new(row as u8, col as u8 + 1), EdgeDir::West),
                    Connection {
                        compatible,
                        surface: nb,
                    },
                );
            }

            // South neighbour
            if row + 1 < 6 {
                let nb = types[row + 1][col];
                let compatible = my_type == nb;
                map.insert(
                    (coord, EdgeDir::South),
                    Connection {
                        compatible,
                        surface: my_type,
                    },
                );
                map.insert(
                    (BiomeCoord::new(row as u8 + 1, col as u8), EdgeDir::North),
                    Connection {
                        compatible,
                        surface: nb,
                    },
                );
            }
        }
    }

    map
}
