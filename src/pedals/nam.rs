use std::path::PathBuf;
use std::collections::HashMap;
use std::hash::Hash;

use neural_amp_modeler::NeuralAmpModeler;
use serde::{ser::SerializeMap, Deserialize, Serialize};
use eframe::egui;

use super::{PedalParameter, PedalParameterValue, PedalTrait};

const NAM_SAVE_PATH: &str = r"NAM";

pub struct Nam {
    modeler: NeuralAmpModeler,
    parameters: HashMap<String, PedalParameter>,

    saved_nam_files: Vec<PathBuf>,
}

impl Clone for Nam {
    fn clone(&self) -> Self {
        let buf_size = self.modeler.get_maximum_buffer_size();
        let new_modeler = NeuralAmpModeler::new_with_maximum_buffer_size(buf_size).expect("Failed to create neural amp modeler");

        let mut new_nam = Nam {
            modeler: new_modeler,
            parameters: self.parameters.clone(),
            saved_nam_files: Self::saved_nam_files()
        };

        new_nam.set_model(self.modeler.get_model_path().unwrap());
        new_nam
    }
}

impl Hash for Nam {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.parameters.values().map(|p| &p.value).for_each(|v| v.hash(state));
    }
}

impl Serialize for Nam {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser_map = serializer.serialize_map(Some(self.parameters.len()))?;
        for (key, value) in &self.parameters {
            ser_map.serialize_entry(key, value)?;
        }
        Ok(ser_map.end()?)
    }
}

impl<'a> Deserialize<'a> for Nam {
    /// `set_config` must be called to set the buffer size if it is greater than the default maximum size
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        let parameters: HashMap<String, PedalParameter> = HashMap::deserialize(deserializer)?;
        let model = parameters.get("model").unwrap().value.as_str().unwrap();
        let mut pedal = Self::new();

        pedal.set_model(model);
        
        Ok(pedal)
    }
}

impl Nam {
    // If buffer size could be greater than the default maximum size, `set_config` must be called to set the buffer size
    pub fn new() -> Self {
        Self::new_with_maximum_buffer_size(512)
    }

    pub fn new_with_maximum_buffer_size(buffer_size: usize) -> Self {
        let mut parameters = HashMap::new();

        let init_model = r"";
        parameters.insert(
            "model".to_string(),
            PedalParameter {
                value: PedalParameterValue::String(init_model.to_string()),
                min: None,
                max: None,
                step: None,
            },
        );
        let modeler = NeuralAmpModeler::new_with_maximum_buffer_size(buffer_size).expect("Failed to create neural amp modeler");

        let mut modeler_pedal = Nam {
            modeler,
            parameters: parameters.clone(),
            saved_nam_files: Self::saved_nam_files(),
        };

        if !init_model.is_empty() {
            modeler_pedal.set_model(init_model);
        }

        modeler_pedal
    }

    pub fn set_model(&mut self, model_path: &str) {
        if model_path.is_empty() {
            self.remove_model();
            return;
        }
        
        if let Err(e) = self.modeler.set_model(model_path) {
            log::error!("Failed to set model: {}", e);
        } else {
            self.parameters.get_mut("model").unwrap().value = PedalParameterValue::String(model_path.to_string());
        }
    }

    pub fn remove_model(&mut self) {
        self.parameters.get_mut("model").unwrap().value = PedalParameterValue::String("".to_string());
        let buffer_size = self.modeler.get_maximum_buffer_size();
        self.modeler = NeuralAmpModeler::new_with_maximum_buffer_size(buffer_size).expect("Failed to create neural amp modeler");
    }

    pub fn get_save_directory() -> Option<PathBuf> {
        Some(homedir::my_home().ok()??.join(NAM_SAVE_PATH))
    }

    pub fn saved_nam_files() -> Vec<PathBuf> {
        let mut files = Vec::new();
        if let Some(dir) = Self::get_save_directory() {
            if dir.exists() {
                for entry in std::fs::read_dir(dir).unwrap() {
                    let entry = entry.unwrap();
                    if entry.path().extension().map_or(false, |ext| ext == "nam") {
                        files.push(entry.path());
                    }
                }
            } else {
                std::fs::create_dir_all(&dir).unwrap();
            }
        } else {
            log::error!("Failed to get NAM save directory");
        }
        files
    }
}

impl PedalTrait for Nam {
    fn set_config(&mut self, buffer_size: usize, _sample_rate: usize) {
        self.modeler.set_maximum_buffer_size(buffer_size);
    }

    fn process_audio(&mut self, buffer: &mut [f32]) {
        self.modeler.process_buffer(buffer);
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    fn set_parameter_value(&mut self,name: &str, value:PedalParameterValue) {
        if !self.parameters.get(name).unwrap().is_valid(&value) {
            log::warn!("Attempted to set invalid value for parameter {}: {:?}", name, value);
            return;
        }

        if let Some(param) = self.parameters.get_mut(name) {
            if name == "model" {
                self.set_model(value.as_str().unwrap());
            } else {
                param.value = value;
            }
        } else {
            log::error!("Parameter {} not found", name);
        }
    }

    fn ui(&mut self, ui: &mut eframe::egui::Ui) -> Option<(String,PedalParameterValue)> {
        // ew, TODO: make NeuralAmpModeler::get_model_path() return a PathBuf instead of a String
        let selected = PathBuf::from(self.modeler.get_model_path().unwrap_or_default());
        let selected_file_name = selected.file_name().unwrap_or_default().to_string_lossy();
        let mut selected_str = selected.to_string_lossy().to_string();
        let old = selected_str.clone();
        
        egui::ComboBox::from_label("Model")
            .selected_text(selected_file_name)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut selected_str, String::new(), "Empty");
                for file in &self.saved_nam_files {
                    let name = file.file_name().unwrap().to_string_lossy();

                    ui.selectable_value(&mut selected_str, file.to_string_lossy().to_string(), name);
                }
            });

        if selected_str != old {
            Some((String::from("model"), PedalParameterValue::String(selected_str)))
        } else {
            None
        }
    }
}
