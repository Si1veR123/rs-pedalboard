use eframe::egui::{self, Widget};
use rs_pedalboard::pedalboard_set::PedalboardSet;

use crate::State;


pub struct PedalboardSetScreen {
    state: &'static State    
}

impl PedalboardSetScreen {
    pub fn new(state: &'static State) -> Self {
        Self {
            state
        }
    }
}

impl Widget for &mut PedalboardSetScreen {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.label(format!("Number of Active Pedalboards: {}", self.state.active_pedalboardset.borrow().pedalboards.len()))
    }
}
