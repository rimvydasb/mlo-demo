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