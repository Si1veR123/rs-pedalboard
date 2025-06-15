use std::collections::HashMap;
use std::hash::Hash;
use std::io::empty;

use super::PedalTrait;
use super::PedalParameter;
use super::PedalParameterValue;
use super::ui::{pedal_knob, pedal_label_rect};

use crate::plugin::vst2::{Vst2Instance, path_from_name, available_plugins};
use crate::plugin::PluginHost;
use crate::unique_time_id;

use eframe::egui::Button;
use eframe::egui::{include_image, self};
use serde::ser::SerializeMap;
use serde::{Serialize, Deserialize};

#[derive(Clone)]
pub struct Vst2 {
    instance: Option<Vst2Instance>,
    // buffer size, sample rate
    config: Option<(usize, usize)>,
    parameters: HashMap<String, PedalParameter>,
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
        let name = parameters_with_idx.get("Plugin").unwrap().1.value.as_str().unwrap().to_string();

        let mut parameters = HashMap::new();
        parameters.insert(String::from("Plugin"), PedalParameter {
            value: PedalParameterValue::String(name.clone()),
            min: None,
            max: None,
            step: None
        });

        let mut parameters_idx_map = HashMap::new();
        if !name.is_empty() {
            for (key, (idx, param)) in parameters_with_idx {
                if let Some(index) = idx {
                    // If index is Some(..), then the parameter is a plugin parameter
                    parameters.insert(key.clone(), param);
                    parameters_idx_map.insert(key, index);
                }
            }
        }

        let available_plugins = available_plugins();
        let empty_vst = Vst2 {
            instance: None,
            config: None,
            parameters: parameters.clone(),
            param_index_map: HashMap::new(),
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
                    Ok(empty_vst)
                }
            }?;

            // Set the saved parameters on the new instance
            deserialized_vst.synchronise_instance();

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
            "Plugin".to_string(),
            PedalParameter {
                value: PedalParameterValue::String("".to_string()),
                min: None,
                max: None,
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
    pub fn set_parameters_from_instance(&mut self) {
        if let Some(instance) = self.instance.as_mut() {
            self.parameters.clear();
            self.parameters.insert(
                "Plugin".to_string(),
                PedalParameter {
                    value: PedalParameterValue::String(instance.info.name.clone()),
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
            self.parameters.clear();
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
    pub fn synchronise_instance(&mut self) {
        if let Some(instance) = self.instance.as_mut() {
            for (name, param) in &self.parameters {
                if name == "Plugin" {
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

    pub fn set_plugin(&mut self, plugin_name: &str) {
        if plugin_name.is_empty() {
            self.instance = None;
            self.set_parameters_from_instance();
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
                self.set_parameters_from_instance();
            },
            Err(_) => {
                log::error!("Failed to load plugin: {}", plugin_name);
                self.instance = None;
                self.set_parameters_from_instance();
            }
        }
    }
}

impl PedalTrait for Vst2 {
    fn set_config(&mut self, buffer_size: usize, sample_rate: usize) {
        if let Some(instance) = self.instance.as_mut() {
            // The instance will not have been configured before this is called, so call set_config
            instance.set_config(buffer_size, sample_rate);
        }

        self.output_buffer.resize(buffer_size, 0.0);
        self.config = Some((buffer_size, sample_rate));
    }

    fn process_audio(&mut self, buffer: &mut [f32]) {
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
        if name == "Plugin" {
            if let PedalParameterValue::String(plugin_name) = value {
                self.set_plugin(&plugin_name);
            }
            return;
        }

        if let Some(parameter) = self.parameters.get_mut(name) {
            if parameter.is_valid(&value) {
                parameter.value = value;
                if let Some(instance) = self.instance.as_mut() {
                    if let Some(&index) = self.param_index_map.get(name) {
                        instance.set_parameter_value(index, parameter.value.as_float().unwrap());
                    } else {
                        log::warn!("Parameter {} not found in plugin instance", name);
                    }
                } else {
                    log::warn!("No plugin instance available to set parameter {}", name);
                }
            }
        }
    }

    fn ui(&mut self, ui: &mut eframe::egui::Ui) -> Option<(String, PedalParameterValue)> {
        let mut plugin_param_change = None;
        if let Some(i) = self.instance.as_mut() {
            plugin_param_change = i.ui_frame(ui);
        }

        let available_rect = ui.available_rect_before_wrap();
        ui.painter().rect_filled(available_rect.with_max_y(available_rect.max.y-20.0), 10.0, eframe::egui::Color32::from_rgb(70, 95, 70));

        ui.label("VST2");

        let mut selected = self.parameters.get("Plugin").unwrap().value.as_str().unwrap().to_string();
        let old = selected.clone();

        egui::ComboBox::from_id_salt(self.id)
            .selected_text(&selected)
            .width(ui.available_width())
            .wrap_mode(egui::TextWrapMode::Truncate)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut selected, String::new(), "Empty");
                for plugin in &self.available_plugins {
                    ui.selectable_value(&mut selected, plugin.clone(), plugin);
                }
            });
            
        if ui.add_enabled(self.instance.as_ref().map(|i| !i.ui_open).unwrap_or(false), Button::new("Window")).clicked() {
            if let Some(instance) = self.instance.as_mut() {
                if instance.ui_open {
                    instance.close_ui();
                } else {
                    instance.open_ui();
                }
            }
        }

        if plugin_param_change.is_some() {
            return plugin_param_change;
        }
            
        if selected != old {
            Some(("Plugin".to_string(), PedalParameterValue::String(selected)))
        } else {
            None
        }
    }
}
