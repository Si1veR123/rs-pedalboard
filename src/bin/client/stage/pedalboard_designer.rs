use core::f32;

use crate::stage::{parameter_window::{draw_parameter_window, ParameterWindowChange}, ClippingState, XRunState};

use super::PedalboardStageScreen;

use eframe::egui::{self, Button, Color32, Layout, Pos2, Rect, RichText, Sense, Ui, UiBuilder, Vec2, Widget};
use rs_pedalboard::pedals::{PedalDiscriminants, PedalParameterValue, PedalTrait};
use egui_dnd;
use strum::IntoEnumIterator;

const PEDAL_ROW_COUNT: usize = 6;
// Must be high enough to fit any pedal
// PEDAL_HEIGHT_RATIO * width = height
const PEDAL_HEIGHT_RATIO: f32 = 2.2;
const MAX_PEDAL_COUNT: usize = 12;

/// Assumes scene rect is smaller than available size
fn bound_scene_rect(scene_rect: &mut Rect, available_size: &Vec2) {
    let delta_max_x = available_size.x - scene_rect.max.x;
    let delta_max_y = available_size.y - scene_rect.max.y;

    scene_rect.min.x = scene_rect.min.x.max(0.0);
    scene_rect.min.y = scene_rect.min.y.max(0.0);

    if delta_max_x < 0.0 {
        scene_rect.min.x += delta_max_x;
        scene_rect.max.x += delta_max_x;
    }

    if delta_max_y < 0.0 {
        scene_rect.min.y += delta_max_y;
        scene_rect.max.y += delta_max_y;
    }
}

fn add_pedal_menu(screen: &mut PedalboardStageScreen, ui: &mut Ui, rect: Rect) {
    let menu_layer_id = egui::LayerId::new(egui::Order::Foreground, ui.id().with("pedal_menu"));
    let mut menu_ui = ui.new_child(
        UiBuilder::new()
            .layer_id(menu_layer_id)
            .max_rect(rect)
            .sense(Sense::hover()),
    );

    menu_ui.painter().rect_filled(
        menu_ui.available_rect_before_wrap(),
        5.0,
        Color32::from_gray(30),
    );

    egui::ScrollArea::vertical()
        .max_height(menu_ui.available_height())
        .show(&mut menu_ui, |ui| {
            ui.add_space(5.0);
            for pedal in PedalDiscriminants::iter() {
                if ui.add_sized(Vec2::new(ui.available_width()*0.95, 35.0), egui::Button::new(pedal.display_name())).clicked() {
                    let new_pedal = pedal.new_pedal();
                    screen.state.add_pedal(&new_pedal);
                    screen.show_pedal_menu = false
                }
                ui.separator();
            }
        });
}

fn current_time_string() -> String {
    format!("{}", chrono::Local::now().format("%H:%M:%S"))
}

pub fn pedalboard_designer(screen: &mut PedalboardStageScreen, ui: &mut Ui) {
    // Status bar at the top. Allocate a top down ui for padding, then a left to right ui inside.
    let vertical_padding = 5.0;
    ui.allocate_ui_with_layout(
        Vec2::new(ui.available_width(), ui.available_height()*0.075 + vertical_padding*2.0),
        Layout::top_down(egui::Align::Center),
        |ui| {
            ui.painter().rect_filled(ui.available_rect_before_wrap(), 5.0, crate::LIGHT_BACKGROUND_COLOR);

            ui.add_space(vertical_padding);

            ui.allocate_ui_with_layout(
                ui.available_size() - Vec2::new(0.0, vertical_padding), // Subtract the amount of padding that will be added after
                Layout::left_to_right(egui::Align::Center),
                |ui| {
                    let can_show_add_button = {
                        let mut pedalboard_set = screen.state.pedalboards.active_pedalboardstage.borrow_mut();
                        let active_index = pedalboard_set.active_pedalboard;
                        let pedalboard = pedalboard_set.pedalboards.get_mut(active_index).unwrap();
                        pedalboard.pedals.len() < MAX_PEDAL_COUNT
                    };

                    ui.add_space(20.0);
                    if ui
                        .add_enabled_ui(
                            can_show_add_button,
                            |ui| {
                                ui.add_sized(
                                    [140.0, ui.available_height()],
                                    egui::Button::new(RichText::new("Add Pedal")).stroke(egui::Stroke::new(1.0, crate::THEME_COLOUR))
                                )
                            },
                        )
                        .inner
                        .clicked()
                    {
                        screen.show_pedal_menu = !screen.show_pedal_menu;
                    };
                    ui.add_space(20.0);

                    ui.columns_const(|[ui_1, ui_2, ui_3, ui_4, ui_5]| {
                        if screen.state.is_connected() {
                            // XRun monitor
                            ui_1.allocate_ui_with_layout(
                                ui_1.available_size(),
                                Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.add_space(10.0);
                                    let xrun_color = match screen.xrun_state {
                                        XRunState::None => Color32::from_rgb(50, 255, 50),
                                        XRunState::Few(_) => Color32::from_rgb(255, 165, 50),
                                        XRunState::Many(_) => Color32::from_rgb(255, 50, 50),
                                    };

                                    ui.label("XRun");
                                    let (_id, rect) = ui.allocate_space(Vec2::splat(20.0));
                                    ui.painter().rect_filled(rect, 2.0, xrun_color);
                                },
                            );

                            // Clipping monitor
                            ui_2.allocate_ui_with_layout(
                                ui_2.available_size(),
                                Layout::left_to_right(egui::Align::Center),
                                |ui| {
                                    ui.label("Clip");
                                    let clipping_color = match screen.clipping_state {
                                        ClippingState::None => Color32::from_rgb(50, 255, 50),
                                        ClippingState::Clipping(_) => Color32::from_rgb(255, 50, 50),
                                    };
                                    let (_id, rect) = ui.allocate_space(Vec2::splat(20.0));
                                    ui.painter().rect_filled(rect, 2.0, clipping_color);
                                },
                            );
                        }

                        let col_vertical_padding = (ui_3.available_height() - 20.0) * 0.5;
                        // CPU Usage
                        ui_3.with_layout(Layout::top_down(egui::Align::Center), |ui| {
                            ui.add_space(col_vertical_padding);
                            let cpu_usage = screen.system.global_cpu_usage();
                            ui.label(format!("CPU: {:.0}%", cpu_usage.round()));
                        });

                        // RAM Usage
                        ui_4.with_layout(Layout::top_down(egui::Align::Center), |ui| {
                            ui.add_space(col_vertical_padding);
                            let memory = screen.system.total_memory();
                            let used_memory = screen.system.used_memory();
                            let memory_usage = used_memory as f32 / memory as f32;
                            ui.label(format!("RAM: {:.0}%", (memory_usage * 100.0).round()));
                        });

                        // Time
                        ui_5.with_layout(Layout::top_down(egui::Align::Center), |ui| {
                            ui.add_space(col_vertical_padding);
                            ui.label(current_time_string());
                        });
                    });
                },
            );

            ui.add_space(vertical_padding);
        },
    );

    // Available rect for the pedalboard itself
    let available_rect = ui.available_rect_before_wrap();
    let drawing_volume_monitor = screen.state.client_settings.borrow().show_volume_monitor && screen.state.is_connected();
    let volume_monitor_width = 5.0;
    let volume_monitor_inside_padding = 0.0;
    let volume_monitor_outside_padding = 5.0;

    let mut pedalboard_available_rect = available_rect;

    // If drawing volume monitor, we can have more y pedal spacing to make up for the less horizontal space
    let pedal_y_spacing: f32;
    if drawing_volume_monitor {
        pedalboard_available_rect = pedalboard_available_rect.shrink2(Vec2::new(
            volume_monitor_width * 2.0 + (volume_monitor_inside_padding + volume_monitor_outside_padding)*2.0,
            0.0
        ));
        pedal_y_spacing = 25.0;
    } else {
        pedal_y_spacing = 10.0;
    }

    let pedal_width = 0.9 * (pedalboard_available_rect.width() / PEDAL_ROW_COUNT as f32);
    let pedal_x_spacing = 0.1 * (pedalboard_available_rect.width() / PEDAL_ROW_COUNT as f32);

    ui.painter().rect_filled(pedalboard_available_rect, 5.0, crate::LIGHT_BACKGROUND_COLOR);

    // Initially set to ZERO, so fill in with available pedalboard rect
    if screen.pedalboard_rect == Rect::ZERO {
        screen.pedalboard_rect = Rect::from_min_size(Pos2::ZERO, pedalboard_available_rect.size());
    }

    // Delete pedal hover button
    let size = 150.0;
    let delete_button_rect = Rect::from_min_size(
        pedalboard_available_rect.max - Vec2::splat(size + 5.0),
        Vec2::splat(size),
    );
    let mut button_ui = ui.new_child(UiBuilder::new()
        .layer_id(egui::LayerId::new(egui::Order::Foreground, ui.id().with("delete_button")))
        .max_rect(delete_button_rect));

    let mut changed: Option<(usize, (String, PedalParameterValue))> = None;
    ui.horizontal(|ui| {
        if drawing_volume_monitor {
            // Input Volume Monitor
            ui.add_space(volume_monitor_outside_padding);
            ui.allocate_ui(Vec2::new(volume_monitor_width, available_rect.height()), |ui| {
                screen.volume_monitors.0.ui(ui)
            });
            ui.add_space(volume_monitor_inside_padding);
        }

        // Main pedalboard rendering
        ui.allocate_ui(pedalboard_available_rect.size(), |ui| {
            changed = egui::Scene::new().zoom_range(1.0..=3.0).show(ui, &mut screen.pedalboard_rect, |ui| {
                ui.allocate_new_ui(
                    UiBuilder::new()
                        .max_rect(Rect { min: Pos2::ZERO, max: pedalboard_available_rect.size().to_pos2() })
                        .layout(Layout::left_to_right(egui::Align::Min)),
                    |ui| {
                        ui.add_space(pedal_x_spacing/2.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(pedal_x_spacing, pedal_y_spacing);
                            let mut changed = None;
        
                            let mut pedalboard_set = screen.state.pedalboards.active_pedalboardstage.borrow_mut();
                            let active_index = pedalboard_set.active_pedalboard;
                            let pedalboard = pedalboard_set.pedalboards.get_mut(active_index).unwrap();
        
                            let dnd_response = egui_dnd::dnd(ui, "pedalboard_designer_dnd").show_sized(pedalboard.pedals.iter_mut().enumerate(), Vec2::new(pedal_width, pedal_width*PEDAL_HEIGHT_RATIO), |ui, (i, item), handle, _state| {
                                let whole_pedal_rect = ui.available_rect_before_wrap();
                                ui.allocate_ui_with_layout(Vec2::new(pedal_width, pedal_width*PEDAL_HEIGHT_RATIO*0.95), Layout::top_down(egui::Align::Center), |ui| {
                                    ui.spacing_mut().item_spacing = Vec2::ZERO;
                                    
                                    let mut command_buffer = Vec::new();
                                    screen.state.get_commands(&format!("pedalmsg{i}"), &mut command_buffer);
                                    if let Some(v) = item.ui(ui, &command_buffer) {
                                        changed = Some((i, v));
                                    }
                                });
        
                                let button_rect = whole_pedal_rect.with_min_y(whole_pedal_rect.max.y - 0.05 * whole_pedal_rect.height());
                                ui.allocate_new_ui(UiBuilder::new().max_rect(button_rect), |ui| {
                                    handle.sense(egui::Sense::DRAG).ui_sized(
                                        ui,
                                        ui.available_size(),
                                        |ui| {
                                            if ui.add_sized(ui.available_size(), Button::new("Drag").sense(egui::Sense::click())).clicked() {
                                                // Open the parameter window
                                                let window_open_id = super::parameter_window::get_window_open_id(item);
                                                ui.ctx().data_mut(
                                                    |r| r.insert_temp(window_open_id, !r.get_temp(window_open_id).unwrap_or(false))
                                                );
                                            };
                                        }
                                    );
                                });
                            });
        
                            let mouse_over_delete = delete_button_rect.contains(ui.ctx().input(|i| i.pointer.hover_pos()).unwrap_or(Pos2::ZERO));
        
                            if dnd_response.is_dragging() {
                                let button = if mouse_over_delete {
                                    Button::new("Delete").fill(Color32::RED.gamma_multiply(0.3))
                                } else {
                                    Button::new("Delete")
                                };
        
                                button_ui.put(button_ui.available_rect_before_wrap(), button);
                            }
        
                            if dnd_response.is_drag_finished() {
                                if let Some(update) = &dnd_response.update {
                                    if mouse_over_delete {
                                        if ui.ctx().input(|i| i.pointer.any_released()) && pedalboard.pedals.len() > 1 {
                                            drop(pedalboard_set);
                                            screen.state.delete_pedal(active_index, update.from);
                                        }
                                    } else {
                                        drop(pedalboard_set);
                                        screen.state.move_pedal(active_index, update.from, update.to);
                                    }
                                }
                            }
        
                            changed
                        }).inner
                    }
                ).inner
            }).inner;
        });
    
        bound_scene_rect(&mut screen.pedalboard_rect, &pedalboard_available_rect.size());

        if drawing_volume_monitor {
            ui.add_space(volume_monitor_inside_padding);

            // Output Volume Monitor
            ui.allocate_ui(Vec2::new(volume_monitor_width, available_rect.height()), |ui| {
                screen.volume_monitors.1.ui(ui)
            });
        }
        
    });

    // Draw any open parameter windows
    {
        let active_pedalboards = screen.state.pedalboards.active_pedalboardstage.borrow();
        for (i, pedal) in active_pedalboards.pedalboards[active_pedalboards.active_pedalboard].pedals.iter().enumerate() {
            match draw_parameter_window(ui, pedal) {
                Some(ParameterWindowChange::ParameterChanged(name, value)) => changed = Some((i, (name, value))),
                None => {},
            }
        }
    }
    
    let active_index = {
        let pedalboard_set = screen.state.pedalboards.active_pedalboardstage.borrow_mut();
        pedalboard_set.active_pedalboard
    };

    if let Some((pedal_index, (name, value))) = changed {
        screen.state.set_parameter(
            active_index,
            pedal_index,
            &name,
            &value,
            false
        );
    }

    if screen.show_pedal_menu {
        add_pedal_menu(screen, ui, pedalboard_available_rect.scale_from_center2(Vec2::new(0.6, 0.9)));
    }
}