#![cfg(feature = "client")]

// Temporary debug harness: reproduces the device-row background painting of
// src/bin/client/midi/mod.rs to see what actually gets drawn.
use eframe::egui;
use egui::Color32;
use egui_extras::{Size, StripBuilder};

const BG: Color32 = Color32::from_gray(22);

fn painted_rects(frames: usize) -> Vec<egui::Rect> {
    let ctx = egui::Context::default();
    let raw_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(400.0, 600.0),
        )),
        ..Default::default()
    };

    let mut rects = Vec::new();
    for _ in 0..frames {
        let mut output = ctx.run_ui(raw_input.clone(), |ui| {
            StripBuilder::new(ui)
                .sizes(
                    Size::Absolute {
                        initial: 60.0,
                        range: egui::Rangef::new(0.0, 60.0),
                    },
                    3,
                )
                .vertical(|mut strip| {
                    for i in 0..3usize {
                        strip.cell(|ui| {
                            let mut rect = ui
                                .ctx()
                                .memory(|m| {
                                    m.data
                                        .get_temp::<egui::Rect>(egui::Id::new("device_rect").with(i))
                                })
                                .unwrap_or(ui.available_rect_before_wrap());
                            rect.set_width(ui.available_width());
                            if i % 2 == 0 {
                                ui.painter().rect_filled(rect, 5.0, BG);
                            }
                            ui.label(format!("row {i}"));
                            let min_rect = ui.min_rect();
                            ui.ctx().memory_mut(|m| {
                                m.data
                                    .insert_temp(egui::Id::new("device_rect").with(i), min_rect);
                            });
                        });
                    }
                });
        });

        rects = output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Rect(rect_shape) if rect_shape.fill == BG => Some(rect_shape.rect),
                _ => None,
            })
            .collect();
        output.textures_delta.clear();
    }
    rects
}

#[test]
fn debug_what_the_device_row_paints() {
    let rects = painted_rects(4);
    eprintln!("Painted {} rects (screen 400x600):", rects.len());
    for rect in &rects {
        eprintln!("  {rect:?}");
    }
    assert!(!rects.is_empty(), "Nothing was painted at all");
}

#[test]
fn debug_real_device_row_structure() {
    let ctx = egui::Context::default();
    let raw_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(
            egui::Pos2::ZERO,
            egui::vec2(400.0, 600.0),
        )),
        ..Default::default()
    };

    let row_height = 60.0;
    let mut rects: Vec<(egui::Rect, egui::Rect)> = Vec::new();
    // 2 device rows, the second one with its settings open
    for frame in 0..6 {
        let open = frame >= 1;
        let mut output = ctx.run_ui(raw_input.clone(), |ui| {
            StripBuilder::new(ui)
                .sizes(
                    Size::Absolute {
                        initial: row_height,
                        range: egui::Rangef::new(0.0, row_height),
                    },
                    2,
                )
                .vertical(|mut strip| {
                    for i in 0..2usize {
                        strip.cell(|ui| {
                            let mut rect = ui
                                .ctx()
                                .memory(|m| {
                                    m.data
                                        .get_temp::<egui::Rect>(egui::Id::new("device_rect").with(i))
                                })
                                .unwrap_or(ui.available_rect_before_wrap());
                            rect.set_width(ui.available_width());
                            if i % 2 == 0 {
                                ui.painter().rect_filled(rect, 5.0, BG);
                            }

                            StripBuilder::new(ui)
                                .size(Size::Absolute {
                                    initial: row_height,
                                    range: egui::Rangef::new(0.0, row_height),
                                })
                                .size(Size::Absolute {
                                    initial: 40.0,
                                    range: egui::Rangef::new(0.0, 40.0),
                                })
                                .vertical(|mut strip| {
                                    strip.strip(|builder| {
                                        builder
                                            .size(Size::Absolute {
                                                initial: 300.0,
                                                range: egui::Rangef::new(0.0, 300.0),
                                            })
                                            .size(Size::Absolute {
                                                initial: 100.0,
                                                range: egui::Rangef::new(0.0, 100.0),
                                            })
                                            .horizontal(|mut strip| {
                                                strip.cell(|ui| {
                                                    ui.horizontal_centered(|ui| {
                                                        ui.label(format!("device {i} - CC 1 Ch 1"));
                                                    });
                                                });
                                                strip.cell(|ui| {
                                                    ui.horizontal_centered(|ui| {
                                                        ui.button("Forget");
                                                    });
                                                });
                                            });
                                    });
                                    strip.cell(|ui| {
                                        ui.vertical_centered(|ui| {
                                            egui::CollapsingHeader::new("Device Settings")
                                                .id_salt(("device_settings", i))
                                                .default_open(open)
                                                .show(ui, |ui| {
                                                    ui.label("Current Value:");
                                                    ui.label("Rename:");
                                                    ui.label("Device Type:");
                                                });
                                        });
                                    });
                                });

                            let min_rect = ui.min_rect();
                            eprintln!(
                                "frame {frame} row {i}: available = {:?} clip = {:?} content min_rect = {min_rect:?}",
                                ui.available_rect_before_wrap(),
                                ui.clip_rect()
                            );
                            ui.ctx().memory_mut(|m| {
                                m.data.insert_temp(
                                    egui::Id::new("device_rect").with(i),
                                    min_rect,
                                );
                            });
                        });
                    }
                });
        });

        rects = output
            .shapes
            .iter()
            .filter_map(|clipped| match &clipped.shape {
                egui::Shape::Rect(rect_shape) if rect_shape.fill == BG => {
                    Some((rect_shape.rect, clipped.clip_rect))
                }
                _ => None,
            })
            .collect();
        output.textures_delta.clear();
        eprintln!("frame {frame}: painted (rect, clip) {rects:?}");
    }
}
