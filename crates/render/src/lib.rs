mod camera;
mod mesh;

pub use camera::{CameraState, EguiWantsPointer, InspectorCamera};
pub use mesh::{build_voxel_meshes, proxy_material_color};

use bevy::prelude::*;
use voxel_core::{BiomeCoord, BiomeType};

// ── Resources owned by this crate ────────────────────────────────────────────

#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub struct FocusedBiome(pub BiomeCoord);

#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub struct LayerCutoff(pub u8);

/// Tracks live mesh entities so rebuild_scene can despawn/respawn them.
#[derive(Resource, Default)]
pub struct SceneEntities {
    pub biome_meshes: Vec<Entity>,
    pub proxy_tiles:  Vec<Entity>,
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app
            .insert_resource(FocusedBiome(BiomeCoord::new(0, 0)))
            .insert_resource(LayerCutoff(12))
            .insert_resource(EguiWantsPointer::default())
            .insert_resource(SceneEntities::default())
            .add_systems(Startup, camera::spawn_camera)
            .add_systems(Update, (
                camera::camera_input,
                camera::update_camera,
            ).chain());
    }
}

// ── Shared material builders ──────────────────────────────────────────────────

pub fn proxy_material(bt: BiomeType) -> StandardMaterial {
    let (r, g, b) = match bt {
        BiomeType::Grass => (0.08, 0.23, 0.08),
        BiomeType::Sand  => (0.30, 0.28, 0.18),
        BiomeType::Water => (0.05, 0.13, 0.30),
        BiomeType::Rock  => (0.18, 0.18, 0.18),
    };
    StandardMaterial {
        base_color: Color::srgb(r, g, b),
        unlit: true,
        double_sided: true,
        cull_mode: None,
        ..default()
    }
}

pub fn proxy_world_offset(coord: BiomeCoord, focused: BiomeCoord) -> Vec3 {
    let spacing = 14.0_f32;
    Vec3::new(
        (coord.col as f32 - focused.col as f32) * spacing + 6.0,
        -2.0,
        (coord.row as f32 - focused.row as f32) * spacing + 6.0,
    )
}
