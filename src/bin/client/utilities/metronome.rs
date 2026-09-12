use eframe::egui::{self, Color32, RichText, Vec2, Widget};

use crate::{state::State, utilities::start_stop_icon};

/// Tempo range and step used by the metronome BPM slider and its -/+ buttons.
const MIN_BPM: u32 = 40;
const MAX_BPM: u32 = 360;
const BPM_STEP: u32 = 1;

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
                    .size(28.0)
                    .color(Color32::from_gray(90)),
            );
            ui.add_space(7.0);

            let mut active = self.state.metronome_active.get();
            let mut bpm = self.state.metronome_bpm.get();
            let mut volume = self.state.metronome_volume.get();
            ui.label(RichText::new(format!("{} BPM", bpm)).size(44.0));

            // BPM Slider with -/+ buttons on either side. The row width is known up
            // front so we can allocate an exactly sized, horizontally centered row
            let slider_width = ui.available_width() * 0.5;
            let button_size = Vec2::new(30.0, 30.0);
            let item_spacing = ui.spacing().item_spacing.x;
            let row_width = button_size.x * 2.0 + slider_width + item_spacing * 2.0;
            // Set the width on the parent UI so the BPM slider (a child UI) inherits
            // it and the Volume slider below keeps the same width.
            ui.style_mut().spacing.slider_width = slider_width;
            ui.allocate_ui_with_layout(
                Vec2::new(row_width, 30.0),
                egui::Layout::left_to_right(egui::Align::Center),
                |ui| {
                    if ui.add_sized(button_size, egui::Button::new("-")).clicked() {
                        bpm = bpm.saturating_sub(BPM_STEP).max(MIN_BPM);
                        self.state.set_metronome(active, bpm, volume);
                    }

                    if ui
                        .add_sized(
                            Vec2::new(slider_width, 30.0),
                            egui::Slider::new(&mut bpm, MIN_BPM..=MAX_BPM).show_value(false),
                        )
                        .changed()
                    {
                        self.state.set_metronome(active, bpm, volume)
                    }

                    if ui.add_sized(button_size, egui::Button::new("+")).clicked() {
                        bpm = bpm.saturating_add(BPM_STEP).min(MAX_BPM);
                        self.state.set_metronome(active, bpm, volume);
                    }
                },
            );

            ui.add_space(10.0);

            // Volume Slider
            ui.label("Volume");
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
