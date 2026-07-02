# Implementation Plan

## Phase 1 — Dev-mode map inspector (first deliverable) — **done**

**Goal:** generate and visually inspect maps, so biomes, cells, resources, and the cell→voxel visual mapping can be
refined _before_ any gameplay exists.

**Active crates:** `core`, `mapgen`, `render`, `app` (inspector mode only), `native`, `web`, `xtask`. **No `sim`.**

**Entry:** `cargo run -p native -- inspect --seed 42` boots straight into the inspector, focused on the seed's most
scenic biome.

**Delivered features**

- 6×6 world generation from a seed on the cell tier (12³ cells per biome): floating-island underground funnel with
  stone/iron/gold deposits, typed surfaces with ponds, world-noise relief with snow-capped mountain peaks.
- 3D render of the focused biome: cell→4³ voxel expansion + face-culled vertex-colored mesh (baked AO + per-voxel
  jitter), translucent water, deterministic underside erosion, soft shadows, distance fog. Other 35 biomes render as
  dimmed proxy slabs. See `docs/rendering.md` for the full renderer spec.
- **Travel:** orthographic orbit / pan / zoom camera; focus via the 6×6 grid panel, Tab, or arrow keys.
- **Inspect:** hover resolves to a **cell** (DDA raycast); panel shows type, resource, cell layer, band, and biome/world
  coordinates.
- **Layer peeling:** slider hides upper cell layers to expose the underground resource funnel.
- **Voxel-pattern preview:** inspector panel draws each cell type's exposed/covered voxel columns straight from the
  renderer's `cell_column` function.
- **Seed controls:** scrub seed, regenerate live, random seed, copy seed.
- **Exports:** ASCII dump (macro map, surface, relief heights, underground layers) and PNG screenshot — also available
  headlessly via `cargo xtask dump / screenshot / check`.

**Verification (in place)**

- `mapgen`: invariant suite + proptests + ASCII insta snapshots + same-seed determinism + resource-rarity bounds
  (`cargo test -p voxel-mapgen`).
- `render`: expansion-rule unit tests; screenshot review loop via `cargo xtask screenshot [--row N --col N]`.
- `web`: `cargo check -p web --target wasm32-unknown-unknown` stays green.

## Phase 2 — Presentation polish (next)

- Decoration props: glTF trees / bushes / era buildings placed on terrain (see "Decoration Props" in
  `docs/rendering.md`).
- Water polish: edge foam/rim tone, the water-transition "vortex" slope from the original spec.
- Rock face pockmark detail; image-snapshot regression for screenshots; headless-browser wasm boot check.

## Phase 3 — Simulation (`sim` crate, later)

- Towers, population allocation, mining against cell resources, era advancement, combat auto-resolve — per
  `docs/gameplay.md`.
