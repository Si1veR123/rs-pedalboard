pub mod tuner;
pub mod metronome;
pub mod recorder;

use eframe::egui::{self, Color32, Layout, RichText, Vec2, Widget};

use crate::state::State;

pub struct UtilitiesScreen {
    pub state: &'static State,
    pub tuner: tuner::TunerWidget,
    pub metronome: metronome::MetronomeWidget,
    pub recorder: recorder::RecorderUtility
}

impl UtilitiesScreen {
    pub fn new(state: &'static State) -> Self {
        Self {
            state,
            tuner: tuner::TunerWidget::new(state),
            metronome: metronome::MetronomeWidget::new(state),
            recorder: recorder::RecorderUtility::new(state)
        }
    }
}

impl Widget for &mut UtilitiesScreen {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        if !self.state.is_connected() {
            return ui.centered_and_justified(|ui| {
                ui.label(RichText::from("Not connected to processor")
                    .color(crate::FAINT_TEXT_COLOR)
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
                ui.add_space(spacing);
                ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), widget_height), Layout::left_to_right(egui::Align::Center), |ui| {
                    let available_width = ui.available_width();
                    ui.add_space(available_width*0.15);
                    ui.allocate_ui_with_layout(Vec2::new(available_width*0.7, ui.available_height()), Layout::top_down(egui::Align::Center), |ui| {
                        let rect = ui.add(&mut self.recorder).rect;
                        ui.painter().rect_stroke(rect, 5.0, (1.0, border), egui::StrokeKind::Middle);
                    })
                });
            }).response
        }).inner
    }
}

pub fn start_stop_icon(ui: &mut egui::Ui, start: bool, rect: egui::Rect, size: f32) {
    let icon_size = Vec2::splat(size);
    let right_arrow_rect = egui::Align2::CENTER_CENTER.align_size_within_rect(icon_size, rect);
    let points = if start {
        vec![right_arrow_rect.left_top(), right_arrow_rect.right_center(), right_arrow_rect.left_bottom()]
    } else {
        vec![right_arrow_rect.left_top(), right_arrow_rect.right_top(), right_arrow_rect.right_bottom(), right_arrow_rect.left_bottom()]
    };
    ui.painter().add(
        egui::Shape::convex_polygon(
            points,
            Color32::from_gray(200),
            egui::Stroke::NONE
        )
    );
}
