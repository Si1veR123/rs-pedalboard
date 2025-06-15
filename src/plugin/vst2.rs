use std::{path::{Path, PathBuf}, sync::{Arc, Mutex, OnceLock}};

use super::PluginHost;
use crate::{pedals::PedalParameterValue, unique_time_id};

use eframe::egui::{self, Id};
use vst2::{buffer::AudioBuffer, host::{Host, PluginInstance, PluginLoader}, plugin::{Info, Plugin}};

#[cfg(target_os = "windows")]
const VST2_PLUGIN_PATH: &str = r"C:\Program Files\Steinberg\VSTPlugins";
#[cfg(target_os = "linux")]
const VST2_PLUGIN_PATH: &str = "/usr/lib/vst";
#[cfg(target_os = "macos")]
const VST2_PLUGIN_PATH: &str = "/Library/Audio/Plug-Ins/VST";

fn get_global_host() -> Arc<Mutex<PedalboardVst2Host>> {
    static HOST: OnceLock<Arc<Mutex<PedalboardVst2Host>>> = OnceLock::new();
    HOST.get_or_init(|| Arc::new(Mutex::new(PedalboardVst2Host))).clone()
}

pub fn path_from_name(name: &str) -> Option<PathBuf> {
    let mut path = PathBuf::from(VST2_PLUGIN_PATH);
    path.push(name);
    if path.exists() {
        Some(path)
    } else {
        None
    }
}

pub fn available_plugins() -> Vec<String> {
    let mut plugins = Vec::new();
    if let Ok(entries) = std::fs::read_dir(VST2_PLUGIN_PATH) {
        for entry in entries.flatten() {
            if let Some(name) = entry.path().file_name().and_then(|n| n.to_str()) {
                if name.ends_with(".dll") {
                    plugins.push(name.to_string());
                }
            }
        }
    }
    plugins
}

struct PedalboardVst2Host;
impl Host for PedalboardVst2Host {
    fn automate(&mut self, index: i32, value: f32) {
        log::info!("Automating parameter {} with value {}", index, value);
    }

    fn get_info(&self) -> (isize, String, String) {
        (1, "Pedalboard VST Host".to_string(), "Pedalboard VST Host".to_string())
    }
}

pub struct Vst2Instance {
    pub instance: PluginInstance,
    pub info: Info,
    in_buffers: Vec<Vec<f32>>,
    out_buffers: Vec<Vec<f32>>,
    id: usize,
    pub ui_open: bool,
    dll_path: PathBuf,
    sample_rate: f32,
    buffer_size: usize,
}

impl Vst2Instance {
    pub fn is_configured(&self) -> bool {
        self.sample_rate > 0.0 && self.buffer_size > 0
    }

    pub fn dll_path(&self) -> &Path {
        self.dll_path.as_path()
    }

    pub fn sample_rate(&self) -> f32 {
        self.sample_rate
    }

    pub fn buffer_size(&self) -> usize {
        self.buffer_size
    }
}

impl Clone for Vst2Instance {
    fn clone(&self) -> Self {
        let mut instance = Self::load(self.dll_path.as_path()).expect("Plugin has previously been loaded - Clone should succeed");
        instance.set_config(self.buffer_size, self.sample_rate as usize);
        instance
    }
}

impl PluginHost for Vst2Instance {
    fn load<P: AsRef<Path>>(path: P) -> Result<Self, ()> {
        let mut loader = PluginLoader::load(path.as_ref(), get_global_host()).map_err(|_| ())?;

        let mut instance = loader.instance().map_err(|_| ())?;

        let info = instance.get_info();
        if info.inputs == 0 || info.outputs == 0 {
            return Err(());
        }

        instance.init();

        Ok(Vst2Instance {
            in_buffers: vec![vec![0.0; 0]; info.inputs as usize],
            out_buffers: vec![vec![0.0; 0]; info.outputs as usize],
            info,
            instance: instance,
            id: unique_time_id(),
            ui_open: false,
            dll_path: path.as_ref().to_path_buf(),
            sample_rate: 0.0,
            buffer_size: 0,
        })
    }

    fn set_config(&mut self, buffer_size: usize, sample_rate: usize) {
        self.buffer_size = buffer_size;
        self.sample_rate = sample_rate as f32;

        self.instance.set_block_size(buffer_size as i64);
        self.instance.set_sample_rate(sample_rate as f32);

        self.in_buffers.iter_mut().for_each(|buf| buf.resize(buffer_size, 0.0));
        self.out_buffers.iter_mut().for_each(|buf| buf.resize(buffer_size, 0.0));
    }

    fn plugin_name(&self) -> String {
        self.instance.get_info().name
    }

    /// Ensure that `set_config` has been called before processing audio.
    fn process(&mut self, input: &mut [f32], output: &mut [f32]) {
        assert_eq!(input.len(), output.len(), "Input and output buffers must have the same length");

        for in_buf in &mut self.in_buffers {
            in_buf.resize(input.len(), 0.0);
            in_buf.as_mut_slice().copy_from_slice(input);
        }

        for out_buf in &mut self.out_buffers {
            out_buf.resize(output.len(), 0.0);
            out_buf.fill(0.0);
        }

        // TODO: remove allocations
        let input_buffer = self.in_buffers.iter_mut().map(|buf| buf.as_mut_slice()).collect::<Vec<_>>();
        let output_buffer = self.out_buffers.iter_mut().map(|buf| buf.as_mut_slice()).collect::<Vec<_>>();

        let buffer = AudioBuffer::new(input_buffer, output_buffer);

        self.instance.process(buffer);

        output.copy_from_slice(&self.out_buffers[0]);
    }

    fn open_ui(&mut self) {
        self.ui_open = true;
    }
    
    fn close_ui(&mut self) {
        self.ui_open = false;
    }

    /// Render the window with the VST parameters, if it is open.
    /// 
    /// This does not directly update the parameter values. If a change is made, the name and value is returned.
    /// The caller is responsible for updating the parameter in the instance.
    fn ui_frame(&mut self, ui: &mut egui::Ui) -> Option<(String, PedalParameterValue)> {
        let mut ui_open_temp = self.ui_open;
        let window = egui::Window::new(&self.info.name)
            .id(Id::new(&self.info.name).with(self.id))
            .open(&mut ui_open_temp)
            .collapsible(false);

        let mut changed_param = None;
        window.show(ui.ctx(), |ui| {
            for parameter_idx in 0..self.parameter_count() {
                let name = self.parameter_name(parameter_idx);
                let mut value = self.parameter_value(parameter_idx);
                let label = self.parameter_label(parameter_idx);

                if ui.add(
                    egui::Slider::new(&mut value, 0.0..=1.0)
                        .text(&name)
                        .suffix(label)
                ).changed() {
                    changed_param = Some((name, PedalParameterValue::Float(value)));
                }
            }
        });

        self.ui_open = ui_open_temp;

        changed_param
    }

    fn parameter_count(&self) -> usize {
        self.info.parameters as usize
    }
    
    fn parameter_name(&self, index: usize) -> String {
        if index < self.info.parameters as usize {
            self.instance.get_parameter_name(index as i32)
        } else {
            log::warn!("Attempted to get name for invalid parameter index: {}", index);
            "Invalid Parameter".to_string()
        }
    }
    
    fn parameter_value(&self, index: usize) -> f32 {
        if index < self.info.parameters as usize {
            self.instance.get_parameter(index as i32)
        } else {
            log::warn!("Attempted to get value for invalid parameter index: {}", index);
            -1.0
        }
    }
    
    fn parameter_label(&self, index: usize) -> String {
        if index < self.info.parameters as usize {
            self.instance.get_parameter_label(index as i32)
        } else {
            log::warn!("Attempted to get label for invalid parameter index: {}", index);
            "Invalid Parameter".to_string()
        }
    }
    
    fn set_parameter_value(&mut self, index: usize, value: f32) {
        if index < self.info.parameters as usize {
            self.instance.set_parameter(index as i32, value);
        } else {
            log::warn!("Attempted to set value for invalid parameter index: {}", index);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_load_vst2_plugin() {
        let plugin_path = PathBuf::from(r"C:\Program Files\Steinberg\VSTPlugins\NA Black.dll");
        let instance = Vst2Instance::load(plugin_path);
        assert!(instance.is_ok());
        let mut instance = instance.unwrap();
        instance.set_config(512, 48000);

        let info = instance.instance.get_info();
        println!("Inputs: {}, Outputs: {}", info.inputs, info.outputs);

        let mut input = vec![0.1; 512];
        let mut output = vec![0.0; 512];

        instance.process(&mut input, &mut output);
        assert_eq!(output.len(), input.len());
        println!("Output: {:?}", &output[..100]); // Print first 100 samples of output
    }
}
