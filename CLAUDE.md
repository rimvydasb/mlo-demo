# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Toolchain Warning

Homebrew installs `rustc` 1.92 which shadows rustup's 1.96. `.cargo/config.toml` pins both `rustc` and `rustdoc` to the
rustup path — all `cargo` commands in the project root automatically use the correct compiler. Never prefix commands
with `RUSTC=...`; the config handles it. If you add a new developer machine, update the absolute paths in
`.cargo/config.toml`.

## Common Commands

```bash
# Run the interactive inspector
cargo run -p native -- inspect --seed 42

# Headless PNG screenshot (opens window briefly, then exits)
cargo xtask screenshot --seed 42 --out shot.png

# ASCII top-down map dump (stdout, no window)
cargo xtask dump --seed 42
cargo xtask dump --seed 42 --row 2 --col 3   # specific biome

# Determinism check
cargo xtask check --seed 42

# Tests
cargo test -p voxel-core -p voxel-mapgen     # all pure-crate tests (fast)
cargo test -p voxel-mapgen --test invariants  # integration + proptest suite only

# Update insta snapshots after intentional mapgen changes
INSTA_UPDATE=always cargo test -p voxel-mapgen --test invariants

# WASM compile check
cargo check -p web --target wasm32-unknown-unknown

# Format Markdown files (Prettier is configured for Markdown only; .rs files use rustfmt)
npx prettier --write "**/*.md"
npx prettier --check "**/*.md"
```

## Architecture

The workspace has a strict dependency layering:

```
voxel-core   (types only, no engine, no std-heavy deps)
    └── voxel-mapgen   (pure mapgen, no Bevy)
            └── xtask  (host-only automation, no Bevy)
    └── voxel-render   (Bevy mesh/camera plugin)
            └── voxel-app  (Bevy app + egui inspector)
                    ├── platforms/native  (clap CLI entry)
                    └── platforms/web     (wasm-bindgen entry)
```

`core` and `mapgen` must stay engine-free — they compile to wasm and are used headlessly in xtask. Any Bevy import
belongs in `render` or `app`.

## Coordinate Systems

There are two coordinate spaces that must not be confused:

- **Voxel space**: `(vx, vy, vz)` where Z is the vertical axis (z=0 is underground layer 1, z=5 is surface, z=6–11 is
  relief/above-ground)
- **World/Bevy space**: voxel `(vx, vy, vz)` maps to Bevy `(vx, vz, vy)` — voxel-Z becomes Bevy-Y (up). This mapping is
  applied in `mesh.rs` when building vertex positions.

Biome coordinates: `BiomeCoord { row, col }` where row=0 is north, col=0 is west, in a 6×6 grid.

## Mapgen: Two-Pass Generation

`mapgen::generate(seed)` runs two sequential passes:

1. **Macro pass** (`macro_pass.rs`): assigns `BiomeType` to each of the 36 biomes via weighted random, then computes
   `Connection` compatibility for all shared edges (compatible = same biome type on both sides).

2. **Interior pass** (`interior.rs`): fills each biome's `VoxelGrid` (12×12×12). Uses world-space coordinates
   (`world_x = col*12 + local_x`) for all noise sampling so biomes tile seamlessly. Layers:
   - z=0–4: `Material::Underground` + element placement via Perlin noise thresholds (elements get rarer toward surface)
   - z=5: `Material::Ground(biome_type)`, always solid, no element (surface layer)
   - z=6–11: `Material::Ground` or `Air` based on height noise; Water biomes and edge voxels stay flat

RNG is `ChaCha8Rng::seed_from_u64(seed)` (macro pass only). Interior uses deterministic Perlin noise seeded from the
world seed via `derive_u32(seed, offset)`, so order of biome generation does not affect results.

**Never use `HashMap` for deterministic paths** — iteration order is non-deterministic. Use `BTreeMap` (already used for
`WorldMap::connections`).

## Rendering: Multi-Mesh Approach

`StandardMaterial::vertex_colors` was removed in Bevy 0.19. Instead, `render/src/mesh.rs::build_voxel_meshes()` returns
`Vec<(Mesh, [f32; 3])>` — one mesh per unique voxel color. Each is spawned as a separate entity with its own
`StandardMaterial { base_color: Color::srgb(r, g, b), .. }`. This means rebuilding the scene despawns and respawns
O(N_colors) entities, tracked in `SceneEntities.biome_meshes: Vec<Entity>`.

`rebuild_scene` in `app/src/lib.rs` only runs when `WorldMapResource`, `FocusedBiome`, or `LayerCutoff` are changed
(Bevy change detection).

## Bevy 0.19 Critical API Differences

These differ from most online examples (which target Bevy 0.14–0.15):

- `EventReader<T>` → `MessageReader<T>` from `bevy::ecs::message::MessageReader`; `EventWriter<T>` → `MessageWriter<T>`
- `AmbientLight` is a **Component** (not a Resource): `commands.spawn(AmbientLight { .. })`
- `ScalingMode` is at `bevy::camera::ScalingMode` (not `bevy::render::camera::ScalingMode`)
- `RenderAssetUsages` is at `bevy::asset::RenderAssetUsages`
- `OrthographicProjection`: use `Projection::from(OrthographicProjection { .. })`, not `Projection::Orthographic(..)`
- `Query::get_single()` → `query.single()` (returns `Result`); `get_single_mut()` → `single_mut()` or
  `iter_mut().next()`
- `WindowResolution::new(u32, u32)` — `From<(f32,f32)>` is gone
- `DirectionalLight.shadows_enabled` → `shadow_maps_enabled`
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

The mapgen invariant suite in `crates/mapgen/tests/invariants.rs` enforces:

- Underground (z=0–4): `Material::Underground`, elements allowed
- Surface (z=5): `Material::Ground(*)`, no element
- Relief (z=6–11): `Material::Air` or `Material::Ground(*)`, no elements, no floating voxels
- Edge continuity: compatible biome borders have matching surface material at the shared edge strip (z=5)
- Determinism: same seed → byte-identical grids

Snapshots live in `crates/mapgen/tests/snapshots/`. Regenerate with `INSTA_UPDATE=always`.
