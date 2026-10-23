use crate::state::State;
use eframe::egui;

pub struct LooperWidget {
    state: &'static State
}

impl LooperWidget {
    pub fn new(state: &'static State) -> Self {
        Self { state }
    }
}

impl egui::Widget for &mut LooperWidget {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        // TODO
        ui.label("Looper")
    }
}
