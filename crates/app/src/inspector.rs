use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use voxel_core::{BiomeCoord, BiomeType, CellType, CELLS, SNOW_Z, SURFACE_Z, WORLD_BIOMES};
use voxel_render::{
    base_color_srgb, cell_column, EguiWantsPointer, FocusedBiome, LayerCutoff, VoxelKind, SUB,
};

use crate::decor::DecorEnabled;
use crate::state::{InspectorState, WorldMapResource};

fn biome_char(bt: BiomeType) -> char {
    match bt {
        BiomeType::Grass => '.',
        BiomeType::Sand => 's',
        BiomeType::Water => '~',
        BiomeType::Rock => '#',
    }
}

pub fn egui_inspector(
    mut contexts: EguiContexts,
    mut state: ResMut<InspectorState>,
    mut world_map: ResMut<WorldMapResource>,
    mut focused: ResMut<FocusedBiome>,
    mut cutoff: ResMut<LayerCutoff>,
    mut egui_wants: ResMut<EguiWantsPointer>,
    mut decor_enabled: ResMut<DecorEnabled>,
) -> Result {
    let ctx = contexts.ctx_mut()?;

    // Build a viewport-sized Ui that panels can tile into.
    let mut viewport_ui = egui::Ui::new(
        ctx.clone(),
        "viewport".into(),
        egui::UiBuilder::new()
            .layer_id(egui::LayerId::background())
            .max_rect(ctx.viewport_rect()),
    );

    egui::Panel::left("inspector_left")
        .resizable(true)
        .default_size(230.0)
        .show_inside(&mut viewport_ui, |ui| {
            ui.heading("Map Inspector");
            ui.separator();

            // ── Seed controls ─────────────────────────────────────────────
            ui.label("Seed");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut state.seed_input);
                if ui.button("Go").clicked() {
                    if let Ok(s) = state.seed_input.trim().parse::<u64>() {
                        state.seed = s;
                        world_map.0 = voxel_mapgen::generate(s);
                    }
                }
            });
            ui.horizontal(|ui| {
                if ui.button("Random seed").clicked() {
                    let s = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_nanos() as u64)
                        .unwrap_or(12345);
                    state.seed = s;
                    state.seed_input = s.to_string();
                    world_map.0 = voxel_mapgen::generate(s);
                }
                if ui.button("Copy").clicked() {
                    ctx.copy_text(state.seed.to_string());
                }
            });
            ui.separator();

            // ── Layer peel ────────────────────────────────────────────────
            ui.label("Layer peel (hide upper cell layers)");
            let mut cut = cutoff.0 as u32;
            if ui
                .add(egui::Slider::new(&mut cut, 1u32..=CELLS as u32).text("layers"))
                .changed()
            {
                cutoff.0 = cut as u8;
            }
            let mut decor_on = decor_enabled.0;
            if ui.checkbox(&mut decor_on, "Fauna & flora").changed() {
                decor_enabled.0 = decor_on;
            }
            ui.separator();

            // ── 6×6 biome selector ────────────────────────────────────────
            ui.label("6×6 biome grid (click to focus)");
            ui.label("legend: . grass  s sand  ~ water  # rock");
            egui::Grid::new("biome_grid")
                .spacing([3.0, 3.0])
                .show(ui, |ui| {
                    for row in 0..WORLD_BIOMES as u8 {
                        for col in 0..WORLD_BIOMES as u8 {
                            let coord = BiomeCoord::new(row, col);
                            let bt = world_map.0.biome_type(coord);
                            let is_focused = coord == focused.0;
                            let label = biome_char(bt).to_string();
                            let btn = egui::Button::new(egui::RichText::new(label).monospace())
                                .selected(is_focused)
                                .min_size(egui::vec2(22.0, 22.0));
                            if ui.add(btn).clicked() {
                                focused.0 = coord;
                            }
                        }
                        ui.end_row();
                    }
                });
            ui.separator();

            // ── Focused biome info ────────────────────────────────────────
            let bt = world_map.0.biome_type(focused.0);
            ui.label(format!(
                "Focused: ({},{}) — {}",
                focused.0.row,
                focused.0.col,
                bt.label()
            ));
            for dir in voxel_core::EdgeDir::all() {
                if let Some(conn) = world_map.0.connection(focused.0, dir) {
                    ui.label(format!(
                        "  {:?}: {} ({})",
                        dir,
                        conn.surface.label(),
                        if conn.compatible {
                            "connected"
                        } else {
                            "blocked"
                        }
                    ));
                }
            }
            ui.separator();

            // ── Cell → voxel expansion preview (dev aid) ──────────────────
            ui.collapsing("Cell → voxel patterns", |ui| {
                expansion_preview(ui);
            });
            ui.separator();

            // ── Export ────────────────────────────────────────────────────
            if ui.button("ASCII Dump (stdout)").clicked() {
                state.export_ascii = true;
            }
            if ui.button("Save PNG screenshot").clicked() {
                state.export_png = true;
            }

            ui.separator();
            ui.label("Navigation: Tab / Arrow keys");
            ui.label("Camera: drag=orbit  scroll=zoom  mid=pan");
        });

    // ── Cell inspector (right float) ─────────────────────────────────────────
    if let Some((cx, cy, cz)) = state.hovered_cell {
        egui::Window::new("Cell")
            .anchor(egui::Align2::RIGHT_TOP, [-10.0, 10.0])
            .collapsible(false)
            .show(ctx, |ui| {
                let grid = world_map.0.biome(focused.0);
                let cell = grid.get(cx, cy, cz);

                ui.label(format!("Local  ({cx}, {cy}, {cz})"));
                let wx = focused.0.col as u16 * CELLS as u16 + cx as u16;
                let wy = focused.0.row as u16 * CELLS as u16 + cy as u16;
                ui.label(format!("World  ({wx}, {wy})  cell layer {}", cz + 1));

                let band = if cz < SURFACE_Z {
                    "Underground (layers 1–5)"
                } else if cz == SURFACE_Z {
                    "Surface (layer 6)"
                } else {
                    "Relief (layers 7–12)"
                };
                ui.label(band);
                ui.label(format!("Type: {}", cell.label()));
                match cell.resource() {
                    Some(res) => ui.label(format!("Resource: {res:?}")),
                    None => ui.label("Resource: —"),
                };
            });
    }

    egui_wants.0 = ctx.egui_wants_pointer_input();
    Ok(())
}

/// Draws each cell type's 4-voxel column (exposed and covered variants) so
/// palette/pattern tweaks are reviewable without regenerating a map.
fn expansion_preview(ui: &mut egui::Ui) {
    const TYPES: [CellType; 6] = [
        CellType::Soil,
        CellType::Sand,
        CellType::Water,
        CellType::Stone,
        CellType::Gold,
        CellType::Iron,
    ];

    ui.label("columns: exposed / covered (top voxel uppermost)");
    egui::Grid::new("expansion_preview")
        .spacing([10.0, 4.0])
        .show(ui, |ui| {
            for cell in TYPES {
                ui.label(cell.label());
                // Use the snow band for stone so the exposed variant shows
                // the snow cap.
                let cz = if cell == CellType::Stone {
                    SNOW_Z
                } else {
                    SURFACE_Z
                };
                column_swatch(ui, cell_column(cell, true, cz));
                column_swatch(ui, cell_column(cell, false, cz));
                ui.end_row();
            }
        });
}

fn column_swatch(ui: &mut egui::Ui, column: [VoxelKind; SUB]) {
    let cell_px = 11.0;
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(cell_px, cell_px * SUB as f32),
        egui::Sense::hover(),
    );
    let painter = ui.painter();
    for (dz, kind) in column.iter().enumerate() {
        // dz=0 is the bottom voxel — draw it lowest.
        let top = rect.bottom() - (dz as f32 + 1.0) * cell_px;
        let r =
            egui::Rect::from_min_size(egui::pos2(rect.left(), top), egui::vec2(cell_px, cell_px));
        match kind {
            VoxelKind::Air => {
                painter.rect_stroke(
                    r.shrink(1.0),
                    0.0,
                    egui::Stroke::new(0.6, egui::Color32::GRAY),
                    egui::StrokeKind::Inside,
                );
            }
            k => {
                let [cr, cg, cb] = base_color_srgb(*k);
                painter.rect_filled(
                    r.shrink(0.5),
                    0.0,
                    egui::Color32::from_rgb(
                        (cr * 255.0) as u8,
                        (cg * 255.0) as u8,
                        (cb * 255.0) as u8,
                    ),
                );
            }
        }
    }
}
