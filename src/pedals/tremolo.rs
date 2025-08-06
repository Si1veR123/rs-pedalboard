use std::collections::HashMap;
use std::hash::Hash;
use eframe::egui::{self, Vec2, Color32, RichText, include_image};
use serde::{Serialize, Deserialize};
use crate::dsp_algorithms::oscillator::{Oscillator, Sine};
use super::{PedalTrait, PedalParameter, PedalParameterValue};
use super::ui::{oscillator_selection_window, pedal_knob};


#[derive(Serialize, Deserialize, Clone)]
pub struct Tremolo {
    parameters: HashMap<String, PedalParameter>,
    #[serde(skip)]
    oscillator_open: bool,
}

impl Hash for Tremolo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.parameters.values().map(|p| &p.value).for_each(|v| v.hash(state));
    }
}

impl Tremolo {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();
        parameters.insert(
            "oscillator".to_string(),
            PedalParameter {
                // Sample rate on oscillators is not used on clients so the hardcoded sample rate is ok
                value: PedalParameterValue::Oscillator(Oscillator::Sine(Sine::new(48000.0, 5.0, 0.0, 0.0))),
                min: None,
                max: None,
                step: None,
            },
        );
        parameters.insert(
            "depth".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None,
            },
        );

        Tremolo {
            parameters,
            oscillator_open: false,
        }
    }
}

impl PedalTrait for Tremolo {
    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        let depth = self.parameters.get("depth").unwrap().value.as_float().unwrap();
        let oscillator = self.parameters.get_mut("oscillator").unwrap().value.as_oscillator_mut().unwrap();

        for sample in buffer.iter_mut() {
            let oscillator_value = oscillator.next().unwrap();
            let modulated_value = oscillator_value * depth;
            *sample *= 1.0 + modulated_value;
        }
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    fn set_config(&mut self, _buffer_size:usize,sample_rate:u32) {
        self.parameters.get_mut("oscillator").unwrap().value.as_oscillator_mut().unwrap().set_sample_rate(sample_rate as f32);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _message_buffer: &[String]) -> Option<(String, PedalParameterValue)> {
        let pedal_width = ui.available_width();
        let pedal_height = ui.available_height();

        ui.add(egui::Image::new(include_image!("images/tremolo.png")));

        let mut to_change = None;

        let depth_param = self.get_parameters().get("depth").unwrap();
        if let Some(value) = pedal_knob(ui, "", depth_param, egui::Vec2::new(0.3, 0.11), 0.4) {
            to_change =  Some(("depth".to_string(), value));
        }

        let offset_x = 0.15 * pedal_width;
        let offset_y = 0.43 * pedal_height;

        let oscillator_button_rect = egui::Rect::from_min_size(
            ui.max_rect().min + Vec2::new(offset_x, offset_y),
            Vec2::new(0.7 * ui.available_width(), 0.15 * ui.available_height())
        );

        if ui.put(oscillator_button_rect, egui::Button::new(
            RichText::new("Oscillator")
                .color(Color32::WHITE)
                .size(13.0)
        )).clicked() {
            self.oscillator_open = !self.oscillator_open;
        };

        if self.oscillator_open {
            if let Some(osc) = oscillator_selection_window(
                ui,
                self.parameters.get("oscillator").unwrap().value.as_oscillator().unwrap(),
                &mut self.oscillator_open,
                false,
                Some(0.1..=20.0)
            ) {
                to_change = Some(("oscillator".to_string(), PedalParameterValue::Oscillator(osc)));
            }
        }

        to_change
    }
}