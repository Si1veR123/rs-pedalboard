use eframe::egui;
use enum_dispatch::enum_dispatch;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::hash::Hash;
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
pub use eq::{
    graphic_eq_editor_ui, EqPresets, GraphicEq7, GraphicEqSettings, SimpleEq, BAND_COUNT,
    BAND_FREQS, EQ_DB_GAIN, MAX_FREQ_HZ, MIN_FREQ_HZ,
};
mod nam;
pub use nam::set_nam_save_path;
pub use nam::Nam;
mod impulse_response;
pub use impulse_response::set_ir_save_path;
pub use impulse_response::ImpulseResponse;
mod noise_gate;
pub use noise_gate::NoiseGate;
mod vst2;
pub use vst2::set_vst2_save_path;
pub use vst2::Vst2;
mod vst3;
pub use vst3::set_vst3_save_path;
pub use vst3::Vst3;
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
mod phaser;
pub use phaser::Phaser;

pub mod parameters;
pub use parameters::{ParameterUILocation, PedalParameter, PedalParameterValue};

pub mod info;
pub mod ui;

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
                tracing::warn!(
                    "Attempted to set invalid value for parameter {}: {:?}",
                    name,
                    value
                );
            }
        }
    }

    /// Returns the name of the parameter that needs to be changed, and its value
    ///
    /// `message_buffer` contains messages from the pedal on the processor to the client
    fn ui(
        &mut self,
        _ui: &mut egui::Ui,
        _message_buffer: &[String],
    ) -> Option<(String, PedalParameterValue)> {
        None
    }

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

    fn parameter_editor_ui(
        &mut self,
        ui: &mut egui::Ui,
        _name: &str,
        parameter: &PedalParameter,
        _location: ParameterUILocation,
    ) -> egui::InnerResponse<Option<PedalParameterValue>> {
        parameter.parameter_editor_ui(ui)
    }

    fn get_string_values(&self, _parameter_name: &str) -> Option<Vec<String>> {
        None
    }

    /// Only call after set_config
    fn reset_buffer(&mut self) {}
}

/// Wrapper enum type for serialization in Vec
#[derive(Serialize, Deserialize, Clone, Hash, EnumDiscriminants)]
#[strum_discriminants(derive(EnumIter, Serialize, Deserialize, Hash))]
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
    SimpleEq(SimpleEq),
    ImpulseResponse(ImpulseResponse),
    Nam(Nam),
    NoiseGate(NoiseGate),
    Overdrive(Overdrive),
    Phaser(Phaser),
    PitchShift(PitchShift),
    Reverb(Reverb),
    Tremolo(Tremolo),
    Vibrato(Vibrato),
    Volume(Volume),
    Vst2(Vst2),
    Vst3(Vst3),
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
            PedalDiscriminants::SimpleEq => Pedal::SimpleEq(SimpleEq::new()),
            PedalDiscriminants::Nam => Pedal::Nam(Nam::new()),
            PedalDiscriminants::ImpulseResponse => Pedal::ImpulseResponse(ImpulseResponse::new()),
            PedalDiscriminants::NoiseGate => Pedal::NoiseGate(NoiseGate::new()),
            PedalDiscriminants::Vst2 => Pedal::Vst2(Vst2::new()),
            PedalDiscriminants::Vst3 => Pedal::Vst3(Vst3::new()),
            PedalDiscriminants::Reverb => Pedal::Reverb(Reverb::new()),
            PedalDiscriminants::Vibrato => Pedal::Vibrato(Vibrato::new()),
            PedalDiscriminants::Tremolo => Pedal::Tremolo(Tremolo::new()),
            PedalDiscriminants::AutoWah => Pedal::AutoWah(AutoWah::new()),
            PedalDiscriminants::Wah => Pedal::Wah(Wah::new()),
            PedalDiscriminants::Compressor => Pedal::Compressor(Compressor::new()),
            PedalDiscriminants::Overdrive => Pedal::Overdrive(Overdrive::new()),
            PedalDiscriminants::Phaser => Pedal::Phaser(Phaser::new()),
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
            PedalDiscriminants::SimpleEq => "Simple EQ",
            PedalDiscriminants::Nam => "Neural Amp Modeler",
            PedalDiscriminants::ImpulseResponse => "Impulse Response",
            PedalDiscriminants::NoiseGate => "Noise Gate",
            PedalDiscriminants::Vst2 => "VST2 Plugin",
            PedalDiscriminants::Vst3 => "VST3 Plugin",
            PedalDiscriminants::Reverb => "Reverb",
            PedalDiscriminants::Vibrato => "Vibrato",
            PedalDiscriminants::Tremolo => "Tremolo",
            PedalDiscriminants::AutoWah => "Auto Wah",
            PedalDiscriminants::Wah => "Wah",
            PedalDiscriminants::Compressor => "Compressor",
            PedalDiscriminants::Overdrive => "Overdrive",
            PedalDiscriminants::Phaser => "Phaser",
            PedalDiscriminants::Distortion => "Distortion",
        }
    }
}
