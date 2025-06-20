use std::path::Path;
use eframe::egui;

use crate::pedals::PedalParameterValue;

//pub mod vst3;
pub mod vst2;

pub trait PluginHost: Sized {
    fn load<P: AsRef<Path>>(path: P) -> Result<Self, ()>;
    fn set_config(&mut self, buffer_size: usize, sample_rate: usize);
    fn plugin_name(&self) -> String;
    fn process(&mut self, input: &mut [f32], output: &mut [f32]);
    fn open_ui(&mut self);
    fn close_ui(&mut self);
    fn ui_frame(&mut self, ui: &mut egui::Ui) -> Option<(String, PedalParameterValue)>;
    fn parameter_count(&self) -> usize;
    fn parameter_name(&self, index: usize) -> String;
    fn parameter_value(&self, index: usize) -> f32;
    fn parameter_label(&self, index: usize) -> String;
    fn set_parameter_value(&mut self, index: usize, value: f32);
}
 