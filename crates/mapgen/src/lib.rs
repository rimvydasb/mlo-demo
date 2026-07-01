mod ascii;
mod interior;
mod macro_pass;

use std::collections::BTreeMap;
use voxel_core::{BiomeCoord, BiomeType, Connection, EdgeDir, Seed, VoxelGrid};

pub use ascii::{ascii_dump, ascii_macro};

pub struct WorldMap {
    pub biome_types: [[BiomeType; 6]; 6],
    biomes: Vec<VoxelGrid>,
    pub connections: BTreeMap<(BiomeCoord, EdgeDir), Connection>,
}

impl WorldMap {
    pub fn biome(&self, coord: BiomeCoord) -> &VoxelGrid {
        &self.biomes[coord.row as usize * 6 + coord.col as usize]
    }

    pub fn biome_type(&self, coord: BiomeCoord) -> BiomeType {
        self.biome_types[coord.row as usize][coord.col as usize]
    }

    pub fn connection(&self, coord: BiomeCoord, dir: EdgeDir) -> Option<Connection> {
        self.connections.get(&(coord, dir)).copied()
    }
}

pub fn generate(seed: Seed) -> WorldMap {
    use rand::SeedableRng;
    use rand_chacha::ChaCha8Rng;

    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    let biome_types = macro_pass::assign_biome_types(&mut rng);
    let connections = macro_pass::compute_connections(&biome_types);

    let mut biomes = Vec::with_capacity(36);
    for row in 0..6u8 {
        for col in 0..6u8 {
            let coord = BiomeCoord::new(row, col);
            let bt = biome_types[row as usize][col as usize];
            let north = row.checked_sub(1).map(|r| biome_types[r as usize][col as usize]);
            let south = if row < 5 { Some(biome_types[row as usize + 1][col as usize]) } else { None };
            let west  = col.checked_sub(1).map(|c| biome_types[row as usize][c as usize]);
            let east  = if col < 5 { Some(biome_types[row as usize][col as usize + 1]) } else { None };
            biomes.push(interior::generate_biome(seed, coord, bt, north, south, west, east));
        }
    }

    WorldMap { biome_types, biomes, connections }
}
