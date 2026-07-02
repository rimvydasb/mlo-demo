## Technical specification

### Two separation axes

| Axis                 | Splits                                                         | Purpose                                                                                                                        |
|----------------------|----------------------------------------------------------------|--------------------------------------------------------------------------------------------------------------------------------|
| **Logic vs. engine** | `core` / `mapgen` / `sim` (pure) vs. `render` / `app` (engine) | Protects pure, deterministic, testable code from Bevy's pre-1.0 churn. Enforced by _dependency direction_.                     |
| **Native vs. WASM**  | thin `platforms/native` & `platforms/web` bins                 | Concentrates `cfg(target_arch = "wasm32")` and web glue in one place; keeps native builds clean. Enforced by _compile target_. |

The proposed "core / rendering / wasm" separation is correct but is these two axes combined.

### Workspace & repository structure

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

### Architecture options considered

| Option                                               | Pros                                                                                                    | Cons                                                                                                                              | Verdict                                                                            |
|------------------------------------------------------|---------------------------------------------------------------------------------------------------------|-----------------------------------------------------------------------------------------------------------------------------------|------------------------------------------------------------------------------------|
| **Hexagonal workspace** (pure core + adapter crates) | Max testability; engine replaceable; pure/impure boundary enforced at compile time; logic runs headless | Some Cargo ceremony; indirection                                                                                                  | **Recommended** — matches "sim is the product" + the pre-1.0-engine / LLM risk     |
| Single crate + feature flags + modules               | Minimal ceremony; fastest start; one tree for the LLM to navigate                                       | Boundary **not** enforced (LLM can leak `bevy` into generation); slower incremental builds at scale; harder to run logic headless | Acceptable only for a throwaway prototype                                          |
| Bevy plugin-per-feature                              | Very Bevy-native; toggle features; ecosystem-aligned                                                    | Plugins are engine-coupled (doesn't isolate logic); crate boilerplate                                                             | Use as **module** organization _inside_ `app`/`render`, not as the top-level split |

### Build, targets & automation

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

### Determinism rules (non-negotiable for LLM-legible results)

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
legend: . grass   s sand   ~ water   # stone
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
