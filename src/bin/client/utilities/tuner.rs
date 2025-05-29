use eframe::egui::{self, Widget};

use crate::state::State;
use rs_pedalboard::dsp_algorithms::yin::freq_to_note;

pub struct TunerWidget {
    pub state: &'static State,
    recent_freq: f32,
    pub active: bool,
}

impl TunerWidget {
    pub fn new(state: &'static State) -> Self {
        Self { state, recent_freq: 0.0, active: false }
    }

    fn smooth_update(&mut self, new_freq: f32) {
        // Smooth the frequency update to avoid abrupt changes
        // If the change is large, assume a note change
        if (new_freq - self.recent_freq).abs() / self.recent_freq > 0.1 {
            self.recent_freq = new_freq;
        } else {
            self.recent_freq = 0.4 * new_freq + 0.6 * self.recent_freq;
        }
    }

    pub fn update_frequency(&mut self) {
        if let Some(cmd) = self.state.get_command("tuner") {
            if let Some(freq) = cmd.get(6..) {
                if let Ok(freq) = freq.parse::<f32>() {
                    self.smooth_update(freq);
                    log::debug!("Tuner frequency updated: {:?}", self.recent_freq);
                } else {
                    log::warn!("Failed to parse frequency from command: {}", cmd);
                }
            } else {
                log::warn!("Tuner command does not contain frequency: {}", cmd);
            }
        }
    }
}

impl Widget for &mut TunerWidget {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        if self.active {
            ui.ctx().request_repaint();
        }

        self.update_frequency();
        let changed = ui.horizontal(|ui| {
            ui.label("Tuner:");
            let mut changed = false;
            changed |= ui.radio_value(&mut self.active, true, "On").changed();
            changed |= ui.radio_value(&mut self.active, false, "Off").changed();
            changed
        }).inner;

        if changed {
            self.state.set_tuner_active(self.active);
        }
        let recent_note = freq_to_note(self.recent_freq);
        ui.label(format!(
            "Note: {:?}, Octave: {}, Semitone Cents Offset: {}",
            recent_note.0, recent_note.1, recent_note.2
        ))
    }
}
