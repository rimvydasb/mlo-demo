//! Drifting voxel clouds over the focused biome.
//!
//! This is a scene system, not part of the deterministic expansion: clouds
//! are animated, stateful across frames, and live in world space rather than
//! cells. Non-determinism is intentional — clouds are the one thing in the
//! renderer that *shouldn't* match seed-for-seed across runs. Screenshots
//! that need byte-stable output disable them via `--no-clouds`.
//!
//! Each cloud is a small cluster of white cubes (per the reference art —
//! clouds keep the voxel language) sharing one translucent material, so a
//! whole cloud fades as a unit. Clouds drift east; past the biome's far edge
//! they fade out slowly (a smoothstep over `FADE_SECS`) and a replacement
//! fades in at the near edge the same way — no pops, no flicker.

use bevy::light::NotShadowCaster;
use bevy::prelude::*;
use rand::rngs::ThreadRng;
use rand::Rng;

/// Off = spawn no clouds at all (deterministic screenshots).
#[derive(Resource, Clone, Copy)]
pub struct CloudsEnabled(pub bool);

pub struct CloudsPlugin;

impl Plugin for CloudsPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, setup_clouds)
            .add_systems(Update, drift_clouds);
    }
}

const CLOUD_COUNT: usize = 6;
/// Just above the terrain (tops out at 12) but below the default camera eye
/// (~17 units up) — a cloud band at eye height fills the frame with cubes.
const ALTITUDE: std::ops::Range<f32> = 12.5..14.5;
/// North-south lane across the biome (world z; the biome spans 0..8).
const LANE: std::ops::Range<f32> = -1.0..9.0;
const SPAWN_X: f32 = -6.0;
/// Fade out once the cloud center clears the biome's far (east) edge.
const FADE_OUT_X: f32 = 11.0;
/// Slow, soft crossfade — short fades read as flicker against the sky.
const FADE_SECS: f32 = 4.0;
/// Slightly translucent, not glassy.
const CLOUD_ALPHA: f32 = 0.9;
/// Cube edge lengths snap to the voxel size so clouds share the terrain's
/// chunky language.
const SNAP: f32 = 0.25;

#[derive(Component)]
struct Cloud {
    speed: f32,
    material: Handle<StandardMaterial>,
    fade: Fade,
}

enum Fade {
    In(f32),
    Full,
    Out(f32),
}

/// Shared unit-cube mesh; every puff is a scaled instance of it.
#[derive(Resource)]
struct CloudAssets {
    cube: Handle<Mesh>,
}

fn setup_clouds(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    enabled: Res<CloudsEnabled>,
) {
    if !enabled.0 {
        return;
    }
    let assets = CloudAssets {
        cube: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
    };
    let mut rng = rand::thread_rng();
    for _ in 0..CLOUD_COUNT {
        // The initial fleet spawns mid-drift at full opacity so the sky is
        // never empty on boot (and screenshots taken on frame 5 see clouds).
        let x = rng.gen_range(SPAWN_X..FADE_OUT_X);
        spawn_cloud(
            &mut commands,
            &assets,
            &mut materials,
            &mut rng,
            x,
            Fade::Full,
        );
    }
    commands.insert_resource(assets);
}

fn spawn_cloud(
    commands: &mut Commands,
    assets: &CloudAssets,
    materials: &mut Assets<StandardMaterial>,
    rng: &mut ThreadRng,
    x: f32,
    fade: Fade,
) {
    let alpha = match fade {
        Fade::Full => CLOUD_ALPHA,
        _ => 0.0,
    };
    let material = materials.add(StandardMaterial {
        base_color: Color::srgba(1.0, 1.0, 1.0, alpha),
        alpha_mode: AlphaMode::Blend,
        perceptual_roughness: 1.0,
        reflectance: 0.06,
        ..default()
    });

    let snap = |v: f32| (v / SNAP).round() * SNAP;
    let mut parent = commands.spawn((
        Cloud {
            speed: rng.gen_range(0.4..0.9),
            material: material.clone(),
            fade,
        },
        Transform::from_xyz(x, rng.gen_range(ALTITUDE), rng.gen_range(LANE)),
        Visibility::default(),
        Name::new("Cloud"),
    ));

    parent.with_children(|cloud| {
        let puffs = rng.gen_range(9..=15);
        for i in 0..puffs {
            // A fat core cube with smaller puffs packed around it, flatter
            // than wide, everything snapped to the voxel grid.
            let (offset, size) = if i == 0 {
                (Vec3::ZERO, 0.9)
            } else {
                let dx = snap(rng.gen_range(-1.25..1.25));
                let dy = snap(rng.gen_range(-0.25..0.25));
                let dz = snap(rng.gen_range(-0.75..0.75));
                let spread = (dx * dx * 0.35 + dz * dz).sqrt();
                let size = snap((0.95 - 0.25 * spread) * rng.gen_range(0.7..1.1));
                (Vec3::new(dx, dy, dz), size.clamp(0.25, 0.9))
            };
            cloud.spawn((
                Mesh3d(assets.cube.clone()),
                MeshMaterial3d(material.clone()),
                Transform::from_translation(offset).with_scale(Vec3::splat(size)),
                NotShadowCaster,
            ));
        }
    });
}

/// Hermite ease: zero-velocity start and end, so fades ramp gently instead
/// of snapping at the endpoints (the old linear fade read as flicker).
fn smoothstep(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

fn drift_clouds(
    mut commands: Commands,
    time: Res<Time>,
    assets: Option<Res<CloudAssets>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut clouds: Query<(Entity, &mut Transform, &mut Cloud)>,
) {
    let Some(assets) = assets else {
        return; // clouds disabled
    };
    let dt = time.delta_secs();
    let mut rng = rand::thread_rng();

    for (entity, mut transform, mut cloud) in &mut clouds {
        transform.translation.x += cloud.speed * dt;

        let alpha = match &mut cloud.fade {
            Fade::In(t) => {
                *t += dt;
                let a = CLOUD_ALPHA * smoothstep((*t / FADE_SECS).min(1.0));
                if *t >= FADE_SECS {
                    cloud.fade = Fade::Full;
                }
                a
            }
            Fade::Full => {
                if transform.translation.x > FADE_OUT_X {
                    cloud.fade = Fade::Out(0.0);
                }
                continue; // alpha already at full — no material write needed
            }
            Fade::Out(t) => {
                *t += dt;
                let a = CLOUD_ALPHA * smoothstep((1.0 - *t / FADE_SECS).max(0.0));
                if *t >= FADE_SECS {
                    commands.entity(entity).despawn();
                    spawn_cloud(
                        &mut commands,
                        &assets,
                        &mut materials,
                        &mut rng,
                        SPAWN_X,
                        Fade::In(0.0),
                    );
                }
                a
            }
        };

        if let Some(mut material) = materials.get_mut(&cloud.material) {
            material.base_color.set_alpha(alpha);
        }
    }
}
