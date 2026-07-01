use bevy::prelude::*;

#[derive(Resource)]
pub struct WorldMapResource(pub voxel_mapgen::WorldMap);

#[derive(Resource)]
pub struct InspectorState {
    pub seed: u64,
    pub seed_input: String,
    pub hovered_voxel: Option<(u8, u8, u8)>,
    pub export_ascii: bool,
    pub export_png:   bool,
}

impl InspectorState {
    pub fn new(seed: u64) -> Self {
        Self {
            seed,
            seed_input: seed.to_string(),
            hovered_voxel: None,
            export_ascii: false,
            export_png:   false,
        }
    }
}
