use eframe::egui::{self, include_image, Color32, ImageButton, RichText, Vec2, Widget};

use crate::state::State;

pub struct MetronomeWidget {
    pub state: &'static State,
    pub bpm: u32,
    // 0.0 to 1.0
    pub volume: f32,
    pub active: bool,
}

impl MetronomeWidget {
    pub fn new(state: &'static State) -> Self {
        Self {
            state,
            bpm: 120,
            volume: 0.5,
            active: false,
        }
    }
}

impl Widget for &mut MetronomeWidget {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.vertical_centered(|ui| {
            ui.add_space(10.0);
            ui.label(RichText::from("Metronome").size(28.0).color(Color32::from_gray(90)));
            ui.add_space(7.0);

            ui.label(RichText::new(format!("{} BPM", self.bpm)).size(44.0));

            // BPM Slider
            ui.style_mut().spacing.slider_width = ui.available_width()*0.5;
            if ui.add_sized(Vec2::new(ui.available_width()*0.5, 30.0),
                egui::Slider::new(&mut self.bpm, 40..=360).show_value(false)
            ).changed() {
                if self.active {
                    self.state.set_metronome_server(self.active, self.bpm, self.volume)
                }
            }

            ui.add_space(10.0);

            // Volume Slider
            ui.label("Volume");
            if ui.add_sized(Vec2::new(ui.available_width()*0.5, 30.0),
                egui::Slider::new(&mut self.volume, 0.0..=1.0).show_value(false)
            ).changed() {
                if self.active {
                    self.state.set_metronome_server(self.active, self.bpm, self.volume)
                }
            }

            // Play/Pause button
            ui.add_space(5.0);
            let image_to_use = if self.active {
                include_image!("../files/pause.png")
            } else {
                include_image!("../files/play.png")
            };

            if ui.add_sized(Vec2::splat(50.0), ImageButton::new(image_to_use).corner_radius(5.0).tint(crate::TEXT_COLOUR)).clicked() {
                self.active = !self.active;
                self.state.set_metronome_server(self.active, self.bpm, self.volume);
            }

            ui.add_space(10.0);
        }).response
    }
}
