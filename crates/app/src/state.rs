use bevy::prelude::*;
use voxel_core::{BiomeCoord, CellType, CELLS, SURFACE_Z, WORLD_BIOMES};

#[derive(Resource)]
pub struct WorldMapResource(pub voxel_mapgen::WorldMap);

/// The biome the inspector boots focused on: the most scenic one, scored by
/// relief drama (snow-capped peaks weigh the most) plus a modest pond bonus.
/// Deterministic per seed, so screenshots stay stable.
pub fn scenic_biome(map: &voxel_mapgen::WorldMap) -> BiomeCoord {
    let mut best = (BiomeCoord::new(0, 0), i32::MIN);
    for row in 0..WORLD_BIOMES as u8 {
        for col in 0..WORLD_BIOMES as u8 {
            let coord = BiomeCoord::new(row, col);
            let grid = map.biome(coord);

            let mut max_relief = 0i32;
            let mut pond_cells = 0i32;
            for y in 0..CELLS as u8 {
                for x in 0..CELLS as u8 {
                    let h = (SURFACE_Z + 1..CELLS as u8)
                        .take_while(|&z| grid.get(x, y, z) != CellType::Air)
                        .count() as i32;
                    max_relief = max_relief.max(h);
                    if grid.get(x, y, SURFACE_Z) == CellType::Water {
                        pond_cells += 1;
                    }
                }
            }

            let score = max_relief * 10 + pond_cells.min(14);
            if score > best.1 {
                best = (coord, score);
            }
        }
    }
    best.0
}

#[derive(Resource)]
pub struct InspectorState {
    pub seed: u64,
    pub seed_input: String,
    /// Cell coords (x, y, z) in the focused biome under the cursor.
    pub hovered_cell: Option<(u8, u8, u8)>,
    pub export_ascii: bool,
    pub export_png: bool,
}

impl InspectorState {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            seed_input: seed.to_string(),
            hovered_cell: None,
            export_ascii: false,
            export_png: false,
        }
    }
}
