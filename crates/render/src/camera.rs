//! Orbit / pan / zoom perspective camera for the inspector.
//!
//! The projection is a natural perspective (≈45° vertical FOV) — the earlier
//! orthographic projection made the diorama read as flat/skewed. Zoom is
//! dolly-based: the scroll wheel moves the eye along the view ray instead of
//! scaling an ortho window.

use bevy::ecs::message::MessageReader;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use std::f32::consts::PI;
use voxel_core::CELLS_XY;

#[derive(Resource)]
pub struct CameraState {
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    /// Eye distance from the target in world units (zoom).
    pub distance: f32,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            // Frame the focused biome (an 8×8 footprint, 12 tall, surface at
            // y=6) from the classic diorama three-quarter view.
            target: Vec3::new(CELLS_XY as f32 / 2.0, 4.5, CELLS_XY as f32 / 2.0),
            yaw: PI / 4.0,
            // Steep enough that the view ray clears the 12-cell-tall
            // fog-of-war shell one gap away (biomes sit only 9 units apart).
            pitch: 0.85,
            distance: 28.0,
        }
    }
}

/// Vertical field of view. ~45° reads as a natural perspective; much narrower
/// starts looking orthographic again, much wider fish-eyes the diorama.
const FOV_Y: f32 = 45.0 * PI / 180.0;
const MIN_DISTANCE: f32 = 6.0;
const MAX_DISTANCE: f32 = 90.0;

#[derive(Component)]
pub struct InspectorCamera;

pub fn spawn_camera(mut commands: Commands) {
    let state = CameraState::default();
    let eye = cam_eye(&state);
    commands.spawn((
        Camera3d::default(),
        Projection::from(PerspectiveProjection {
            fov: FOV_Y,
            ..default()
        }),
        Transform::from_translation(eye).looking_at(state.target, Vec3::Y),
        crate::camera_fog(),
        InspectorCamera,
    ));
    commands.insert_resource(state);
}

pub fn update_camera(state: Res<CameraState>, mut q: Query<&mut Transform, With<InspectorCamera>>) {
    let Some(mut tf) = q.iter_mut().next() else {
        return;
    };
    let eye = cam_eye(&state);
    *tf = Transform::from_translation(eye).looking_at(state.target, Vec3::Y);
}

pub fn camera_input(
    mut state: ResMut<CameraState>,
    buttons: Res<ButtonInput<MouseButton>>,
    mut motion: MessageReader<MouseMotion>,
    mut scroll: MessageReader<MouseWheel>,
    egui_wants: Option<Res<EguiWantsPointer>>,
) {
    if egui_wants.map(|e| e.0).unwrap_or(false) {
        motion.clear();
        scroll.clear();
        return;
    }

    for ev in scroll.read() {
        // Proportional dolly: equal scroll steps feel equal at any zoom.
        let delta = ev.y * 0.05;
        state.distance =
            (state.distance - delta * state.distance).clamp(MIN_DISTANCE, MAX_DISTANCE);
    }

    for ev in motion.read() {
        if buttons.pressed(MouseButton::Left) {
            state.yaw -= ev.delta.x * 0.008;
            // Allow a peek under the island but not a full flip.
            state.pitch = (state.pitch + ev.delta.y * 0.008).clamp(-0.5, PI / 2.0 - 0.05);
        }
        if buttons.pressed(MouseButton::Middle) {
            let right = Vec3::new(state.yaw.cos(), 0.0, -state.yaw.sin());
            let fwd_xz = Vec3::new(-state.yaw.sin(), 0.0, -state.yaw.cos());
            let pan = state.distance / 20.0 * 0.04;
            state.target -= right * ev.delta.x * pan;
            state.target += fwd_xz * ev.delta.y * pan;
        }
    }
}

fn cam_eye(s: &CameraState) -> Vec3 {
    s.target
        + Vec3::new(
            s.yaw.sin() * s.pitch.cos() * s.distance,
            s.pitch.sin() * s.distance,
            s.yaw.cos() * s.pitch.cos() * s.distance,
        )
}

#[derive(Resource, Default)]
pub struct EguiWantsPointer(pub bool);
