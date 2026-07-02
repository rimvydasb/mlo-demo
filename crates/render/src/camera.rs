//! Orbit / pan / zoom orthographic camera for the inspector.

use bevy::camera::ScalingMode;
use bevy::ecs::message::MessageReader;
use bevy::input::mouse::{MouseMotion, MouseWheel};
use bevy::prelude::*;
use std::f32::consts::PI;

#[derive(Resource)]
pub struct CameraState {
    pub target: Vec3,
    pub yaw: f32,
    pub pitch: f32,
    pub distance: f32,
    /// Orthographic viewport height in world units (zoom).
    pub scale: f32,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            // Frame the focused biome (a 12-unit cube with its surface at
            // y=6) from the classic diorama three-quarter view.
            target: Vec3::new(6.0, 4.5, 6.0),
            yaw: PI / 4.0,
            pitch: 0.6,
            distance: 60.0,
            scale: 17.5,
        }
    }
}

#[derive(Component)]
pub struct InspectorCamera;

pub fn spawn_camera(mut commands: Commands) {
    let state = CameraState::default();
    let eye = cam_eye(&state);
    commands.spawn((
        Camera3d::default(),
        Projection::from(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: state.scale,
            },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_translation(eye).looking_at(state.target, Vec3::Y),
        crate::camera_fog(),
        InspectorCamera,
    ));
    commands.insert_resource(state);
}

pub fn update_camera(
    state: Res<CameraState>,
    mut q: Query<(&mut Transform, &mut Projection), With<InspectorCamera>>,
) {
    let Some((mut tf, mut proj)) = q.iter_mut().next() else {
        return;
    };
    let eye = cam_eye(&state);
    *tf = Transform::from_translation(eye).looking_at(state.target, Vec3::Y);
    if let Projection::Orthographic(ref mut ortho) = *proj {
        ortho.scaling_mode = ScalingMode::FixedVertical {
            viewport_height: state.scale,
        };
    }
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
        let delta = ev.y * 0.05;
        state.scale = (state.scale - delta * state.scale).clamp(3.0, 80.0);
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
            let pan = state.scale / 20.0 * 0.04;
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
