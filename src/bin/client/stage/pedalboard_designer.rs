use core::f32;

use crate::stage::{
    parameter_window::{draw_parameter_window, ParameterWindowChange},
    ClippingState, XRunState,
};

use super::PedalboardStageScreen;

use eframe::egui::{
    self, Button, Color32, Layout, Pos2, Rect, RichText, Sense, Ui, UiBuilder, Vec2, Widget,
};
use rs_pedalboard::pedals::{PedalDiscriminants, PedalParameterValue, PedalTrait};
use strum::IntoEnumIterator;

const PEDAL_ROW_COUNT: usize = 6;
// Must be high enough to fit any pedal
// PEDAL_HEIGHT_RATIO * width = height
const PEDAL_HEIGHT_RATIO: f32 = 2.2;
const MAX_PEDAL_COUNT: usize = 12;

/// How many rows the CPU/RAM/time column in the status bar has
const CPU_RAM_TIME_ROW_COUNT: f32 = 3.0;

const MIN_ZOOM: f32 = 1.0;
const MAX_ZOOM: f32 = 3.0;
/// How much one tap on the zoom buttons changes the zoom
const ZOOM_STEP: f32 = 1.5;

/// The scene rect (camera) to use for a pedalboard rect of `available_size` at
/// the given zoom. The Scene scales its content by
/// `available_size / scene_rect.size()`, so deriving the scene rect from the
/// current available size and zoom makes the rendered zoom exactly `zoom`, no
/// matter how the available rect changed since the scene rect was created.
fn scene_rect_for(center: Pos2, available_size: Vec2, zoom: f32) -> Rect {
    Rect::from_center_size(center, available_size / zoom)
}

/// Size of the pedals in scene coordinates, mirroring the layout in
/// [`pedalboard_designer`]. Used to bound the camera so that every pedal can be
/// reached by panning and zooming.
fn pedalboard_content_size(
    pedal_width: f32,
    pedal_x_spacing: f32,
    pedal_y_spacing: f32,
    pedal_count: usize,
) -> Vec2 {
    let columns = pedal_count.clamp(1, PEDAL_ROW_COUNT);
    let rows = pedal_count.div_ceil(PEDAL_ROW_COUNT).max(1);
    let pedal_height = pedal_width * PEDAL_HEIGHT_RATIO;

    Vec2::new(
        pedal_x_spacing * 0.5
            + columns as f32 * pedal_width
            + (columns as f32 - 1.0) * pedal_x_spacing,
        rows as f32 * pedal_height + (rows as f32 - 1.0) * pedal_y_spacing,
    )
}

/// Bounds the scene rect (the camera) to the pedal content, so that the content
/// can always be reached by panning and so that the camera can't be dragged
/// into empty space. The camera size, and therefore the zoom, is left alone.
fn bound_scene_rect(scene_rect: &mut Rect, content_size: Vec2) {
    let camera_size = scene_rect.size();
    // When the content is smaller than the camera on an axis, the only valid
    // position on that axis is the start of the content
    let max_min = (content_size - camera_size).max(Vec2::ZERO);
    let min = Pos2::new(
        scene_rect.min.x.clamp(0.0, max_min.x),
        scene_rect.min.y.clamp(0.0, max_min.y),
    );

    *scene_rect = Rect::from_min_size(min, camera_size);
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
                if ui
                    .add_sized(
                        Vec2::new(ui.available_width() * 0.95, 35.0),
                        egui::Button::new(pedal.display_name()),
                    )
                    .clicked()
                {
                    let new_pedal = pedal.new_pedal();
                    screen.state.add_pedal_to_active(&new_pedal, false);
                    screen.show_pedal_menu = false
                }
                ui.separator();
            }
        });
}

fn current_time_string() -> String {
    format!("{}", chrono::Local::now().format("%H:%M:%S"))
}

#[tracing::instrument(level = "trace", skip_all)]
pub fn pedalboard_designer(screen: &mut PedalboardStageScreen, ui: &mut Ui) {
    // Status bar at the top. Allocate a top down ui for padding, then a left to right ui inside.
    // It has to be tall enough for the CPU/RAM/time column, which is three rows
    let vertical_padding = 5.0;
    let rows_height = ui.text_style_height(&egui::TextStyle::Body) * CPU_RAM_TIME_ROW_COUNT;
    ui.allocate_ui_with_layout(
        Vec2::new(
            ui.available_width(),
            (ui.available_height() * 0.075).max(rows_height) + vertical_padding * 2.0,
        ),
        Layout::top_down(egui::Align::Center),
        |ui| {
            ui.painter().rect_filled(
                ui.available_rect_before_wrap(),
                5.0,
                crate::LIGHT_BACKGROUND_COLOR,
            );

            ui.add_space(vertical_padding);

            ui.allocate_ui_with_layout(
                ui.available_size() - Vec2::new(0.0, vertical_padding), // Subtract the amount of padding that will be added after
                Layout::left_to_right(egui::Align::Center),
                |ui| {
                    let can_show_add_button = {
                        let mut pedalboard_set =
                            screen.state.pedalboards.active_pedalboardstage.borrow_mut();
                        let active_index = pedalboard_set.active_pedalboard;
                        let pedalboard = pedalboard_set.pedalboards.get_mut(active_index).unwrap();
                        pedalboard.pedals.len() < MAX_PEDAL_COUNT
                    };

                    ui.add_space(20.0);
                    if ui
                        .add_enabled_ui(can_show_add_button, |ui| {
                            ui.add_sized(
                                [ui.available_width() * 0.25, ui.available_height()],
                                egui::Button::new(RichText::new("Add Pedal"))
                                    .stroke(egui::Stroke::new(1.0_f32, crate::THEME_COLOR)),
                            )
                        })
                        .inner
                        .clicked()
                    {
                        screen.show_pedal_menu = !screen.show_pedal_menu;
                    };
                    ui.add_space(20.0);

                    // Zoom controls for the pedalboard. Touch panels that present
                    // themselves as a mouse can't send pinch gestures, so the
                    // zoom has to be settable with buttons
                    let zoom_button_size =
                        Vec2::splat(ui.available_height().min(ui.available_width() * 0.08));
                    let zoom = screen.pedalboard_zoom;

                    if ui.add_sized(zoom_button_size, Button::new("-")).clicked() {
                        screen.pedalboard_zoom = (zoom / ZOOM_STEP).clamp(MIN_ZOOM, MAX_ZOOM);
                    }

                    if ui.add_sized(zoom_button_size, Button::new("+")).clicked() {
                        screen.pedalboard_zoom = (zoom * ZOOM_STEP).clamp(MIN_ZOOM, MAX_ZOOM);
                    }

                    ui.add_space(20.0);

                    ui.columns_const(|[ui_1, ui_2, ui_3]| {
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
                                        ClippingState::Clipping(_) => {
                                            Color32::from_rgb(255, 50, 50)
                                        }
                                    };
                                    let (_id, rect) = ui.allocate_space(Vec2::splat(20.0));
                                    ui.painter().rect_filled(rect, 2.0, clipping_color);
                                },
                            );
                        }

                        // CPU, RAM and time, stacked in one column so that the
                        // status bar has room for the zoom buttons
                        ui_3.with_layout(Layout::top_down(egui::Align::Center), |ui| {
                            ui.spacing_mut().item_spacing = Vec2::ZERO;

                            let cpu_usage = screen.system.global_cpu_usage();
                            let memory = screen.system.total_memory();
                            let used_memory = screen.system.used_memory();
                            let memory_usage = used_memory as f32 / memory as f32;

                            let rows = [
                                format!("CPU: {:.0}%", cpu_usage.round()),
                                format!("RAM: {:.0}%", (memory_usage * 100.0).round()),
                                current_time_string(),
                            ];

                            // Vertically center the rows in the status bar
                            let rows_height =
                                ui.text_style_height(&egui::TextStyle::Body) * rows.len() as f32;
                            ui.add_space(((ui.available_height() - rows_height) * 0.5).max(0.0));

                            for row in rows {
                                ui.label(row);
                            }
                        });
                    });
                },
            );

            ui.add_space(vertical_padding);
        },
    );

    // Available rect for the pedalboard itself
    let available_rect = ui.available_rect_before_wrap();
    let drawing_volume_monitor =
        screen.state.client_settings.borrow().show_volume_monitor && screen.state.is_connected();
    let volume_monitor_width = 5.0;
    let volume_monitor_inside_padding = 0.0;
    let volume_monitor_outside_padding = 5.0;

    let mut pedalboard_available_rect = available_rect;

    // If drawing volume monitor, we can have more y pedal spacing to make up for the less horizontal space
    let pedal_y_spacing: f32;
    if drawing_volume_monitor {
        pedalboard_available_rect = pedalboard_available_rect.shrink2(Vec2::new(
            volume_monitor_width * 2.0
                + (volume_monitor_inside_padding + volume_monitor_outside_padding) * 2.0,
            0.0,
        ));
        pedal_y_spacing = 25.0;
    } else {
        pedal_y_spacing = 10.0;
    }

    let pedal_width = 0.9 * (pedalboard_available_rect.width() / PEDAL_ROW_COUNT as f32);
    let pedal_x_spacing = 0.1 * (pedalboard_available_rect.width() / PEDAL_ROW_COUNT as f32);

    let pedal_count = {
        let pedalboard_set = screen.state.pedalboards.active_pedalboardstage.borrow();
        pedalboard_set.pedalboards[pedalboard_set.active_pedalboard]
            .pedals
            .len()
    };

    ui.painter().rect_filled(
        pedalboard_available_rect,
        5.0,
        crate::LIGHT_BACKGROUND_COLOR,
    );

    // The scene rect is the camera of the Scene, in pedalboard (scene)
    // coordinates. It is derived from the current available rect and zoom every
    // frame, because the Scene scales its content by
    // `available_size / scene_rect.size()`. Only setting it once (as before)
    // meant that every later change of the available rect - the window sizing
    // itself on startup, the volume monitor being shown or hidden, the status
    // bar needing more room, ... - silently changed the zoom and left the camera
    // a few pixels off, which could then be panned into and snapped back from.
    let pedalboard_scene_center = if screen.pedalboard_rect == Rect::ZERO {
        // Initially set to ZERO, so start at the center of the pedalboard rect
        (pedalboard_available_rect.size() * 0.5).to_pos2()
    } else {
        screen.pedalboard_rect.center()
    };
    screen.pedalboard_rect = scene_rect_for(
        pedalboard_scene_center,
        pedalboard_available_rect.size(),
        screen.pedalboard_zoom,
    );

    // Delete pedal hover button
    let size = 150.0;
    let delete_button_rect = Rect::from_min_size(
        pedalboard_available_rect.max - Vec2::splat(size + 5.0),
        Vec2::splat(size),
    );
    let mut button_ui = ui.new_child(
        UiBuilder::new()
            .layer_id(egui::LayerId::new(
                egui::Order::Foreground,
                ui.id().with("delete_button"),
            ))
            .max_rect(delete_button_rect),
    );

    let mut changed: Option<(u32, (String, PedalParameterValue))> = None;
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
            egui::Scene::new().zoom_range(MIN_ZOOM..=MAX_ZOOM).show(ui, &mut screen.pedalboard_rect, |ui| {
                ui.scope_builder(
                    UiBuilder::new()
                        .max_rect(Rect { min: Pos2::ZERO, max: pedalboard_available_rect.size().to_pos2() })
                        .layout(Layout::left_to_right(egui::Align::Min)),
                    |ui| {
                        ui.add_space(pedal_x_spacing/2.0);
                        ui.horizontal_wrapped(|ui| {
                            ui.spacing_mut().item_spacing = Vec2::new(pedal_x_spacing, pedal_y_spacing);

                            let mut pedalboard_set = screen.state.pedalboards.active_pedalboardstage.borrow_mut();
                            let active_index = pedalboard_set.active_pedalboard;
                            let active_pedalboard = &mut pedalboard_set.pedalboards[active_index];
                            let active_id = active_pedalboard.get_id();

                            let dnd_response = egui_dnd::dnd(ui, "pedalboard_designer_dnd").show_sized(active_pedalboard.pedals.iter_mut(), Vec2::new(pedal_width, pedal_width*PEDAL_HEIGHT_RATIO), |ui, pedal, handle, _state| {
                                let whole_pedal_rect = ui.available_rect_before_wrap();
                                ui.allocate_ui_with_layout(Vec2::new(pedal_width, pedal_width*PEDAL_HEIGHT_RATIO*0.95), Layout::top_down(egui::Align::Center), |ui| {
                                    ui.spacing_mut().item_spacing = Vec2::ZERO;

                                    let mut command_buffer = Vec::new();
                                    screen.state.get_commands(&format!("pedalmsg{}", pedal.get_id()), &mut command_buffer);
                                    if let Some(v) = pedal.ui(ui, &command_buffer) {
                                        changed = Some((pedal.get_id(), v));
                                    }
                                });

                                let button_rect = whole_pedal_rect.with_min_y(whole_pedal_rect.max.y - 0.05 * whole_pedal_rect.height());
                                ui.scope_builder(UiBuilder::new().max_rect(button_rect), |ui| {
                                    handle.sense(egui::Sense::DRAG).ui_sized(
                                        ui,
                                        ui.available_size(),
                                        |ui| {
                                            if ui.add_sized(ui.available_size(), Button::new("Click/Drag").sense(egui::Sense::click())).clicked() {
                                                // Open the parameter window
                                                let window_open_id = super::parameter_window::get_window_open_id(pedal);
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
                                    let pedal_id = active_pedalboard.pedals[update.from].get_id();
                                    if mouse_over_delete {
                                        if ui.ctx().input(|i| i.pointer.any_released()) {
                                            drop(pedalboard_set);
                                            screen.state.delete_pedal(active_id, pedal_id, false);
                                        }
                                    } else {
                                        drop(pedalboard_set);
                                        screen.state.move_pedal(active_id, pedal_id, update.to, false);
                                    }
                                }
                            }
                        })
                    }
                )
            });
        });

        // The Scene zooms itself for pinch gestures (multi-touch screens) and
        // ctrl+scroll (mouse), so adopt whatever zoom it applied, then size the
        // camera to exactly `available_size / zoom` again so the scale can't
        // drift
        let scene_scale =
            (pedalboard_available_rect.size() / screen.pedalboard_rect.size()).min_elem();
        if scene_scale.is_finite() && scene_scale > 0.0 {
            screen.pedalboard_zoom = scene_scale.clamp(MIN_ZOOM, MAX_ZOOM);
        }
        screen.pedalboard_rect.max = screen.pedalboard_rect.min
            + pedalboard_available_rect.size() / screen.pedalboard_zoom;

        // Keep the camera inside the pedals, so that all pedals can be reached
        // by panning and the camera can't be dragged into empty space
        bound_scene_rect(
            &mut screen.pedalboard_rect,
            pedalboard_content_size(pedal_width, pedal_x_spacing, pedal_y_spacing, pedal_count),
        );

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
        let mut active_pedalboards = screen.state.pedalboards.active_pedalboardstage.borrow_mut();
        let active_pedalboard = active_pedalboards.active_pedalboard;
        let active_pedalboard_id = active_pedalboards.pedalboards[active_pedalboard].get_id();

        if PedalboardStageScreen::check_cached_midi_devices_invalid(ui.ctx()) {
            screen.cached_midi_devices =
                screen.state.midi_state.borrow().get_all_parameter_devices();
        }

        for pedal in active_pedalboards.pedalboards[active_pedalboard]
            .pedals
            .iter_mut()
        {
            match draw_parameter_window(
                ui,
                active_pedalboard_id,
                pedal,
                &screen.cached_midi_devices,
            ) {
                Some(ParameterWindowChange::ParameterChanged(name, value)) => {
                    changed = Some((pedal.get_id(), (name, value)))
                }
                Some(ParameterWindowChange::AddMidiFunction(
                    parameter_path,
                    midi_function_values,
                    device_id,
                )) => {
                    screen
                        .state
                        .midi_state
                        .borrow_mut()
                        .add_midi_parameter_function_to_device(
                            parameter_path,
                            midi_function_values,
                            device_id,
                        );
                }
                Some(ParameterWindowChange::RemoveMidiFunction(parameter, device_id)) => {
                    screen
                        .state
                        .midi_state
                        .borrow_mut()
                        .remove_midi_parameter_function_from_device(&parameter, device_id);
                }
                Some(ParameterWindowChange::ChangeMidiFunctionDevice(
                    parameter,
                    new_id,
                    old_id,
                )) => {
                    let midi_state = screen.state.midi_state.borrow_mut();
                    if let Some(parameter_functions) =
                        midi_state.remove_midi_parameter_function_from_device(&parameter, old_id)
                    {
                        midi_state.add_midi_parameter_function_to_device(
                            parameter,
                            parameter_functions,
                            new_id,
                        );
                    }
                }
                None => {}
            }
        }
    }

    if let Some((pedal_id, (name, value))) = changed {
        let active_pedalboard_id = {
            let pedalboard_set = screen.state.pedalboards.active_pedalboardstage.borrow_mut();
            pedalboard_set.pedalboards[pedalboard_set.active_pedalboard].get_id()
        };

        screen
            .state
            .set_parameter(active_pedalboard_id, pedal_id, name, value, false);
    }

    if screen.show_pedal_menu {
        add_pedal_menu(
            screen,
            ui,
            pedalboard_available_rect.scale_from_center2(Vec2::new(0.6, 0.9)),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The scale that egui's Scene applies to the content for a scene rect
    fn scene_scale(available_size: Vec2, scene_rect: Rect) -> f32 {
        (available_size / scene_rect.size()).min_elem()
    }

    fn approx(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.001
    }

    #[test]
    fn zoom_is_kept_when_the_available_rect_changes() {
        let available_size = Vec2::new(600.0, 400.0);
        let scene_rect = scene_rect_for(Pos2::ZERO, available_size, 1.0);
        assert!(approx(scene_scale(available_size, scene_rect), 1.0));

        // The available rect changed (window startup sizing, the volume monitor
        // being shown or hidden, a taller status bar, ...). The old code only set
        // the scene rect once, which gave a scale below 1.0 here. The Scene
        // clamped that back up to the minimum zoom, leaving a few pixels that
        // could be panned into and snapped back from.
        let smaller_size = Vec2::new(580.0, 400.0);
        assert!(!approx(scene_scale(smaller_size, scene_rect), 1.0));

        // Re-deriving the scene rect from the new available size keeps the zoom
        let scene_rect = scene_rect_for(scene_rect.center(), smaller_size, 1.0);
        assert!(approx(scene_scale(smaller_size, scene_rect), 1.0));
    }

    #[test]
    fn zooming_keeps_the_center_of_the_view() {
        let available_size = Vec2::new(600.0, 400.0);
        let scene_rect = scene_rect_for(Pos2::ZERO, available_size, 1.0);

        let zoomed = scene_rect_for(scene_rect.center(), available_size, 2.0);
        assert!(approx(scene_scale(available_size, zoomed), 2.0));
        assert_eq!(zoomed.center(), scene_rect.center());
    }

    #[test]
    fn zoom_steps_stay_within_the_zoom_range() {
        let mut zoom = MIN_ZOOM;
        for _ in 0..10 {
            zoom = (zoom * ZOOM_STEP).clamp(MIN_ZOOM, MAX_ZOOM);
        }
        assert!(approx(zoom, MAX_ZOOM));

        for _ in 0..10 {
            zoom = (zoom / ZOOM_STEP).clamp(MIN_ZOOM, MAX_ZOOM);
        }
        assert!(approx(zoom, MIN_ZOOM));
    }

    #[test]
    fn bound_pins_the_camera_when_the_content_fits() {
        // At 1x the camera is the whole pedalboard and the pedals fit in it, so
        // there is nothing to pan and the camera is pinned to the content origin
        let mut camera = Rect::from_min_size(Pos2::new(50.0, -10.0), Vec2::new(600.0, 400.0));
        bound_scene_rect(&mut camera, Vec2::new(560.0, 380.0));
        assert_eq!(
            camera,
            Rect::from_min_size(Pos2::ZERO, Vec2::new(600.0, 400.0))
        );
    }

    #[test]
    fn bound_lets_the_camera_reach_the_ends_of_the_content() {
        let camera_size = Vec2::new(200.0, 100.0);
        let content_size = Vec2::new(500.0, 400.0);

        // Dragged past the end of the content
        let mut camera = Rect::from_min_size(Pos2::new(1000.0, 1000.0), camera_size);
        bound_scene_rect(&mut camera, content_size);
        assert_eq!(camera.min, Pos2::new(300.0, 300.0));
        assert_eq!(camera.size(), camera_size);

        // Dragged before the start of the content
        let mut camera = Rect::from_min_size(Pos2::new(-100.0, -100.0), camera_size);
        bound_scene_rect(&mut camera, content_size);
        assert_eq!(camera.min, Pos2::ZERO);
        assert_eq!(camera.size(), camera_size);
    }

    #[test]
    fn bound_pins_only_the_axis_where_the_content_fits() {
        // Content fits horizontally, is bigger vertically: the x position is
        // pinned and y can be panned
        let mut camera = Rect::from_min_size(Pos2::new(120.0, 500.0), Vec2::new(500.0, 100.0));
        bound_scene_rect(&mut camera, Vec2::new(500.0, 400.0));
        assert_eq!(camera.min, Pos2::new(0.0, 300.0));
    }

    #[test]
    fn zoomed_in_camera_can_pan_over_the_content() {
        let available_size = Vec2::new(600.0, 400.0);
        let zoom = 2.0;
        let pedal_width = 0.9 * (available_size.x / PEDAL_ROW_COUNT as f32);
        let pedal_x_spacing = 0.1 * (available_size.x / PEDAL_ROW_COUNT as f32);
        let content_size =
            pedalboard_content_size(pedal_width, pedal_x_spacing, 10.0, MAX_PEDAL_COUNT);

        let mut camera = scene_rect_for(Pos2::ZERO, available_size, zoom);
        assert!(approx(scene_scale(available_size, camera), zoom));

        // Zoomed in the camera is smaller than the content, so it can be moved to
        // the far end of the content instead of being pinned to its start
        camera = Rect::from_min_size(Pos2::new(10_000.0, 10_000.0), camera.size());
        bound_scene_rect(&mut camera, content_size);
        assert!(camera.min.x > 0.0 && camera.min.y > 0.0);
        assert!(approx(
            camera.min.x,
            content_size.x - available_size.x / zoom
        ));
        assert!(approx(
            camera.min.y,
            content_size.y - available_size.y / zoom
        ));
        // Panning never changes the zoom
        assert!(approx(scene_scale(available_size, camera), zoom));
    }

    #[test]
    fn content_size_grows_a_row_at_a_time() {
        let (pedal_width, x_spacing, y_spacing) = (150.0, 15.0, 10.0);
        let row_height = pedal_width * PEDAL_HEIGHT_RATIO;

        // Up to PEDAL_ROW_COUNT pedals fit in a single row
        for pedal_count in 1..=PEDAL_ROW_COUNT {
            let content = pedalboard_content_size(pedal_width, x_spacing, y_spacing, pedal_count);
            let expected_x = x_spacing * 0.5
                + pedal_count as f32 * pedal_width
                + (pedal_count as f32 - 1.0) * x_spacing;
            assert!(approx(content.x, expected_x));
            assert!(approx(content.y, row_height));
        }

        // More pedals wrap onto a next row
        for pedal_count in PEDAL_ROW_COUNT + 1..=MAX_PEDAL_COUNT {
            let content = pedalboard_content_size(pedal_width, x_spacing, y_spacing, pedal_count);
            assert!(approx(content.y, 2.0 * row_height + y_spacing));
        }

        // Boards needing more than two rows keep growing
        let content =
            pedalboard_content_size(pedal_width, x_spacing, y_spacing, 2 * PEDAL_ROW_COUNT + 1);
        assert!(approx(content.y, 3.0 * row_height + 2.0 * y_spacing));
    }
}
