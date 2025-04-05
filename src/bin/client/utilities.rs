use eframe::egui::{self, Widget};

pub struct UtilitiesScreen {
    
}

impl UtilitiesScreen {
    pub fn new() -> Self {
        Self {
            
        }
    }
}

impl Widget for &mut UtilitiesScreen {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.label("Utilities Screen")
    }
}