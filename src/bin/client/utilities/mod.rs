pub mod tuner;
pub mod metronome;

use eframe::egui::{self, Color32, Layout, RichText, Vec2, Widget};

use crate::state::State;

pub struct UtilitiesScreen {
    pub state: &'static State,
    pub tuner: tuner::TunerWidget,
    pub metronome: metronome::MetronomeWidget,
}

impl UtilitiesScreen {
    pub fn new(state: &'static State) -> Self {
        Self {
            state,
            tuner: tuner::TunerWidget::new(state),
            metronome: metronome::MetronomeWidget::new(state),
        }
    }
}

impl Widget for &mut UtilitiesScreen {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        if !self.state.socket.borrow_mut().is_connected() {
            return ui.centered_and_justified(|ui| {
                ui.label(RichText::from("Not connected to server")
                    .color(Color32::from_gray(130))
                    .size(50.0)
                );
            }).response;
        }

        let border = Color32::from_gray(35);

        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.vertical_centered(|ui| {
                let spacing = 8.0;
                let widget_height = ui.available_height() / 2.0 - spacing * 3.0;
                ui.add_space(spacing);
                ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), widget_height), Layout::left_to_right(egui::Align::Center), |ui| {
                    let available_width = ui.available_width();
                    ui.add_space(available_width*0.15);
                    ui.allocate_ui_with_layout(Vec2::new(available_width*0.7, ui.available_height()), Layout::top_down(egui::Align::Center), |ui| {
                        let rect = ui.add(&mut self.tuner).rect;
                        ui.painter().rect_stroke(rect, 5.0, (1.0, border), egui::StrokeKind::Middle);
                    })
                });
                ui.add_space(spacing);
                ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), widget_height), Layout::left_to_right(egui::Align::Center), |ui| {
                    let available_width = ui.available_width();
                    ui.add_space(available_width*0.15);
                    ui.allocate_ui_with_layout(Vec2::new(available_width*0.7, ui.available_height()), Layout::top_down(egui::Align::Center), |ui| {
                        let rect = ui.add(&mut self.metronome).rect;
                        ui.painter().rect_stroke(rect, 5.0, (1.0, border), egui::StrokeKind::Middle);
                    })
                });
            }).response
        }).inner
    }
}