use std::collections::HashMap;
use std::hash::Hash;
use eframe::egui::{self, include_image, Color32, RichText, Vec2};
use serde::{ser::SerializeMap, Deserialize, Serialize};
use super::{PedalTrait, PedalParameter, PedalParameterValue};
use crate::{dsp_algorithms::{oscillator::{Oscillator, Sine}, variable_delay::VariableDelayLine}, pedals::ui::{oscillator_selection_window, pedal_knob, pedal_label_rect}};

#[derive(Clone)]
pub struct Vibrato {
    delay_line: VariableDelayLine,
    parameters: HashMap<String, PedalParameter>,
    oscillator_open: bool,
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
        let depth_ms = parameters.get("depth").unwrap().value.as_float().unwrap();

        let max_delay_samples = (48000.0 * depth_ms / 1000.0).ceil() as usize;

        let variable_delay = VariableDelayLine::new(max_delay_samples+1);

        Ok(Self {
            delay_line: variable_delay,
            parameters: parameters.clone(),
            oscillator_open: false,
        })
    }
}

impl Vibrato {
    pub fn new() -> Self {
        let max_delay_ms = 15.0f32;
        let max_delay_samples = (48000.0 * max_delay_ms / 1000.0).ceil() as usize;

        let oscillator = Oscillator::Sine(Sine::new(48000.0, 5.0, 0.0, 0.0));

        let mut parameters = HashMap::new();
        parameters.insert(
            "depth".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(5.0),
                min: Some(PedalParameterValue::Float(0.1)),
                max: Some(PedalParameterValue::Float(max_delay_ms)),
                step: Some(PedalParameterValue::Float(0.1)),
            },
        );
        parameters.insert(
            "oscillator".to_string(),
            PedalParameter {
                value: PedalParameterValue::Oscillator(oscillator),
                min: None,
                max: None,
                step: None,
            },
        );

        Self {
            delay_line: VariableDelayLine::new(max_delay_samples+1),
            parameters,
            oscillator_open: false,
        }
    }
}

impl PedalTrait for Vibrato {
    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        let depth_ms = self.parameters["depth"].value.as_float().unwrap();
        let oscillator = match &mut self.parameters.get_mut("oscillator").unwrap().value {
            PedalParameterValue::Oscillator(ref mut osc) => osc,
            _ => return,
        };
    
        let depth_samples = 48000.0 * depth_ms / 1000.0;
    
        for sample in buffer.iter_mut() {
            self.delay_line.buffer.push_front(*sample);
            self.delay_line.buffer.pop_back();
    
            let lfo_value = 0.5 * (1.0 + oscillator.next().unwrap());
    
            let current_delay = lfo_value * depth_samples;
    
            let delayed_sample = self.delay_line.get_sample(current_delay);
    
            *sample = delayed_sample;
        }
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    fn ui(&mut self, ui: &mut egui::Ui, _message_buffer: &[String]) -> Option<(String, PedalParameterValue)> {
        ui.add(egui::Image::new(include_image!("images/pedal_base.png")));

        let mut to_change = None;

        let depth_param = self.get_parameters().get("depth").unwrap();
        if let Some(value) = pedal_knob(ui, RichText::new("Depth").color(Color32::BLACK).size(8.0), depth_param, egui::Vec2::new(0.38, 0.02), 0.25) {
            to_change =  Some(("depth".to_string(), value));
        }

        let offset_x = 0.2 * ui.available_width();
        let offset_y = 0.3 * ui.available_height();

        let oscillator_button_rect = egui::Rect::from_min_size(
            ui.max_rect().min + Vec2::new(offset_x, offset_y),
            Vec2::new(0.6 * ui.available_width(), 0.1 * ui.available_height())
        );

        if ui.put(oscillator_button_rect, egui::Button::new(
            RichText::new("Oscillator")
                .color(Color32::WHITE)
                .size(9.0)
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

        let pedal_rect = ui.max_rect();
        ui.put(pedal_label_rect(pedal_rect), egui::Label::new(
            egui::RichText::new("Vibrato")
                .color(egui::Color32::from_black_alpha(200))
        ));

        to_change
    }
}