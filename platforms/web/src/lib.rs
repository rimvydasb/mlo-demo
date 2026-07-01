use wasm_bindgen::prelude::*;

#[wasm_bindgen(start)]
pub fn main() {
    voxel_app::run_inspector(42);
}
