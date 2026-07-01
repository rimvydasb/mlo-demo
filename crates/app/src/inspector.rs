use bevy::prelude::*;
use bevy_egui::{egui, EguiContexts};
use voxel_core::{BiomeCoord, BiomeType, Material};
use voxel_render::{EguiWantsPointer, FocusedBiome, LayerCutoff};

use crate::state::{InspectorState, WorldMapResource};

fn biome_char(bt: BiomeType) -> char {
    match bt {
        BiomeType::Grass => '.',
        BiomeType::Sand  => 's',
        BiomeType::Water => '~',
        BiomeType::Rock  => '#',
    }
}

fn biome_label(bt: BiomeType) -> &'static str {
    match bt {
        BiomeType::Grass => "Grass",
        BiomeType::Sand  => "Sand",
        BiomeType::Water => "Water",
        BiomeType::Rock  => "Rock",
    }
}

pub fn egui_inspector(
    mut contexts:   EguiContexts,
    mut state:      ResMut<InspectorState>,
    mut world_map:  ResMut<WorldMapResource>,
    mut focused:    ResMut<FocusedBiome>,
    mut cutoff:     ResMut<LayerCutoff>,
    mut egui_wants: ResMut<EguiWantsPointer>,
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
        .default_size(220.0)
        .show_inside(&mut viewport_ui, |ui| {
            ui.heading("Map Inspector");
            ui.separator();

            // ── Seed controls ─────────────────────────────────────────────
            ui.label("Seed");
            ui.horizontal(|ui| {
                ui.text_edit_singleline(&mut state.seed_input);
                if ui.button("Go").clicked() {
                    if let Ok(s) = state.seed_input.trim().parse::<u64>() {
                        state.seed  = s;
                        world_map.0 = voxel_mapgen::generate(s);
                    }
                }
            });
            if ui.button("Random seed").clicked() {
                let s = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.as_nanos() as u64)
                    .unwrap_or(12345);
                state.seed       = s;
                state.seed_input = s.to_string();
                world_map.0      = voxel_mapgen::generate(s);
            }
            ui.separator();

            // ── Layer cutoff ───────────────────────────────────────────────
            ui.label("Layer cutoff (peel top-down)");
            let mut cut = cutoff.0 as u32;
            if ui.add(egui::Slider::new(&mut cut, 1u32..=12).text("show z <")).changed() {
                cutoff.0 = cut as u8;
            }
            ui.separator();

            // ── 6×6 biome selector ────────────────────────────────────────
            ui.label("6x6 biome grid (click to focus)");
            ui.label("legend: . grass  s sand  ~ water  # rock");
            egui::Grid::new("biome_grid")
                .spacing([3.0, 3.0])
                .show(ui, |ui| {
                    for row in 0..6u8 {
                        for col in 0..6u8 {
                            let coord      = BiomeCoord::new(row, col);
                            let bt         = world_map.0.biome_type(coord);
                            let is_focused = coord == focused.0;
                            let label      = biome_char(bt).to_string();
                            let btn = egui::Button::new(
                                egui::RichText::new(label).monospace()
                            )
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
            let c  = &world_map.0.connections;
            ui.label(format!(
                "Focused: ({},{}) — {}",
                focused.0.row, focused.0.col, biome_label(bt)
            ));
            for dir in voxel_core::EdgeDir::all() {
                let key = (focused.0, dir);
                if let Some(conn) = c.get(&key) {
                    ui.label(format!(
                        "  {:?}: {} ({})",
                        dir,
                        biome_label(conn.surface),
                        if conn.compatible { "connected" } else { "blocked" }
                    ));
                }
            }
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

    // ── Voxel inspector (right float) ────────────────────────────────────────
    if let Some((vx, vy, vz)) = state.hovered_voxel {
        egui::Window::new("Voxel")
            .anchor(egui::Align2::RIGHT_TOP, [-10.0, 10.0])
            .collapsible(false)
            .show(ctx, |ui| {
                ui.label(format!("Local  ({vx}, {vy}, {vz})"));
                let wx = focused.0.col as u16 * 12 + vx as u16;
                let wy = focused.0.row as u16 * 12 + vy as u16;
                ui.label(format!("World  ({wx}, {wy}, layer {})", vz + 1));

                let band = match vz {
                    0..=4 => "Underground (layers 1-5)",
                    5     => "Surface (layer 6)",
                    _     => "Relief (layers 7-12)",
                };
                ui.label(band);

                let grid  = world_map.0.biome(focused.0);
                let voxel = grid.get(vx, vy, vz);
                let mat_s = match voxel.material {
                    Material::Air         => "Air".to_string(),
                    Material::Ground(bt)  => format!("Ground({:?})", bt),
                    Material::Underground => "Underground".to_string(),
                };
                ui.label(format!("Material: {mat_s}"));
                if let Some(e) = voxel.element {
                    ui.label(format!("Element: {:?}", e));
                }
            });
    }

    egui_wants.0 = ctx.egui_wants_pointer_input();
    Ok(())
}
