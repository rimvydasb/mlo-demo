//! Bevy rendering for the biome inspector: cell→voxel expansion, meshing,
//! camera, lighting, and the dimmed proxy tiles for unfocused biomes.

mod beautify;
mod camera;
pub mod decor;
mod expansion;
mod mesh;

pub use beautify::{rule_hash01, BeautifyOptions};
pub use camera::{CameraState, EguiWantsPointer, InspectorCamera};
pub use decor::{plan_decor, DecorInstance, DecorKind, DecorModel};
pub use expansion::{
    cell_column, expand, voxel_hash01, VoxelKind, VoxelVolume, SUB, VOX_XY, VOX_Z,
};
pub use mesh::{
    base_color_linear, base_color_srgb, build_biome_meshes, build_cell_shell_mesh, BiomeMeshes,
    VOXEL_SIZE,
};

use bevy::light::CascadeShadowConfigBuilder;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use voxel_core::{BiomeCoord, CELLS_XY, CELLS_Z};

/// Soft sky backdrop; fog fades into the same color so distant proxies
/// dissolve into the sky.
pub const SKY_COLOR: Color = Color::srgb(0.64, 0.75, 0.86);

// ── Resources owned by this crate ────────────────────────────────────────────

#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub struct FocusedBiome(pub BiomeCoord);

/// Show cell layers with z < cutoff (1..=CELLS_Z). CELLS_Z = everything.
#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub struct LayerCutoff(pub u8);

/// Tracks live mesh entities so rebuild_scene can despawn/respawn them.
#[derive(Resource, Default)]
pub struct SceneEntities {
    pub biome_meshes: Vec<Entity>,
    pub proxy_tiles: Vec<Entity>,
    /// Fauna/flora prop roots on the focused biome (`voxel-app::decor`).
    pub decorations: Vec<Entity>,
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(FocusedBiome(BiomeCoord::new(0, 0)))
            .insert_resource(LayerCutoff(CELLS_Z as u8))
            .init_resource::<BeautifyOptions>()
            .insert_resource(EguiWantsPointer::default())
            .insert_resource(SceneEntities::default())
            .insert_resource(ClearColor(SKY_COLOR))
            .add_systems(Startup, (camera::spawn_camera, spawn_lights))
            .add_systems(
                Update,
                (camera::camera_input, camera::update_camera).chain(),
            );
    }
}

fn spawn_lights(mut commands: Commands) {
    // Key light: warm sun with soft-edged shadow maps — this is what makes
    // the diorama read as a physical object.
    commands.spawn((
        DirectionalLight {
            illuminance: 9_500.0,
            color: Color::srgb(1.0, 0.97, 0.90),
            shadow_maps_enabled: true,
            ..default()
        },
        // Low-ish lateral sun so relief casts readable shadows across the
        // surface instead of hiding them under the geometry.
        Transform::from_xyz(30.0, 22.0, 2.0).looking_at(Vec3::new(4.0, 6.0, 4.0), Vec3::Y),
        // One tight cascade: crisp shadows on the focused biome, and the
        // island's shadow never lands on far-away proxy tiles.
        CascadeShadowConfigBuilder {
            num_cascades: 1,
            maximum_distance: 90.0,
            ..default()
        }
        .build(),
    ));

    // Cool fill from the default camera side, no shadows: the sun lights the
    // east faces, this lifts the south/west faces the viewer actually sees
    // so they stay readable instead of crushing to black.
    commands.spawn((
        DirectionalLight {
            illuminance: 4_200.0,
            color: Color::srgb(0.75, 0.83, 1.0),
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-6.0, 16.0, 24.0).looking_at(Vec3::new(4.0, 5.0, 4.0), Vec3::Y),
    ));

    // AmbientLight is a component (not a Resource) in Bevy 0.19.
    commands.spawn(AmbientLight {
        color: Color::srgb(0.80, 0.87, 1.0),
        brightness: 750.0,
        ..default()
    });
}

/// Fog component for the inspector camera (attached in spawn_camera).
pub fn camera_fog() -> DistanceFog {
    // The perspective camera orbits ~22 units out; the greyed-out neighbour
    // biomes sit 9 units apart, so fog starts past the first ring and only
    // dissolves the far corners of the world into the sky. Kept gentle on
    // purpose: the fog-of-war shells must stay readable — memorizing biome
    // silhouettes is the point.
    DistanceFog {
        color: SKY_COLOR,
        falloff: FogFalloff::Linear {
            start: 45.0,
            end: 140.0,
        },
        ..default()
    }
}

// ── Materials ─────────────────────────────────────────────────────────────────

/// Single material for all opaque terrain; per-voxel color and AO live in
/// the mesh's vertex colors.
pub fn terrain_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::WHITE,
        perceptual_roughness: 0.95,
        metallic: 0.0,
        reflectance: 0.12,
        ..default()
    }
}

pub fn water_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, 0.72),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 0.15,
        metallic: 0.0,
        reflectance: 0.35,
        ..default()
    }
}

// ── Fog-of-war shells (the 35 unfocused biomes) ──────────────────────────────

/// Uniform grey for the fog-of-war cell shells of unfocused biomes. One
/// material for every biome type on purpose: unfocused biomes are known by
/// silhouette only. Lit (not unlit) so faces shade and distance fog applies.
pub fn proxy_material() -> StandardMaterial {
    StandardMaterial {
        base_color: Color::srgb(0.42, 0.44, 0.47),
        perceptual_roughness: 1.0,
        reflectance: 0.04,
        ..default()
    }
}

/// World translation of a biome's cell (0,0,0) corner relative to the
/// focused biome (which sits at the origin). Biomes are separated by exactly
/// one cell of air: spacing = footprint + 1.
pub fn proxy_world_offset(coord: BiomeCoord, focused: BiomeCoord) -> Vec3 {
    let spacing = (CELLS_XY + 1) as f32;
    Vec3::new(
        (coord.col as f32 - focused.col as f32) * spacing,
        0.0,
        (coord.row as f32 - focused.row as f32) * spacing,
    )
}
