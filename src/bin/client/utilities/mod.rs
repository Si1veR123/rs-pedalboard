pub mod looper;
pub mod metronome;
pub mod recorder;
pub mod tuner;

use eframe::egui::{self, Color32, Layout, RichText, Vec2, Widget};

use crate::{state::State, ui_helpers::set_large_checkbox_style};

pub(super) const HEADING_FONT_SIZE: f32 = 45.0;
pub(super) const BODY_FONT_SIZE: f32 = 35.0;
pub(super) const SMALL_BODY_FONT_SIZE: f32 = 25.0;

pub struct UtilitiesScreen {
    pub state: &'static State,
    pub tuner: tuner::TunerWidget,
    pub metronome: metronome::MetronomeWidget,
    pub recorder: recorder::RecorderUtility,
    pub looper: looper::ui::LooperWidget,
}

impl UtilitiesScreen {
    pub fn new(state: &'static State) -> Self {
        Self {
            state,
            tuner: tuner::TunerWidget::new(state),
            metronome: metronome::MetronomeWidget::new(state),
            recorder: recorder::RecorderUtility::new(state),
            looper: looper::ui::LooperWidget::new(state),
        }
    }
}

impl Widget for &mut UtilitiesScreen {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        set_large_checkbox_style(ui);

        if !self.state.is_connected() {
            return ui
                .centered_and_justified(|ui| {
                    ui.label(
                        RichText::from("Not connected to processor")
                            .color(crate::FAINT_TEXT_COLOR)
                            .size(50.0),
                    );
                })
                .response;
        }

        let border = Color32::from_gray(35);

        egui::ScrollArea::vertical()
            .show(ui, |ui| {
                ui.vertical_centered(|ui| {
                    let spacing = 8.0;
                    let widget_height = ui.available_height() / 2.0 - spacing * 3.0;
                    ui.add_space(spacing);
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), widget_height),
                        Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            let available_width = ui.available_width();
                            ui.add_space(available_width * 0.15);
                            ui.allocate_ui_with_layout(
                                Vec2::new(available_width * 0.7, ui.available_height()),
                                Layout::top_down(egui::Align::Center),
                                |ui| {
                                    let rect = ui.add(&mut self.tuner).rect;
                                    ui.painter().rect_stroke(
                                        rect,
                                        5.0,
                                        (1.0, border),
                                        egui::StrokeKind::Middle,
                                    );
                                },
                            )
                        },
                    );
                    ui.add_space(spacing);
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), widget_height),
                        Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            let available_width = ui.available_width();
                            ui.add_space(available_width * 0.15);
                            ui.allocate_ui_with_layout(
                                Vec2::new(available_width * 0.7, ui.available_height()),
                                Layout::top_down(egui::Align::Center),
                                |ui| {
                                    let rect = ui.add(&mut self.metronome).rect;
                                    ui.painter().rect_stroke(
                                        rect,
                                        5.0,
                                        (1.0, border),
                                        egui::StrokeKind::Middle,
                                    );
                                },
                            )
                        },
                    );
                    ui.add_space(spacing);
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), widget_height),
                        Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            let available_width = ui.available_width();
                            ui.add_space(available_width * 0.15);
                            ui.allocate_ui_with_layout(
                                Vec2::new(available_width * 0.7, ui.available_height()),
                                Layout::top_down(egui::Align::Center),
                                |ui| {
                                    let rect = ui.add(&mut self.recorder).rect;
                                    ui.painter().rect_stroke(
                                        rect,
                                        5.0,
                                        (1.0, border),
                                        egui::StrokeKind::Middle,
                                    );
                                },
                            )
                        },
                    );
                    ui.add_space(spacing);
                    ui.allocate_ui_with_layout(
                        Vec2::new(ui.available_width(), widget_height),
                        Layout::left_to_right(egui::Align::Center),
                        |ui| {
                            let available_width = ui.available_width();
                            ui.add_space(available_width * 0.15);
                            ui.allocate_ui_with_layout(
                                Vec2::new(available_width * 0.7, ui.available_height()),
                                Layout::top_down(egui::Align::Center),
                                |ui| {
                                    let rect = ui.add(&mut self.looper).rect;
                                    ui.painter().rect_stroke(
                                        rect,
                                        5.0,
                                        (1.0, border),
                                        egui::StrokeKind::Middle,
                                    );
                                },
                            )
                        },
                    );
                })
                .response
            })
            .inner
    }
}
