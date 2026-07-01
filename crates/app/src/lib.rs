mod inspector;
pub mod state;

pub use state::{InspectorState, WorldMapResource};

use bevy::prelude::*;
use bevy::window::PrimaryWindow;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::ecs::message::MessageWriter;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use voxel_core::{BiomeCoord, Material};
use voxel_render::{
    build_voxel_meshes, proxy_world_offset, proxy_material,
    FocusedBiome, InspectorCamera, LayerCutoff, RenderPlugin, SceneEntities,
    EguiWantsPointer,
};

// ── Public entry points ───────────────────────────────────────────────────────

/// Headless-ish screenshot: opens a minimized window, renders a few frames,
/// captures via Bevy's Screenshot API, saves to `out`, then exits.
pub fn run_screenshot(seed: u64, out: std::path::PathBuf) {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: "screenshot".into(),
                    resolution: bevy::window::WindowResolution::new(1280, 800),
                    visible: true,
                    ..default()
                }),
                ..default()
            }),
            RenderPlugin,
        ))
        .insert_resource(WorldMapResource(voxel_mapgen::generate(seed)))
        .insert_resource(InspectorState::new(seed))
        .insert_resource(ScreenshotOutPath(out))
        .add_systems(Update, (rebuild_scene, drive_screenshot))
        .run();
}

#[derive(Resource)]
struct ScreenshotOutPath(std::path::PathBuf);

fn drive_screenshot(
    mut commands:  Commands,
    path:          Res<ScreenshotOutPath>,
    mut frame:     Local<u32>,
    mut exit:      MessageWriter<AppExit>,
) {
    *frame += 1;
    match *frame {
        // Frame 5: request screenshot (after scene has had several frames to render)
        5 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path.0.clone()));
        }
        // Frame 12: give GPU enough time to complete async capture, then exit
        12 => { exit.write(AppExit::Success); }
        _ => {}
    }
}

pub fn run_inspector(seed: u64) {
    App::new()
        .add_plugins((
            DefaultPlugins.set(WindowPlugin {
                primary_window: Some(Window {
                    title: format!("Voxel MLM Inspector — seed {seed}"),
                    resolution: bevy::window::WindowResolution::new(1280, 800),
                    ..default()
                }),
                ..default()
            }),
            EguiPlugin::default(),
            RenderPlugin,
        ))
        .insert_resource(WorldMapResource(voxel_mapgen::generate(seed)))
        .insert_resource(InspectorState::new(seed))
        .add_systems(EguiPrimaryContextPass, inspector::egui_inspector)
        .add_systems(Update, (
            rebuild_scene,
            pick_voxel,
            handle_keyboard,
            handle_exports,
            handle_screenshot,
        ))
        .run();
}

// ── Scene rebuild ─────────────────────────────────────────────────────────────

fn rebuild_scene(
    mut commands:  Commands,
    mut meshes:    ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    world_map:     Res<WorldMapResource>,
    focused:       Res<FocusedBiome>,
    cutoff:        Res<LayerCutoff>,
    mut scene:     ResMut<SceneEntities>,
) {
    if !world_map.is_changed() && !focused.is_changed() && !cutoff.is_changed() {
        return;
    }

    // Despawn old entities
    for e in scene.biome_meshes.drain(..) {
        commands.entity(e).despawn();
    }
    for e in scene.proxy_tiles.drain(..) {
        commands.entity(e).despawn();
    }

    // Focused biome: one mesh entity per voxel color group
    let grid = world_map.0.biome(focused.0);
    for (mesh, [r, g, b]) in build_voxel_meshes(grid, cutoff.0) {
        let e = commands.spawn((
            Mesh3d(meshes.add(mesh)),
            MeshMaterial3d(materials.add(StandardMaterial {
                base_color: Color::srgb(r, g, b),
                perceptual_roughness: 0.9,
                metallic: 0.0,
                double_sided: true,
                cull_mode: None,
                ..default()
            })),
            Transform::default(),
            Name::new("BiomeMesh"),
        )).id();
        scene.biome_meshes.push(e);
    }

    // Proxy tiles for the other 35 biomes
    for row in 0..6u8 {
        for col in 0..6u8 {
            let coord = BiomeCoord::new(row, col);
            if coord == focused.0 { continue; }
            let bt     = world_map.0.biome_type(coord);
            let offset = proxy_world_offset(coord, focused.0);
            let e = commands.spawn((
                Mesh3d(meshes.add(Cuboid::new(11.0, 0.4, 11.0))),
                MeshMaterial3d(materials.add(proxy_material(bt))),
                Transform::from_translation(offset),
                Name::new(format!("Proxy_{row}_{col}")),
            )).id();
            scene.proxy_tiles.push(e);
        }
    }
}

// ── Voxel picking ─────────────────────────────────────────────────────────────

fn pick_voxel(
    cameras:    Query<(&Camera, &GlobalTransform), With<InspectorCamera>>,
    windows:    Query<&Window, With<PrimaryWindow>>,
    world_map:  Res<WorldMapResource>,
    focused:    Res<FocusedBiome>,
    cutoff:     Res<LayerCutoff>,
    mut state:  ResMut<InspectorState>,
    egui_wants: Res<EguiWantsPointer>,
) {
    if egui_wants.0 {
        state.hovered_voxel = None;
        return;
    }
    let Ok((camera, cam_tf)) = cameras.single() else { return; };
    let Ok(window) = windows.single() else { return; };
    let Some(cursor) = window.cursor_position() else {
        state.hovered_voxel = None;
        return;
    };

    let Ok(ray) = camera.viewport_to_world(cam_tf, cursor) else { return; };
    let grid = world_map.0.biome(focused.0);
    state.hovered_voxel = march_ray(ray, grid, cutoff.0);
}

fn march_ray(ray: Ray3d, grid: &voxel_core::VoxelGrid, cutoff: u8) -> Option<(u8, u8, u8)> {
    let origin = ray.origin;
    let dir    = Vec3::from(*ray.direction);
    let step   = 0.15_f32;
    let mut t  = 0.0_f32;

    while t < 80.0 {
        let p  = origin + dir * t;
        let vx = p.x.floor() as i32;
        let vy = p.z.floor() as i32;
        let vz = p.y.floor() as i32;

        if vx >= 0 && vy >= 0 && vz >= 0 && vx < 12 && vy < 12 && vz < 12 {
            let vz8 = vz as u8;
            if vz8 < cutoff {
                let voxel = grid.get(vx as u8, vy as u8, vz8);
                if voxel.material != Material::Air {
                    return Some((vx as u8, vy as u8, vz8));
                }
            }
        }
        t += step;
    }
    None
}

// ── Keyboard navigation ───────────────────────────────────────────────────────

fn handle_keyboard(
    keyboard:    Res<ButtonInput<KeyCode>>,
    mut focused: ResMut<FocusedBiome>,
    egui_wants:  Res<EguiWantsPointer>,
) {
    if egui_wants.0 { return; }

    let (r, c) = (focused.0.row, focused.0.col);
    if keyboard.just_pressed(KeyCode::Tab) {
        let nc = (c + 1) % 6;
        focused.0 = BiomeCoord::new(if nc == 0 { (r + 1) % 6 } else { r }, nc);
    }
    if keyboard.just_pressed(KeyCode::ArrowRight) && c < 5 {
        focused.0 = BiomeCoord::new(r, c + 1);
    }
    if keyboard.just_pressed(KeyCode::ArrowLeft) && c > 0 {
        focused.0 = BiomeCoord::new(r, c - 1);
    }
    if keyboard.just_pressed(KeyCode::ArrowDown) && r < 5 {
        focused.0 = BiomeCoord::new(r + 1, c);
    }
    if keyboard.just_pressed(KeyCode::ArrowUp) && r > 0 {
        focused.0 = BiomeCoord::new(r - 1, c);
    }
}

// ── Screenshot handler ────────────────────────────────────────────────────

fn handle_screenshot(mut commands: Commands, mut state: ResMut<InspectorState>) {
    if state.export_png {
        state.export_png = false;
        let path = format!("screenshot_seed{}.png", state.seed);
        commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path));
    }
}

// ── Export handlers ───────────────────────────────────────────────────────────

fn handle_exports(
    mut state: ResMut<InspectorState>,
    world_map: Res<WorldMapResource>,
    focused:   Res<FocusedBiome>,
) {
    if state.export_ascii {
        state.export_ascii = false;
        let macro_text = voxel_mapgen::ascii_macro(&world_map.0);
        let biome_text = voxel_mapgen::ascii_dump(&world_map.0, focused.0);
        let content    = format!("{}\n{}", macro_text, biome_text);
        let filename   = format!(
            "dump_seed{}_r{}c{}.txt",
            state.seed, focused.0.row, focused.0.col
        );
        match std::fs::write(&filename, &content) {
            Ok(_)  => println!("Saved {filename}"),
            Err(e) => eprintln!("Failed to save {filename}: {e}"),
        }
        println!("{content}");
    }
}
