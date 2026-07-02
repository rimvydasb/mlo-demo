# Rendering Specification

A god-game / early-RTS in the spirit of _Mega-Lo-Mania_ (Sensible Software, 1991), rendered as voxel biomes. Target:
WebAssembly + wgpu (web). Future: iOS. Development is LLM-assisted (Claude CLI) with the user as architect.

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

| Cell Type | Band                 | Rendering Notes           | Resource Name |
|-----------|----------------------|---------------------------|---------------|
| soil      | Any                  | Grass on top, dirt below. | None          |
| sand      | Surface              | All "beach yellow" sand.  | None          |
| water     | Surface, Underground | Blue, transparent 50%.    | water         |
| stone     | Any                  | Gray stone.               | stone         |
| gold      | Underground          | Yellow stone.             | gold          |
| iron      | Underground          | Reddish stone.            | iron          |
| air       | Above-ground         | Empty space.              | None          |

### Cell's Voxels Rendering

**Top Z** - top surface

| Cell Type | Rule         | Voxel Layers (Z) | Rendering Notes |
|-----------|--------------|------------------|-----------------|
| soil      | Top Z == air | 4                | 1 grass, 3 dirt |
| soil      | Top Z != air | 4                | 4 dirt          |
| sand      | Top Z == air | 3                | 1 air, 3 sand   |
| sand      | Top Z != air | 4                | 4 sand          |
| water     | Top Z == air | 2                | 2 air, 2 water  |
| water     | Top Z != air | 4                | 4 water         |
| stone     |              | 4                | 4 stone         |
| gold      |              | 4                | 4 gold          |
| iron      |              | 4                | 4 iron          |
| air       |              | 4                | 4 air           |

### Biome anatomy (Z axis in cells, 1 = bottom)

| Layers | Band        | Contents                                                                                                     |
|--------|-------------|--------------------------------------------------------------------------------------------------------------|
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
|---------|------------|------------------|-----------|
| stone   | any        | at least 30%     |           |
| iron    | 1–5        | 10%              |           |
| gold    | 1–5        | 5%               |           |

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