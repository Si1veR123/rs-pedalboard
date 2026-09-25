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
    /// The string a parameter's value is displayed as, e.g. "6000.0 Hz", instead of its raw
    /// number. It takes the place of the number readout of the parameter's slider.
    fn parameter_value_display_string(
        &self,
        _name: &str,
        _value: &PedalParameterValue,
    ) -> Option<String> {
        // By default a parameter has no display string, so its raw number is shown
        None
    }

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
        name: &str,
        parameter: &PedalParameter,
        _location: ParameterUILocation,
    ) -> egui::InnerResponse<Option<PedalParameterValue>> {
        let value_display_string = self.parameter_value_display_string(name, &parameter.value);
        parameter.parameter_editor_ui(ui, value_display_string.as_deref())
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

#[cfg(test)]
mod tests {
    use super::*;

    /// A real pedal's parameter shows its display string in its slider, in the place of the
    /// slider's own number.
    #[test]
    fn a_parameter_slider_shows_the_display_string_of_its_pedal() {
        let mut pedal = Pedal::Tremolo(Tremolo::new());
        let name = "Depth";
        pedal.set_parameter_value(name, PedalParameterValue::Float(0.5));
        let parameter = pedal
            .get_parameters()
            .get(name)
            .unwrap_or_else(|| panic!("no {name} parameter"))
            .clone();

        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(
            egui::RawInput {
                screen_rect: Some(egui::Rect::from_min_size(
                    egui::Pos2::ZERO,
                    egui::vec2(800.0, 600.0),
                )),
                ..Default::default()
            },
            |ui| {
                pedal.parameter_editor_ui(
                    ui,
                    name,
                    &parameter,
                    ParameterUILocation::ParameterWindow,
                );
            },
        );
        // Nothing renders the frame, so the font texture updates are discarded by hand
        output.textures_delta.clear();

        let drawn_texts: Vec<&str> = output
            .shapes
            .iter()
            .filter_map(|clipped_shape| match &clipped_shape.shape {
                egui::Shape::Text(text_shape) => Some(text_shape.galley.text()),
                _ => None,
            })
            .collect();

        assert!(
            drawn_texts.contains(&"50%"),
            "the slider doesn't show the parameter's display string: {drawn_texts:?}"
        );
        // The display string takes the place of the slider's own number, which is gone
        assert!(
            !drawn_texts.iter().any(|text| text.parse::<f64>().is_ok()),
            "the slider still shows its own number: {drawn_texts:?}"
        );
    }
}
