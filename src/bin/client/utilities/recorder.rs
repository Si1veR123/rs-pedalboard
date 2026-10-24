use std::time::Instant;

use eframe::egui::{self, RichText, Vec2, Widget};

use crate::{FAINT_TEXT_COLOR, state::State, ui_helpers::start_stop_icon, utilities::{BODY_FONT_SIZE, HEADING_FONT_SIZE}};

pub struct RecorderUtility {
    state: &'static State,
}

impl RecorderUtility {
    pub fn new(state: &'static State) -> Self {
        Self { state }
    }
}

impl Widget for &mut RecorderUtility {
    fn ui(self, ui: &mut eframe::egui::Ui) -> eframe::egui::Response {
        let recording_time = self.state.recording_time.get();

        ui.vertical_centered(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::from("Record")
                    .size(HEADING_FONT_SIZE)
            );
            ui.add_space(7.0);
            if let Some(start_time) = recording_time {
                let button_response = ui.add_sized(Vec2::splat(50.0), egui::Button::new(""));
                if button_response.clicked() {
                    self.state.set_recording(false);
                }

                start_stop_icon(ui, false, button_response.rect, 30.0);

                ui.add_space(10.0);

                let elapsed = Instant::now().duration_since(start_time);
                ui.label(
                    RichText::new(format!(
                        "Recording... {:02}:{:02}",
                        elapsed.as_secs() / 60,
                        elapsed.as_secs() % 60
                    ))
                    .size(BODY_FONT_SIZE)
                    .color(FAINT_TEXT_COLOR),
                );

                ui.add_space(10.0);
            } else {
                let button_response = ui.add_sized(Vec2::splat(50.0), egui::Button::new(""));
                if button_response.clicked() {
                    self.state.set_recording(true);
                }

                start_stop_icon(ui, true, button_response.rect, 30.0);

                ui.add_space(10.0);

                let mut save_clean = self.state.recording_save_clean.get();
                if ui
                    .checkbox(&mut save_clean, RichText::new("Save clean").size(BODY_FONT_SIZE))
                    .on_hover_text("Save the recording with and without pedal effects")
                    .changed()
                {
                    self.state.set_recorder_clean(save_clean);
                }
            }
        })
        .response
    }
}
