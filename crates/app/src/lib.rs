mod assets;
mod clouds;
mod decor;
mod inspector;
mod picking;
pub mod state;

pub use assets::DecorAssets;
pub use clouds::{CloudsEnabled, CloudsPlugin};
pub use decor::{DecorEnabled, DecorPlugin};
pub use state::{scenic_biome, InspectorState, WorldMapResource};

use bevy::ecs::message::MessageWriter;
use bevy::prelude::*;
use bevy::render::view::screenshot::{save_to_disk, Screenshot};
use bevy::window::PrimaryWindow;
use bevy_egui::{EguiPlugin, EguiPrimaryContextPass};
use voxel_core::{BiomeCoord, WORLD_BIOMES};
use voxel_render::{
    build_biome_meshes, expand, proxy_material, proxy_world_offset, terrain_material,
    water_material, BeautifyOptions, EguiWantsPointer, FocusedBiome, InspectorCamera, LayerCutoff,
    RenderPlugin, SceneEntities,
};

// ── Public entry points ───────────────────────────────────────────────────────

/// Presentation toggles shared by the inspector and the screenshot runner.
#[derive(Clone, Copy)]
pub struct AppOptions {
    /// Drifting clouds over the focused biome. Intentionally
    /// non-deterministic — disable for byte-stable screenshots.
    pub clouds: bool,
    /// The optional MICROHEIGHT beautification rule (A/B flag).
    pub microheight: bool,
    /// Fauna & flora decoration props. Placement is deterministic, but the
    /// idle animations are time-based — disable for byte-stable screenshots.
    pub decor: bool,
}

impl Default for AppOptions {
    fn default() -> Self {
        Self {
            clouds: true,
            microheight: false,
            decor: true,
        }
    }
}

impl AppOptions {
    fn beautify(self) -> BeautifyOptions {
        BeautifyOptions {
            microheight: self.microheight,
        }
    }
}

/// Headless-ish screenshot: opens a window, renders a few frames, captures
/// via Bevy's Screenshot API, saves to `out`, then exits. Focuses `biome`,
/// or the seed's most scenic biome when `None`.
pub fn run_screenshot(
    seed: u64,
    out: std::path::PathBuf,
    biome: Option<BiomeCoord>,
    opts: AppOptions,
) {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: "screenshot".into(),
                    resolution: bevy::window::WindowResolution::new(1280, 800),
                    visible: true,
                    ..default()
                }),
                ..default()
            })
            .set(assets::asset_plugin()),
        RenderPlugin,
        CloudsPlugin,
        DecorPlugin,
    ));
    let map = voxel_mapgen::generate(seed);
    let focus = biome.unwrap_or_else(|| scenic_biome(&map));
    app.insert_resource(FocusedBiome(focus))
        .insert_resource(WorldMapResource(map))
        .insert_resource(InspectorState::new(seed))
        .insert_resource(CloudsEnabled(opts.clouds))
        .insert_resource(DecorEnabled(opts.decor))
        .insert_resource(opts.beautify())
        .insert_resource(ScreenshotOutPath(out))
        .add_systems(Update, (rebuild_scene, drive_screenshot))
        .run();
}

#[derive(Resource)]
struct ScreenshotOutPath(std::path::PathBuf);

fn drive_screenshot(
    mut commands: Commands,
    path: Res<ScreenshotOutPath>,
    mut frame: Local<u32>,
    mut waited: Local<u32>,
    mut exit: MessageWriter<AppExit>,
    server: Res<AssetServer>,
    decor_assets: Res<DecorAssets>,
    decor_enabled: Res<DecorEnabled>,
) {
    // Hold the capture countdown until the decor GLBs (and their textures)
    // are in, so props are never captured half-loaded. Capped so a broken
    // asset can't hang the runner.
    *waited += 1;
    if decor_enabled.0 && *waited < 600 && !decor_assets.all_loaded(&server) {
        return;
    }
    *frame += 1;
    match *frame {
        // Frame 5: request screenshot (after the scene has had frames to render)
        5 => {
            commands
                .spawn(Screenshot::primary_window())
                .observe(save_to_disk(path.0.clone()));
        }
        // Frame 12: give the GPU time to complete the async capture, then exit
        12 => {
            exit.write(AppExit::Success);
        }
        _ => {}
    }
}

pub fn run_inspector(seed: u64, opts: AppOptions) {
    let mut app = App::new();
    app.add_plugins((
        DefaultPlugins
            .set(WindowPlugin {
                primary_window: Some(Window {
                    title: format!("Voxel MLM Inspector — seed {seed}"),
                    resolution: bevy::window::WindowResolution::new(1280, 800),
                    ..default()
                }),
                ..default()
            })
            .set(assets::asset_plugin()),
        EguiPlugin::default(),
        RenderPlugin,
        CloudsPlugin,
        DecorPlugin,
    ));
    let map = voxel_mapgen::generate(seed);
    app.insert_resource(FocusedBiome(scenic_biome(&map)))
        .insert_resource(WorldMapResource(map))
        .insert_resource(InspectorState::new(seed))
        .insert_resource(CloudsEnabled(opts.clouds))
        .insert_resource(DecorEnabled(opts.decor))
        .insert_resource(opts.beautify())
        .add_systems(EguiPrimaryContextPass, inspector::egui_inspector)
        .add_systems(
            Update,
            (
                rebuild_scene,
                pick_cell,
                handle_keyboard,
                handle_exports,
                handle_screenshot,
            ),
        )
        .run();
}

// ── Scene rebuild ─────────────────────────────────────────────────────────────

/// Cached material handles — the meshes change on rebuild, the materials
/// never do.
#[derive(Resource)]
struct TerrainMaterials {
    opaque: Handle<StandardMaterial>,
    water: Handle<StandardMaterial>,
}

fn rebuild_scene(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    cached: Option<Res<TerrainMaterials>>,
    world_map: Res<WorldMapResource>,
    focused: Res<FocusedBiome>,
    cutoff: Res<LayerCutoff>,
    state: Res<InspectorState>,
    beautify: Res<BeautifyOptions>,
    mut scene: ResMut<SceneEntities>,
) {
    if !world_map.is_changed() && !focused.is_changed() && !cutoff.is_changed() {
        return;
    }

    let (opaque_mat, water_mat) = match cached {
        Some(c) => (c.opaque.clone(), c.water.clone()),
        None => {
            let handles = TerrainMaterials {
                opaque: materials.add(terrain_material()),
                water: materials.add(water_material()),
            };
            let out = (handles.opaque.clone(), handles.water.clone());
            commands.insert_resource(handles);
            out
        }
    };

    for e in scene.biome_meshes.drain(..) {
        commands.entity(e).despawn();
    }
    for e in scene.proxy_tiles.drain(..) {
        commands.entity(e).despawn();
    }

    // Focused biome: expand cells → voxels, mesh, spawn (opaque + water).
    let grid = world_map.0.biome(focused.0);
    let volume = expand(grid, cutoff.0, focused.0, state.seed, *beautify);
    let built = build_biome_meshes(&volume, focused.0, state.seed);

    if let Some(mesh) = built.opaque {
        let e = commands
            .spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(opaque_mat),
                Transform::default(),
                Name::new("BiomeTerrain"),
            ))
            .id();
        scene.biome_meshes.push(e);
    }
    if let Some(mesh) = built.water {
        let e = commands
            .spawn((
                Mesh3d(meshes.add(mesh)),
                MeshMaterial3d(water_mat),
                Transform::default(),
                Name::new("BiomeWater"),
            ))
            .id();
        scene.biome_meshes.push(e);
    }

    // Proxy tiles for the other 35 biomes.
    for row in 0..WORLD_BIOMES as u8 {
        for col in 0..WORLD_BIOMES as u8 {
            let coord = BiomeCoord::new(row, col);
            if coord == focused.0 {
                continue;
            }
            let bt = world_map.0.biome_type(coord);
            let offset = proxy_world_offset(coord, focused.0);
            let e = commands
                .spawn((
                    Mesh3d(meshes.add(Cuboid::new(11.0, 0.6, 11.0))),
                    MeshMaterial3d(materials.add(proxy_material(bt))),
                    Transform::from_translation(offset),
                    Name::new(format!("Proxy_{row}_{col}")),
                ))
                .id();
            scene.proxy_tiles.push(e);
        }
    }
}

// ── Cell picking ──────────────────────────────────────────────────────────────

fn pick_cell(
    cameras: Query<(&Camera, &GlobalTransform), With<InspectorCamera>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    world_map: Res<WorldMapResource>,
    focused: Res<FocusedBiome>,
    cutoff: Res<LayerCutoff>,
    mut state: ResMut<InspectorState>,
    egui_wants: Res<EguiWantsPointer>,
) {
    if egui_wants.0 {
        state.hovered_cell = None;
        return;
    }
    let Ok((camera, cam_tf)) = cameras.single() else {
        return;
    };
    let Ok(window) = windows.single() else {
        return;
    };
    let Some(cursor) = window.cursor_position() else {
        state.hovered_cell = None;
        return;
    };

    let Ok(ray) = camera.viewport_to_world(cam_tf, cursor) else {
        return;
    };
    let grid = world_map.0.biome(focused.0);
    state.hovered_cell = picking::raycast_cell(ray, grid, cutoff.0);
}

// ── Keyboard navigation ───────────────────────────────────────────────────────

fn handle_keyboard(
    keyboard: Res<ButtonInput<KeyCode>>,
    mut focused: ResMut<FocusedBiome>,
    egui_wants: Res<EguiWantsPointer>,
) {
    if egui_wants.0 {
        return;
    }

    let max = WORLD_BIOMES as u8 - 1;
    let (r, c) = (focused.0.row, focused.0.col);
    if keyboard.just_pressed(KeyCode::Tab) {
        let nc = (c + 1) % (max + 1);
        focused.0 = BiomeCoord::new(if nc == 0 { (r + 1) % (max + 1) } else { r }, nc);
    }
    if keyboard.just_pressed(KeyCode::ArrowRight) && c < max {
        focused.0 = BiomeCoord::new(r, c + 1);
    }
    if keyboard.just_pressed(KeyCode::ArrowLeft) && c > 0 {
        focused.0 = BiomeCoord::new(r, c - 1);
    }
    if keyboard.just_pressed(KeyCode::ArrowDown) && r < max {
        focused.0 = BiomeCoord::new(r + 1, c);
    }
    if keyboard.just_pressed(KeyCode::ArrowUp) && r > 0 {
        focused.0 = BiomeCoord::new(r - 1, c);
    }
}

// ── Export handlers ───────────────────────────────────────────────────────────

fn handle_screenshot(mut commands: Commands, mut state: ResMut<InspectorState>) {
    if state.export_png {
        state.export_png = false;
        let path = format!("screenshot_seed{}.png", state.seed);
        commands
            .spawn(Screenshot::primary_window())
            .observe(save_to_disk(path));
    }
}

fn handle_exports(
    mut state: ResMut<InspectorState>,
    world_map: Res<WorldMapResource>,
    focused: Res<FocusedBiome>,
) {
    if state.export_ascii {
        state.export_ascii = false;
        let macro_text = voxel_mapgen::ascii_macro(&world_map.0);
        let biome_text = voxel_mapgen::ascii_dump(&world_map.0, focused.0);
        let content = format!("{}\n{}", macro_text, biome_text);
        let filename = format!(
            "dump_seed{}_r{}c{}.txt",
            state.seed, focused.0.row, focused.0.col
        );
        match std::fs::write(&filename, &content) {
            Ok(_) => println!("Saved {filename}"),
            Err(e) => eprintln!("Failed to save {filename}: {e}"),
        }
        println!("{content}");
    }
}
