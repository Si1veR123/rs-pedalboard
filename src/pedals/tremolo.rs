use std::collections::HashMap;
use std::hash::Hash;
use eframe::egui::{self, include_image};
use serde::{Serialize, Deserialize};
use crate::dsp_algorithms::oscillator::{Oscillator, Sine};
use crate::pedals::ui::pedal_switch;
use crate::unique_time_id;
use super::{PedalTrait, PedalParameter, PedalParameterValue};
use super::ui::pedal_knob;

#[derive(Serialize, Deserialize, Clone)]
pub struct Tremolo {
    parameters: HashMap<String, PedalParameter>,
    id: u32
}

impl Hash for Tremolo {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
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
                min: Some(PedalParameterValue::Float(0.1)),
                max: Some(PedalParameterValue::Float(20.0)),
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
        parameters.insert(
            "active".to_string(),
            PedalParameter {
                value: PedalParameterValue::Bool(true),
                min: None,
                max: None,
                step: None,
            },
        );

        Tremolo {
            parameters,
            id: unique_time_id()
        }
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = unique_time_id();
        cloned
    }
}

impl PedalTrait for Tremolo {
    fn get_id(&self) -> u32 {
        self.id
    }

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
        ui.add(egui::Image::new(include_image!("images/tremolo.png")));

        let mut to_change = None;

        let depth_param = self.get_parameters().get("depth").unwrap();
        if let Some(value) = pedal_knob(ui, "", depth_param, egui::Vec2::new(0.3, 0.11), 0.4) {
            to_change =  Some(("depth".to_string(), value));
        }

        let active_param = self.get_parameters().get("active").unwrap().value.as_bool().unwrap();
        if let Some(value) = pedal_switch(ui, active_param, egui::Vec2::new(0.33, 0.72), 0.16) {
            to_change = Some(("active".to_string(), PedalParameterValue::Bool(value)));
        }

        to_change
    }
}