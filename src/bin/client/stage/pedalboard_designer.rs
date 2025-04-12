use super::PedalboardStageScreen;

use eframe::egui::{self, Color32, Layout, Pos2, Rect, RichText, Sense, UiBuilder, Vec2};
use rs_pedalboard::pedals::{PedalTrait, PedalDiscriminants};
use egui_dnd;
use strum::IntoEnumIterator;

pub fn pedalboard_designer(screen: &mut PedalboardStageScreen, ui: &mut egui::Ui) -> egui::Response {
    let available_rect = ui.available_rect_before_wrap();

    ui.add_space(15.0);

    ui.allocate_ui_with_layout(
        Vec2::new(available_rect.width(), available_rect.height()*0.1),
        Layout::top_down(egui::Align::Center),
        |ui| {
        if ui.button(RichText::new("Add Pedal").size(30.0).strong()).clicked() {
            screen.show_pedal_menu = !screen.show_pedal_menu;
        }
    });

    let mut pedalboard_set = screen.state.active_pedalboardstage.borrow_mut();
    let active_index = pedalboard_set.active_pedalboard;
    let pedalboard = pedalboard_set.pedalboards.get_mut(active_index).unwrap();
    let pedalboard_name = pedalboard.name.clone();

    let pedal_width = 0.2 * available_rect.width();

    let num_pedals = pedalboard.pedals.len();
    // 5 per row
    let top_row_num_pedals = num_pedals.min(5) as f32;
    let top_row_space_around = (available_rect.width() - pedal_width * top_row_num_pedals) / (top_row_num_pedals + 1.0);

    ui.add_space(available_rect.height()*0.05);

    let inner_r = ui.horizontal_wrapped(|ui| {
        let mut to_change = None;

        egui_dnd::dnd(ui, "pedal_dnd").show(
            pedalboard.pedals.iter_mut().enumerate(),
            |ui, (i, item), handle, _state| {
                ui.add_space(top_row_space_around);
                ui.allocate_ui(Vec2::new(pedal_width, 0.0), |ui| {
                    if let Some(to_change_opt) = item.ui(ui) {
                        to_change = Some((i, to_change_opt));
                    }
                });
            }
        );
        
        to_change
    });

    drop(pedalboard_set);

    if let Some((pedal_index, (name, value))) = inner_r.inner {
        dbg!("Setting paramater with args {:?}", (pedal_index, &name, &value));
        screen.state.set_parameter(
            &pedalboard_name,
            pedal_index,
            &name,
            &value
        );
    }

    if screen.show_pedal_menu {
        let mut pedalboard_set = screen.state.active_pedalboardstage.borrow_mut();
        let pedalboard = pedalboard_set.pedalboards.get_mut(active_index).unwrap();
        ui.allocate_new_ui(UiBuilder::new().max_rect(available_rect.scale_from_center2(Vec2 { x: 0.5, y: 0.9 })), |ui| {
            ui.painter().rect_filled(
                ui.available_rect_before_wrap(),
                20.0,
                Color32::WHITE.linear_multiply(0.03),
            );

            egui::ScrollArea::vertical()
                .max_height(ui.available_height())
                .show(ui, |ui| {
                    for pedal in PedalDiscriminants::iter() {
                        if ui.label(format!("{:?}", pedal)).clicked() {
                            let new_pedal = pedal.new_pedal();
                            pedalboard.pedals.push(new_pedal);
                            screen.show_pedal_menu = false
                        }
                    }
                });
        });
    }

    inner_r.response
}