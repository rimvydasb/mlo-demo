# Voxel Mega-Lo-Mania — Game Specification (Draft v0.2)

A god-game / early-RTS in the spirit of _Mega-Lo-Mania_ (Sensible Software, 1991), rendered as voxel biomes. Target:
WebAssembly + wgpu (web). Future: iOS. Development is LLM-assisted (Claude CLI) with the user as architect.

---

## Terminology

- **Biome** — a 12×12×12 cells, the atomic unit of the world. Biomes are arranged in a 6×6 grid.
- **World** — the 6×6 grid of biomes. Each match happens in one world.
- **Era** — a technological age assigned to each biome. Biomes can be in different eras at the same time.
- **Cell** — the logical unit of gameplay: one type (grass, sand, water, rock, dirt, air), holds at most one resource,
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
  rock↔rock). Incompatible neighbors have **no connection**.
- The connecting edge strip is **flat** (no relief) so movement across it is clean.
- **Gameplay role of connection type (proposed, tunable):** edge type gates which armies may cross. Land types are
  crossable by land armies; **water connections require naval- or air-capable units** (a later tech tier).

---

## Procedural generation (`mapgen` crate)

`mapgen` operates exclusively on cells. Voxel expansion is `render`'s concern.

### Building Phases

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

---

END OF REVIEW.

---

## 5. Factions

- **4 deities**: player + 3 AI. Colors/identities follow MLM convention (Scarlet, Caesar, Oberon, Madcap) or your own
  theme.
- AI archetypes (flavor, tunable): aggressive, strategic, defensive/alliance-prone, chaotic.
- AI lives entirely in the **pure `sim` crate** so AI-vs-AI matches can run headless for balancing.

---

## 6. Core loop & allocation tasks

Each tick: allocate population → tasks progress → idle population breeds.

| Task                  | Effect                                                                                                               |
|-----------------------|----------------------------------------------------------------------------------------------------------------------|
| **Research / Invent** | Design weapons, shields, defences. Completing designs raises tech level. A **Laboratory** unlocks high-tier designs. |
| **Mine**              | Allocate miners to extract elements from cell layers 1–5. A **Mine** unlocks deeper elements.                        |
| **Build**             | Construct Mine, Factory, or Laboratory.                                                                              |
| **Produce**           | Factory production runs for advanced weapons that can't be built instantly. Runs until elements run out.             |
| **Army**              | Equip men with available weapons; send to a target biome. Auto-combat.                                               |
| **Defend**            | Slot defensive weapons into building turrets.                                                                        |
| **Idle**              | Unassigned men breed, growing the workforce.                                                                         |

Player chooses the split per owned biome. More men on a task = faster/stronger; more idle men = faster population
growth. That trade-off is the core skill.

## 7. Buildings & tech gates

| Building       | Unlocks                                                           | Gate (tunable) |
|----------------|-------------------------------------------------------------------|----------------|
| **Tower**      | HQ, houses population, defensive turrets. Destroyed → biome lost. | Start          |
| **Mine**       | Access to deeper element cell layers.                             | Mid tech       |
| **Factory**    | Production of advanced weapons.                                   | Higher tech    |
| **Laboratory** | Invention of top-tier designs.                                    | Highest tech   |

A newly settled tower starts its own research from scratch (re-invent the wheel), though it can use weapons the
conquering army brought. Each building occupies one cell at cell layer 6.

---

## 8. Tech progression / ages

**Demo default — within-match age climb:**

- Completing N designs advances your **tech level**; tech level advances your **age** (caveman → classical → industrial
  → modern → future), changing visuals and unlocking buildings/weapons.
- Weapon tiers scale from sticks/rocks up through projectile, mechanized, and high-tech (lasers, missiles, aircraft).
  **OPEN — campaign layer (deferred):**

- Wrapping multiple 6×6 grids into a sequential campaign, each grid starting at a higher **epoch ceiling**, reproduces
  MLM's full 28-island structure.
- _This is a scaling path, not a v1 feature._ Flagged for the architect to confirm whether it must exist from day one
  (it changes save/meta structure).

---

## 9. Combat & expansion

- **Combat is formula-resolved**, no projectiles, no pathing. `army_strength = f(men_count, weapon_tier)`; defender gets
  a turret/height bonus. When an army occupies a biome containing an enemy, combat resolves automatically over ticks
  until one side is gone.
- **Tower destroyed → biome lost** to its owner.
- **Expansion**: sending an army or unarmed men into an empty **type-connected** biome builds a new tower there. Larger
  forces build faster. Expansion routes are constrained by the cell-layer-6 connection graph (see §3.3).

## 10. Win / lose

- **Win:** all rival towers destroyed across the 36 biomes.
- **Lose:** you hold no towers and have no army in the field.
- **Alliances** (MLM feature) — deferred; see §16.

## 11. Time / tick model

- **Fixed low-rate simulation tick** (start 2–10 Hz; tune for feel). Allocation, breeding, mining, research, production,
  and combat all resolve on this tick.
- Rendering runs at display rate and reads sim state; it does **not** drive sim.
- The sim tick is **deterministic** — prerequisite for headless balancing and optional replays/lockstep later. Build it
  deterministic from the first commit.

## 12. Rendering model

### 12.1 Two-tier expansion

The render crate consumes the logical cell grid produced by `mapgen` and turns it into geometry via two channels:

1. **Terrain voxels.** A pure function `expand(cell, neighbors) → [Voxel; 64]` maps each cell to its 4×4×4 visual block.
   **All cells expand to the same 4³ footprint**; variation lives in _content_, not in size:
    - **Grass:** 3 voxel layers of dirt + 1 top layer of green.
    - **Sand:** 4 voxel layers of sand.
    - **Water:** 2 bottom layers of water + 2 top layers of air (open surface).
    - **Rock:** 4 voxel layers of stone.
    - **Dirt / underground:** 4 voxel layers of dirt/element-tinted stone.
    - **Air:** empty. Keeping the outer shape uniform avoids Z-stacking special cases at cell boundaries and keeps the
      cell-layer-6 edge strip cleanly flat.
2. **Surface decoration (instanced meshes).** Trees, rocks, tower buildings, mine headframes etc. are **placed as
   instanced meshes** on top of the terrain, one per cell that calls for one. They are _not_ sub-voxel patterns:
   building a tree out of voxels is expensive and ugly at this scale, and the reference aesthetic (chunky terrain +
   crisp props) wants meshes here. Placement is deterministic from the cell + seed. This hybrid matches the target
   visual (chunky voxel terrain, mesh props) and keeps the voxel count tractable: 48³ ≈ 110 k voxels per biome before
   face-culling, only the focused biome is meshed in detail.

### 12.2 Camera and scope

- **Only the focused biome is fully rendered**: terrain voxel mesh (naive face-culling is enough at 48³) + decoration
  meshes + lighting.
- The **other 35 biomes render as dimmed silhouette proxies** in the 6×6 layout — low-detail, desaturated. Detailed
  sector status lives in a **2D strategic map panel** (egui), mirroring MLM's island map.
- **Orthographic fixed-angle camera** for the isometric diorama look; also simplifies biome and cell picking (uniform
  pick-ray direction). Picking resolves to a **cell**, not a voxel — voxels have no gameplay identity.

### 12.3 Determinism of expansion

`expand` is pure. The same cell type and (where used) cell coordinates always produce the same voxel block and the same
decoration placements. This keeps the render-time PNG snapshots in §17.6 stable across runs.

---

## 13. Architecture

Three crates. The engine carries little code by design.

| Crate        | Role                                                                                                                                                                       | Engine deps     |
|--------------|----------------------------------------------------------------------------------------------------------------------------------------------------------------------------|-----------------|
| **`mapgen`** | Deterministic two-pass generation. Operates on cells. Pure logic, headless-testable.                                                                                       | none            |
| **`sim`**    | The product: MLM allocation engine — population, tasks, tech/ages, elements, buildings, combat, expansion, AI, win/lose. Operates on cells. Pure, deterministic, headless. | none            |
| **`app`**    | Bevy 0.19 + bevy_egui. egui for allocation UI + strategic map; Bevy for the 3D biome viewport, ortho camera, and shadow proxies. Drives the sim tick.                      | Bevy, bevy_egui |

Cell→voxel expansion lives in `render` (see §17.2); `app` consumes it.

**Engine choice rationale.** Rendering is trivial (one detailed biome + shadows), so almost no code touches the engine.
Treat **Bevy as a replaceable shell**: when a future Bevy release breaks APIs, the blast radius is `app` only. egui
handles the UI-heavy part of the game with a far more stable API than bevy_ui — the LLM-friendly choice.

**Version pinning (LLM-driven dev).**

- Pin **Bevy 0.19** and the **matching bevy_egui release** together for the whole demo. Mismatched bevy_egui is a
  classic LLM trap.
- Feed the LLM the 0.18→0.19 migration guide when generating Bevy code; its training lags the current API.
- Do not chase 0.20 mid-project.

## 14. Determinism & balancing strategy

- `mapgen` and `sim` are pure → unit-tested headless, fast, in CI.
- **Balance by simulation:** run thousands of headless AI-vs-AI matches to measure win rates per strategy and per
  starting biome, then tune element costs, breeding rate, combat formula, and tech gates. This is the only sane way to
  balance an allocation game, and it's free once `sim` is engine-independent.
- Voxel expansion is irrelevant to balancing — `sim` runs without `render`.

## 15. Milestones (build order)

Front-loads the pure, testable crates — ideal for LLM-assisted dev against a stable contract.

1. **`mapgen` MVP** — 6×6 grid, two-pass gen, biome anatomy in cells, border-continuity test. Headless.
2. **`sim` MVP** — single faction: population + idle breeding + research → tech level. Headless, tested.
3. **`app` MVP** — Bevy renders one biome with cell→voxel expansion (naive mesh), ortho camera, egui panel showing sim
   state. First visible build.
4. **`sim`** — elements/mining, buildings, production, army creation.
5. **`sim`** — combat resolution + expansion via type-connections + win/lose + 1 AI opponent.
6. **`sim`** — tech ages, scale to 4 AI gods with personalities.
7. **`app`** — shadow proxies for the 35 biomes, strategic map panel, biome selection, decoration meshes (trees,
   towers).
8. **Balance pass** — headless AI-vs-AI batch runs.

## 16. Deferred decisions (tunable, not blocking v1)

- Exact element set and costs.
- Combat formula constants.
- Shadow-proxy visual style (silhouette mesh vs. flat 2D map vs. both).
- Alliances between gods.
- Campaign layer of multiple grids across epochs (§8).
- Water in depressions (below cell layer 6) vs. flat at cell layer 6.
- WFC upgrade for macro generation.
- iOS port (post-demo).
- **Voxel resolution per cell.** 4³ is the v1 choice. If terrain looks too coarse in practice, bump to 8³ — `expand` is
  the only function that changes. Doing this later is cheap by design.
- **Per-cell sub-voxel variation.** v1: cells of the same type expand identically. Later: hash on cell coordinates to
  add micro-variation (tufts of grass, rock cracks) without touching `mapgen` or `sim`.

---

## 17. Technical specification

### 17.1 Two separation axes

| Axis                 | Splits                                                         | Purpose                                                                                                                        |
|----------------------|----------------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------|
| **Logic vs. engine** | `core` / `mapgen` / `sim` (pure) vs. `render` / `app` (engine) | Protects pure, deterministic, testable code from Bevy's pre-1.0 churn. Enforced by _dependency direction_.                     |
| **Native vs. WASM**  | thin `platforms/native` & `platforms/web` bins                 | Concentrates `cfg(target_arch = "wasm32")` and web glue in one place; keeps native builds clean. Enforced by _compile target_. |

The proposed "core / rendering / wasm" separation is correct but is these two axes combined.

### 17.2 Workspace & repository structure

Virtual workspace (no root package). Dependencies point **inward** to `core`.

```
voxel-mlm/
├─ Cargo.toml                # [workspace] virtual manifest + [workspace.dependencies]
├─ rust-toolchain.toml       # pin toolchain (track stable; matches Bevy MSRV)
├─ crates/
│  ├─ core/                  # domain types: CellGrid, BiomeType, CellType, ElementId, coords, Seed. std-light, no engine.
│  ├─ mapgen/                # deterministic two-pass generation of cell grids. → core. no engine.
│  ├─ sim/                   # MLM allocation engine (Phase 3+), operates on cells. → core. no engine.
│  ├─ render/                # cell→voxel expansion, meshing, camera, materials, decoration instancing, shadow proxies, headless render-to-image. → core + bevy.
│  └─ app/                   # Bevy App assembly, bevy_egui UI, dev inspector, mode wiring. → render, mapgen, sim.
├─ platforms/
│  ├─ native/                # bin: desktop entry. clap CLI (`inspect`, `screenshot`, later `play`). → app.
│  └─ web/                   # bin: wasm-bindgen entry + web shell (index.html, JS loader). cfg(wasm32). → app.
├─ xtask/                    # cargo-xtask automation (host-only, std). build-web, serve, screenshot, gen-map, dump.
├─ assets/
├─ tests/                    # cross-crate integration + golden/snapshot fixtures (or per-crate tests/).
└─ .github/workflows/ci.yml
```

| Crate    | Depends on                           | Engine-coupled | Target               |
|----------|--------------------------------------|----------------|----------------------|
| `core`   | —                                    | no             | all                  |
| `mapgen` | core                                 | no             | all                  |
| `sim`    | core                                 | no             | all                  |
| `render` | core, bevy                           | yes            | all (incl. headless) |
| `app`    | render, mapgen, sim, bevy, bevy_egui | yes            | all                  |
| `native` | app                                  | yes            | host                 |
| `web`    | app                                  | yes            | wasm32               |
| `xtask`  | std only                             | no             | host                 |

**Cell/voxel boundary as a compile-time guarantee.** `core` defines `Cell` and `CellGrid`. `Voxel` is defined in
`render` and **does not exist** in `core`, `mapgen`, or `sim`. The dependency direction makes this impossible to violate
without a deliberate edit to `Cargo.toml` — exactly the kind of rail you want under LLM-driven dev.

### 17.3 Architecture options considered

| Option                                               | Pros                                                                                                    | Cons                                                                                                                              | Verdict                                                                            |
|------------------------------------------------------|---------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------|
| **Hexagonal workspace** (pure core + adapter crates) | Max testability; engine replaceable; pure/impure boundary enforced at compile time; logic runs headless | Some Cargo ceremony; indirection                                                                                                  | **Recommended** — matches "sim is the product" + the pre-1.0-engine / LLM risk     |
| Single crate + feature flags + modules               | Minimal ceremony; fastest start; one tree for the LLM to navigate                                       | Boundary **not** enforced (LLM can leak `bevy` into generation); slower incremental builds at scale; harder to run logic headless | Acceptable only for a throwaway prototype                                          |
| Bevy plugin-per-feature                              | Very Bevy-native; toggle features; ecosystem-aligned                                                    | Plugins are engine-coupled (doesn't isolate logic); crate boilerplate                                                             | Use as **module** organization _inside_ `app`/`render`, not as the top-level split |

### 17.4 Build, targets & automation

- **Thin platform bins, fat library crates.** `native` and `web` only construct the app and run it; all logic is in
  libraries so both targets share it and tests target the libraries.
- **`cargo-xtask`** is the build orchestrator (cross-platform, no makefiles). Verbs: `build-web`, `serve`,
  `screenshot --seed N`, `gen-map --seed N`, `dump --seed N`. The LLM runs `cargo xtask <verb>` deterministically
  instead of memorizing wasm tooling.
- **WASM pipeline:** `cargo build --target wasm32-unknown-unknown` → `wasm-bindgen` glue → `wasm-opt` for size,
  orchestrated by `xtask build-web`. **Trunk** is a simpler alternative if you want `trunk serve` to manage it. Pin \*
  \*Bevy 0.19 + the matching bevy_egui\*\* together.
- **std-light pure crates.** Keep `core`/`mapgen`/`sim` free of `std`-only and engine deps (full `no_std + alloc` is
  optional rigor). Smaller WASM, reusable, and it forces the boundary.

### 17.5 Determinism rules (non-negotiable for LLM-legible results)

- **RNG:** `rand_chacha::ChaCha8Rng::seed_from_u64(seed)` — explicit and version/cross-platform stable. Do **not** use
  `StdRng` (stability not guaranteed) or any thread/time RNG in pure crates.
- **Collections:** no `HashMap` iteration in deterministic paths — use `BTreeMap`, sorted iteration, or `indexmap`.
- **Floats:** intra-target determinism is guaranteed; **native↔WASM may differ** on noise/transcendental functions.
  Phase 1 requires byte-identical output _within_ a target and a _tolerance_ across targets. Future lockstep-sim
  determinism across platforms needs **fixed-point integer math** — flagged, not built yet.
- **Determinism test:** same seed run twice → identical serialized cell grid (intra-target byte equality). Voxel
  expansion is a pure function of the cell grid → covered transitively.

### 17.6 Testing & verification strategy

Principle: **an LLM verifies what it can read or see.** Three channels, priority order:

| Layer         | Verification method                                                                                                         | The LLM's channel to understand the result                                                                 |
|---------------|-----------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------------------------------|
| `core`        | unit tests                                                                                                                  | reads assertions                                                                                           |
| `mapgen`      | property/invariant tests (`proptest`) + ASCII-dump snapshots (`insta`) + same-seed determinism                              | reads pass/fail **and the shrunk minimal failing seed**; reads the ASCII map as text; reads snapshot diffs |
| `sim` (later) | golden-trace snapshots + property (monotonicity, conservation) + `debug_assert!` invariants + headless AI-vs-AI batch stats | reads trace diffs + win-rate stat tables                                                                   |
| `render`      | headless render-to-PNG + image-snapshot regression; covers cell→voxel expansion + decoration                                | **multimodal model views the PNG**; reads the diff verdict                                                 |
| `web`/wasm    | boot smoke test (no panic) + native↔wasm map parity (tolerance)                                                             | reads pass/fail + parity report                                                                            |

**Invariants to assert over all seeds (`mapgen`, expressed in cells):** ground exactly at cell layer 6; resources only
in cell layers 1–5; relief only in cell layers 7–12; adjacent biomes' shared cell-layer-6 edge matches type and is flat;
no floating cells; height within bounds.

**Text projection (channel 2), example — cell layer 6 top-down of one biome:**

```
legend: . grass   s sand   ~ water   # rock
~~~~ssss....
~~~~ssss....
~~~~ssss....
....ssss....
```

Plus a 6×6 macro view where `=`/`|` mark type-matched connections and blank marks none. ASCII dumps live at the **cell**
tier — never voxel — because cells are the gameplay truth.

**Image channel (channel 3):** `cargo xtask screenshot --seed 42 --out shot.png` renders one frame headless (offscreen
render-to-texture + readback), including voxel expansion and decoration. The multimodal model inspects `shot.png`;
`image`-based snapshot diff (SSIM/threshold) catches regressions in CI.

**Tooling:** `cargo nextest` (faster, cleaner output), `insta` (snapshots; `cargo insta review`), `proptest` (
shrinking), `image` (screenshot diff), `clippy` + `rustfmt` in CI. CI matrix: native test (mac/linux) + wasm build &
smoke + lint + snapshot suites. The LLM reads CI output.

**Mandate:** the ASCII-dump and headless-screenshot capabilities ship in **Phase 1**, because they are simultaneously
the inspector's exports, the test harness, and the LLM's eyes.

---

## 18. Phase 1 — Dev-mode map inspector (first deliverable)

**Goal:** generate and visually inspect maps, so biomes, cells, resources, and the cell→voxel visual mapping can be
refined _before_ any gameplay exists.

**Active crates:** `core`, `mapgen`, `render`, `app` (inspector mode only), `native`, `web`, `xtask`. **No `sim`.**

**Entry:** `cargo run -p native -- inspect --seed 42` boots straight into the inspector (dev mode is the only mode in
Phase 1).

**Features**

- Generate the 6×6 grid (start with a single biome, then the grid) from a seed.
- 3D render of the focused biome: cell→voxel expansion + naive face-culled mesh (48³ voxels per biome max).
- **Travel:** orthographic camera with orbit / pan / zoom; cycle/select the focused biome across the 6×6; others shown
  as dimmed proxies.
- **Inspect:** hover/click resolves to a **cell** (not a voxel). egui panel shows cell type, element, cell layer, and
  biome/cell coordinates.
- **Layer peeling:** slider to hide upper cell layers and expose the underground (cell layers 1–5) for resource
  inspection.
- **Voxel-pattern preview (dev aid):** a side panel showing the `expand(cell_type)` voxel pattern for each cell type, so
  tweaks to the visual mapping are reviewable without re-generating a whole map.
- **Seed controls:** scrub seed, regenerate live, copy seed.
- **Exports (double as test artifacts):** ASCII top-down dump at the cell tier; headless PNG screenshot of the rendered
  voxel scene. **Phase 1 verification**

- `mapgen`: property tests (invariants in §17.6) + ASCII-dump snapshots + same-seed determinism.
- `render`: headless screenshot of a fixed seed/camera → PNG, multimodal review + image-snapshot regression. Includes
  voxel expansion correctness.
- `web`: boots in a headless browser without panic; native↔wasm map parity within float tolerance. **Exit criteria:**
  you can launch the inspector on any seed, travel the 6×6, peel cell layers to inspect resources, see the cell→voxel
  expansion live, export an ASCII dump and a PNG, and the full `mapgen` invariant suite is green on both native and
  wasm.
