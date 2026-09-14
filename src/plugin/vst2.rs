use std::{
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, OnceLock,
    },
};

use crate::{pedals::PedalParameterValue, unique_time_id};

use eframe::egui::{self, Id};
use vst::{
    host::{Host, HostBuffer, PluginInstance, PluginLoader},
    plugin::{Info, Plugin, PluginParameters},
};

#[cfg(target_os = "windows")]
pub const VST2_PLUGIN_PATH: &str = r"C:\Program Files\Steinberg\VSTPlugins";
#[cfg(target_os = "linux")]
pub const VST2_PLUGIN_PATH: &str = "/usr/lib/vst";
#[cfg(target_os = "macos")]
pub const VST2_PLUGIN_PATH: &str = "/Library/Audio/Plug-Ins/VST";

fn get_global_host() -> Arc<Mutex<PedalboardVst2Host>> {
    static HOST: OnceLock<Arc<Mutex<PedalboardVst2Host>>> = OnceLock::new();
    HOST.get_or_init(|| Arc::new(Mutex::new(PedalboardVst2Host)))
        .clone()
}

fn create_host() -> Arc<Mutex<PedalboardVst2Host>> {
    Arc::new(Mutex::new(PedalboardVst2Host))
}

static USE_VST2_GLOBAL_HOST: AtomicBool = AtomicBool::new(true);

pub fn set_use_vst2_global_host(use_global_host: bool) {
    USE_VST2_GLOBAL_HOST.store(use_global_host, Ordering::SeqCst);
}

pub fn use_vst2_global_host() -> bool {
    USE_VST2_GLOBAL_HOST.load(Ordering::SeqCst)
}

fn get_host_for_loader() -> Arc<Mutex<PedalboardVst2Host>> {
    if use_vst2_global_host() {
        get_global_host()
    } else {
        create_host()
    }
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

struct PedalboardVst2Host;
impl Host for PedalboardVst2Host {
    fn automate(&self, _index: i32, _value: f32) {
        //tracing::info!("Automating parameter {} with value {}", index, value);
    }

    fn get_info(&self) -> (isize, String, String) {
        (
            1,
            "Pedalboard VST Host".to_string(),
            "Pedalboard VST Host".to_string(),
        )
    }
}

pub struct Vst2Instance {
    pub instance: PluginInstance,
    pub info: Info,
    params: Arc<dyn PluginParameters>,
    in_buffers: Vec<Vec<f32>>,
    out_buffers: Vec<Vec<f32>>,
    host_buffer: HostBuffer<f32>,
    id: u32,
    pub ui_open: bool,
    dll_path: PathBuf,
    sample_rate: f32,
    buffer_size: usize,
}

impl Vst2Instance {
    pub fn is_configured(&self) -> bool {
        self.sample_rate > 0.0
            && self.buffer_size > 0
            && !self.in_buffers.is_empty()
            && !self.out_buffers.is_empty()
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
        let mut instance = Self::load(self.dll_path.as_path())
            .expect("Plugin has previously been loaded - Clone should succeed");
        instance.set_config(self.buffer_size, self.sample_rate as u32);
        instance
    }
}

impl Vst2Instance {
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, ()> {
        let mut loader =
            PluginLoader::load(path.as_ref(), get_host_for_loader()).map_err(|_| ())?;

        let mut instance = loader.instance().map_err(|_| ())?;

        let info = instance.get_info();
        if info.inputs == 0 || info.outputs == 0 {
            return Err(());
        }

        instance.init();

        let params = instance.get_parameter_object();
        // `HostBuffer` owns the per-channel pointers that are handed to the plugin when processing.
        let host_buffer = HostBuffer::from_info(&info);

        let in_buffers = Vec::new();
        let out_buffers = Vec::new();

        Ok(Vst2Instance {
            in_buffers,
            out_buffers,
            host_buffer,
            info,
            instance: instance,
            params,
            id: unique_time_id(),
            ui_open: false,
            dll_path: path.as_ref().to_path_buf(),
            sample_rate: 0.0,
            buffer_size: 0,
        })
    }

    pub fn set_config(&mut self, buffer_size: usize, sample_rate: u32) {
        self.buffer_size = buffer_size;
        self.sample_rate = sample_rate as f32;

        self.instance.set_block_size(buffer_size as i64);
        self.instance.set_sample_rate(sample_rate as f32);

        // One channel buffer per plugin channel. `process` trims these to the current block
        // length, so the capacity allocated here is reused instead of being reallocated.
        self.in_buffers = (0..self.info.inputs)
            .map(|_| vec![0.0; buffer_size])
            .collect();

        self.out_buffers = (0..self.info.outputs)
            .map(|_| vec![0.0; buffer_size])
            .collect();
    }

    pub fn plugin_name(&self) -> String {
        self.instance.get_info().name
    }

    /// Ensure that `set_config` has been called before processing audio.
    pub fn process(&mut self, input: &mut [f32], output: &mut [f32]) {
        assert_eq!(
            input.len(),
            output.len(),
            "Input and output buffers must have the same length"
        );
        assert!(
            self.is_configured(),
            "VST instance must be configured with sample rate and buffer size before processing"
        );
        assert!(
            input.len() <= self.buffer_size,
            "Input buffer length must not exceed configured buffer size"
        );

        output.fill(0.0);

        // `HostBuffer::bind` takes the block length from the bound slices, so the channel
        // buffers are sized to exactly `input.len()` samples before binding them.
        for in_buf in &mut self.in_buffers {
            in_buf.clear();
            in_buf.extend_from_slice(input);
        }
        for out_buf in &mut self.out_buffers {
            out_buf.clear();
            out_buf.resize(input.len(), 0.0);
        }

        {
            // `HostBuffer` owns the per-channel pointers and points them at the channel buffers,
            // giving the plugin the `AudioBuffer` it expects. The borrows of the channel buffers
            // end with this scope, so their processed contents can be read back below.
            let mut buffer = self
                .host_buffer
                .bind(&self.in_buffers, self.out_buffers.as_mut_slice());

            self.instance.process(&mut buffer);
        }

        // Average the plugin's output channels into the output buffer.
        for out_buf in &self.out_buffers {
            for (output_sample, channel_sample) in output.iter_mut().zip(out_buf.iter()) {
                *output_sample += channel_sample / self.out_buffers.len() as f32;
            }
        }
    }

    pub fn open_ui(&mut self) {
        self.ui_open = true;
    }

    pub fn close_ui(&mut self) {
        self.ui_open = false;
    }

    /// Render the window with the VST parameters, if it is open.
    ///
    /// This does not directly update the parameter values. If a change is made, the name and value is returned.
    /// The caller is responsible for updating the parameter in the instance.
    pub fn ui_frame(&mut self, ui: &mut egui::Ui) -> Option<(String, PedalParameterValue)> {
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

                if ui
                    .add(
                        egui::Slider::new(&mut value, 0.0..=1.0)
                            .text(&name)
                            .suffix(label),
                    )
                    .changed()
                {
                    changed_param = Some((name, PedalParameterValue::Float(value)));
                }
            }
        });

        self.ui_open = ui_open_temp;

        changed_param
    }

    pub fn parameter_count(&self) -> usize {
        self.info.parameters as usize
    }

    pub fn parameter_name(&self, index: usize) -> String {
        if index < self.info.parameters as usize {
            self.params.get_parameter_name(index as i32)
        } else {
            tracing::warn!(
                "Attempted to get name for invalid parameter index: {}",
                index
            );
            "Invalid Parameter".to_string()
        }
    }

    pub fn parameter_value(&self, index: usize) -> f32 {
        if index < self.info.parameters as usize {
            self.params.get_parameter(index as i32)
        } else {
            tracing::warn!(
                "Attempted to get value for invalid parameter index: {}",
                index
            );
            -1.0
        }
    }

    pub fn parameter_label(&self, index: usize) -> String {
        if index < self.info.parameters as usize {
            self.params.get_parameter_label(index as i32)
        } else {
            tracing::warn!(
                "Attempted to get label for invalid parameter index: {}",
                index
            );
            "Invalid Parameter".to_string()
        }
    }

    pub fn set_parameter_value(&mut self, index: usize, value: f32) {
        if index < self.info.parameters as usize {
            self.params.set_parameter(index as i32, value);
        } else {
            tracing::warn!(
                "Attempted to set value for invalid parameter index: {}",
                index
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    #[ignore = "requires a locally installed VST2 plugin"]
    fn test_load_vst2_plugin() {
        let plugin_path = PathBuf::from(r"C:\Program Files\Common Files\VST2\ValhallaFreqEcho_x64.dll");
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
