# Rendering Specification

A god-game / early-RTS in the spirit of _Mega-Lo-Mania_ (Sensible Software, 1991), rendered as voxel biomes. Target:
WebAssembly + wgpu (web). Future: iOS. Development is LLM-assisted (Claude CLI) with the user as architect.

---

## Reference Art

Three concept images live in `docs/reference/`. `sample1.jpg` and `sample3.jpg` are identical (kept as a duplicate
reference); `sample2.jpg` is a distinct piece. Together they set the visual target for a focused biome.

- **`sample1.jpg` / `sample3.jpg`** — smooth, low-poly diorama: three snow-capped peaks with hard-edged faceted shading,
  rounded cone pine trees, a sunken circular pool with a visible "spring" hole, a single wooden cabin with a curved
  shingle roof, all sitting on a soil block with a clean vertical dirt underside.
- **`sample2.jpg`** — full cubic-voxel diorama (Minecraft-style): blocky pine trees and bushes built from visible unit
  cubes, a sandy clearing with a timber-frame building, a river of flat-shaded blue water, and — notably — an eroded,
  jagged underside where the dirt layer breaks apart into loose gray stone chunks instead of ending in a flat plane.

**Style decision (resolved):** the two directions are not the same renderer, and this project uses both, split by role.
**Terrain** (ground, relief, water) stays true cell-voxel geometry — axis-aligned unit cubes per `sample2`'s language,
matching the project's actual approach (`cell → 4×4×4 voxel` expansion, one mesh per flat color, per the "Rendering:
Multi-Mesh Approach" section of CLAUDE.md). **Props** — trees, bushes, buildings/castles, and any other organic or
curved geometry such as curved roofs or rounded tree canopies — are **imported 3D models**, not composed from cell
voxels; see "Decoration Props" below. Sculpted mountain faces stay part of the voxel terrain (faceted, not curved) — see
"Relief Shading."

---

## Terminology

- **Biome** — a 12×12×12 cells, the atomic unit of the world. Biomes are arranged in a 6×6 grid.
- **World** — the 6×6 grid of biomes. Each match happens in one world.
- **Era** — a technological age assigned to each biome. Biomes can be in different eras at the same time.
- **Cell** — the logical unit of gameplay: one type (grass, sand, water, stone, dirt, air), holds at most one resource,
  mined by one miner, built on by one building. 12³ cells per biome.
- **Voxel** or **Terrain voxel** — the visual sub-unit a cell expands into at render time. Purely cosmetic; no gameplay
  semantics. 4³ voxels per cell.

## Gameplay Concept

You are one of four competing deities. You seize biomes on a 6×6 world, settle towers, allocate population to research /
mining / production / armies / defence, advance your civilization through technological ages, and conquer by destroying
every rival tower. You never micro individual units; you allocate population and direct armies at target biomes. Combat
auto-resolves.

### Design pillars (inherited from MLM)

1. **Allocation over micro.** The decision is _how many people on which task_, not clicking soldiers. No unit
   babysitting.
2. **Expansion vs. advancement tension.** More towers build armies faster; one strong tower researches faster. The
   player constantly trades breadth for depth.
3. **No fog of micromanagement.** Combat, breeding, and mining run themselves once allocated. The skill is in the
   numbers, not the APM.
4. **Resource-gated tech.** Designs cost elements; elements come from biomes. Where you settle decides what you can
   build.

---

## World model

### Terminology (two-tier grid)

A biome is therefore **12³ = 1,728 cells logically** and **48³ = 110,592 voxels visually**. The expansion is a pure
function `cell_type → 4×4×4 voxel pattern` that lives in `render`. `mapgen` and `sim` never see voxels.

### Grid

- One match = a **6×6 grid of 36 biomes**. Same as in original MLM, not all biomes will be rendered leaving an
  intentional gap in the world.
- Each biome is a **12×12×12 cell cube** (logical).
- Each cell expands at render time into a **4×4×4 voxel block** (visual).

### Cell Types

| Cell Type | Band                                 | Rendering Notes                                                                                             | Resource Name |
| --------- | ------------------------------------ | ----------------------------------------------------------------------------------------------------------- | ------------- |
| soil      | Any                                  | Grass on top, dirt below.                                                                                   | None          |
| sand      | Surface                              | All "beach yellow" sand.                                                                                    | None          |
| water     | Surface, Underground                 | Blue, transparent 50%.                                                                                      | water         |
| stone     | Any                                  | Gray stone.                                                                                                 | stone         |
| gold      | Underground                          | Yellow stone.                                                                                               | gold          |
| iron      | Underground                          | Reddish stone.                                                                                              | iron          |
| air       | Above-ground                         | Empty space.                                                                                                | None          |
| snow      | Relief (cell layer ≥ 11), stone only | White cap on tall stone peaks; cosmetic top-voxel layer, not a full-depth cell type — see § Relief Shading. | None          |

### Material Palette (from reference art)

Colors observed in `docs/reference/`, for tuning `StandardMaterial::base_color` per cell type:

| Cell Type    | Reference color                                                                                               | Notes                                                                                                          |
| ------------ | ------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- |
| soil (grass) | Saturated mid-green top, burnt-orange/brown dirt below                                                        | Grass reads more saturated than a pastel green                                                                 |
| soil (dirt)  | Warm brown, slightly redder than neutral gray                                                                 | Same hue reused for the underside skirt (see "Focused Biome Presentation")                                     |
| sand         | Warm tan/khaki, close to the path/beach color in both refs                                                    |                                                                                                                |
| water        | Two variants seen: deep teal-blue (still pool) and lighter cyan (flowing stream) with visible surface texture | Flat 50% transparency alone reads flatter than the reference; a lighter rim/foam tone at edges would help      |
| stone        | Neutral mid-gray, faceted rather than smooth                                                                  | Mountains and underside rubble share this gray                                                                 |
| gold         | Not depicted in reference art                                                                                 | Keep yellow-stone per current spec; no visual reference yet                                                    |
| iron         | Not depicted in reference art                                                                                 | Keep reddish-stone per current spec; no visual reference yet                                                   |
| snow         | Off-white/pale blue-gray                                                                                      | Cosmetic top-voxel material for high-elevation relief only, not a new gameplay cell type; see "Relief Shading" |

### Cell's Voxels Rendering

**Top Z** - top surface

| Cell Type | Rule                                    | Voxel Layers (Z) | Rendering Notes |
| --------- | --------------------------------------- | ---------------- | --------------- |
| soil      | Top Z == air                            | 4                | 1 grass, 3 dirt |
| soil      | Top Z != air                            | 4                | 4 dirt          |
| sand      | Top Z == air                            | 3                | 1 air, 3 sand   |
| sand      | Top Z != air                            | 4                | 4 sand          |
| water     | Top Z == air                            | 2                | 2 air, 2 water  |
| water     | Top Z != air                            | 4                | 4 water         |
| stone     | Relief (cell layer ≥ 11) & Top Z == air | 4                | 1 snow, 3 stone |
| stone     | Otherwise                               | 4                | 4 stone         |
| gold      |                                         | 4                | 4 gold          |
| iron      |                                         | 4                | 4 iron          |
| air       |                                         | 4                | 4 air           |

### Biome anatomy (Z axis in cells, 1 = bottom)

| Layers | Band        | Contents                                                                                                     |
| ------ | ----------- | ------------------------------------------------------------------------------------------------------------ |
| 1–5    | Underground | Resource deposits.                                                                                           |
| 6      | Surface     | Connection layer. Towers and buildings sit here. Water surfaces sit here. Edge strips are flat and typed.    |
| 7–12   | Above       | Relief: hills, mountains, otherwise air. Cosmetic + line-of-sight flavor. No gameplay collision in the demo. |

All layer indices in this spec refer to **cell layers** unless stated otherwise. A cell layer corresponds to 4 voxel
layers along Z.

### Connections

- Adjacent biomes connect **only at cell layer 6**, by **surface-type match** (water↔water, sand↔sand, grass↔grass,
  stone↔stone). Incompatible neighbors have **no connection**.
- The connecting edge strip is **flat** (no relief) so movement across it is clean.
- **Gameplay role of connection type (proposed, tunable):** edge type gates which armies may cross. Land types are
  crossable by land armies; **water connections require naval- or air-capable units** (a later tech tier).

### Relief Shading (Mountains)

Both reference pieces cap tall relief with **snow**, distinct from the stone below it — not a uniform gray peak. Snow is
a **cosmetic top-voxel material, not a new gameplay cell type** — the six cell types in Terminology are unchanged. Per §
Cell's Voxels Rendering: any `stone` cell in the relief band (layers 7–12) whose top voxel is exposed to air **and**
sits at cell layer ≥ 11 (the top two layers of the relief band) renders its top voxel layer as snow instead of stone —
the same top-layer-override pattern already used for grass-on-soil.

- Snow only applies to relief columns tall enough to reach layer 11+; low hills and flat biomes show no snow, matching
  the reference (only the tallest peaks are capped).
- Mountain faces in the reference are faceted (hard-edged low-poly), not smoothed — this favors flat-shaded
  per-voxel-face normals over vertex smoothing, which the voxel mesh approach already implies.
- Rock faces show scattered dark "pockmark" detail (small cavities/boulders) — treat as a stretch-goal texture/normal
  variation, not required for the initial terrain renderer.

---

## Procedural generation (`mapgen` crate)

`mapgen` operates exclusively on cells. Voxel expansion is `render`'s concern.

### Building Phases

>

**Phase I World Build:**

1. Generate the 6×6 grid of biomes, each with a 12³ cell grid, and render one biome for inspection.
2. For each of the 60 internal borders, assign a connection type using a **compatibility rule-table** keyed on the two
   biome types.

**Phase II Biome Build:** (for each biome)

1. Update 12³ cell grid honoring: its 4 prescribed edge connection types (flat typed strip at cell layer 6),
   world-space-noise relief in layers 7–12, and element distribution in layers 1–5.
2. Shape 1–5 (Underground) layers to the funnel to mimic floating island topography.

**Phase III Cell Shaping:** (for each cell)

1. Shape cell based on `Cell's Voxels Rendering` rules above
2. For each water to cell transition shape the slope:
   - If the cell is water and the neighbor on x or y is not water, then the water cell will have bottom layer of vortex
     of the inherited type of the neighbor cell

### Determinism contract

- `generate(world_seed, biome_coord) -> Biome` is **pure and deterministic** and returns a cell grid.
- Noise is sampled in **world cell coordinates** (`biome_coord * 12 + local_cell`) so adjacent biomes share a continuous
  field.
- **Test:** for any two adjacent biomes, the shared cell-layer-6 edge strip matches type cell-for-cell and is flat. Runs
  headless in CI; no GPU.

### Elements / resources

| Element | Cell Layer | Rarity (tunable) | Hard Rule |
| ------- | ---------- | ---------------- | --------- |
| stone   | any        | at least 30%     |           |
| iron    | 1–5        | 10%              |           |
| gold    | 1–5        | 5%               |           |

## Focused Biome Presentation

Both reference pieces render a biome as a **free-floating chunk**, not a flat-bottomed box: the underground material
tapers from a clean vertical dirt wall into an irregular, jagged mass of loose stone at the bottom, then stops. This is
purely cosmetic — there is no cell data below cell layer 1 — and should be treated as a decorative "skirt" mesh appended
below the logical 12³ grid only for the **focused** biome (the other 35 biomes already render as dimmed silhouette
proxies per § Lighting & atmosphere below, so they don't need a skirt).

- The skirt should be procedurally irregular (e.g. a noise-cut silhouette per outer face) rather than a mirrored flat
  underside, so it reads as "broken off the world" rather than "boxed."
- Skirt coloring reuses the underground materials already present in the biome's layers 1–5 (mostly dirt/stone) — no new
  resource-bearing geometry.
- Scope: this is additional geometry generated in `render`, not `mapgen`; it must not affect the cell-tier invariant
  tests in the Invariant Tests section. It should still be seeded/deterministic for a stable screenshot in
  `cargo xtask screenshot`, but does not need `mapgen`'s determinism-test guarantees.

## Decoration Props (imported 3D assets, future scope)

Both reference images dress the terrain with decoration that is **not** cell/voxel geometry:

- **Trees** — rounded pine canopies.
- **Bushes/shrubs** — small ground-cover clusters, occasionally flowering.
- **Buildings** — a wooden cabin/hut in the references, but the era-driven building set will include grander structures
  (e.g. castles) as tech advances. Curved roofs and non-axis-aligned silhouettes rule out voxel composition for any of
  these.
- **Path/plaza** — a cleared strip under the building; this stays voxel terrain (sand/dirt), it's not a prop.

**Props are imported 3D models (e.g. glTF/GLB), not composed from cell voxels.** They're placed on top of the voxel
terrain by `render`/`app`, which needs an asset-loading pipeline (Bevy `AssetServer` + `Gltf` handles) separate from the
procedural voxel mesher in `mesh.rs`. Whether placement (which prop, where, how many) is deterministic per `mapgen`
output (e.g. seeded by biome coordinate) or purely cosmetic per `render` is an open question for later.

None of this is required to hit "ability to render biomes" — that's terrain only: cell → voxel → mesh. Documented here
so the prop pipeline has a landing spot when it's tackled.

## Lighting & atmosphere

- **Soft shadows.** The scene's single `DirectionalLight` casts soft-edged shadows (Bevy's soft/PCF shadow maps,
  `shadow_maps_enabled = true`), not hard-edged ones, so the focused biome reads as a physical diorama rather than a
  flat cutout. Shadow resolution only needs to cover the single in-view biome (48³ voxels + decoration meshes), not the
  full 6×6 grid.
- **Very light fog.** A subtle distance fog (Bevy's `DistanceFog` component, low density) is applied outward from the
  camera. The fixed-angle orthographic camera has no natural perspective depth cue, so this faint fog is the primary way
  distance reads visually — it should fade the dimmed silhouette proxies of the other 35 biomes slightly more than the
  focused biome, while staying light enough that nothing becomes illegible.
- Both are **purely cosmetic**: they affect `render`/`app` only and have no bearing on `mapgen`/`sim` determinism or the
  cell-tier invariant tests in §17.6.
