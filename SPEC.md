# Voxel Mega-Lo-Mania — Game Specification (Draft v0.1)

A god-game / early-RTS in the spirit of *Mega-Lo-Mania* (Sensible Software, 1991),
rendered as voxel biomes. Target: WebAssembly + wgpu (web). Future: iOS.
Development is LLM-assisted (Claude CLI) with the user as architect.

---

## 1. Concept

You are one of four competing deities. You seize biomes on a 6×6 world, settle
towers, allocate population to research / mining / production / armies / defence,
advance your civilization through technological ages, and conquer by destroying
every rival tower. You never micro individual units; you allocate population and
direct armies at target biomes. Combat auto-resolves.

## 2. Design pillars (inherited from MLM)

1. **Allocation over micro.** The decision is *how many people on which task*, not
   clicking soldiers. No unit babysitting.
2. **Expansion vs. advancement tension.** More towers build armies faster; one
   strong tower researches faster. The player constantly trades breadth for depth.
3. **No fog of micromanagement.** Combat, breeding, and mining run themselves once
   allocated. The skill is in the numbers, not the APM.
4. **Resource-gated tech.** Designs cost elements; elements come from biomes. Where
   you settle decides what you can build.

---

## 3. World model

### 3.1 Grid
- One match = a **6×6 grid of 36 biomes**.
- Each biome is a **12×12×12 voxel cube**.

### 3.2 Biome anatomy (Z axis, 1 = bottom)

| Layers | Band | Contents |
|--------|------|----------|
| 1–5 | Underground | Element deposits. Sand/stone shallow and common; rarer elements deeper and sparser. The element mix defines the biome's strategic value. |
| 6 | Ground surface | Connection layer. Towers and buildings sit here. Water surfaces sit here. Edge strips are flat and typed. |
| 7–12 | Above-ground | Relief: hills, mountains, otherwise air. Cosmetic + line-of-sight flavor. No gameplay collision in the demo. |

### 3.3 Connections
- Adjacent biomes connect **only at layer 6**, by **surface-type match**
  (water↔water, sand↔sand, grass↔grass, rock↔rock). Incompatible neighbors have
  **no connection**.
- The connecting edge strip is **flat** (no relief) so movement across it is clean.
- **Gameplay role of connection type (proposed, tunable):** edge type gates which
  armies may cross. Land types are crossable by land armies; **water connections
  require naval- or air-capable units** (a later tech tier). This gives the
  type-match rule strategic teeth instead of being cosmetic.

---

## 4. Procedural generation (`mapgen` crate)

### 4.1 Two-pass
1. **Macro pass** — build the 6×6 graph. For each of the 60 internal borders,
   assign a connection type using a **compatibility rule-table** keyed on the two
   biome types. (Rule-table, *not* Wave Function Collapse, for the demo: WFC
   backtracks and is hard to debug under LLM-driven dev. WFC is a documented later
   upgrade if richer adjacency is wanted.)
2. **Interior pass** — for each biome, generate the 12³ honoring: its biome type,
   its 4 prescribed edge connection types (flat typed strip at layer 6),
   world-space-noise relief in layers 7–12, and element distribution in layers 1–5.

### 4.2 Determinism contract
- `generate(world_seed, biome_coord) -> Biome` is **pure and deterministic**.
- Noise is sampled in **world coordinates** (`biome_coord * 12 + local`) so adjacent
  biomes share a continuous field.
- **Test:** for any two adjacent biomes, the shared layer-6 edge strip matches type
  cell-for-cell and is flat. Runs headless in CI; no GPU.

### 4.3 Elements / resources
- A small **element set**. Sand and stone are the common base; a handful of rarer
  elements gate advanced designs. (Exact set is tunable; start with ~4–6.)
- Some elements are surface-gatherable; others require a **Mine** building to reach
  the deeper layers.

---

## 5. Factions

- **4 deities**: player + 3 AI. Colors/identities follow MLM convention (Scarlet,
  Caesar, Oberon, Madcap) or your own theme.
- AI archetypes (flavor, tunable): aggressive, strategic, defensive/alliance-prone,
  chaotic.
- AI lives entirely in the **pure `sim` crate** so AI-vs-AI matches can run headless
  for balancing.

---

## 6. Core loop & allocation tasks

Each tick: allocate population → tasks progress → idle population breeds.

| Task | Effect |
|------|--------|
| **Research / Invent** | Design weapons, shields, defences. Completing designs raises tech level. A **Laboratory** unlocks high-tier designs. |
| **Mine** | Allocate miners to extract elements from layers 1–5. A **Mine** unlocks deeper elements. |
| **Build** | Construct Mine, Factory, or Laboratory. |
| **Produce** | Factory production runs for advanced weapons that can't be built instantly. Runs until elements run out. |
| **Army** | Equip men with available weapons; send to a target biome. Auto-combat. |
| **Defend** | Slot defensive weapons into building turrets. |
| **Idle** | Unassigned men breed, growing the workforce. |

Player chooses the split per owned biome. More men on a task = faster/stronger;
more idle men = faster population growth. That trade-off is the core skill.

## 7. Buildings & tech gates

| Building | Unlocks | Gate (tunable) |
|----------|---------|----------------|
| **Tower** | HQ, houses population, defensive turrets. Destroyed → biome lost. | Start |
| **Mine** | Access to deeper element layers. | Mid tech |
| **Factory** | Production of advanced weapons. | Higher tech |
| **Laboratory** | Invention of top-tier designs. | Highest tech |

A newly settled tower starts its own research from scratch (re-invent the wheel),
though it can use weapons the conquering army brought.

---

## 8. Tech progression / ages

**Demo default — within-match age climb:**
- Completing N designs advances your **tech level**; tech level advances your **age**
  (caveman → classical → industrial → modern → future), changing visuals and
  unlocking buildings/weapons.
- Weapon tiers scale from sticks/rocks up through projectile, mechanized, and
  high-tech (lasers, missiles, aircraft).

**OPEN — campaign layer (deferred):**
- Wrapping multiple 6×6 grids into a sequential campaign, each grid starting at a
  higher **epoch ceiling**, reproduces MLM's full 28-island structure.
- *This is a scaling path, not a v1 feature.* Flagged for the architect to confirm
  whether it must exist from day one (it changes save/meta structure).

---

## 9. Combat & expansion

- **Combat is formula-resolved**, no projectiles, no pathing.
  `army_strength = f(men_count, weapon_tier)`; defender gets a turret/height bonus.
  When an army occupies a biome containing an enemy, combat resolves automatically
  over ticks until one side is gone.
- **Tower destroyed → biome lost** to its owner.
- **Expansion**: sending an army or unarmed men into an empty **type-connected**
  biome builds a new tower there. Larger forces build faster. Expansion routes are
  constrained by the layer-6 connection graph (see §3.3).

## 10. Win / lose

- **Win:** all rival towers destroyed across the 36 biomes.
- **Lose:** you hold no towers and have no army in the field.
- **Alliances** (MLM feature) — deferred; see §16.

## 11. Time / tick model

- **Fixed low-rate simulation tick** (start 2–10 Hz; tune for feel). Allocation,
  breeding, mining, research, production, and combat all resolve on this tick.
- Rendering runs at display rate and reads sim state; it does **not** drive sim.
- The sim tick is **deterministic** — prerequisite for headless balancing and
  optional replays/lockstep later. Build it deterministic from the first commit.

## 12. Rendering model

- **Only the focused biome is fully rendered**: voxel mesh (naive face-culling is
  enough at 12³), relief, lighting.
- The **other 35 biomes render as dimmed silhouette proxies** in the 6×6 layout —
  low-detail, desaturated. Detailed sector status lives in a **2D strategic map
  panel** (egui), mirroring MLM's island map.
- **Orthographic fixed-angle camera** for the isometric diorama look; also
  simplifies biome picking (uniform pick-ray direction).

---

## 13. Architecture

Three crates. The engine carries little code by design.

| Crate | Role | Engine deps |
|-------|------|-------------|
| **`mapgen`** | Deterministic two-pass generation. Pure logic, headless-testable. | none |
| **`sim`** | The product: MLM allocation engine — population, tasks, tech/ages, elements, buildings, combat, expansion, AI, win/lose. Pure, deterministic, headless. | none |
| **`app`** | Bevy 0.19 + bevy_egui. egui for allocation UI + strategic map; Bevy for the 3D biome viewport, ortho camera, and shadow proxies. Drives the sim tick. | Bevy, bevy_egui |

**Engine choice rationale.** Rendering is trivial (one detailed biome + shadows),
so almost no code touches the engine. Treat **Bevy as a replaceable shell**: when a
future Bevy release breaks APIs, the blast radius is `app` only. egui handles the
UI-heavy part of the game with a far more stable API than bevy_ui — the
LLM-friendly choice.

**Version pinning (LLM-driven dev).**
- Pin **Bevy 0.19** and the **matching bevy_egui release** together for the whole
  demo. Mismatched bevy_egui is a classic LLM trap.
- Feed the LLM the 0.18→0.19 migration guide when generating Bevy code; its training
  lags the current API.
- Do not chase 0.20 mid-project.

## 14. Determinism & balancing strategy

- `mapgen` and `sim` are pure → unit-tested headless, fast, in CI.
- **Balance by simulation:** run thousands of headless AI-vs-AI matches to measure
  win rates per strategy and per starting biome, then tune element costs, breeding
  rate, combat formula, and tech gates. This is the only sane way to balance an
  allocation game, and it's free once `sim` is engine-independent.

## 15. Milestones (build order)

Front-loads the pure, testable crates — ideal for LLM-assisted dev against a stable
contract.

1. **`mapgen` MVP** — 6×6 grid, two-pass gen, biome anatomy, border-continuity test. Headless.
2. **`sim` MVP** — single faction: population + idle breeding + research → tech level. Headless, tested.
3. **`app` MVP** — Bevy renders one biome (naive mesh), ortho camera, egui panel showing sim state. First visible build.
4. **`sim`** — elements/mining, buildings, production, army creation.
5. **`sim`** — combat resolution + expansion via type-connections + win/lose + 1 AI opponent.
6. **`sim`** — tech ages, scale to 4 AI gods with personalities.
7. **`app`** — shadow proxies for the 35 biomes, strategic map panel, biome selection.
8. **Balance pass** — headless AI-vs-AI batch runs.

## 16. Deferred decisions (tunable, not blocking v1)

- Exact element set and costs.
- Combat formula constants.
- Shadow-proxy visual style (silhouette mesh vs. flat 2D map vs. both).
- Alliances between gods.
- Campaign layer of multiple grids across epochs (§8).
- Water in depressions (below layer 6) vs. flat at layer 6.
- WFC upgrade for macro generation.
- iOS port (post-demo).

---

## 17. Technical specification

### 17.1 Two separation axes

| Axis | Splits | Purpose |
|------|--------|---------|
| **Logic vs. engine** | `core` / `mapgen` / `sim` (pure) vs. `render` / `app` (engine) | Protects pure, deterministic, testable code from Bevy's pre-1.0 churn. Enforced by *dependency direction*. |
| **Native vs. WASM** | thin `platforms/native` & `platforms/web` bins | Concentrates `cfg(target_arch = "wasm32")` and web glue in one place; keeps native builds clean. Enforced by *compile target*. |

The proposed "core / rendering / wasm" separation is correct but is these two axes combined.

### 17.2 Workspace & repository structure

Virtual workspace (no root package). Dependencies point **inward** to `core`.

```
voxel-mlm/
├─ Cargo.toml                # [workspace] virtual manifest + [workspace.dependencies]
├─ rust-toolchain.toml       # pin toolchain (track stable; matches Bevy MSRV)
├─ crates/
│  ├─ core/                  # domain types: VoxelGrid, BiomeType, ElementId, coords, Seed. std-light, no engine.
│  ├─ mapgen/                # deterministic two-pass generation. → core. no engine.
│  ├─ sim/                   # MLM allocation engine (Phase 3+). → core. no engine.
│  ├─ render/                # meshing, camera, materials, shadow proxies, headless render-to-image. → core + bevy.
│  └─ app/                   # Bevy App assembly, bevy_egui UI, dev inspector, mode wiring. → render, mapgen, sim.
├─ platforms/
│  ├─ native/                # bin: desktop entry. clap CLI (`inspect`, `screenshot`, later `play`). → app.
│  └─ web/                   # bin: wasm-bindgen entry + web shell (index.html, JS loader). cfg(wasm32). → app.
├─ xtask/                    # cargo-xtask automation (host-only, std). build-web, serve, screenshot, gen-map, dump.
├─ assets/
├─ tests/                    # cross-crate integration + golden/snapshot fixtures (or per-crate tests/).
└─ .github/workflows/ci.yml
```

| Crate | Depends on | Engine-coupled | Target |
|-------|-----------|----------------|--------|
| `core` | — | no | all |
| `mapgen` | core | no | all |
| `sim` | core | no | all |
| `render` | core, bevy | yes | all (incl. headless) |
| `app` | render, mapgen, sim, bevy, bevy_egui | yes | all |
| `native` | app | yes | host |
| `web` | app | yes | wasm32 |
| `xtask` | std only | no | host |

### 17.3 Architecture options considered

| Option | Pros | Cons | Verdict |
|--------|------|------|---------|
| **Hexagonal workspace** (pure core + adapter crates) | Max testability; engine replaceable; pure/impure boundary enforced at compile time; logic runs headless | Some Cargo ceremony; indirection | **Recommended** — matches "sim is the product" + the pre-1.0-engine / LLM risk |
| Single crate + feature flags + modules | Minimal ceremony; fastest start; one tree for the LLM to navigate | Boundary **not** enforced (LLM can leak `bevy` into generation); slower incremental builds at scale; harder to run logic headless | Acceptable only for a throwaway prototype |
| Bevy plugin-per-feature | Very Bevy-native; toggle features; ecosystem-aligned | Plugins are engine-coupled (doesn't isolate logic); crate boilerplate | Use as **module** organization *inside* `app`/`render`, not as the top-level split |

### 17.4 Build, targets & automation

- **Thin platform bins, fat library crates.** `native` and `web` only construct the app and run it; all logic is in libraries so both targets share it and tests target the libraries.
- **`cargo-xtask`** is the build orchestrator (cross-platform, no makefiles). Verbs: `build-web`, `serve`, `screenshot --seed N`, `gen-map --seed N`, `dump --seed N`. The LLM runs `cargo xtask <verb>` deterministically instead of memorizing wasm tooling.
- **WASM pipeline:** `cargo build --target wasm32-unknown-unknown` → `wasm-bindgen` glue → `wasm-opt` for size, orchestrated by `xtask build-web`. **Trunk** is a simpler alternative if you want `trunk serve` to manage it. Pin **Bevy 0.19 + the matching bevy_egui** together.
- **std-light pure crates.** Keep `core`/`mapgen`/`sim` free of `std`-only and engine deps (full `no_std + alloc` is optional rigor). Smaller WASM, reusable, and it forces the boundary.

### 17.5 Determinism rules (non-negotiable for LLM-legible results)

- **RNG:** `rand_chacha::ChaCha8Rng::seed_from_u64(seed)` — explicit and version/cross-platform stable. Do **not** use `StdRng` (stability not guaranteed) or any thread/time RNG in pure crates.
- **Collections:** no `HashMap` iteration in deterministic paths — use `BTreeMap`, sorted iteration, or `indexmap`.
- **Floats:** intra-target determinism is guaranteed; **native↔WASM may differ** on noise/transcendental functions. Phase 1 requires byte-identical output *within* a target and a *tolerance* across targets. Future lockstep-sim determinism across platforms needs **fixed-point integer math** — flagged, not built yet.
- **Determinism test:** same seed run twice → identical serialized grid (intra-target byte equality).

### 17.6 Testing & verification strategy

Principle: **an LLM verifies what it can read or see.** Three channels, priority order:

| Layer | Verification method | The LLM's channel to understand the result |
|-------|--------------------|--------------------------------------------|
| `core` | unit tests | reads assertions |
| `mapgen` | property/invariant tests (`proptest`) + ASCII-dump snapshots (`insta`) + same-seed determinism | reads pass/fail **and the shrunk minimal failing seed**; reads the ASCII map as text; reads snapshot diffs |
| `sim` (later) | golden-trace snapshots + property (monotonicity, conservation) + `debug_assert!` invariants + headless AI-vs-AI batch stats | reads trace diffs + win-rate stat tables |
| `render` | headless render-to-PNG + image-snapshot regression | **multimodal model views the PNG**; reads the diff verdict |
| `web`/wasm | boot smoke test (no panic) + native↔wasm map parity (tolerance) | reads pass/fail + parity report |

**Invariants to assert over all seeds (`mapgen`):** ground exactly at layer 6; resources only in layers 1–5; relief only in 7–12; adjacent biomes' shared layer-6 edge matches type and is flat; no floating voxels; height within bounds.

**Text projection (channel 2), example — layer-6 top-down of one biome:**
```
legend: . grass   s sand   ~ water   # rock
~~~~ssss....
~~~~ssss....
~~~~ssss....
....ssss....
```
Plus a 6×6 macro view where `=`/`|` mark type-matched connections and blank marks none.

**Image channel (channel 3):** `cargo xtask screenshot --seed 42 --out shot.png` renders one frame headless (offscreen render-to-texture + readback). The multimodal model inspects `shot.png`; `image`-based snapshot diff (SSIM/threshold) catches regressions in CI.

**Tooling:** `cargo nextest` (faster, cleaner output), `insta` (snapshots; `cargo insta review`), `proptest` (shrinking), `image` (screenshot diff), `clippy` + `rustfmt` in CI. CI matrix: native test (mac/linux) + wasm build & smoke + lint + snapshot suites. The LLM reads CI output.

**Mandate:** the ASCII-dump and headless-screenshot capabilities ship in **Phase 1**, because they are simultaneously the inspector's exports, the test harness, and the LLM's eyes.

---

## 18. Phase 1 — Dev-mode map inspector (first deliverable)

**Goal:** generate and visually inspect maps, so biomes, resources, and visual components can be refined *before* any gameplay exists.

**Active crates:** `core`, `mapgen`, `render`, `app` (inspector mode only), `native`, `web`, `xtask`. **No `sim`.**

**Entry:** `cargo run -p native -- inspect --seed 42` boots straight into the inspector (dev mode is the only mode in Phase 1).

**Features**
- Generate the 6×6 grid (start with a single biome, then the grid) from a seed.
- 3D voxel render of the focused biome (naive face-culled mesh is enough at 12³).
- **Travel:** orthographic camera with orbit / pan / zoom; cycle/select the focused biome across the 6×6; others shown as dimmed proxies.
- **Inspect:** hover/click a voxel or biome → egui panel showing type, element, layer, coordinates.
- **Layer peeling:** slider to hide upper layers and expose the underground (1–5) for resource inspection.
- **Seed controls:** scrub seed, regenerate live, copy seed.
- **Exports (double as test artifacts):** ASCII top-down dump; headless PNG screenshot.

**Phase 1 verification**
- `mapgen`: property tests (invariants in §17.6) + ASCII-dump snapshots + same-seed determinism.
- `render`: headless screenshot of a fixed seed/camera → PNG, multimodal review + image-snapshot regression.
- `web`: boots in a headless browser without panic; native↔wasm map parity within float tolerance.

**Exit criteria:** you can launch the inspector on any seed, travel the 6×6, peel layers to inspect resources, export an ASCII dump and a PNG, and the full `mapgen` invariant suite is green on both native and wasm.
