use super::PedalboardStageScreen;

use eframe::egui;
use rs_pedalboard::pedals::PedalTrait;

pub fn pedalboard_designer(screen: &mut PedalboardStageScreen, ui: &mut egui::Ui) -> egui::Response {
    let mut pedalboard_set = screen.state.active_pedalboardstage.borrow_mut();
    let active_index = pedalboard_set.active_pedalboard;
    let pedalboard = pedalboard_set.pedalboards.get_mut(active_index).unwrap();

    ui.vertical(|ui| {
        for (i, pedal) in pedalboard.pedals.iter_mut().enumerate() {
            let changed = pedal.ui(ui);
            if let Some(name) = changed {
                let value = pedal.get_parameters().get(&name).unwrap().value.clone();
                screen.socket.borrow_mut().set_parameter(active_index, i, &name, &value);
            }
        }
    }).response
}