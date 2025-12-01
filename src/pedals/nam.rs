use std::{path::PathBuf, vec};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use neural_amp_modeler::NeuralAmpModeler;
use serde::{ser::SerializeMap, Deserialize, Serialize};
use eframe::egui::{self, include_image, Vec2};
use egui_directory_combobox::{DirectoryComboBox, DirectoryNode};

use super::{ui::pedal_knob, PedalParameter, PedalParameterValue, PedalTrait};
use crate::pedals::ui::{pedal_switch, sideways_arrow};
use crate::pedals::ParameterUILocation;
use crate::{forward_slash_path, unique_time_id, SAVE_DIR};

const NAM_SAVE_PATH: &str = r"NAM";

pub struct Nam {
    modeler: NeuralAmpModeler,
    parameters: HashMap<String, PedalParameter>,

    dry_buffer: Vec<f32>,

    combobox_widget: DirectoryComboBox,
    midi_min_combobox_widget: DirectoryComboBox,
    midi_max_combobox_widget: DirectoryComboBox,
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
            midi_min_combobox_widget: self.midi_min_combobox_widget.clone(),
            midi_max_combobox_widget: self.midi_max_combobox_widget.clone(),
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
        let mut parameters = self.parameters.clone();
        // If the model path is in the pedalboard NAM directory, store it as a relative path
        if let Some(model_path) = self.parameters.get("Model").and_then(|p| p.value.as_str()).map(PathBuf::from) {
            if let Some(save_dir) = Self::get_save_directory() {
                if let Ok(relative_path) = model_path.strip_prefix(&save_dir) {
                    // Convert relative paths to use forward slashes for cross platform compatibility
                    // Not used for absolute path as they are not intended to be portable
                    let relative_path_converted = forward_slash_path(relative_path);
                    parameters.get_mut("Model").unwrap().value = PedalParameterValue::String(relative_path_converted.to_string_lossy().to_string());
                }
            }
        }
        ser_map.serialize_entry("parameters", &parameters)?;
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

        // If model is a relative path, make it absolute based on the save directory
        let model_path = PathBuf::from(model);
        let model = if model_path.is_relative() {
            if let Some(save_dir) = Self::get_save_directory() {
                save_dir.join(model_path)
            } else {
                tracing::warn!("Failed to get save directory, removing relative model path");
                PathBuf::new()
            }
        } else {
            model_path
        };

        // Default buffer size, can be changed later with `set_config`
        let modeler = NeuralAmpModeler::new_with_maximum_buffer_size(512).expect("Failed to create neural amp modeler");

        let mut pedal = Nam {
            modeler,
            parameters: parameters.clone(),
            dry_buffer: vec![0.0; 512],
            folders_state: 0,
            combobox_widget: Self::get_empty_directory_combo_box(helper.id),
            midi_min_combobox_widget: Self::get_empty_directory_combo_box(egui::Id::new(helper.id).with("midi_min")),
            midi_max_combobox_widget: Self::get_empty_directory_combo_box(egui::Id::new(helper.id).with("midi_max")),
            id: helper.id
        };

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
            midi_min_combobox_widget: Self::get_empty_directory_combo_box(egui::Id::new(id).with("midi_min")),
            midi_max_combobox_widget: Self::get_empty_directory_combo_box(egui::Id::new(id).with("midi_max")),
            id
        }
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = unique_time_id();
        cloned
    }

    fn get_empty_directory_combo_box(id: impl std::hash::Hash) -> DirectoryComboBox {
        let roots = match Self::get_save_directory() {
            Some(main_save_dir) => vec![DirectoryNode::from_path(&main_save_dir)],
            None => {
                tracing::warn!("Failed to get main save directory");
                vec![]
            }
        };

        DirectoryComboBox::new_from_nodes(roots)
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
        
        let canon_path = match dunce::canonicalize(&model_path) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Failed to canonicalize model path {:?}: {}", model_path, e);
                return;
            }
        };

        let string_path = match canon_path.to_str() {
            Some(s) => s.to_string(),
            None => {
                tracing::warn!("Model path is not valid unicode");
                return;
            }
        };

        if let Err(e) = self.modeler.set_model(model_path) {
            tracing::error!("Failed to set model: {}", e);
        } else {
            self.parameters.get_mut("Model").unwrap().value = PedalParameterValue::String(string_path);

            // Update combobox to match new selection (in case it was not set from the combobox itself)
            let model_path = self.modeler.get_model_path();
            self.combobox_widget.set_selection(model_path);
        }
    }

    pub fn remove_model(&mut self) {
        self.parameters.get_mut("Model").unwrap().value = PedalParameterValue::String("".to_string());
        let buffer_size = self.modeler.get_maximum_buffer_size();
        self.modeler = NeuralAmpModeler::new_with_maximum_buffer_size(buffer_size).expect("Failed to create neural amp modeler");
        self.combobox_widget.set_selection::<&str>(None);
    }

    pub fn get_save_directory() -> Option<PathBuf> {
        Some(dunce::canonicalize(homedir::my_home().ok()??.join(SAVE_DIR).join(NAM_SAVE_PATH)).ok()?)
    }

    /// Update the main pedal value, and midi min and max combobox widgets if the root directories have changed
    fn update_combobox_nodes(&mut self, ui: &mut egui::Ui) {
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
                tracing::warn!("Failed to get main save directory");
            }
            let model_path = self.combobox_widget.selected().and_then(|p| p.to_str().map(|s| s.to_string()));
            self.combobox_widget = Self::get_empty_directory_combo_box(self.id);
            self.combobox_widget.set_selection(model_path);

            let midi_min_path = self.midi_min_combobox_widget.selected().and_then(|p| p.to_str().map(|s| s.to_string()));
            self.midi_min_combobox_widget = Self::get_empty_directory_combo_box(egui::Id::new(self.id).with("midi_min"));
            self.midi_min_combobox_widget.set_selection(midi_min_path);

            let midi_max_path = self.midi_max_combobox_widget.selected().and_then(|p| p.to_str().map(|s| s.to_string()));
            self.midi_max_combobox_widget = Self::get_empty_directory_combo_box(egui::Id::new(self.id).with("midi_max"));
            self.midi_max_combobox_widget.set_selection(midi_max_path);

            // If there is only one root directory, use its children as the roots
            let nodes = if roots.len() == 1 {
                match roots.pop().unwrap() {
                    egui_directory_combobox::DirectoryNode::Directory(_, children) => {
                        children
                    },
                    _ => roots
                }
            } else {
                roots
            };

            self.combobox_widget.roots = nodes.clone();
            self.midi_min_combobox_widget.roots = nodes.clone();
            self.midi_max_combobox_widget.roots = nodes;
        }
    }

    fn show_model_combobox(&mut self, ui: &mut egui::Ui, parameter: Option<&PedalParameter>, location: ParameterUILocation) -> egui::InnerResponse<Option<PedalParameterValue>> {
        self.update_combobox_nodes(ui);

        let combobox_to_show = match location {
            ParameterUILocation::Pedal => &mut self.combobox_widget,
            ParameterUILocation::ParameterWindow => &mut self.combobox_widget,
            ParameterUILocation::MidiMin => &mut self.midi_min_combobox_widget,
            ParameterUILocation::MidiMax => &mut self.midi_max_combobox_widget
        };

        if let Some(param) = parameter {
            if let PedalParameterValue::String(model_path) = &param.value {
                if model_path.is_empty() {
                    combobox_to_show.set_selection::<&str>(None);
                } else {
                    combobox_to_show.set_selection(Some(model_path));
                }
            }
        }

        let old = combobox_to_show.selected().map(|p| p.to_path_buf());
        let response = ui.add_sized(Vec2::new(ui.available_width(), 15.0), &mut *combobox_to_show);

        let mut to_change = None;
        if old.as_ref().map(|p| p.as_path()) != combobox_to_show.selected() {
            match combobox_to_show.selected() {
                Some(path) => {
                    match path.to_str() {
                        Some(s) => {
                            let selected_str = s.to_string();
                            to_change = Some(PedalParameterValue::String(selected_str));
                        },
                        None => {
                            tracing::warn!("Selected model path is not valid unicode");
                        }
                    }
                },
                None => {
                    to_change = Some(PedalParameterValue::String("".to_string()));
                }
            }
        }

        egui::InnerResponse {
            inner: to_change,
            response
        }
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
            tracing::warn!("NeuralAmpModeler expected sample rate {} does not match provided sample rate {}", self.modeler.expected_sample_rate(), sample_rate);
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

    fn reset_buffer(&mut self) {
        self.modeler.reset_and_prewarm_model(self.modeler.expected_sample_rate(), self.modeler.get_maximum_buffer_size());
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    fn set_parameter_value(&mut self,name: &str, value: PedalParameterValue) {
        if !self.parameters.get(name).unwrap().is_valid(&value) {
            tracing::warn!("Attempted to set invalid value for parameter {}: {:?}", name, value);
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
            tracing::error!("Parameter {} not found", name);
        }
    }

    fn get_string_values(&self,_parameter_name: &str) -> Option<Vec<String>> {
        Some(self.combobox_widget.get_all_paths().iter().map(|p| p.to_string_lossy().to_string()).collect())
    }

    fn parameter_editor_ui(&mut self, ui: &mut egui::Ui, name: &str, parameter: &PedalParameter, location: ParameterUILocation) -> egui::InnerResponse<Option<PedalParameterValue>> {
        if name == "Model" {
            ui.spacing_mut().combo_width = ui.available_width();
            
            self.show_model_combobox(ui, Some(parameter), location)
        } else {
            parameter.parameter_editor_ui(ui)
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _message_buffer: &[String]) -> Option<(String,PedalParameterValue)> {
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

        combo_ui.spacing_mut().combo_width = combo_ui.available_width();

        if let Some(new_model_value) = self.show_model_combobox(&mut combo_ui, None, ParameterUILocation::Pedal).inner {
            to_change = Some(("Model".to_string(), new_model_value));
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
                    tracing::warn!("Selected model path is not valid unicode");
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
                    tracing::warn!("Selected model path is not valid unicode");
                }
            }
        };

        if let Some(value) = pedal_knob(ui, "", "Gain", self.parameters.get("Gain").unwrap(), Vec2::new(0.05, 0.06), 0.25, self.id) {
            to_change = Some(("Gain".to_string(), value));
        }
        if let Some(value) = pedal_knob(ui, "", "Dry/Wet", self.parameters.get("Dry/Wet").unwrap(), Vec2::new(0.375, 0.06), 0.25, self.id) {
            to_change = Some(("Dry/Wet".to_string(), value));
        }
        if let Some(value) = pedal_knob(ui, "", "Level", self.parameters.get("Level").unwrap(), Vec2::new(0.7, 0.06), 0.25, self.id) {
            to_change = Some(("Level".to_string(), value));
        }


        let active_param = self.get_parameters().get("Active").unwrap().value.as_bool().unwrap();
        if let Some(value) = pedal_switch(ui, active_param, egui::Vec2::new(0.33, 0.72), 0.16) {
            to_change = Some(("Active".to_string(), PedalParameterValue::Bool(value)));
        }

        to_change
    }
}
