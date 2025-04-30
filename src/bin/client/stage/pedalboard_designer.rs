use core::f32;

use super::PedalboardStageScreen;

use eframe::egui::{self, Button, Color32, Layout, Pos2, Rect, RichText, Sense, Ui, UiBuilder, Vec2};
use rs_pedalboard::pedals::{PedalDiscriminants, PedalParameterValue, PedalTrait};
use egui_dnd::{self, DragDropItem};
use strum::IntoEnumIterator;

const PEDAL_ROW_COUNT: usize = 6;
// Must be high enough to fit any pedal
// TODO: Make this dynamic
// PEDAL_HEIGHT_RATIO * width = height
const PEDAL_HEIGHT_RATIO: f32 = 2.1;

/// Assumes scene rect is smaller than available size
pub fn bound_scene_rect(scene_rect: &mut Rect, available_size: &Vec2) {
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

pub fn add_pedal_menu(screen: &mut PedalboardStageScreen, ui: &mut egui::Ui, rect: Rect) {
    let mut pedalboard_set = screen.state.active_pedalboardstage.borrow_mut();
    let active_index = pedalboard_set.active_pedalboard;

    let pedalboard = pedalboard_set.pedalboards.get_mut(active_index).unwrap();

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
                if ui.add_sized(Vec2::new(ui.available_width()*0.95, 35.0), egui::Button::new(format!("{:?}", pedal))).clicked() {
                    let new_pedal = pedal.new_pedal();
                    screen.state.socket.borrow_mut().add_pedal(&new_pedal);
                    pedalboard.pedals.push(new_pedal);
                    screen.show_pedal_menu = false
                }
                ui.separator();
            }
        });
}

pub fn pedalboard_designer(screen: &mut PedalboardStageScreen, ui: &mut egui::Ui) {
    ui.add_space(5.0);

    ui.allocate_ui_with_layout(
        ui.available_size() * Vec2::new(1.0, 0.1),
        Layout::top_down(egui::Align::Center),
        |ui| {
        if ui.button(RichText::new("Add Pedal").size(30.0).strong()).clicked() {
            screen.show_pedal_menu = !screen.show_pedal_menu;
        }
    });

    ui.add_space(5.0);

    // Available rect for the pedalboard itself
    let available_rect = ui.available_rect_before_wrap();

    let mut pedalboard_set = screen.state.active_pedalboardstage.borrow_mut();
    let active_index = pedalboard_set.active_pedalboard;
    let pedalboard = pedalboard_set.pedalboards.get_mut(active_index).unwrap();
    let pedalboard_name = pedalboard.name.clone();

    let pedal_width = 0.85 * (available_rect.width() / PEDAL_ROW_COUNT as f32);
    let pedal_spacing = 0.15 * (available_rect.width() / PEDAL_ROW_COUNT as f32);

    // Initially set to ZERO, so fill in with available pedalboard rect
    if screen.pedalboard_rect == Rect::ZERO {
        screen.pedalboard_rect = Rect::from_min_size(Pos2::ZERO, available_rect.size());
    }

    // Delete pedal hover button
    let size = 150.0;
    let delete_button_rect = Rect::from_min_size(
        available_rect.max - Vec2::splat(size + 5.0),
        Vec2::splat(size),
    );
    let mut child = ui.new_child(UiBuilder::new()
        .layer_id(egui::LayerId::new(egui::Order::Foreground, ui.id().with("delete_button")))
        .max_rect(delete_button_rect));
    

    let changed: Option<(usize, (String, PedalParameterValue))> = egui::Scene::new().zoom_range(1.0..=3.0).show(ui, &mut screen.pedalboard_rect, |ui| {
        ui.allocate_new_ui(
            UiBuilder::new()
                .max_rect(Rect { min: Pos2::ZERO, max: available_rect.size().to_pos2() })
                .layout(Layout::left_to_right(egui::Align::Min)),
            |ui| {
                ui.painter().rect_filled(ui.available_rect_before_wrap(), 5.0, Color32::from_gray(20));
                ui.add_space(pedal_spacing/2.0);
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(pedal_spacing, 0.0);

                    let mut changed = None;

                    let dnd_response = egui_dnd::dnd(ui, "pedalboard_designer_dnd").show_sized(pedalboard.pedals.iter_mut().enumerate(), Vec2::new(pedal_width, pedal_width*PEDAL_HEIGHT_RATIO), |ui, (i, item), handle, _state| {
                        if let Some(v) = item.ui(ui) {
                            changed = Some((i, v));
                        }

                        handle.ui_sized(
                            ui,
                            Vec2::new(pedal_width, pedal_width*PEDAL_HEIGHT_RATIO*0.05),
                            |ui| {
                                ui.add_sized(ui.available_size(), Button::new("Drag"));
                            }
                        );
                    });

                    let mouse_over_delete = delete_button_rect.contains(ui.ctx().input(|i| i.pointer.hover_pos()).unwrap_or(Pos2::ZERO));

                    if dnd_response.is_dragging() {
                        let button = if mouse_over_delete {
                            Button::new("Delete").fill(Color32::RED.gamma_multiply(0.3))
                        } else {
                            Button::new("Delete")
                        };

                        child.put(child.available_rect_before_wrap(), button);
                    }

                    if dnd_response.is_drag_finished() {
                        if let Some(update) = &dnd_response.update {
                            if mouse_over_delete {
                                if ui.ctx().input(|i| i.pointer.any_released()) && pedalboard.pedals.len() > 1 {
                                    pedalboard.pedals.remove(update.from);
                                    screen.state.socket.borrow_mut().delete_pedal(active_index, update.from);
                                }
                            } else {
                                screen.state.socket.borrow_mut().move_pedal(
                                    active_index,
                                    update.from,
                                    update.to
                                );
                                egui_dnd::utils::shift_vec(update.from, update.to, &mut pedalboard.pedals);
                            }
                        }
                    }

                    changed
                }).inner
            }
        ).inner
    }).inner;

    bound_scene_rect(&mut screen.pedalboard_rect, &available_rect.size());

    drop(pedalboard_set);

    if let Some((pedal_index, (name, value))) = changed {
        screen.state.set_parameter(
            &pedalboard_name,
            pedal_index,
            &name,
            &value
        );
    }

    if screen.show_pedal_menu {
        add_pedal_menu(screen, ui, available_rect.scale_from_center2(Vec2::new(0.6, 0.9)));
    }
}