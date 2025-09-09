use std::collections::HashMap;
use std::hash::Hash;
use crate::dsp_algorithms::oscillator::Oscillator;
use enum_dispatch::enum_dispatch;
use serde::{ Deserialize, Serialize};
use eframe::egui;
use strum_macros::{EnumDiscriminants, EnumIter};

mod volume;
pub use volume::Volume;
mod fuzz;
pub use fuzz::Fuzz;
mod pitch_shift;
pub use pitch_shift::PitchShift;
mod modulation;
pub use modulation::{Chorus, Flanger};
mod delay;
pub use delay::Delay;
mod eq;
pub use eq::GraphicEq7;
mod nam;
pub use nam::Nam;
mod impulse_response;
pub use impulse_response::ImpulseResponse;
mod noise_gate;
pub use noise_gate::NoiseGate;
mod vst2;
pub use vst2::Vst2;
mod reverb;
pub use reverb::Reverb;
mod vibrato;
pub use vibrato::Vibrato;
mod tremolo;
pub use tremolo::Tremolo;
mod autowah;
pub use autowah::AutoWah;
mod wah;
pub use wah::Wah;
mod compressor;
pub use compressor::Compressor;
mod overdrive;
pub use overdrive::Overdrive;
mod distortion;
pub use distortion::Distortion;

mod ui;

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PedalParameter {
    pub value: PedalParameterValue,
    // min and max are used for floats and ints
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<PedalParameterValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<PedalParameterValue>,
    // For floats only
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<PedalParameterValue>
}

impl PedalParameter {
    pub fn is_valid(&self, value: &PedalParameterValue) -> bool {
        match value {
            PedalParameterValue::Float(value) => {
                if let Some(PedalParameterValue::Float(min)) = self.min {
                    if *value < min {
                        return false;
                    }
                }
                if let Some(PedalParameterValue::Float(max)) = self.max {
                    if *value > max {
                        return false;
                    }
                }
                
                // Don't validate float step, but it can be used for hinting to UI

                true
            }
            PedalParameterValue::Int(value) => {
                if let Some(PedalParameterValue::Int(min)) = self.min {
                    if *value < min {
                        return false;
                    }
                }
                if let Some(PedalParameterValue::Int(max)) = self.max {
                    if *value > max {
                        return false;
                    }
                }
                true
            },
            _ => true
        }
    }

    pub fn int_to_float(&self) -> Self {
        if let PedalParameterValue::Int(value) = self.value {
            let new_parameter = PedalParameter {
                value: PedalParameterValue::Float(value as f32),
                min: Some(PedalParameterValue::Float(self.min.clone().unwrap().as_int().unwrap() as f32)),
                max: Some(PedalParameterValue::Float(self.max.clone().unwrap().as_int().unwrap() as f32)),
                step: None
            };
            new_parameter
        } else {
            panic!("PedalParameter::int_to_float called on non-int parameter");
        }
    }

    pub fn float_to_int(&self) -> Self {
        if let PedalParameterValue::Float(value) = self.value {
            let new_parameter = PedalParameter {
                value: PedalParameterValue::Int(value as i16),
                min: Some(PedalParameterValue::Int(self.min.clone().unwrap().as_float().unwrap() as i16)),
                max: Some(PedalParameterValue::Int(self.max.clone().unwrap().as_float().unwrap() as i16)),
                step: None
            };
            new_parameter
        } else {
            panic!("PedalParameter::float_to_int called on non-float parameter");
        }
    }

    pub fn parameter_editor_ui(&self, ui: &mut egui::Ui, width: f32) -> egui::InnerResponse<Option<PedalParameterValue>> {
        let mut to_change = None;

        let response = match self.value {
            PedalParameterValue::Float(mut f) => {
                let init_value = f;
                let min = self.min.clone().unwrap().as_float().unwrap_or(0.0);
                let max = self.max.clone().unwrap().as_float().unwrap_or(1.0);
                let response = ui.add(egui::Slider::new(&mut f, min..=max).max_decimals(2));
                f = f.clamp(min, max);
                if f != init_value {
                    to_change = Some(PedalParameterValue::Float(f));
                }
                response
            }
            PedalParameterValue::Bool(mut b) => {
                let init_value = b;
                let response = ui.checkbox(&mut b, "");
                if b != init_value {
                    to_change = Some(PedalParameterValue::Bool(b));
                }
                response
            }
            PedalParameterValue::Int(mut i) => {
                let init_value = i;
                let min = self.min.clone().unwrap().as_int().unwrap_or(0);
                let max = self.max.clone().unwrap().as_int().unwrap_or(100);
                let response = ui.add(egui::Slider::new(&mut i, min..=max));

                if i != init_value {
                    to_change = Some(PedalParameterValue::Int(i));
                }

                response
            }
            PedalParameterValue::Oscillator(_) => {
                let inner_response = ui::oscillator_selection_window(ui, &self, width, false);
                if let Some(oscillator) = inner_response.inner {
                    to_change = Some(PedalParameterValue::Oscillator(oscillator));
                }
                inner_response.response
            }

            PedalParameterValue::String(_) => {
                let mut text = self.value.as_str().unwrap().to_string();
                let response = ui.text_edit_singleline(&mut text);
                if response.changed() {
                    to_change = Some(PedalParameterValue::String(text));
                };
                response
            }
        };

        egui::InnerResponse::new(to_change, response)
    }
}


#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum PedalParameterValue {
    Float(f32),
    String(String),
    Bool(bool),
    Int(i16),
    Oscillator(Oscillator)
}

impl Hash for PedalParameterValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            PedalParameterValue::Float(value) => value.to_bits().hash(state),
            PedalParameterValue::String(value) => value.hash(state),
            PedalParameterValue::Bool(value) => value.hash(state),
            PedalParameterValue::Int(value) => value.hash(state),
            PedalParameterValue::Oscillator(osc) => osc.hash(state),
        }
    }
}

impl PedalParameterValue {
    pub fn as_float(&self) -> Option<f32> {
        match self {
            PedalParameterValue::Float(value) => Some(*value),
            _ => None
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            PedalParameterValue::String(value) => Some(value),
            _ => None
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            PedalParameterValue::Bool(value) => Some(*value),
            _ => None
        }
    }

    pub fn as_int(&self) -> Option<i16> {
        match self {
            PedalParameterValue::Int(value) => Some(*value),
            _ => None
        }
    }

    pub fn as_oscillator(&self) -> Option<&Oscillator> {
        match self {
            PedalParameterValue::Oscillator(osc) => Some(osc),
            _ => None
        }
    }

    pub fn as_oscillator_mut(&mut self) -> Option<&mut Oscillator> {
        match self {
            PedalParameterValue::Oscillator(osc) => Some(osc),
            _ => None
        }
    }
}

#[enum_dispatch]
pub trait PedalTrait {
    /// message_buffer is where messages to send to the client can be passed
    fn process_audio(&mut self, buffer: &mut [f32], message_buffer: &mut Vec<String>);

    fn get_parameters(&self) -> &HashMap<String, PedalParameter>;
    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter>;

    fn set_parameter_value(&mut self, name: &str, value: PedalParameterValue) {
        let parameters = self.get_parameters_mut();
        if let Some(parameter) = parameters.get_mut(name) {
            if parameter.is_valid(&value) {
                parameter.value = value;
            } else {
                log::warn!("Attempted to set invalid value for parameter {}: {:?}", name, value);
            }
        }
    }

    /// Returns the name of the parameter that needs to be changed, and its value
    /// 
    /// `message_buffer` contains messages from the pedal on the server to the client
    fn ui(&mut self, _ui: &mut egui::Ui, _message_buffer: &[String]) -> Option<(String, PedalParameterValue)> { None }

    /// Call after creating a pedal so that it can set up its internal state
    fn set_config(&mut self, _buffer_size: usize, _sample_rate: u32) {}

    fn is_active(&self) -> bool {
        if let Some(param) = self.get_parameters().get("Active") {
            if let PedalParameterValue::Bool(active) = &param.value {
                return *active;
            }
        }
        true
    }

    fn get_id(&self) -> u32;
}

/// Wrapper enum type for serialization in Vec
#[derive(Serialize, Deserialize, Clone, Hash, EnumDiscriminants)]
#[strum_discriminants(derive(EnumIter))]
#[enum_dispatch(PedalTrait)]
pub enum Pedal {
    AutoWah(AutoWah),
    Chorus(Chorus),
    Compressor(Compressor),
    Delay(Delay),
    Distortion(Distortion),
    Flanger(Flanger),
    Fuzz(Fuzz),
    GraphicEq7(GraphicEq7),
    ImpulseResponse(ImpulseResponse),
    Nam(Nam),
    NoiseGate(NoiseGate),
    Overdrive(Overdrive),
    PitchShift(PitchShift),
    Reverb(Reverb),
    Tremolo(Tremolo),
    Vibrato(Vibrato),
    Volume(Volume),
    Vst2(Vst2),
    Wah(Wah),
}

impl PedalDiscriminants {
    pub fn new_pedal(&self) -> Pedal {
        match self {
            PedalDiscriminants::Volume => Pedal::Volume(Volume::new()),
            PedalDiscriminants::Fuzz => Pedal::Fuzz(Fuzz::new()),
            PedalDiscriminants::PitchShift => Pedal::PitchShift(PitchShift::new()),
            PedalDiscriminants::Chorus => Pedal::Chorus(Chorus::new()),
            PedalDiscriminants::Flanger => Pedal::Flanger(Flanger::new()),
            PedalDiscriminants::Delay => Pedal::Delay(Delay::new()),
            PedalDiscriminants::GraphicEq7 => Pedal::GraphicEq7(GraphicEq7::new()),
            PedalDiscriminants::Nam => Pedal::Nam(Nam::new()),
            PedalDiscriminants::ImpulseResponse => Pedal::ImpulseResponse(ImpulseResponse::new()),
            PedalDiscriminants::NoiseGate => Pedal::NoiseGate(NoiseGate::new()),
            PedalDiscriminants::Vst2 => Pedal::Vst2(Vst2::new()),
            PedalDiscriminants::Reverb => Pedal::Reverb(Reverb::new()),
            PedalDiscriminants::Vibrato => Pedal::Vibrato(Vibrato::new()),
            PedalDiscriminants::Tremolo => Pedal::Tremolo(Tremolo::new()),
            PedalDiscriminants::AutoWah => Pedal::AutoWah(AutoWah::new()),
            PedalDiscriminants::Wah => Pedal::Wah(Wah::new()),
            PedalDiscriminants::Compressor => Pedal::Compressor(Compressor::new()),
            PedalDiscriminants::Overdrive => Pedal::Overdrive(Overdrive::new()),
            PedalDiscriminants::Distortion => Pedal::Distortion(Distortion::new()),
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            PedalDiscriminants::Volume => "Volume",
            PedalDiscriminants::Fuzz => "Fuzz",
            PedalDiscriminants::PitchShift => "Pitch Shift",
            PedalDiscriminants::Chorus => "Chorus",
            PedalDiscriminants::Flanger => "Flanger",
            PedalDiscriminants::Delay => "Delay",
            PedalDiscriminants::GraphicEq7 => "Graphic EQ",
            PedalDiscriminants::Nam => "Neural Amp Modeler",
            PedalDiscriminants::ImpulseResponse => "Impulse Response",
            PedalDiscriminants::NoiseGate => "Noise Gate",
            PedalDiscriminants::Vst2 => "VST2 Plugin",
            PedalDiscriminants::Reverb => "Reverb",
            PedalDiscriminants::Vibrato => "Vibrato",
            PedalDiscriminants::Tremolo => "Tremolo",
            PedalDiscriminants::AutoWah => "Auto Wah",
            PedalDiscriminants::Wah => "Wah",
            PedalDiscriminants::Compressor => "Compressor",
            PedalDiscriminants::Overdrive => "Overdrive",
            PedalDiscriminants::Distortion => "Distortion",
        }
    }
}
