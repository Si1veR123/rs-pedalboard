use super::PedalboardStageScreen;

use eframe::egui::{self, Vec2};
use rs_pedalboard::pedals::PedalTrait;
use egui_dnd;

pub fn pedalboard_designer(screen: &mut PedalboardStageScreen, ui: &mut egui::Ui) -> egui::Response {
    let mut pedalboard_set = screen.state.active_pedalboardstage.borrow_mut();
    let active_index = pedalboard_set.active_pedalboard;
    let pedalboard = pedalboard_set.pedalboards.get_mut(active_index).unwrap();

    let designer_width = ui.available_width();
    let designer_height = ui.available_height();

    let pedal_width = 0.1 * designer_width;

    let num_pedals = pedalboard.pedals.len();
    // 5 per row
    let top_row_num_pedals = num_pedals.min(5) as f32;
    let top_row_space_around = (designer_width - pedal_width * top_row_num_pedals) / (top_row_num_pedals + 1.0);

    ui.horizontal_wrapped(|ui| {
        egui_dnd::dnd(ui, "pedal_dnd").show(
            pedalboard.pedals.iter_mut(),
            |ui, item, handle, _state| {
                ui.add_space(top_row_space_around);
                ui.allocate_ui(Vec2::new(pedal_width, 0.0), |ui| {
                    item.ui(ui);
                });
            }
        )
    }).response
}