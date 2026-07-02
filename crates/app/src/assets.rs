//! Asset management for decoration props (fauna & flora GLB scenes).
//!
//! Single responsibility: resolve the workspace asset root and hold the
//! scene handles for every catalog model. What gets placed where is the
//! planner's job (`voxel-render::decor`); spawning and animating entities is
//! `crate::decor`'s.

use bevy::asset::AssetPlugin;
use bevy::ecs::message::MessageReader;
use bevy::gltf::GltfAssetLabel;
use bevy::prelude::*;
use voxel_render::DecorKind;

/// Bevy `AssetPlugin` configured for this workspace: game assets live in
/// `<workspace root>/assets`, curated from `imports/` (see
/// docs/rendering-fauna-flora.md — `imports/` is never referenced at
/// runtime). Bevy's default asset root resolves against the *running
/// package's* manifest dir, which breaks in a workspace, so the native build
/// bakes the workspace-root path at compile time. On wasm the default
/// relative `assets/` URL is correct — the folder is served with the site.
pub fn asset_plugin() -> AssetPlugin {
    #[cfg(target_arch = "wasm32")]
    {
        AssetPlugin::default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        AssetPlugin {
            file_path: concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets").to_string(),
            ..Default::default()
        }
    }
}

/// Scene handles for every decor model, indexed `[kind][variant]` in catalog
/// order. Built once at plugin init (`FromWorld` needs the `AssetServer`);
/// the GLBs themselves load asynchronously — spawned `WorldAssetRoot`s
/// simply pop in when ready. (Bevy 0.19: glTF scenes load as `WorldAsset`,
/// not the retired `Scene` asset.)
#[derive(Resource)]
pub struct DecorAssets {
    by_kind: Vec<Vec<Handle<WorldAsset>>>,
}

impl FromWorld for DecorAssets {
    fn from_world(world: &mut World) -> Self {
        let server = world.resource::<AssetServer>();
        let by_kind = DecorKind::ALL
            .iter()
            .map(|kind| {
                kind.models()
                    .iter()
                    .map(|m| server.load(GltfAssetLabel::Scene(0).from_asset(m.path)))
                    .collect()
            })
            .collect();
        Self { by_kind }
    }
}

impl DecorAssets {
    /// Handle for a planned instance. `variant` indexes the kind's catalog.
    pub fn scene(&self, kind: DecorKind, variant: usize) -> Handle<WorldAsset> {
        self.by_kind[kind.index()][variant].clone()
    }

    /// True once every GLB (with its meshes/materials/textures) is loaded.
    /// The screenshot runner waits on this so props are never half-in.
    pub fn all_loaded(&self, server: &AssetServer) -> bool {
        self.by_kind
            .iter()
            .flatten()
            .all(|h| server.is_loaded_with_dependencies(h.id()))
    }
}

// ── Palette harmonization ─────────────────────────────────────────────────────

/// Kenney's nature-kit foliage palette is teal (`leafsGreen` ≈ linear
/// (0.16, 0.79, 0.67)), which clashes with the terrain's yellow-green grass
/// (`base_color_srgb`). Retint those known foliage colors to palette greens;
/// everything else (petals, bark, the cube-pets colormap texture) passes
/// through untouched. Source colors are glTF `baseColorFactor`s, which Bevy
/// loads as **linear** RGB.
const RETINTS: [([f32; 3], [f32; 3]); 3] = [
    // linear source → target sRGB
    ([0.161, 0.788, 0.671], [0.33, 0.68, 0.22]), // leafsGreen → canopy green
    ([0.169, 0.651, 0.667], [0.22, 0.52, 0.19]), // leafsDark  → pine green
    ([0.173, 0.847, 0.722], [0.41, 0.75, 0.26]), // grass      → meadow green
];

/// Runs over freshly loaded `StandardMaterial`s. GLB materials are shared
/// per asset, so one retint covers every instance; only `Added` events are
/// handled, so our own edit (a `Modified` event) can't loop.
pub fn harmonize_decor_materials(
    mut events: MessageReader<AssetEvent<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    for ev in events.read() {
        let AssetEvent::Added { id } = ev else {
            continue;
        };
        let Some(mut mat) = materials.get_mut(*id) else {
            continue;
        };
        let lin = mat.base_color.to_linear();
        for (src, dst) in RETINTS {
            let close = (lin.red - src[0]).abs() < 0.02
                && (lin.green - src[1]).abs() < 0.02
                && (lin.blue - src[2]).abs() < 0.02;
            if close {
                mat.base_color = Color::srgb(dst[0], dst[1], dst[2]);
            }
        }
    }
}
