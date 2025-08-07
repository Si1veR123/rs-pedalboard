use std::{path::PathBuf, vec};
use std::collections::HashMap;
use std::hash::Hash;

use neural_amp_modeler::NeuralAmpModeler;
use serde::{ser::SerializeMap, Deserialize, Serialize};
use eframe::egui::{self, include_image, Vec2};

use super::{ui::pedal_knob, PedalParameter, PedalParameterValue, PedalTrait};
use crate::pedals::ui::pedal_switch;
use crate::{unique_time_id, SAVE_DIR};

const NAM_SAVE_PATH: &str = r"NAM";

pub struct Nam {
    modeler: NeuralAmpModeler,
    parameters: HashMap<String, PedalParameter>,

    dry_buffer: Vec<f32>,
    saved_nam_files: Vec<PathBuf>,
    // Used to generate a unique ID for the drop down menu
    id: u32
}

impl Clone for Nam {
    fn clone(&self) -> Self {
        let buf_size = self.modeler.get_maximum_buffer_size();
        let new_modeler = NeuralAmpModeler::new_with_maximum_buffer_size(buf_size).expect("Failed to create neural amp modeler");

        let mut new_nam = Nam {
            modeler: new_modeler,
            parameters: self.parameters.clone(),
            dry_buffer: vec![0.0; buf_size],
            saved_nam_files: Self::saved_nam_files(),
            id: unique_time_id()
        };

        if let Some(model_path) = self.modeler.get_model_path() {
            new_nam.set_model(model_path);
        }
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

        parameters.insert(
            "gain".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.00)),
                max: Some(PedalParameterValue::Float(3.0)),
                step: Some(PedalParameterValue::Float(0.05)),
            },
        );

        parameters.insert(
            "dry_wet".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: Some(PedalParameterValue::Float(0.01)),
            },
        );

        parameters.insert(
            "level".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(3.0)),
                step: Some(PedalParameterValue::Float(0.05)),
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

        let modeler = NeuralAmpModeler::new_with_maximum_buffer_size(buffer_size).expect("Failed to create neural amp modeler");

        let mut modeler_pedal = Nam {
            modeler,
            parameters: parameters.clone(),
            dry_buffer: vec![0.0; buffer_size],
            saved_nam_files: Self::saved_nam_files(),
            id: unique_time_id()
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
        Some(homedir::my_home().ok()??.join(SAVE_DIR).join(NAM_SAVE_PATH))
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
    fn get_id(&self) -> u32 {
        self.id
    }

    fn set_config(&mut self, buffer_size: usize, sample_rate: u32) {
        self.modeler.set_maximum_buffer_size(buffer_size);
        let expected_sample_rate = self.modeler.expected_sample_rate() as u32;
        if expected_sample_rate != 0 && expected_sample_rate != sample_rate {
            log::warn!("NeuralAmpModeler expected sample rate {} does not match provided sample rate {}", self.modeler.expected_sample_rate(), sample_rate);
        }
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        let gain = self.parameters.get("gain").unwrap().value.as_float().unwrap();
        let dry_wet = self.parameters.get("dry_wet").unwrap().value.as_float().unwrap();
        let level = self.parameters.get("level").unwrap().value.as_float().unwrap();

        buffer.iter_mut().for_each(|sample| {
            *sample *= gain;
        });

        self.dry_buffer.clear();
        self.dry_buffer.extend_from_slice(buffer);
        self.modeler.process_buffer(buffer);

        for (i, sample) in buffer.iter_mut().enumerate() {
            let mixed_sample = (*sample * dry_wet) + (self.dry_buffer[i] * (1.0 - dry_wet));
            *sample = mixed_sample * level;
        }
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

    fn ui(&mut self, ui: &mut egui::Ui, _message_buffer: &[String]) -> Option<(String,PedalParameterValue)> {
        let pedal_rect = ui.available_rect_before_wrap();
        ui.add(egui::Image::new(include_image!("images/nam.png")));

        // ew, TODO: make NeuralAmpModeler::get_model_path() return a PathBuf instead of a String
        let selected = PathBuf::from(self.modeler.get_model_path().unwrap_or_default());
        let selected_file_name = selected.file_name().unwrap_or_default().to_string_lossy();
        let mut selected_str = selected.to_string_lossy().to_string();
        let old = selected_str.clone();
        
        let mut to_change = None;

        let combo_box_rect = pedal_rect
            .scale_from_center2(
                Vec2::new(0.9, 0.1)
            ).translate(
                Vec2::new(0.0, -0.08*pedal_rect.height())
            );
        let mut combo_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(combo_box_rect)
        );

        egui::ComboBox::from_id_salt(self.id)
            .selected_text(selected_file_name)
            .width(combo_ui.available_width())
            .wrap_mode(egui::TextWrapMode::Truncate)
            .show_ui(&mut combo_ui, |ui| {
                ui.selectable_value(&mut selected_str, String::new(), "Empty");
                for file in &self.saved_nam_files {
                    let name = file.file_name().unwrap().to_string_lossy();

                    ui.selectable_value(&mut selected_str, file.to_string_lossy().to_string(), &name[..name.len()-4]); // remove the .nam extension
                }
            });

        if let Some(value) = pedal_knob(ui, "", self.parameters.get("gain").unwrap(), Vec2::new(0.05, 0.12), 0.25) {
            to_change = Some(("gain".to_string(), value));
        }
        if let Some(value) = pedal_knob(ui, "", self.parameters.get("dry_wet").unwrap(), Vec2::new(0.375, 0.12), 0.25) {
            to_change = Some(("dry_wet".to_string(), value));
        }
        if let Some(value) = pedal_knob(ui, "", self.parameters.get("level").unwrap(), Vec2::new(0.7, 0.12), 0.25) {
            to_change = Some(("level".to_string(), value));
        }

        let active_param = self.get_parameters().get("active").unwrap().value.as_bool().unwrap();
        if let Some(value) = pedal_switch(ui, active_param, egui::Vec2::new(0.33, 0.72), 0.16) {
            to_change = Some(("active".to_string(), PedalParameterValue::Bool(value)));
        }

        if selected_str != old {
            Some((String::from("model"), PedalParameterValue::String(selected_str)))
        } else {
            if let Some(to_change) = to_change {
                Some(to_change)
            } else {
                None
            }
        }
    }
}
