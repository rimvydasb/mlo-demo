//! Procedural world generation, cell tier only.
//!
//! `generate(seed)` runs two passes:
//! 1. **Macro pass** — assigns a `BiomeType` to each of the 36 biomes and
//!    computes edge `Connection`s (compatible = same type on both sides).
//! 2. **Interior pass** — fills each biome's 12³ `CellGrid` from world-space
//!    noise so fields continue seamlessly across borders.
//!
//! No Bevy, no voxels: the cosmetic cell → voxel expansion lives in `render`.

mod ascii;
mod interior;
mod macro_pass;

use std::collections::BTreeMap;
use voxel_core::{BiomeCoord, BiomeType, CellGrid, Connection, EdgeDir, Seed, WORLD_BIOMES};

pub use ascii::{ascii_dump, ascii_macro};

pub struct WorldMap {
    pub biome_types: [[BiomeType; WORLD_BIOMES]; WORLD_BIOMES],
    biomes: Vec<CellGrid>,
    pub connections: BTreeMap<(BiomeCoord, EdgeDir), Connection>,
}

impl WorldMap {
    pub fn biome(&self, coord: BiomeCoord) -> &CellGrid {
        &self.biomes[coord.row as usize * WORLD_BIOMES + coord.col as usize]
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

    // The RNG feeds the macro pass only; the interior pass uses seed-derived
    // noise in world coordinates, so biome order can never affect output.
    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    let biome_types = macro_pass::assign_biome_types(&mut rng);
    let connections = macro_pass::compute_connections(&biome_types);

    let mut biomes = Vec::with_capacity(WORLD_BIOMES * WORLD_BIOMES);
    for row in 0..WORLD_BIOMES as u8 {
        for col in 0..WORLD_BIOMES as u8 {
            let coord = BiomeCoord::new(row, col);
            let bt = biome_types[row as usize][col as usize];
            biomes.push(interior::generate_biome(seed, coord, bt));
        }
    }

    WorldMap {
        biome_types,
        biomes,
        connections,
    }
}
