use eframe::egui::{self, Layout, Response, Vec2, Widget};

use crate::state::State;

pub struct SettingsScreen {
    state: &'static State,
}

impl SettingsScreen {
    pub fn new(state: &'static State) -> Self {
        Self { state }
    }
}

impl Widget for &mut SettingsScreen {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        ui.add_space(ui.available_height()*0.05);
        ui.horizontal(|ui| {
            ui.add_space(ui.available_width()*0.05);
            ui.allocate_ui_with_layout(ui.available_size()*Vec2::new(0.9, 0.9), Layout::top_down(egui::Align::Min), |ui| {
                egui::ScrollArea::vertical().show(ui, |ui| {
                    ui.label("Client Settings");
                    ui.separator();
        
                    ui.label("Server Settings");
                    ui.separator();
                })
            });
        }).response
    }
}