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
cargo xtask screenshot --seed 42 --no-clouds --no-decor --out shot.png  # byte-stable (clouds + decor animations are non-deterministic)
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

- **Cell** (`voxel-core::CellType`, `CellGrid`): the logical/gameplay unit. 8×8×12 per biome (`CELLS_XY = 8` footprint,
  `CELLS_Z = 12` layers). Types: Air, Soil, Sand, Water, Stone, Gold, Iron. A cell _is_ its resource
  (`CellType::resource()`) — there is no separate element field. `mapgen` and future `sim` code operate on cells only.
- **Voxel** (`voxel-render::VoxelKind`, `VoxelVolume`): the cosmetic visual sub-unit. Each cell expands to 4×4×4 voxels
  at render time (32×32×48 per biome, `VOX_XY`/`VOX_Z`) via `expansion.rs::cell_column` — grass tops on exposed soil
  (snow tops in Winter biomes), sunken water surfaces, snow caps on tall stone (`SNOW_Z`; any exposed stone in Winter),
  then the `beautify.rs` passes (SLOPES chamfers toward lower neighbors — **interior only, never on the biome's outer
  edge ring**, the rim stays a crisp cliff; CLIFF FRACTURES on exposed faces; optional MICROHEIGHT; RETOP regrows
  grass/snow on carved tops; GRASS OVERHANG drapes a green rim — snow rim in Winter — down cliff sides), plus
  deterministic underside erosion for the floating-island look. `expand()` takes the `BiomeType` for the winter
  dressing. Every random choice is `rule_hash01` (world voxel coords + per-rule discriminator). Voxels never leak out of
  `render`.

Snow is a `VoxelKind` only, **not** a `CellType`. `BiomeType`s: Grass, Sand, Water, Winter (Winter replaced Rock: soil
surface like Grass but snow-dressed, snow-pine flora, penguin/polar fauna). See the biome-type table in
docs/rendering.md.

## Fauna & Flora Decorations

Decoration props (trees, palms, flowers, grass props, animals, fish — all cosmetic GLB models from Kenney packs) are
split across three tiers (see `docs/rendering-fauna-flora.md`): `render/src/decor.rs` holds the catalog + the pure
deterministic placement planner (`plan_decor`, cell-tier, unit-tested); `app/src/assets.rs` owns asset management
(workspace `assets/` root via `asset_plugin()`, `DecorAssets` handles, palette harmonization of Kenney's teal foliage);
`app/src/decor.rs` spawns/despawns instances with the scene rebuild and drives hop/swim transform animations. Runtime
assets live in workspace-root `assets/models/{fauna,flora}/` — **never reference `imports/` at runtime**; it holds the
raw source packs only. Kenney GLBs may reference `Textures/colormap.png` by relative URI (cube-pets and platformer-kit
do) — copy the texture along with the GLB or the model loads invisible. Bevy 0.19 loads glTF scenes as `WorldAsset`
spawned via `WorldAssetRoot` (not `Scene`/ `SceneRoot`). Placement invariants: everything except fish requires a **flat
top** (no lateral neighbor column lower — exactly the cells slopes/fractures carve; props there would float); palms on
sand only; trees on soil (interior + local-flat; Winter biomes plant only the snow-tree variants and skip flowers/grass
props); fish submerged in surface water; animals never over water (Winter soil uses the penguin/polar set). `plan_decor`
takes the `BiomeType`. Grass tops are guaranteed flat (GRASS TUFTS removed — FLAT TOPS contract) and the underground has
no single-voxel air holes (pinhole seal) plus 0–2 hidden cave pockets per biome (`expansion.rs`).

## Coordinate Systems

- **Cell space**: `(x, y, z)` with Z up; x, y < 8 (`CELLS_XY`), z < 12 (`CELLS_Z`); z=0 is the deepest underground layer
  (cell layer 1), z=5 (`SURFACE_Z`) is the surface, z=6–11 is relief. Cell layers in docs/UI are 1-based (layer = z +
  1).
- **World/Bevy space**: 1 cell = 1.0 world unit, 1 voxel = 0.25. Cell/voxel `(x, y, z)` maps to Bevy `(x, z, y)` —
  grid-Z becomes Bevy-Y (up). Applied in `mesh.rs` (vertex emit) and inverted in `app/src/picking.rs` (cursor ray → cell
  DDA). The axis swap flips winding handedness — the face table in `mesh.rs` is ordered so emitted world-space triangles
  are CCW; keep backface culling in mind if you touch it.
- **Biome coordinates**: `BiomeCoord { row, col }`, row 0 = north, col 0 = west, 6×6 grid.

## Mapgen: Three-Pass Generation

`mapgen::generate(seed)` runs three sequential passes:

1. **Macro pass** (`macro_pass.rs`): weighted-random `BiomeType` per biome, then `Connection` compatibility for all
   shared edges (compatible = same biome type on both sides).

2. **Interior pass** (`interior.rs`): fills each biome's 8×8×12 `CellGrid` using world-space noise coordinates
   (`world_x = col*8 + x`) so fields tile seamlessly:
   - z=0–4: floating-island **funnel** — fixed centered nested rectangles, top-down 6×6 → 5×4 → 4×3 → 3×2 → 2×1
     (`FUNNEL` table; no noise on the footprint), with deposits — stone ≥30%, iron ~10%, gold ~5% of solid underground
     cells.
   - z=5: surface, always solid, typed by biome — the widest band (8×8); interior ponds in grass/sand/winter biomes;
     interior sand **islands** in water biomes; the one-cell edge ring is always the pure biome type (edge continuity
     depends on this).
   - z=6–11: relief = rolling hills + sparse high-frequency mountain peaks; fades flat within 2 cells of a border; water
     biomes (islands included) and pond columns stay flat; columns ≥4 high become stone.

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
(with AO-driven quad diagonal flips). Scene = 2 mesh entities for the focused biome (at the origin) + 35 grey
**fog-of-war cell shells** (`build_cell_shell_mesh`: one grey unit cube per non-air cell, silhouette only, no voxel
detail/decor) placed a footprint+1 = 9 units apart (`proxy_world_offset` — exactly one cell of air between biomes),
tracked in `SceneEntities` and rebuilt by `rebuild_scene` in `app/src/lib.rs` only when `WorldMapResource`,
`FocusedBiome`, or `LayerCutoff` change (Bevy change detection). Material handles are cached in `TerrainMaterials`
(opaque/water/grey proxy) — meshes change on rebuild, materials never do. The camera is a natural **perspective**
projection (FOV ≈ 45°, dolly zoom); default pitch clears the 12-tall neighbor shells.

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
