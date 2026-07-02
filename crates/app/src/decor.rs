//! Decoration spawning & animation (scene tier).
//!
//! `voxel-render::decor` plans *what* stands *where* (pure, deterministic);
//! this module turns the plan into Bevy scene instances and keeps the fauna
//! alive with lightweight transform animation. Decorations are cosmetic
//! only — no game logic ever reads these entities.
//!
//! Like the clouds, animation is intentionally time-based and therefore not
//! byte-stable across runs; `--no-decor` exists for stable screenshots.

use std::f32::consts::{PI, TAU};

use bevy::prelude::*;
use voxel_render::{plan_decor, DecorKind, FocusedBiome, LayerCutoff, SceneEntities};

use crate::assets::DecorAssets;
use crate::state::{InspectorState, WorldMapResource};

/// Master toggle (CLI `--no-decor`, inspector checkbox). Flipping it live
/// respawns/despawns all props.
#[derive(Resource, Clone, Copy, PartialEq, Eq)]
pub struct DecorEnabled(pub bool);

pub struct DecorPlugin;

impl Plugin for DecorPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DecorAssets>()
            .insert_resource(DecorEnabled(true))
            .add_systems(
                Update,
                (
                    (rebuild_decor, animate_decor).chain(),
                    crate::assets::harmonize_decor_materials,
                ),
            );
    }
}

// ── Components ────────────────────────────────────────────────────────────────

/// How a decoration moves. Derived from its kind at spawn time.
enum Motion {
    /// Flora: stands still.
    Still,
    /// Land animals: idle bob, a small periodic hop, and a lazy look-around
    /// sway.
    Hop,
    /// Fish: a slow circular lap inside the water cell, gently bobbing,
    /// nose along the swim direction.
    Swim,
}

#[derive(Component)]
struct Decor {
    anchor: Vec3,
    yaw: f32,
    /// Per-instance de-sync in [0, 1) from the planner — neighbours never
    /// move in lockstep.
    phase: f32,
    motion: Motion,
}

// ── Systems ───────────────────────────────────────────────────────────────────

/// Respawn all decorations when the same inputs that rebuild the terrain
/// scene change (world map, focused biome, layer peel) or the toggle flips.
fn rebuild_decor(
    mut commands: Commands,
    world_map: Res<WorldMapResource>,
    focused: Res<FocusedBiome>,
    cutoff: Res<LayerCutoff>,
    state: Res<InspectorState>,
    enabled: Res<DecorEnabled>,
    assets: Res<DecorAssets>,
    mut scene: ResMut<SceneEntities>,
) {
    if !world_map.is_changed()
        && !focused.is_changed()
        && !cutoff.is_changed()
        && !enabled.is_changed()
    {
        return;
    }

    for e in scene.decorations.drain(..) {
        commands.entity(e).despawn();
    }
    if !enabled.0 {
        return;
    }

    let grid = world_map.0.biome(focused.0);
    for inst in plan_decor(grid, cutoff.0, focused.0, state.seed) {
        let motion = match inst.kind {
            DecorKind::Animal => Motion::Hop,
            DecorKind::Fish => Motion::Swim,
            _ => Motion::Still,
        };
        let e = commands
            .spawn((
                WorldAssetRoot(assets.scene(inst.kind, inst.variant)),
                Transform {
                    translation: inst.translation,
                    rotation: Quat::from_rotation_y(inst.yaw),
                    scale: Vec3::splat(inst.scale),
                },
                Decor {
                    anchor: inst.translation,
                    yaw: inst.yaw,
                    phase: inst.phase,
                    motion,
                },
                Name::new(format!("Decor{:?}", inst.kind)),
            ))
            .id();
        scene.decorations.push(e);
    }
}

/// Drive the idle animations. Pure transform work off `Time` — cheap enough
/// to run unconditionally over the few dozen props of the focused biome.
fn animate_decor(time: Res<Time>, mut props: Query<(&Decor, &mut Transform)>) {
    let t = time.elapsed_secs();
    for (d, mut tf) in props.iter_mut() {
        match d.motion {
            Motion::Still => {}
            Motion::Hop => {
                // Small hop once per cycle (cycle length varies per animal),
                // flat on the ground the rest of the time.
                let period = 2.2 + d.phase * 1.8;
                let cycle = ((t + d.phase * 20.0) / period).fract();
                let hop = if cycle < 0.22 {
                    0.16 * (cycle / 0.22 * PI).sin()
                } else {
                    0.0
                };
                tf.translation.y = d.anchor.y + hop;
                // Lazy look-around sway.
                tf.rotation = Quat::from_rotation_y(d.yaw + 0.18 * (t * 0.9 + d.phase * TAU).sin());
            }
            Motion::Swim => {
                // One lap of a small circle every ~12–20 s, staying inside
                // the water cell (radius + body length < half a cell).
                let lap = 12.0 + d.phase * 8.0;
                let a = TAU * (t / lap + d.phase);
                let r = 0.15;
                tf.translation = d.anchor + Vec3::new(r * a.cos(), 0.0, r * a.sin());
                tf.translation.y = d.anchor.y + 0.03 * (t * 1.1 + d.phase * TAU).sin();
                // Nose along the tangent of the circle (model faces -Z).
                tf.rotation = Quat::from_rotation_y(PI - a);
            }
        }
    }
}
