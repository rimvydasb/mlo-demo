# Rendering Specification

A god-game / early-RTS in the spirit of _Mega-Lo-Mania_ (Sensible Software, 1991), rendered as voxel biomes. Target:
WebAssembly + wgpu (web). Future: iOS. Development is LLM-assisted (Claude CLI) with the user as architect.

This document describes the **implemented** Phase 1 terrain renderer (see `crates/render`, `crates/mapgen`) plus the
agreed design direction for later phases. Sections marked _future scope_ are not implemented yet.

---

## Reference Art

Three concept images live in `docs/reference/`. `sample1.jpg` and `sample3.jpg` are identical (kept as a duplicate
reference); `sample2.jpg` is a distinct piece. Together they set the visual target for a focused biome:

- **`sample1.jpg` / `sample3.jpg`** — smooth, low-poly diorama: snow-capped peaks, pine trees, a sunken pool, a cabin,
  all sitting on a soil block with a clean dirt underside.
- **`sample2.jpg`** — full cubic-voxel diorama (Minecraft-style): blocky terrain built from visible unit cubes and —
  notably — an eroded, jagged underside where the dirt breaks apart into loose gray stone chunks.

**Style decision (resolved):** **terrain** (ground, relief, water) is true cell-voxel geometry — axis-aligned unit cubes
per `sample2`'s language, produced by the `cell → 4×4×4 voxel` expansion. **Props** (trees, bushes, buildings) will be
imported 3D models placed on top of the terrain — _future scope_, see "Decoration Props" below. The floating island's
jagged underside comes from procedural voxel erosion, per `sample2`.

---

## Terminology

- **Biome** — a 12×12×12 grid of cells, the atomic unit of the world. Biomes are arranged in a 6×6 grid.
- **World** — the 6×6 grid of biomes. Each match happens in one world.
- **Era** — a technological age assigned to each biome. Biomes can be in different eras at the same time.
- **Cell** — the logical unit of gameplay: one type (soil, sand, water, stone, gold, iron, air), holds at most one
  resource, mined by one miner, built on by one building. 12³ cells per biome. Lives in `voxel-core::CellType` /
  `CellGrid`.
- **Voxel** or **Terrain voxel** — the visual sub-unit a cell expands into at render time. Purely cosmetic; no gameplay
  semantics. 4³ voxels per cell. Lives in `voxel-render::VoxelKind` / `VoxelVolume`; `mapgen` and `sim` never see
  voxels.

A biome is therefore **12³ = 1,728 cells logically** and **48³ = 110,592 voxels visually**.

## Gameplay Concept

You are one of four competing deities. You seize biomes on a 6×6 world, settle towers, allocate population to research /
mining / production / armies / defence, advance your civilization through technological ages, and conquer by destroying
every rival tower. You never micro individual units; you allocate population and direct armies at target biomes. Combat
auto-resolves.

### Design pillars (inherited from MLM)

1. **Allocation over micro.** The decision is _how many people on which task_, not clicking soldiers.
2. **Expansion vs. advancement tension.** More towers build armies faster; one strong tower researches faster.
3. **No fog of micromanagement.** Combat, breeding, and mining run themselves once allocated.
4. **Resource-gated tech.** Designs cost elements; elements come from biomes. Where you settle decides what you can
   build.

---

## World Model

### Cell Types (`voxel-core::CellType`)

A cell **is** its resource where one exists — there is no separate element field.

| Cell Type | Band                  | Resource | Notes                                          |
| --------- | --------------------- | -------- | ---------------------------------------------- |
| soil      | Any                   | —        | Dirt; grows a grass top voxel layer if exposed |
| sand      | Surface, relief       | —        | Beach yellow                                   |
| water     | Surface               | water    | Translucent, sunken surface                    |
| stone     | Any                   | stone    | Gray rock; snow-capped in the high relief band |
| gold      | Underground           | gold     | Yellow stone                                   |
| iron      | Underground           | iron     | Reddish stone                                  |
| air       | Underground\*, relief | —        | \*Underground air = outside the island funnel  |

**Snow is not a cell type.** It is a cosmetic `VoxelKind` applied by the expansion to the top voxel layer of exposed
stone cells at cell layer ≥ 11 (`voxel-core::SNOW_Z`).

### Biome anatomy (Z axis in cells, layer 1 = bottom, `z` = layer − 1)

| Layers | Band        | Contents                                                                                                   |
| ------ | ----------- | ---------------------------------------------------------------------------------------------------------- |
| 1–5    | Underground | Floating-island funnel of soil salted with resource deposits. Tapers toward the bottom tip.                |
| 6      | Surface     | Always solid, typed by the biome. Interior ponds in grass/sand biomes. Edge strips are flat and typed.     |
| 7–12   | Above       | Relief: rolling hills + sparse mountain peaks, otherwise air. Cosmetic + line-of-sight flavor; no gameplay |
|        |             | collision in the demo.                                                                                     |

### Connections

- Adjacent biomes connect **only at cell layer 6**, by **surface-type match** (water↔water, sand↔sand, grass↔grass,
  stone↔stone). Incompatible neighbors have **no connection**.
- The one-cell edge ring of every biome is always the biome's own surface type and carries **no relief and no ponds**,
  so compatible borders match cell-for-cell and movement across them is clean. This is enforced by the invariant tests.
- **Gameplay role (proposed, tunable):** edge type gates which armies may cross; water connections require naval- or
  air-capable units (a later tech tier).

---

## Procedural Generation (`mapgen` crate)

`mapgen` operates exclusively on cells and is engine-free (compiles to wasm, runs headless in CI).

`generate(seed) -> WorldMap` runs two passes:

1. **Macro pass** (`macro_pass.rs`) — weighted-random `BiomeType` per biome (grass 40 / sand 25 / water 20 / rock 15)
   using `ChaCha8Rng::seed_from_u64(seed)`, then a `Connection` for every internal border (compatible = same type on
   both sides). Connections live in a `BTreeMap` — **never `HashMap` on deterministic paths** (iteration order).

2. **Interior pass** (`interior.rs`) — fills each biome's 12³ `CellGrid`:
   - **Underground (z 0–4):** the island funnel. Walked top-down per column: the layer under the surface is always full
     (the surface always has support), deeper layers keep a shrinking, noise-perturbed footprint, and the first cut
     truncates everything below it — so underground mass always hangs from the layer above. Cells inside the funnel are
     soil salted with deposits: **stone ≥ 30%** (denser toward the bottom, so the underside reads as rubble), **iron
     ~10%** (z ≤ 3), **gold ~5%** (z ≤ 2). Rarities are enforced by a loose-bounds invariant test and measurable via
     `cargo run -p voxel-mapgen --example stats`.
   - **Surface (z 5):** the biome's surface cell everywhere; grass/sand biomes get interior **ponds** carved where a
     pond noise field exceeds a threshold (never on the edge ring).
   - **Relief (z 6–11):** column heights = rolling-hills field **plus** a sparse mountain-peak field (higher frequency
     than the hills — with only 6 relief layers, peaks must stay a few cells wide or they clip into flat-topped mesas).
     Heights fade to zero over the three cells nearest a biome edge. Water biomes and pond cells stay flat. Tall columns
     (≥ 4) are bare stone (mountains); low relief keeps the biome's surface material; rock biomes are stone throughout.
     No floating cells by construction.

### Determinism contract

- All interior noise is Perlin, seeded from the world seed via `derive_u32(seed, offset)` and sampled in **world cell
  coordinates** (`biome_col * 12 + local_x`), so fields are continuous across borders and biome generation order can
  never affect results. The ChaCha RNG feeds the macro pass only.
- **Tests** (`crates/mapgen/tests/invariants.rs`, headless): same-seed byte-equality, per-band cell-type constraints,
  underground hangs-from-above, edge-ring purity, compatible-border strip equality, resource rarity bounds, proptest
  over random seeds, and insta ASCII snapshots. Regenerate snapshots after intentional changes with
  `INSTA_UPDATE=always`.

---

## Cell → Voxel Expansion (`render/src/expansion.rs`)

The expansion is a pure function of the cell grid, the inspector's layer cutoff, and the biome's world position + seed.
`cell_column(cell, top_air, cz)` gives each cell's 4 voxel sub-layers (also rendered as swatches in the inspector's
"Cell → voxel patterns" panel):

**Top Z == air** means the cell above is air (or peeled away by the layer cutoff — peeled soil regrows a grass top,
exactly like natural terrain).

| Cell Type | Rule                     | Voxel column (bottom → top)           |
| --------- | ------------------------ | ------------------------------------- |
| soil      | Top Z == air             | dirt, dirt, dirt, **grass**           |
| soil      | Top Z != air             | dirt ×4                               |
| sand      | Top Z == air             | sand ×3, **air** (sunken)             |
| sand      | Top Z != air             | sand ×4                               |
| water     | Top Z == air             | water ×2, **air ×2** (sunken surface) |
| water     | Top Z != air             | water ×4                              |
| stone     | Top Z == air and cz ≥ 10 | stone ×3, **snow**                    |
| stone     | otherwise                | stone ×4                              |
| gold      |                          | gold ×4                               |
| iron      |                          | iron ×4                               |
| air       |                          | (no voxels)                           |

### Underside erosion (the "floating island" break)

After expansion, two erosion passes nibble voxels off the exposed underground boundary (side/bottom exposure only —
peeled top layers stay crisp), with probability rising toward the island's bottom tip. Combined with the cell-tier
funnel this turns clean cell-sized steps into the ragged rubble underside of `sample2`. Deterministic per world voxel
coordinate + seed (`voxel_hash01`), so screenshots are stable. This is render-side cosmetics: it never touches `mapgen`
data or its invariant tests.

---

## Meshing (`render/src/mesh.rs`)

- **One opaque mesh + one translucent water mesh per biome** — not one mesh per color. Bevy 0.19's `StandardMaterial`
  consumes `Mesh::ATTRIBUTE_COLOR` (vertex colors, linear space) automatically, so per-voxel color lives in the mesh and
  both materials are shared white handles (`terrain_material()`, `water_material()`).
- Naive face culling against opaque neighbors; water faces render only against air. Faces are wound CCW for default
  backface culling (the voxel→world axis swap flips handedness — the face table accounts for it).
- **Baked ambient occlusion:** classic 3-neighbour corner AO per vertex, four levels (1.0 / 0.82 / 0.66 / 0.5),
  multiplied into vertex color; quad diagonals flip to interpolate AO smoothly. Water skips AO.
- **Per-voxel jitter:** deterministic hash of world voxel coords scales value per material (grass patchiest, snow most
  uniform) — this is what makes flat color fields read as living voxel art.
- Coordinates: cell = 1.0 world unit, voxel = 0.25. Voxel `(x, y, z)` → Bevy world `(x, z, y)` — voxel-Z is world-up.

### Material palette (sRGB, authored against the reference art)

| VoxelKind | sRGB               | Jitter |
| --------- | ------------------ | ------ |
| grass     | (0.36, 0.70, 0.22) | 0.13   |
| dirt      | (0.56, 0.36, 0.22) | 0.10   |
| sand      | (0.89, 0.80, 0.55) | 0.06   |
| water     | (0.16, 0.52, 0.80) | 0.04   |
| stone     | (0.37, 0.37, 0.39) | 0.14   |
| gold      | (0.90, 0.73, 0.22) | 0.18   |
| iron      | (0.68, 0.39, 0.30) | 0.14   |
| snow      | (0.94, 0.96, 0.99) | 0.03   |

Water material: white base at alpha 0.72, `AlphaMode::Blend`, low roughness for a specular sheen.

---

## Lighting & Atmosphere (`render/src/lib.rs`)

- **Key sun** — warm directional light, low-ish and lateral so relief casts readable shadows across the surface,
  `shadow_maps_enabled: true` with a single tight shadow cascade (crisp on the focused biome; the island's shadow never
  lands on far proxy tiles).
- **Cool fill** — a second, shadowless directional from the default camera side lifts the south/west faces the viewer
  actually sees, keeping baked AO readable instead of crushing to black.
- **Ambient** — sky-tinted `AmbientLight` (a component in Bevy 0.19, not a resource).
- **Fog** — `DistanceFog` on the camera, linear falloff starting just past the focused biome's depth range, fading the
  proxy ring into the sky color. `ClearColor` and fog share `SKY_COLOR`.

## Focused Biome Presentation

- The focused biome renders in full voxel detail at the world origin.
- The other 35 biomes render as **dimmed, lit proxy slabs** floating at the focused biome's surface altitude (spacing 15
  world units), so the world reads as an archipelago of floating islands receding into fog.
- The inspector boots focused on the seed's **most scenic biome** (deterministic score: relief drama + pond bonus —
  `voxel-app::scenic_biome`), so `cargo run -p native -- inspect --seed N` always opens on a good composition.
  `cargo xtask screenshot` accepts `--row/--col` to frame any specific biome instead.

## Inspector (`app` crate)

- **Travel:** orbit / pan / zoom orthographic camera; click the 6×6 grid, or Tab / arrow keys.
- **Inspect:** hover resolves to a **cell** (not a voxel) via an Amanatides–Woo DDA through the 12³ grid; the panel
  shows local/world coords, cell layer, band, type, and resource.
- **Layer peel:** slider hides cell layers z ≥ cutoff; peeled soil grows grass tops so cross-sections stay readable.
- **Expansion preview:** the "Cell → voxel patterns" panel draws every cell type's exposed/covered voxel columns from
  the same `cell_column` function the mesher uses.
- **Exports:** ASCII dump (macro map + surface + relief heights + all underground layers) and PNG screenshot.

## Decoration Props (imported 3D assets, _future scope_)

Trees, bushes, and era-driven buildings are **imported 3D models (glTF/GLB), not composed from cell voxels** — curved
roofs and rounded canopies rule out voxel composition. They will be placed on top of the voxel terrain by `render`/`app`
via Bevy's `AssetServer`, separate from the procedural mesher. Whether placement is seeded by biome coordinate or purely
cosmetic is an open question. None of this is required for Phase 1 terrain rendering; documented so the prop pipeline
has a landing spot.

Stretch goals from the reference art, also future scope: pockmark detail on rock faces, lighter rim/foam tone at water
edges, and the water-edge "vortex" slope transition from the original spec's Phase III.
