use std::time::Instant;

use eframe::egui::{self, Color32, RichText, Vec2, Widget};

use crate::state::State;
use super::start_stop_icon;

pub struct RecorderUtility {
    state: &'static State,
    pub recording_time: Option<Instant>,
    pub save_clean: bool
}

impl RecorderUtility {
    pub fn new(state: &'static State) -> Self {
        Self {
            state,
            recording_time: None,
            save_clean: false
        }
    }
}

impl Widget for &mut RecorderUtility {
    fn ui(self, ui: &mut eframe::egui::Ui) -> eframe::egui::Response {
        ui.vertical_centered(|ui| {
            ui.add_space(10.0);
            ui.label(RichText::from("Record").size(28.0).color(Color32::from_gray(90)));
            ui.add_space(7.0);
            if let Some(start_time) = self.recording_time {
                let button_response = ui.add_sized(
                    Vec2::splat(50.0),
                    egui::Button::new("")
                );
                if button_response.clicked() {
                    self.state.stop_recording_server();
                    self.recording_time = None;
                }

                start_stop_icon(ui, false, button_response.rect, 30.0);

                ui.add_space(10.0);

                let elapsed = Instant::now().duration_since(start_time);
                ui.label(RichText::new(format!("Recording... {:02}:{:02}", elapsed.as_secs() / 60, elapsed.as_secs() % 60)).size(30.0));

                ui.add_space(10.0);
            } else {
                let button_response = ui.add_sized(
                    Vec2::splat(50.0),
                    egui::Button::new("")
                );
                if button_response.clicked() {
                    self.state.start_recording_server();
                    self.recording_time = Some(Instant::now());
                }

                start_stop_icon(ui, true, button_response.rect, 30.0);

                ui.add_space(10.0);

                if ui.checkbox(&mut self.save_clean, RichText::new("Save clean").size(30.0)).on_hover_text("Save the recording with and without pedal effects").changed() {
                    self.state.set_recorder_clean_server(self.save_clean);
                }
            }
        }).response
        
    }

    
}
