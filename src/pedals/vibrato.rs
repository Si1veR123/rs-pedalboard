use std::collections::HashMap;
use std::hash::Hash;
use eframe::egui::{self, include_image};
use serde::{ser::SerializeMap, Deserialize, Serialize};
use super::{PedalTrait, PedalParameter, PedalParameterValue};
use crate::{
    dsp_algorithms::{oscillator::{Oscillator, Sine},
    variable_delay::VariableDelayLine},
    pedals::ui::{pedal_knob, pedal_switch},
    unique_time_id
};

pub struct Vibrato {
    delay_line: Option<VariableDelayLine>,
    parameters: HashMap<String, PedalParameter>,
    id: u32
}

impl Clone for Vibrato {
    fn clone(&self) -> Self {
        Vibrato {
            delay_line: self.delay_line.clone(),
            parameters: self.parameters.clone(),
            id: unique_time_id()
        }
    }
}

impl Hash for Vibrato {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.parameters.values().map(|p| &p.value).for_each(|v| v.hash(state));
    }
}

impl Serialize for Vibrato {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser_map = serializer.serialize_map(Some(self.parameters.len()))?;
        for (key, value) in &self.parameters {
            ser_map.serialize_entry(key, value)?;
        }
        ser_map.end()
    }
}

impl<'a> Deserialize<'a> for Vibrato {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        let parameters = HashMap::<String, PedalParameter>::deserialize(deserializer)?;

        Ok(Self {
            delay_line: None,
            parameters: parameters.clone(),
            id: unique_time_id()
        })
    }
}

impl Vibrato {
    pub fn new() -> Self {
        // Oscilallator sample rate not used on client, and is set later in `set_config` on server, so its ok to be hardcoded
        let oscillator = Oscillator::Sine(Sine::new(48000.0, 5.0, 0.0, 0.0));

        let mut parameters = HashMap::new();
        parameters.insert(
            "depth".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(5.0),
                min: Some(PedalParameterValue::Float(0.01)),
                max: Some(PedalParameterValue::Float(15.0)),
                step: Some(PedalParameterValue::Float(0.1)),
            },
        );
        parameters.insert(
            "oscillator".to_string(),
            PedalParameter {
                value: PedalParameterValue::Oscillator(oscillator),
                min: Some(PedalParameterValue::Float(0.1)),
                max: Some(PedalParameterValue::Float(20.0)),
                step: None,
            },
        );
        parameters.insert(
            "dry_wet".to_string(),
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

        Self {
            delay_line: None,
            parameters,
            id: unique_time_id()
        }
    }
}

impl PedalTrait for Vibrato {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        if self.delay_line.is_none() {
            log::warn!("Vibrato pedal not initialized. Call set_config before processing audio.");
            return;
        }

        let dry_wet = self.parameters.get("dry_wet").unwrap().value.as_float().unwrap();
        let oscillator = self.parameters.get_mut("oscillator").unwrap().value.as_oscillator_mut().unwrap();
        let delay_line = self.delay_line.as_mut().unwrap();

        for sample in buffer.iter_mut() {
            delay_line.buffer.push_front(*sample);
            delay_line.buffer.pop_back();
    
            let lfo_value = 0.5 * (1.0 + oscillator.next().unwrap());
    
            let current_delay = lfo_value * delay_line.max_delay() as f32;
    
            let delayed_sample = delay_line.get_sample(current_delay);
    
            *sample = delayed_sample * dry_wet + *sample * (1.0 - dry_wet);
        }
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    fn ui(&mut self, ui: &mut egui::Ui, _message_buffer: &[String]) -> Option<(String, PedalParameterValue)> {
        ui.add(egui::Image::new(include_image!("images/vibrato.png")));

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

    fn set_config(&mut self, _buffer_size:usize,sample_rate:u32) {
        let depth_ms = self.parameters.get("depth").unwrap().value.as_float().unwrap();
        let max_delay_samples = (sample_rate as f32 * depth_ms / 1000.0).ceil() as usize;

        self.delay_line = Some(VariableDelayLine::new(max_delay_samples));

        self.parameters.get_mut("oscillator").unwrap().value.as_oscillator_mut().unwrap().set_sample_rate(sample_rate as f32);
    }
}