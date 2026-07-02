//! Bevy rendering for the biome inspector: cell→voxel expansion, meshing,
//! camera, lighting, and the dimmed proxy tiles for unfocused biomes.

mod beautify;
mod camera;
mod expansion;
mod mesh;

pub use beautify::{rule_hash01, BeautifyOptions};
pub use camera::{CameraState, EguiWantsPointer, InspectorCamera};
pub use expansion::{cell_column, expand, voxel_hash01, VoxelKind, VoxelVolume, SUB, VOX};
pub use mesh::{base_color_linear, base_color_srgb, build_biome_meshes, BiomeMeshes, VOXEL_SIZE};

use bevy::light::CascadeShadowConfigBuilder;
use bevy::pbr::{DistanceFog, FogFalloff};
use bevy::prelude::*;
use voxel_core::{BiomeCoord, BiomeType, CELLS, SURFACE_Z};

/// Soft sky backdrop; fog fades into the same color so distant proxies
/// dissolve into the sky.
pub const SKY_COLOR: Color = Color::srgb(0.64, 0.75, 0.86);

// ── Resources owned by this crate ────────────────────────────────────────────

#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub struct FocusedBiome(pub BiomeCoord);

/// Show cell layers with z < cutoff (1..=12). 12 = everything.
#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub struct LayerCutoff(pub u8);

/// Tracks live mesh entities so rebuild_scene can despawn/respawn them.
#[derive(Resource, Default)]
pub struct SceneEntities {
    pub biome_meshes: Vec<Entity>,
    pub proxy_tiles: Vec<Entity>,
}

// ── Plugin ────────────────────────────────────────────────────────────────────

pub struct RenderPlugin;

impl Plugin for RenderPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(FocusedBiome(BiomeCoord::new(0, 0)))
            .insert_resource(LayerCutoff(CELLS as u8))
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
        Transform::from_xyz(30.0, 22.0, 2.0).looking_at(Vec3::new(6.0, 6.0, 6.0), Vec3::Y),
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
        Transform::from_xyz(-6.0, 16.0, 24.0).looking_at(Vec3::new(6.0, 5.0, 6.0), Vec3::Y),
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
    // The camera orbits at a fixed 60-unit distance; the focused biome spans
    // roughly ±12 units of view depth around that. Fog starts just past it
    // so only the proxy ring fades toward the sky.
    DistanceFog {
        color: SKY_COLOR,
        falloff: FogFalloff::Linear {
            start: 74.0,
            end: 150.0,
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

// ── Proxy tiles (the 35 unfocused biomes) ────────────────────────────────────

/// Dimmed, lit slab standing in for an unfocused biome. Lit (not unlit) so
/// distance fog applies and the archipelago sits in the same atmosphere.
pub fn proxy_material(bt: BiomeType) -> StandardMaterial {
    let (r, g, b) = match bt {
        BiomeType::Grass => (0.22, 0.38, 0.18),
        BiomeType::Sand => (0.52, 0.46, 0.30),
        BiomeType::Water => (0.15, 0.30, 0.48),
        BiomeType::Rock => (0.33, 0.33, 0.36),
    };
    StandardMaterial {
        base_color: Color::srgb(r, g, b),
        perceptual_roughness: 1.0,
        reflectance: 0.05,
        ..default()
    }
}

/// Proxy slab center position. Proxies float at the focused biome's surface
/// altitude so the world reads as an archipelago of floating islands.
pub fn proxy_world_offset(coord: BiomeCoord, focused: BiomeCoord) -> Vec3 {
    let spacing = 15.0_f32;
    let surface_y = SURFACE_Z as f32 + 0.75;
    Vec3::new(
        (coord.col as f32 - focused.col as f32) * spacing + CELLS as f32 / 2.0,
        surface_y,
        (coord.row as f32 - focused.row as f32) * spacing + CELLS as f32 / 2.0,
    )
}
