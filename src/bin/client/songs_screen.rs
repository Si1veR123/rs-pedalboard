use eframe::egui::{self, Widget};

use crate::State;

pub struct SongsScreen {
    state: &'static State,
}

impl SongsScreen {
    pub fn new(state: &'static State) -> Self {
        Self {
            state,
        }
    }
}

impl Widget for &mut SongsScreen {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.label("Songs Screen")
    }
}