use std::collections::HashMap;
use std::hash::Hash;
use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use super::PedalTrait;
use super::PedalParameter;
use super::PedalParameterValue;
use super::ui::pedal_knob;

use crate::forward_slash_path;
use crate::pedals::ui::pedal_switch;
use crate::pedals::ParameterUILocation;
use crate::plugin::vst2::{Vst2Instance, VST2_PLUGIN_PATH};
use crate::unique_time_id;

use eframe::egui::RichText;
use eframe::egui::{self, Button, Color32, Layout, UiBuilder, Vec2, include_image};
use serde::ser::SerializeMap;
use serde::{Serialize, Deserialize};
use egui_directory_combobox::{DirectoryComboBox, DirectoryNode};

#[derive(Clone)]
pub struct Vst2 {
    instance: Option<Vst2Instance>,
    // buffer size, sample rate
    config: Option<(usize, u32)>,
    parameters: HashMap<String, PedalParameter>,
    // Map of parameter names to their index in the plugin instance
    param_index_map: HashMap<String, usize>,
    output_buffer: Vec<f32>,
    combobox_widget: DirectoryComboBox,
    midi_min_combobox_widget: DirectoryComboBox,
    midi_max_combobox_widget: DirectoryComboBox,
    folders_state: u32,
    id: u32
}

impl Serialize for Vst2 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let parameters_with_idx: HashMap<String, (Option<usize>, PedalParameter)> = self.parameters.iter().map(|(k, v)| {
            let idx = self.param_index_map.get(k).cloned();
            let mut value = v.clone();

            // If the parameter is "Plugin", store only the relative path if it is in the VST2_PLUGIN_PATH directory
            if k == "Plugin" {
                if let PedalParameterValue::String(path) = &v.value {
                    if let Some(save_dir) = dunce::canonicalize(Path::new(VST2_PLUGIN_PATH)).ok() {
                        if let Ok(relative_path) = Path::new(path).strip_prefix(&save_dir) {
                            // Convert relative paths to use forward slashes for cross platform compatibility
                            // Not used for absolute path as they are not intended to be portable
                            let relative_path_converted = forward_slash_path(relative_path);
                            value.value = PedalParameterValue::String(relative_path_converted.to_string_lossy().to_string());
                        }
                    }
                }
            }

            (k.clone(), (idx, v.clone()))
        }).collect();

        let mut ser_map = serializer.serialize_map(Some(2))?;
        ser_map.serialize_entry("id", &self.id)?;
        ser_map.serialize_entry("parameters", &parameters_with_idx)?;
        ser_map.end()
    }
}

impl<'a> Deserialize<'a> for Vst2 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        #[derive(Deserialize)]
        struct Vst2Data {
            id: u32,
            parameters_with_idx: HashMap<String, (Option<usize>, PedalParameter)>,
        }
        let helper = Vst2Data::deserialize(deserializer)?;

        let parameters_with_idx = helper.parameters_with_idx;
        let mut path = parameters_with_idx.get("Plugin").unwrap().1.value.as_str().unwrap().to_string();
        // If the path is relative, make it absolute using the VST2_PLUGIN_PATH directory
        if !path.is_empty() && Path::new(&path).is_relative() {
            if let Some(save_dir) = dunce::canonicalize(Path::new(VST2_PLUGIN_PATH)).ok() {
                path = save_dir.join(path).to_string_lossy().to_string();
            }
        }

        let dry_wet = parameters_with_idx.get("Dry/Wet").unwrap().1.value.as_float().unwrap_or(1.0);
        let active = parameters_with_idx.get("Active").unwrap().1.value.as_bool().unwrap_or(true);

        let mut parameters = HashMap::new();
        parameters.insert(String::from("Plugin"), PedalParameter {
            value: PedalParameterValue::String(path.clone()),
            min: None,
            max: None,
            step: None
        });
        parameters.insert(String::from("Dry/Wet"), PedalParameter {
            value: PedalParameterValue::Float(dry_wet),
            min: Some(PedalParameterValue::Float(0.0)),
            max: Some(PedalParameterValue::Float(1.0)),
            step: None
        });
        parameters.insert(String::from("Active"), PedalParameter {
            value: PedalParameterValue::Bool(active),
            min: None,
            max: None,
            step: None
        });

        let mut param_index_map = HashMap::new();
        if !path.is_empty() {
            for (key, (idx, param)) in parameters_with_idx {
                if let Some(index) = idx {
                    // If index is Some(..), then the parameter is a plugin parameter
                    parameters.insert(key.clone(), param);
                    param_index_map.insert(key, index);
                }
            }
        }

        let id = helper.id;

        let mut vst_pedal = Vst2 {
            instance: None,
            config: None,
            parameters,
            param_index_map,
            output_buffer: Vec::new(),
            combobox_widget: Self::get_empty_directory_combo_box(id),
            midi_min_combobox_widget: Self::get_empty_directory_combo_box(egui::Id::new(id).with("midi_min")),
            midi_max_combobox_widget: Self::get_empty_directory_combo_box(egui::Id::new(id).with("midi_max")),
            folders_state: 0,
            id
        };
        if path.is_empty() {
            Ok(vst_pedal)
        } else {
            let path = PathBuf::from(path);
            if !path.exists() {
                tracing::error!("Plugin {:?} not found", &path);
                vst_pedal.sync_instance_to_parameters();
                return Ok(vst_pedal);
            }

            vst_pedal.set_plugin(path);

            // Set the saved parameters on the new instance
            vst_pedal.sync_parameters_to_instance();

            Ok(vst_pedal)
        }        
    }
}


impl Hash for Vst2 {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Vst2 {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();
        parameters.insert(
            "Plugin".to_string(),
            PedalParameter {
                value: PedalParameterValue::String("".to_string()),
                min: None,
                max: None,
                step: None
            },
        );
        parameters.insert(
            "Dry/Wet".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None
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

        let id = unique_time_id();
        Vst2 {
            instance: None,
            config: None,
            parameters,
            param_index_map: HashMap::new(),
            output_buffer: Vec::new(),
            combobox_widget: Self::get_empty_directory_combo_box(id),
            midi_min_combobox_widget: Self::get_empty_directory_combo_box(egui::Id::new(id).with("midi_min")),
            midi_max_combobox_widget: Self::get_empty_directory_combo_box(egui::Id::new(id).with("midi_max")),
            folders_state: 0,
            id
        }
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = unique_time_id();
        cloned
    }

    fn get_empty_directory_combo_box(id: impl std::hash::Hash) -> DirectoryComboBox {
        let roots = DirectoryNode::try_from_path(VST2_PLUGIN_PATH)
            .map(|node| vec![node])
            .unwrap_or_else(|| {
                tracing::warn!("Failed to get default VST2 save directory: {}", VST2_PLUGIN_PATH);
                vec![]
            });

        DirectoryComboBox::new_from_nodes(roots)
            .with_id(egui::Id::new("vst2_combobox").with(id))
            .with_wrap_mode(egui::TextWrapMode::Truncate)
            .show_extensions(false)
            .select_files_only(true)
            .with_filter(Arc::new(|path: &std::path::Path| {
                if path.is_dir() {
                    true
                } else if let Some(ext) = path.extension() {
                    ext == "vst"
                        || (cfg!(target_os = "windows") && ext == "dll")
                        || (cfg!(target_os = "linux") && ext == "so")
                        || (cfg!(target_os = "macos") && ext == "dll")
                } else {
                    false
                }
            }))
    }

    /// Update the pedal's parameters to the parameters of the current plugin instance
    pub fn sync_instance_to_parameters(&mut self) {
        self.parameters.retain(|k, _| k == "Dry/Wet" || k == "Active");
        
        if let Some(instance) = self.instance.as_mut() {
            match instance.dll_path().to_str() {
                Some(path) => {
                    self.parameters.insert(
                        "Plugin".to_string(),
                        PedalParameter {
                            value: PedalParameterValue::String(path.to_string()),
                            min: None,
                            max: None,
                            step: None
                        },
                    );
                },
                None => {
                    tracing::warn!("Plugin path is not valid unicode, removing plugin instance.");
                    self.parameters.insert(
                        "Plugin".to_string(),
                        PedalParameter {
                            value: PedalParameterValue::String("".to_string()),
                            min: None,
                            max: None,
                            step: None
                        },
                    );
                    self.instance = None;
                    self.param_index_map.clear();
                    return;
                }
            }

            for i in 0..instance.parameter_count() {
                let name = instance.parameter_name(i);
                let value = instance.parameter_value(i);
                self.parameters.insert(
                    name.clone(),
                    PedalParameter {
                        value: PedalParameterValue::Float(value),
                        min: Some(PedalParameterValue::Float(0.0)),
                        max: Some(PedalParameterValue::Float(1.0)),
                        step: None
                    }
                );

                self.param_index_map.insert(name, i);
            }
        } else {
            self.parameters.insert(
                "Plugin".to_string(),
                PedalParameter {
                    value: PedalParameterValue::String("".to_string()),
                    min: None,
                    max: None,
                    step: None
                },
            );
            self.param_index_map.clear();
        }
    }

    /// Set the instance's parameters to the values stored in `self.parameters`.
    pub fn sync_parameters_to_instance(&mut self) {
        if let Some(instance) = self.instance.as_mut() {
            for (name, param) in &self.parameters {
                if name == "Plugin" || name == "Dry/Wet" || name == "Active" {
                    continue;
                }
                if let Some(&index) = self.param_index_map.get(name) {
                    instance.set_parameter_value(index, param.value.as_float().unwrap());
                } else {
                    tracing::warn!("Parameter {} not found in plugin instance", name);
                }
            }
        } else {
            tracing::warn!("No plugin instance available to synchronise parameters");
        }
    }

    pub fn set_plugin<P: AsRef<Path>>(&mut self, plugin_path: P) {
        let canon_path = match dunce::canonicalize(plugin_path.as_ref()) {
            Ok(p) => p,
            Err(e) => {
                tracing::error!("Failed to canonicalize plugin path {:?}: {}", plugin_path.as_ref(), e);
                self.instance = None;
                self.sync_instance_to_parameters();
                return;
            }
        };

        let new_plugin_instance = Vst2Instance::load(canon_path);
        match new_plugin_instance {
            Ok(mut instance) => {
                if let Some((bs, sr)) = self.config {
                    instance.set_config(bs, sr);
                }
                self.instance = Some(instance);
                self.sync_instance_to_parameters();
                self.combobox_widget.set_selection(Some(plugin_path.as_ref()));
            },
            Err(_) => {
                tracing::error!("Failed to load plugin: {}", plugin_path.as_ref().display());
                self.instance = None;
                self.sync_instance_to_parameters();
            }
        }
    }

    /// Update the main pedal value, and midi min and max combobox widgets if the root directories have changed
    fn update_combobox_nodes(&mut self, ui: &mut egui::Ui) {
        // Refresh the list of root directories if it has changed
        let new_root_directories: Option<Vec<egui_directory_combobox::DirectoryNode>> = ui.ctx().memory_mut(|m| {
            let state = m.data.get_temp_mut_or("vst2_folders_state".into(), 1u32);
            if *state != self.folders_state {
                self.folders_state = *state;
                m.data.get_temp("vst2_folders".into()).as_ref().cloned()
            } else {
                None
            }
        });

        if let Some(mut roots) = new_root_directories {
            if let Some(node) = DirectoryNode::try_from_path(VST2_PLUGIN_PATH) {
                roots.push(node);
            } else {
                tracing::warn!("Failed to get default VST2 save directory: {}", VST2_PLUGIN_PATH);
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

    fn show_vst_combobox(&mut self, ui: &mut egui::Ui, parameter: Option<&PedalParameter>, location: ParameterUILocation) -> egui::InnerResponse<Option<PedalParameterValue>> {
        self.update_combobox_nodes(ui);

        let combobox_to_show = match location {
            ParameterUILocation::Pedal => &mut self.combobox_widget,
            ParameterUILocation::ParameterWindow => &mut self.combobox_widget,
            ParameterUILocation::MidiMin => &mut self.midi_min_combobox_widget,
            ParameterUILocation::MidiMax => &mut self.midi_max_combobox_widget
        };

        if let Some(param) = parameter {
            if let PedalParameterValue::String(vst) = &param.value {
                if vst.is_empty() {
                    combobox_to_show.set_selection::<&str>(None);
                } else {
                    combobox_to_show.set_selection(Some(vst));
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
                            tracing::warn!("Selected VST2 path is not valid unicode");
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

impl PedalTrait for Vst2 {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn set_config(&mut self, buffer_size: usize, sample_rate: u32) {
        if let Some(instance) = self.instance.as_mut() {
            instance.set_config(buffer_size, sample_rate);
        }

        self.output_buffer.resize(buffer_size, 0.0);
        self.config = Some((buffer_size, sample_rate));
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        // Config will be set on the processor. If it is not set, we cannot process audio.
        match self.config {
            Some((b, _)) => assert!(buffer.len() <= b, "Buffer size exceeds configured max buffer size"),
            None => return
        }

        let dry_wet = self.parameters.get("Dry/Wet").unwrap().value.as_float().unwrap();

        if let Some(instance) = self.instance.as_mut() {
            instance.process(buffer, &mut self.output_buffer[..buffer.len()]);

            // Mix using dry/wet
            for (output_sample, processed_sample) in buffer.iter_mut().zip(self.output_buffer.iter()) {
                *output_sample = *output_sample * (1.0 - dry_wet) + processed_sample * dry_wet;
            }
        }
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    fn set_parameter_value(&mut self, name: &str, value: PedalParameterValue) {
        if name == "Plugin" {
            if let PedalParameterValue::String(plugin_path) = value {
                if plugin_path.is_empty() {
                    self.instance = None;
                    self.combobox_widget.set_selection::<&str>(None);
                    self.sync_instance_to_parameters();
                    return;
                } else {
                    self.set_plugin(plugin_path);
                }
            }
            return;
        }

        if let Some(parameter) = self.parameters.get_mut(name) {
            if parameter.is_valid(&value) {
                parameter.value = value;

                // If the parameter is a parameter on the plugin, then set it on the plugin instance
                if let Some(&index) = self.param_index_map.get(name) {
                    if let Some(instance) = self.instance.as_mut() {
                        instance.set_parameter_value(index, parameter.value.as_float().unwrap());
                    }
                }
            }
        }
    }

    fn get_string_values(&self,_parameter_name: &str) -> Option<Vec<String>> {
        Some(self.combobox_widget.get_all_paths().iter().map(|p| p.to_string_lossy().to_string()).collect())
    }

    fn parameter_editor_ui(&mut self, ui: &mut egui::Ui, name: &str, parameter: &PedalParameter, location: ParameterUILocation) -> egui::InnerResponse<Option<PedalParameterValue>> {
        if name == "Plugin" {
            ui.spacing_mut().combo_width = ui.available_width();
            
            self.show_vst_combobox(ui, Some(parameter), location)
        } else {
            parameter.parameter_editor_ui(ui)
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _message_buffer: &[String]) -> Option<(String, PedalParameterValue)> {
        let mut plugin_param_change = None;
        if let Some(i) = self.instance.as_mut() {
            plugin_param_change = i.ui_frame(ui);
        }

        let mut to_change = None;

        let mut img_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(ui.available_rect_before_wrap())
        );

        img_ui.add(egui::Image::new(include_image!("images/pedal_gradient.png")).tint(Color32::from_rgb(18, 105, 50)));

        ui.allocate_ui_with_layout(
            ui.available_size() * Vec2::new(0.9, 1.0),
            Layout::top_down(egui::Align::Center),

            |ui| {
                ui.add_space(31.0);
                
                ui.label(egui::RichText::new("VST2").size(23.0));

                ui.add_space(5.0);

                ui.spacing_mut().combo_width = ui.available_width();
                if let Some(value) = self.show_vst_combobox(ui, None, ParameterUILocation::Pedal).inner {
                    to_change = Some(("Plugin".to_string(), value));
                }

                ui.add_space(5.0);

                if ui.add_enabled(
                    self.instance.as_ref().map(|i| !i.ui_open).unwrap_or(false),
                    Button::new(RichText::new("Parameters").size(14.0))
                ).clicked() {
                    if let Some(instance) = self.instance.as_mut() {
                        if instance.ui_open {
                            instance.close_ui();
                        } else {
                            instance.open_ui();
                        }
                    }
                }

                ui.add_space(5.0);
                    
                if let Some(value) = pedal_knob(ui, RichText::new("Dry/Wet").color(Color32::WHITE).size(8.0), "Dry/Wet", self.parameters.get("Dry/Wet").unwrap(), Vec2::new(0.325, 0.55), 0.35, self.id) {
                    to_change = Some(("Dry/Wet".to_string(), value));
                }
            }
        );

        let active_param = self.get_parameters().get("Active").unwrap().value.as_bool().unwrap();
        if let Some(value) = pedal_switch(ui, active_param, egui::Vec2::new(0.33, 0.72), 0.16) {
            to_change = Some(("Active".to_string(), PedalParameterValue::Bool(value)));
        }

        if plugin_param_change.is_some() {
            return plugin_param_change;
        }

        to_change
    }
}
