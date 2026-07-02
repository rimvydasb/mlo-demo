# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Toolchain Warning

Homebrew installs `rustc` 1.92 which shadows rustup's 1.96. `.cargo/config.toml` pins both `rustc` and `rustdoc` to the
rustup path — all `cargo` commands in the project root automatically use the correct compiler. Never prefix commands
with `RUSTC=...`; the config handles it. If you add a new developer machine, update the absolute paths in
`.cargo/config.toml`.

## Common Commands

```bash
# Run the interactive inspector (boots on the seed's most scenic biome)
cargo run -p native -- inspect --seed 42

# Headless PNG screenshot (opens window briefly, then exits)
cargo xtask screenshot --seed 42 --out shot.png
cargo xtask screenshot --seed 42 --row 3 --col 5 --out shot.png   # specific biome
cargo xtask screenshot --seed 42 --no-clouds --out shot.png       # byte-stable (clouds are non-deterministic)
cargo xtask screenshot --seed 42 --microheight --out shot.png     # A/B the optional MICROHEIGHT rule

# ASCII top-down map dump (stdout, no window)
cargo xtask dump --seed 42
cargo xtask dump --seed 42 --row 2 --col 3   # specific biome

# Determinism check
cargo xtask check --seed 42

# Cell-type distribution stats (for tuning deposit rarities)
cargo run -p voxel-mapgen --example stats

# Tests
cargo test -p voxel-core -p voxel-mapgen -p voxel-render   # all pure-crate tests (fast)
cargo test -p voxel-mapgen --test invariants               # integration + proptest suite only

# Update insta snapshots after intentional mapgen changes
INSTA_UPDATE=always cargo test -p voxel-mapgen --test invariants

# WASM compile check
cargo check -p web --target wasm32-unknown-unknown

# Format Markdown files (Prettier is configured for Markdown only; .rs files use rustfmt / cargo fmt)
npx prettier --write "**/*.md"
npx prettier --check "**/*.md"
```

## Architecture

The workspace has a strict dependency layering:

```
voxel-core   (cell-tier types only, no engine, no std-heavy deps)
    └── voxel-mapgen   (pure mapgen on cells, no Bevy)
            └── xtask  (host-only automation, no Bevy)
    └── voxel-render   (Bevy: cell→voxel expansion, meshing, camera, lighting)
            └── voxel-app  (Bevy app + egui inspector, picking, scene rebuild)
                    ├── platforms/native  (clap CLI entry)
                    └── platforms/web     (wasm-bindgen entry)
```

`core` and `mapgen` must stay engine-free — they compile to wasm and are used headlessly in xtask. Any Bevy import
belongs in `render` or `app`.

## Two-Tier Grid: Cells vs Voxels

This is the load-bearing separation of the whole codebase (see `docs/rendering.md`):

- **Cell** (`voxel-core::CellType`, `CellGrid`): the logical/gameplay unit. 12³ per biome. Types: Air, Soil, Sand,
  Water, Stone, Gold, Iron. A cell _is_ its resource (`CellType::resource()`) — there is no separate element field.
  `mapgen` and future `sim` code operate on cells only.
- **Voxel** (`voxel-render::VoxelKind`, `VoxelVolume`): the cosmetic visual sub-unit. Each cell expands to 4×4×4 voxels
  at render time (48³ per biome) via `expansion.rs::cell_column` — grass tops on exposed soil, sunken water surfaces,
  snow caps on tall stone (`SNOW_Z`), then the `beautify.rs` passes (SLOPES chamfers toward lower neighbors, CLIFF
  FRACTURES on exposed faces, optional MICROHEIGHT, RETOP regrows grass/snow on carved tops, GRASS OVERHANG drapes a
  green rim down cliff sides, GRASS TUFTS at ~8% coverage), plus deterministic underside erosion for the floating-island
  look. Every random choice is `rule_hash01` (world voxel coords + per-rule discriminator). Voxels never leak out of
  `render`.

Snow is a `VoxelKind` only, **not** a `CellType`.

## Coordinate Systems

- **Cell space**: `(x, y, z)` with Z up; z=0 is the deepest underground layer (cell layer 1), z=5 (`SURFACE_Z`) is the
  surface, z=6–11 is relief. Cell layers in docs/UI are 1-based (layer = z + 1).
- **World/Bevy space**: 1 cell = 1.0 world unit, 1 voxel = 0.25. Cell/voxel `(x, y, z)` maps to Bevy `(x, z, y)` —
  grid-Z becomes Bevy-Y (up). Applied in `mesh.rs` (vertex emit) and inverted in `app/src/picking.rs` (cursor ray → cell
  DDA). The axis swap flips winding handedness — the face table in `mesh.rs` is ordered so emitted world-space triangles
  are CCW; keep backface culling in mind if you touch it.
- **Biome coordinates**: `BiomeCoord { row, col }`, row 0 = north, col 0 = west, 6×6 grid.

## Mapgen: Three-Pass Generation

`mapgen::generate(seed)` runs three sequential passes:

1. **Macro pass** (`macro_pass.rs`): weighted-random `BiomeType` per biome, then `Connection` compatibility for all
   shared edges (compatible = same biome type on both sides).

2. **Interior pass** (`interior.rs`): fills each biome's 12³ `CellGrid` using world-space noise coordinates
   (`world_x = col*12 + x`) so fields tile seamlessly:
   - z=0–4: floating-island **funnel** (walked top-down so mass always hangs from the layer above; the layer under the
     surface is always full) with deposits — stone ≥30%, iron ~10%, gold ~5% of solid underground cells.
   - z=5: surface, always solid, typed by biome; interior ponds in grass/sand biomes; the one-cell edge ring is always
     the pure biome type (edge continuity depends on this).
   - z=6–11: relief = rolling hills + sparse high-frequency mountain peaks; fades flat within 3 cells of a border; water
     biomes and pond columns stay flat; columns ≥4 high become stone.

3. **Beach pass** (`beach_pass.rs`): in grass biomes, flips soil surface cells to sand around large ponds (≥6 cells,
   4-connected) — 92% at 1 step from water, 35% at 2 steps. Never on the edge ring, never under relief columns. This is
   a cell-type change (mining/connections see sand), which is why it lives in mapgen, not render.

RNG is `ChaCha8Rng::seed_from_u64(seed)` (macro pass only). Interior uses Perlin noise seeded via
`derive_u32(seed, offset)`; the beach pass uses a deterministic hash of world cell coords. Biome generation order can
never affect results.

**Never use `HashMap` for deterministic paths** — iteration order is non-deterministic. Use `BTreeMap` (already used for
`WorldMap::connections`).

## Rendering: Vertex-Colored Meshes

**Mesh vertex colors work in Bevy 0.19**: insert `Mesh::ATTRIBUTE_COLOR` (Float32x4, **linear** color space) and
`StandardMaterial` picks it up automatically (shader def `VERTEX_COLORS`). Do not build one mesh per color.

`render/src/mesh.rs::build_biome_meshes()` returns one **opaque mesh** (all solid voxels, shared white
`terrain_material()`) and one **translucent water mesh** (`water_material()`, alpha blend). Baked into vertex colors:
per-voxel value jitter (deterministic `voxel_hash01` of world voxel coords) and classic 3-neighbour ambient occlusion
(with AO-driven quad diagonal flips). Scene = 2 mesh entities per focused biome + 35 proxy slabs, tracked in
`SceneEntities` and rebuilt by `rebuild_scene` in `app/src/lib.rs` only when `WorldMapResource`, `FocusedBiome`, or
`LayerCutoff` change (Bevy change detection). Material handles are cached in `TerrainMaterials` — meshes change on
rebuild, materials never do.

Lighting lives in `render/src/lib.rs::spawn_lights`: warm key sun (shadow maps on, single tight cascade) + cool
shadowless fill from the camera side + ambient; `DistanceFog` on the camera fades proxies into `SKY_COLOR`. If you
retune the palette (`base_color_srgb`), remember the tone mapper compresses grays — verify with a screenshot, not by
eyeballing sRGB values.

## Bevy 0.19 Critical API Differences

These differ from most online examples (which target Bevy 0.14–0.15):

- `EventReader<T>` → `MessageReader<T>` from `bevy::ecs::message::MessageReader`; `EventWriter<T>` → `MessageWriter<T>`
- `AmbientLight` is a **Component** (not a Resource): `commands.spawn(AmbientLight { .. })`
- `ScalingMode` is at `bevy::camera::ScalingMode` (not `bevy::render::camera::ScalingMode`)
- `RenderAssetUsages` is at `bevy::asset::RenderAssetUsages`
- `CascadeShadowConfigBuilder` is at `bevy::light::CascadeShadowConfigBuilder`; spawn `.build()` next to the light
- `OrthographicProjection`: use `Projection::from(OrthographicProjection { .. })`, not `Projection::Orthographic(..)`
- `Query::get_single()` → `query.single()` (returns `Result`); `get_single_mut()` → `single_mut()` or
  `iter_mut().next()`
- `WindowResolution::new(u32, u32)` — `From<(f32,f32)>` is gone
- `DirectionalLight.shadows_enabled` → `shadow_maps_enabled`
- Vertex colors: no `StandardMaterial` toggle — presence of `Mesh::ATTRIBUTE_COLOR` enables them
- Screenshots: `commands.spawn(Screenshot::primary_window()).observe(save_to_disk(path))`

## bevy_egui 0.40 Critical API Differences

- `EguiPlugin` → `EguiPlugin::default()`
- egui systems **must** run in `EguiPrimaryContextPass` schedule, not `Update`
- `EguiContexts::ctx_mut()` returns `Result<&mut Context, _>` — always `.unwrap()` or use `?` (the egui system returns
  `Result`)
- Panels: create a `viewport_ui` from `ctx.viewport_rect()`, then
  `egui::Panel::left("id").show_inside(&mut viewport_ui, ...)` — do **not** call `Panel::show(ctx, ...)`
- `ctx.wants_pointer_input()` → `ctx.egui_wants_pointer_input()`

## WASM

The `web` crate needs `getrandom = { version = "0.2", features = ["js"] }` as an explicit wasm-target dependency because
`rand 0.8 → rand_core 0.6 → getrandom 0.2` pulls in getrandom 0.2 which requires the `js` feature for
`wasm32-unknown-unknown`. This is already in `platforms/web/Cargo.toml`.

## Invariant Tests

The mapgen invariant suite in `crates/mapgen/tests/invariants.rs` enforces (all on the **cell tier**):

- Underground (z=0–4): only Air/Soil/Stone/Gold/Iron; every solid cell has a solid cell directly above (funnel hangs
  from the surface)
- Surface (z=5): never Air; edge ring is exactly the biome's surface type; interior is surface type, (grass/sand only)
  pond water, or (grass only) beach sand
- Relief (z=6–11): only Air/Soil/Sand/Stone; no resources; no floating cells; flat on the edge ring and above water
- Edge continuity: compatible borders match cell-for-cell along the shared layer-6 strip
- Beach belt: sand in grass biomes always sits within 2 steps of surface water, never on the edge ring
- Resource rarity: stone ≥30%, iron/gold within loose bounds around 10%/5%
- Determinism: same seed → byte-identical grids (plus proptest over random seeds)

Snapshots live in `crates/mapgen/tests/snapshots/`. Regenerate with `INSTA_UPDATE=always`. The render crate has its own
unit tests for the expansion rules (grass tops, sunken water, snow caps, expansion determinism) and the beautification
passes (slope chamfers, grass overhang, tuft coverage bounds, microheight flag, and a no-floating-voxels check above the
erosion band over a real generated map).
