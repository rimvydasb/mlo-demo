# Gameplay Specification

A god-game / early-RTS in the spirit of _Mega-Lo-Mania_ (Sensible Software, 1991), rendered as voxel biomes. Target:
WebAssembly + wgpu (web). Future: iOS. Development is LLM-assisted (Claude CLI) with the user as architect.

---

## Factions

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
    - **Stone:** 4 voxel layers of stone.
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



### 12.4 Determinism of expansion

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
  add micro-variation (tufts of grass, stone cracks) without touching `mapgen` or `sim`.
