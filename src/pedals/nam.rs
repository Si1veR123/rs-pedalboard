use std::{path::PathBuf, vec};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use neural_amp_modeler::NeuralAmpModeler;
use serde::{ser::SerializeMap, Deserialize, Serialize};
use eframe::egui::{self, include_image, Vec2};
use egui_directory_combobox::DirectoryComboBox;

use super::{ui::pedal_knob, PedalParameter, PedalParameterValue, PedalTrait};
use crate::pedals::ui::{pedal_switch, sideways_arrow};
use crate::{unique_time_id, SAVE_DIR};

const NAM_SAVE_PATH: &str = r"NAM";

pub struct Nam {
    modeler: NeuralAmpModeler,
    parameters: HashMap<String, PedalParameter>,

    dry_buffer: Vec<f32>,

    combobox_widget: DirectoryComboBox,
    folders_state: u32, // Used to track changes in the root directories settings
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
            combobox_widget: self.combobox_widget.clone(),
            folders_state: self.folders_state,
            id: self.id
        };

        if let Some(model_path) = self.modeler.get_model_path() {
            new_nam.set_model(model_path.to_path_buf());
        }
        new_nam
    }
}

impl Hash for Nam {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Serialize for Nam {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser_map = serializer.serialize_map(Some(2))?;
        ser_map.serialize_entry("id", &self.id)?;
        ser_map.serialize_entry("parameters", &self.parameters)?;
        ser_map.end()
    }
}

impl<'a> Deserialize<'a> for Nam {
    /// `set_config` must be called to set the buffer size if it is greater than the default maximum size
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        #[derive(Deserialize)]
        struct NamData {
            id: u32,
            parameters: HashMap<String, PedalParameter>,
        }
        let helper = NamData::deserialize(deserializer)?;

        let parameters = helper.parameters;
        let model = parameters.get("Model").unwrap().value.as_str().unwrap();
        // Default buffer size, can be changed later with `set_config`
        let modeler = NeuralAmpModeler::new_with_maximum_buffer_size(512).expect("Failed to create neural amp modeler");

        let mut pedal = Nam {
            modeler,
            parameters: parameters.clone(),
            dry_buffer: vec![0.0; 512],
            folders_state: 0,
            combobox_widget: Self::get_empty_directory_combo_box(helper.id),
            id: helper.id
        };

        match PathBuf::from(model).canonicalize() {
            Ok(model) => {
                pedal.set_model(model);
            },
            Err(e) => log::warn!("Failed to set model path ({model}) during deserialization: {e}"),
        };
        
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

        parameters.insert(
            "Model".to_string(),
            PedalParameter {
                value: PedalParameterValue::String("".to_string()),
                min: None,
                max: None,
                step: None,
            },
        );

        parameters.insert(
            "Gain".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.00)),
                max: Some(PedalParameterValue::Float(3.0)),
                step: Some(PedalParameterValue::Float(0.05)),
            },
        );

        parameters.insert(
            "Dry/Wet".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: Some(PedalParameterValue::Float(0.01)),
            },
        );

        parameters.insert(
            "Level".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(3.0)),
                step: Some(PedalParameterValue::Float(0.05)),
            },
        );

        parameters.insert(
            "Active".to_string(),
            PedalParameter {
                value: PedalParameterValue::Bool(true),
                min: None,
                max: None,
                step: None,
            },
        );

        let modeler = NeuralAmpModeler::new_with_maximum_buffer_size(buffer_size).expect("Failed to create neural amp modeler");

        let id = unique_time_id();
        Nam {
            modeler,
            parameters: parameters.clone(),
            dry_buffer: vec![0.0; buffer_size],
            folders_state: 0,
            combobox_widget: Self::get_empty_directory_combo_box(id),
            id
        }
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = unique_time_id();
        cloned
    }

    fn get_empty_directory_combo_box(id: impl std::hash::Hash) -> DirectoryComboBox {
        DirectoryComboBox::new_from_nodes(vec![])
            .with_id(egui::Id::new("nam_combobox").with(id))
            .with_wrap_mode(egui::TextWrapMode::Truncate)
            .show_extensions(false)
            .select_files_only(true)
            .with_filter(Arc::new(|path: &std::path::Path| {
                if path.is_dir() {
                    true
                } else if let Some(ext) = path.extension() {
                    ext == "nam"
                } else {
                    false
                }
            }))
    }


    pub fn set_model(&mut self, model_path: PathBuf) {
        if model_path.as_os_str().is_empty() {
            self.remove_model();
            return;
        }
        
        let string_path = match model_path.to_str() {
            Some(s) => s.to_string(),
            None => {
                log::warn!("Model path is not valid unicode");
                return;
            }
        };

        if let Err(e) = self.modeler.set_model(model_path) {
            log::error!("Failed to set model: {}", e);
        } else {
            self.parameters.get_mut("Model").unwrap().value = PedalParameterValue::String(string_path);
        }
    }

    pub fn remove_model(&mut self) {
        self.parameters.get_mut("Model").unwrap().value = PedalParameterValue::String("".to_string());
        let buffer_size = self.modeler.get_maximum_buffer_size();
        self.modeler = NeuralAmpModeler::new_with_maximum_buffer_size(buffer_size).expect("Failed to create neural amp modeler");
    }

    pub fn get_save_directory() -> Option<PathBuf> {
        Some(homedir::my_home().ok()??.join(SAVE_DIR).join(NAM_SAVE_PATH))
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
        self.dry_buffer.resize(buffer_size, 0.0);
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        let gain = self.parameters.get("Gain").unwrap().value.as_float().unwrap();
        let dry_wet = self.parameters.get("Dry/Wet").unwrap().value.as_float().unwrap();
        let level = self.parameters.get("Level").unwrap().value.as_float().unwrap();

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

    fn set_parameter_value(&mut self,name: &str, value: PedalParameterValue) {
        if !self.parameters.get(name).unwrap().is_valid(&value) {
            log::warn!("Attempted to set invalid value for parameter {}: {:?}", name, value);
            return;
        }

        if let Some(param) = self.parameters.get_mut(name) {
            if name == "Model" {
                let value_str = value.as_str().unwrap();

                if value_str.is_empty() {
                    self.remove_model();
                } else {
                    self.set_model(PathBuf::from(value_str));
                }
            } else {
                param.value = value;
            }
        } else {
            log::error!("Parameter {} not found", name);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _message_buffer: &[String]) -> Option<(String,PedalParameterValue)> {
        // Refresh the list of root directories if it has changed
        let new_root_directories: Option<Vec<egui_directory_combobox::DirectoryNode>> = ui.ctx().memory_mut(|m| {
            let state = m.data.get_temp_mut_or("nam_folders_state".into(), 1u32);
            if *state != self.folders_state {
                self.folders_state = *state;
                m.data.get_temp("nam_folders".into()).as_ref().cloned()
            } else {
                None
            }
        });

        if let Some(mut roots) = new_root_directories {
            if let Some(main_save_dir) = Self::get_save_directory() {
                roots.push(egui_directory_combobox::DirectoryNode::from_path(&main_save_dir));
            } else {
                log::warn!("Failed to get main save directory");
            }
            let model_path = self.parameters.get("Model").unwrap().value.as_str().unwrap();
            self.combobox_widget = Self::get_empty_directory_combo_box(self.id);
            self.combobox_widget.set_selection(match model_path {
                s if s.is_empty() => None,
                s => Some(s)
            });

            // If there is only one root directory, use its children as the roots
            if roots.len() == 1 {
                match roots.pop().unwrap() {
                    egui_directory_combobox::DirectoryNode::Directory(_, children) => {
                        self.combobox_widget.roots = children;
                    },
                    _ => self.combobox_widget.roots = roots
                }
            } else {
                self.combobox_widget.roots = roots;
            }
        }

        let pedal_rect = ui.available_rect_before_wrap();
        ui.add(egui::Image::new(include_image!("images/nam.png")));

        let combo_box_rect = pedal_rect
            .scale_from_center2(
                Vec2::new(0.9, 0.1)
            ).translate(
                Vec2::new(0.0, -0.15*pedal_rect.height())
            );

        let mut combo_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(combo_box_rect)
        );

        let mut to_change = None;

        let old = self.combobox_widget.selected().map(|p| p.to_path_buf());
        combo_ui.spacing_mut().combo_width = combo_ui.available_width();
        combo_ui.add_sized(Vec2::new(combo_ui.available_width(), 15.0), &mut self.combobox_widget);
        if old.as_ref().map(|p| p.as_path()) != self.combobox_widget.selected() {
            match self.combobox_widget.selected() {
                Some(path) => {
                    match path.to_str() {
                        Some(s) => {
                            let selected_str = s.to_string();
                            to_change = Some((String::from("Model"), PedalParameterValue::String(selected_str)));
                        },
                        None => {
                            log::warn!("Selected model path is not valid unicode");
                        }
                    }
                },
                None => {
                    to_change = Some((String::from("Model"), PedalParameterValue::String("".to_string())));
                }
            }
        }

        let button_rect = combo_box_rect.translate(Vec2::new(0.0, combo_box_rect.height() + 0.02*pedal_rect.height()));
        let mut button_ui = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(button_rect)
                .layout(egui::Layout::left_to_right(egui::Align::Min))
        );
        let button_size = Vec2::new(button_rect.width()*0.47, button_rect.height());
        let left_button_response = button_ui.add_sized(
            button_size,
            egui::Button::new("")
        );
        
        sideways_arrow(ui, left_button_response.rect, true);

        if left_button_response.clicked() {
            self.combobox_widget.select_previous_file();
            if let Some(path) = self.combobox_widget.selected() {
                if let Some(s) = path.to_str() {
                    to_change = Some((String::from("Model"), PedalParameterValue::String(s.to_string())));
                } else {
                    log::warn!("Selected model path is not valid unicode");
                }
            }
        };
        button_ui.add_space(button_rect.width()*0.06);
        let right_button_response = button_ui.add_sized(
            button_size,
            egui::Button::new("")
        );
        
        sideways_arrow(ui, right_button_response.rect, false);

        if right_button_response.clicked() {
            self.combobox_widget.select_next_file();
            if let Some(path) = self.combobox_widget.selected() {
                if let Some(s) = path.to_str() {
                    to_change = Some((String::from("Model"), PedalParameterValue::String(s.to_string())));
                } else {
                    log::warn!("Selected model path is not valid unicode");
                }
            }
        };

        if let Some(value) = pedal_knob(ui, "", self.parameters.get("Gain").unwrap(), Vec2::new(0.05, 0.06), 0.25) {
            to_change = Some(("Gain".to_string(), value));
        }
        if let Some(value) = pedal_knob(ui, "", self.parameters.get("Dry/Wet").unwrap(), Vec2::new(0.375, 0.06), 0.25) {
            to_change = Some(("Dry/Wet".to_string(), value));
        }
        if let Some(value) = pedal_knob(ui, "", self.parameters.get("Level").unwrap(), Vec2::new(0.7, 0.06), 0.25) {
            to_change = Some(("Level".to_string(), value));
        }

        let active_param = self.get_parameters().get("Active").unwrap().value.as_bool().unwrap();
        if let Some(value) = pedal_switch(ui, active_param, egui::Vec2::new(0.33, 0.72), 0.16) {
            to_change = Some(("Active".to_string(), PedalParameterValue::Bool(value)));
        }

        to_change
    }
}
