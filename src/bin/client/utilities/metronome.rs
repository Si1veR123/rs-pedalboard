use eframe::egui::{self, RichText, Vec2, Widget};

use crate::{
    state::State,
    ui_helpers::{start_stop_icon, integer_selector},
    utilities::{HEADING_FONT_SIZE, BODY_FONT_SIZE}
};

const MIN_BPM: u32 = 40;
const MAX_BPM: u32 = 360;

pub struct MetronomeWidget {
    pub state: &'static State,
}

impl MetronomeWidget {
    pub fn new(state: &'static State) -> Self {
        Self { state }
    }
}

impl Widget for &mut MetronomeWidget {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.vertical_centered(|ui| {
            ui.add_space(10.0);
            ui.label(
                RichText::from("Metronome")
                    .size(HEADING_FONT_SIZE)
            );
            ui.add_space(7.0);

            let mut active = self.state.metronome_active.get();
            let mut bpm = self.state.metronome_bpm.get();
            let mut volume = self.state.metronome_volume.get();
            ui.label(RichText::new(format!("{} BPM", bpm)).size(BODY_FONT_SIZE));

            if integer_selector(ui, &mut bpm, MIN_BPM..=MAX_BPM) {
                self.state.set_metronome(active, bpm, volume);
            }

            ui.add_space(10.0);

            // Volume Slider
            ui.label(RichText::new("Volume").size(BODY_FONT_SIZE));
            if ui
                .add_sized(
                    Vec2::new(ui.available_width() * 0.5, 30.0),
                    egui::Slider::new(&mut volume, 0.0..=1.0).show_value(false),
                )
                .changed()
            {
                if active {
                    self.state.set_metronome(active, bpm, volume)
                }
            }

            // Play/Pause button
            ui.add_space(5.0);

            let button_response = ui.add_sized(Vec2::splat(50.0), egui::Button::new(""));
            if button_response.clicked() {
                active = !active;
                self.state.set_metronome(active, bpm, volume);
            }

            start_stop_icon(ui, !active, button_response.rect, 30.0);

            ui.add_space(10.0);
        })
        .response
    }
}
