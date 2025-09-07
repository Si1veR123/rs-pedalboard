use eframe::egui::{self, Color32, RichText, Vec2, Widget};

use crate::{state::State, utilities::start_stop_icon};

pub struct MetronomeWidget {
    pub state: &'static State,
}

impl MetronomeWidget {
    pub fn new(state: &'static State) -> Self {
        Self {
            state
        }
    }
}

impl Widget for &mut MetronomeWidget {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.vertical_centered(|ui| {
            ui.add_space(10.0);
            ui.label(RichText::from("Metronome").size(28.0).color(Color32::from_gray(90)));
            ui.add_space(7.0);

            let mut active = self.state.metronome_active.get();
            let mut bpm = self.state.metronome_bpm.get();
            let mut volume = self.state.metronome_volume.get();
            ui.label(RichText::new(format!("{} BPM", bpm)).size(44.0));

            // BPM Slider
            ui.style_mut().spacing.slider_width = ui.available_width()*0.5;
            if ui.add_sized(Vec2::new(ui.available_width()*0.5, 30.0),
                egui::Slider::new(&mut bpm, 40..=360).show_value(false)
            ).changed() {
                if active {
                    self.state.set_metronome(active, bpm, volume)
                }
            }

            ui.add_space(10.0);

            // Volume Slider
            ui.label("Volume");
            if ui.add_sized(Vec2::new(ui.available_width()*0.5, 30.0),
                egui::Slider::new(&mut volume, 0.0..=1.0).show_value(false)
            ).changed() {
                if active {
                    self.state.set_metronome(active, bpm, volume)
                }
            }

            // Play/Pause button
            ui.add_space(5.0);

            let button_response = ui.add_sized(
                Vec2::splat(50.0),
                egui::Button::new("")
            );
            if button_response.clicked() {
                active = !active;
                self.state.set_metronome(active, bpm, volume);
            }

            start_stop_icon(ui, true, button_response.rect, 30.0);

            ui.add_space(10.0);
        }).response
    }
}
