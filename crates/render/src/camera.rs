use std::f32::consts::PI;
use bevy::prelude::*;
use bevy::camera::ScalingMode;
use bevy::ecs::message::MessageReader;
use bevy::input::mouse::{MouseMotion, MouseWheel};

#[derive(Resource)]
pub struct CameraState {
    pub target:   Vec3,
    pub yaw:      f32,
    pub pitch:    f32,
    pub distance: f32,
    pub scale:    f32,
}

impl Default for CameraState {
    fn default() -> Self {
        Self {
            target:   Vec3::new(6.0, 5.0, 6.0),
            yaw:      PI / 4.0,
            pitch:    PI / 4.5,
            distance: 40.0,
            scale:    22.0,
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
            scaling_mode: ScalingMode::FixedVertical { viewport_height: state.scale },
            ..OrthographicProjection::default_3d()
        }),
        Transform::from_translation(eye).looking_at(state.target, Vec3::Y),
        InspectorCamera,
    ));
    commands.insert_resource(state);

    commands.spawn((
        DirectionalLight {
            illuminance: 12_000.0,
            ..default()
        },
        Transform::from_xyz(10.0, 20.0, 10.0).looking_at(Vec3::ZERO, Vec3::Y),
    ));
    // AmbientLight is a component (not a Resource) in Bevy 0.19
    commands.spawn(AmbientLight {
        color: Color::WHITE,
        brightness: 400.0,
        ..default()
    });
}

pub fn update_camera(
    state: Res<CameraState>,
    mut q: Query<(&mut Transform, &mut Projection), With<InspectorCamera>>,
) {
    let Some((mut tf, mut proj)) = q.iter_mut().next() else { return; };
    let eye = cam_eye(&state);
    *tf = Transform::from_translation(eye).looking_at(state.target, Vec3::Y);
    if let Projection::Orthographic(ref mut ortho) = *proj {
        ortho.scaling_mode = ScalingMode::FixedVertical { viewport_height: state.scale };
    }
}

pub fn camera_input(
    mut state:    ResMut<CameraState>,
    buttons:      Res<ButtonInput<MouseButton>>,
    mut motion:   MessageReader<MouseMotion>,
    mut scroll:   MessageReader<MouseWheel>,
    egui_wants:   Option<Res<EguiWantsPointer>>,
) {
    if egui_wants.map(|e| e.0).unwrap_or(false) {
        motion.clear();
        scroll.clear();
        return;
    }

    for ev in scroll.read() {
        let delta = ev.y * 0.05;
        state.distance = (state.distance - delta * state.distance).clamp(5.0, 120.0);
        state.scale    = (state.scale    - delta * state.scale).clamp(3.0, 80.0);
    }

    for ev in motion.read() {
        if buttons.pressed(MouseButton::Left) {
            state.yaw   -= ev.delta.x * 0.008;
            state.pitch  = (state.pitch + ev.delta.y * 0.008)
                .clamp(-PI / 2.0 + 0.05, PI / 2.0 - 0.05);
        }
        if buttons.pressed(MouseButton::Middle) {
            let right   = Vec3::new(state.yaw.cos(), 0.0, -state.yaw.sin()).normalize();
            let fwd_xz  = Vec3::new(-state.yaw.sin(), 0.0, -state.yaw.cos()).normalize();
            let pan     = state.scale / 20.0 * 0.04;
            state.target -= right  * ev.delta.x * pan;
            state.target += fwd_xz * ev.delta.y * pan;
        }
    }
}

fn cam_eye(s: &CameraState) -> Vec3 {
    s.target + Vec3::new(
        s.yaw.sin() * s.pitch.cos() * s.distance,
        s.pitch.sin() * s.distance,
        s.yaw.cos() * s.pitch.cos() * s.distance,
    )
}

#[derive(Resource, Default)]
pub struct EguiWantsPointer(pub bool);
