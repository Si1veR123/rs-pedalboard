pub mod tuner;

use eframe::egui::{self, Widget};

use crate::state::State;

pub struct UtilitiesScreen {
    tuner: tuner::TunerWidget,
}

impl UtilitiesScreen {
    pub fn new(state: &'static State) -> Self {
        Self {
            tuner: tuner::TunerWidget::new(state),
        }
    }
}

impl Widget for &mut UtilitiesScreen {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.add(&mut self.tuner)
    }
}