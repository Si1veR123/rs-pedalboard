use std::collections::HashMap;
use std::hash::Hash;

use super::PedalTrait;
use super::PedalParameter;
use super::PedalParameterValue;
use super::ui::pedal_knob;

use crate::plugin::vst2::{Vst2Instance, path_from_name, available_plugins};
use crate::plugin::PluginHost;
use crate::unique_time_id;

use eframe::egui::RichText;
use eframe::egui::{self, Button, Color32, Layout, UiBuilder, Vec2, include_image};
use serde::ser::SerializeMap;
use serde::{Serialize, Deserialize};

#[derive(Clone)]
pub struct Vst2 {
    instance: Option<Vst2Instance>,
    // buffer size, sample rate
    config: Option<(usize, usize)>,
    parameters: HashMap<String, PedalParameter>,
    // Map of parameter names to their index in the plugin instance
    param_index_map: HashMap<String, usize>,
    output_buffer: Vec<f32>,
    available_plugins: Vec<String>,
    id: usize
}

impl Serialize for Vst2 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser_map = serializer.serialize_map(Some(self.parameters.len()))?;
        for (key, value) in &self.parameters {
            let idx = self.param_index_map.get(key).cloned();
            ser_map.serialize_entry(key, &(idx, value))?;
        }
        ser_map.end()
    }
}

impl<'a> Deserialize<'a> for Vst2 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        let parameters_with_idx: HashMap<String, (Option<usize>, PedalParameter)> = HashMap::deserialize(deserializer)?;
        let name = parameters_with_idx.get("plugin").unwrap().1.value.as_str().unwrap().to_string();
        let dry_wet = parameters_with_idx.get("dry_wet").unwrap().1.value.as_float().unwrap_or(1.0);

        let mut parameters = HashMap::new();
        parameters.insert(String::from("plugin"), PedalParameter {
            value: PedalParameterValue::String(name.clone()),
            min: None,
            max: None,
            step: None
        });
        parameters.insert(String::from("dry_wet"), PedalParameter {
            value: PedalParameterValue::Float(dry_wet),
            min: Some(PedalParameterValue::Float(0.0)),
            max: Some(PedalParameterValue::Float(1.0)),
            step: None
        });

        let mut param_index_map = HashMap::new();
        if !name.is_empty() {
            for (key, (idx, param)) in parameters_with_idx {
                if let Some(index) = idx {
                    // If index is Some(..), then the parameter is a plugin parameter
                    parameters.insert(key.clone(), param);
                    param_index_map.insert(key, index);
                }
            }
        }

        let available_plugins = available_plugins();
        let mut empty_vst = Vst2 {
            instance: None,
            config: None,
            parameters,
            param_index_map,
            output_buffer: Vec::new(),
            available_plugins,
            id: unique_time_id()
        };
        if name.is_empty() {
            Ok(empty_vst)
        } else {
            let path = path_from_name(&name);
            if path.is_none() {
                log::error!("Plugin {} not found", name);
                empty_vst.sync_instance_to_parameters();
                return Ok(empty_vst);
            }

            let new_plugin_instance = Vst2Instance::load(path.unwrap());
            let mut deserialized_vst = match new_plugin_instance {
                Ok(instance) => Ok(Vst2 {
                    instance: Some(instance),
                    ..empty_vst
                }),
                Err(_) => {
                    log::error!("Failed to load plugin: {}", name);
                    empty_vst.sync_instance_to_parameters();
                    Ok(empty_vst)
                }
            }?;

            // Set the saved parameters on the new instance
            deserialized_vst.sync_parameters_to_instance();

            Ok(deserialized_vst)
        }        
    }
}

impl Hash for Vst2 {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.parameters.values().map(|p| &p.value).for_each(|v| v.hash(state));
    }
}

impl Vst2 {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();
        parameters.insert(
            "plugin".to_string(),
            PedalParameter {
                value: PedalParameterValue::String("".to_string()),
                min: None,
                max: None,
                step: None
            },
        );
        parameters.insert(
            "dry_wet".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(1.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(1.0)),
                step: None
            },
        );

        Vst2 {
            instance: None,
            config: None,
            parameters,
            param_index_map: HashMap::new(),
            output_buffer: Vec::new(),
            available_plugins: available_plugins(),
            id: unique_time_id()
        }
    }

    /// Update the pedal's parameters to the parameters of the current plugin instance
    pub fn sync_instance_to_parameters(&mut self) {
        if let Some(instance) = self.instance.as_mut() {
            self.parameters.retain(|k, _| k == "dry_wet");

            let plugin_file_name = instance.dll_path().file_name().expect("Instance dll path should have file name").to_string_lossy().to_string();
            self.parameters.insert(
                "plugin".to_string(),
                PedalParameter {
                    value: PedalParameterValue::String(plugin_file_name),
                    min: None,
                    max: None,
                    step: None
                },
            );

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
            self.parameters.retain(|k, _| k == "dry_wet");
            self.parameters.insert(
                "plugin".to_string(),
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
                if name == "plugin" || name == "dry_wet" {
                    continue; // Skip the plugin name parameter
                }
                if let Some(&index) = self.param_index_map.get(name) {
                    instance.set_parameter_value(index, param.value.as_float().unwrap());
                } else {
                    log::warn!("Parameter {} not found in plugin instance", name);
                }
            }
        } else {
            log::warn!("No plugin instance available to synchronise parameters");
        }
    }

    /// `plugin_name` should be the file name of the plugin including '.dll' extension
    pub fn set_plugin(&mut self, plugin_name: &str) {
        if plugin_name.is_empty() {
            self.instance = None;
            self.sync_instance_to_parameters();
            return;
        }

        let path = path_from_name(plugin_name);
        if path.is_none() {
            log::error!("Plugin {} not found", plugin_name);
            return;
        }
        
        let new_plugin_instance = Vst2Instance::load(path.unwrap());
        match new_plugin_instance {
            Ok(mut instance) => {
                if let Some((bs, sr)) = self.config {
                    instance.set_config(bs, sr);
                }
                self.instance = Some(instance);
                self.sync_instance_to_parameters();
            },
            Err(_) => {
                log::error!("Failed to load plugin: {}", plugin_name);
                self.instance = None;
                self.sync_instance_to_parameters();
            }
        }
    }
}

impl PedalTrait for Vst2 {
    fn set_config(&mut self, buffer_size: usize, sample_rate: usize) {
        if let Some(instance) = self.instance.as_mut() {
            instance.set_config(buffer_size, sample_rate);
        }

        self.output_buffer.resize(buffer_size, 0.0);
        self.config = Some((buffer_size, sample_rate));
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        // Config will be set on the server. If it is not set, we cannot process audio.
        match self.config {
            Some((b, _)) => assert!(buffer.len() <= b, "Buffer size exceeds configured max buffer size"),
            None => return
        }

        if let Some(instance) = self.instance.as_mut() {
            instance.process(buffer, &mut self.output_buffer[..buffer.len()]);
            buffer.copy_from_slice(&self.output_buffer[..buffer.len()]);
        }
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    fn set_parameter_value(&mut self, name: &str, value: PedalParameterValue) {
        if name == "plugin" {
            if let PedalParameterValue::String(plugin_name) = value {
                self.set_plugin(&plugin_name);
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

    fn ui(&mut self, ui: &mut egui::Ui, _message_buffer: &[String]) -> Option<(String, PedalParameterValue)> {
        let mut plugin_param_change = None;
        if let Some(i) = self.instance.as_mut() {
            plugin_param_change = i.ui_frame(ui);
        }

        let mut selected = self.parameters.get("plugin").unwrap().value.as_str().unwrap().to_string();
        let old = selected.clone();
        let mut knob_to_change = None;

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

                egui::ComboBox::from_id_salt(self.id)
                    .selected_text(&selected)
                    .width(ui.available_width())
                    .wrap_mode(egui::TextWrapMode::Truncate)
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut selected, String::new(), "Empty");
                        for plugin in &self.available_plugins {
                            ui.selectable_value(&mut selected, plugin.clone(), &plugin[..plugin.len()-4]); // Remove the ".dll" extension
                        }
                    });
                
                ui.add_space(5.0);

                if ui.add_enabled(
                    self.instance.as_ref().map(|i| !i.ui_open).unwrap_or(false),
                    Button::new("Parameters")
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
                    
                if let Some(value) = pedal_knob(ui, RichText::new("Dry/Wet").color(Color32::WHITE).size(8.0), self.parameters.get("dry_wet").unwrap(), Vec2::new(0.325, 0.55), 0.35) {
                    knob_to_change = Some(("dry_wet".to_string(), value));
                }
            }
        );

        if plugin_param_change.is_some() {
            return plugin_param_change;
        }

        if knob_to_change.is_some() {
            return knob_to_change;
        }
            
        if selected != old {
            Some(("plugin".to_string(), PedalParameterValue::String(selected)))
        } else {
            None
        }
    }
}
