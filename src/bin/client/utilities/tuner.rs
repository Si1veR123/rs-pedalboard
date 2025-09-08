use std::time::Instant;

use eframe::egui::{self, Color32, RichText, Vec2, Widget};

use crate::state::State;
use rs_pedalboard::dsp_algorithms::yin::{freq_to_note, SERVER_UPDATE_FREQ_MS};

pub struct TunerWidget {
    pub state: &'static State,
    recent_freq: f32,
    recent_freq_smooth: f32,
    last_update: Instant,
    command_buffer: Vec<String>
}

impl TunerWidget {
    pub fn new(state: &'static State) -> Self {
        Self { state, recent_freq: 0.0, recent_freq_smooth: 0.0, command_buffer: Vec::with_capacity(1), last_update: Instant::now() }
    }

    pub fn update_frequency(&mut self) {
        // Smooth the recent_freq_smooth towards recent_freq
        let update_frac = self.last_update.elapsed().as_millis() as f32 / SERVER_UPDATE_FREQ_MS as f32;
        self.last_update = Instant::now();
        if self.recent_freq_smooth != self.recent_freq {
            self.recent_freq_smooth += (self.recent_freq - self.recent_freq_smooth) * update_frac;
        }

        self.state.get_commands("tuner", &mut self.command_buffer);
        if !self.command_buffer.is_empty() {
            let cmd = self.command_buffer.remove(0);
            if let Ok(freq) = cmd.parse::<f32>() {
                self.recent_freq = freq;
                if (self.recent_freq - self.recent_freq_smooth).abs() > 5.0 {
                    self.recent_freq_smooth = self.recent_freq;
                }

                log::debug!("Tuner frequency updated: {:?}", self.recent_freq);
            } else {
                log::warn!("Failed to parse frequency from command: {}", cmd);
            }
        }
    }
}


impl Widget for &mut TunerWidget {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let active = self.state.tuner_active.get();

        if active {
            self.update_frequency();
            ui.ctx().request_repaint();
        }

        let (note_name, octave, cents_offset) = if self.recent_freq_smooth == 0.0 {
            let question = String::from("?");
            (question.clone(), question.clone(), 0.0)
        } else {
            let recent_note = freq_to_note(self.recent_freq_smooth);
            (recent_note.0.to_string(), recent_note.1.to_string(), recent_note.2)
        };
        
        ui.vertical_centered(|ui| {
            ui.add_space(10.0);
            ui.label(RichText::from("Tuner").size(30.0).color(Color32::from_gray(90)));
            ui.add_space(7.0);

            // Note name
            ui.label(RichText::new(format!("{}", note_name)).size(50.0));
            // Octave
            ui.label(RichText::new(octave).size(25.0));

            // Cents offset
            let bar_height = 50.0;
            let bg_im = egui::Image::new(egui::include_image!("../files/tuner_bar.png")).max_height(bar_height);
            let bg_response = ui.add(bg_im);
            let needle_im = egui::Image::new(egui::include_image!("../files/tuner_needle.png"))
                .max_height(bar_height-10.0)
                .tint(crate::BACKGROUND_COLOR);
            let needle_size = match needle_im.load_for_size(ui.ctx(), Vec2::splat(50.0)).expect("Failed to load needle image size").size() {
                Some(size) => size,
                None => {
                    log::warn!("Failed to load needle image size");
                    return;
                }
            };

            let bar_width = bg_response.rect.width();
            let needle_x_frac = (cents_offset as f32+50.0) / 100.0;
            let needle_x = (bar_width * needle_x_frac).clamp(0.0, bar_width - needle_size.x);

            let min = bg_response.rect.min + Vec2::new(needle_x - needle_size.x / 2.0, bar_height - needle_size.y);

            needle_im.paint_at(ui, egui::Rect{
                min,
                max: min + Vec2::new(needle_size.x, needle_size.y),
            });

            // Cents offset label
            let cents_label = if cents_offset == 0.0 {
                String::from("0")
            } else if cents_offset > 0.0 {
                format!("+{}", cents_offset.round() as isize)
            } else {
                format!("{}", cents_offset.round() as isize)
            };
            ui.label(RichText::new(cents_label).size(20.0));

            ui.add_space(10.0);
        }).response
    }
}
