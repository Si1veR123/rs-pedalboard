use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use crate::dsp_algorithms::impluse_response::{IRConvolver, load_ir};
use crate::pedals::ui::{pedal_switch, sideways_arrow};
use crate::pedals::ParameterUILocation;
use crate::{forward_slash_path, unique_time_id, SAVE_DIR};
use egui_directory_combobox::DirectoryComboBox;
use serde::{ser::SerializeMap, Deserialize, Serialize};
use eframe::egui::{self, include_image, Vec2};

use super::{ui::pedal_knob, PedalParameter, PedalParameterValue, PedalTrait};

const IR_SAVE_PATH: &str = r"IR";

#[derive(Clone)]
pub struct ImpulseResponse {
    parameters: HashMap<String, PedalParameter>,

    combobox_widget: DirectoryComboBox,
    midi_min_combobox_widget: DirectoryComboBox,
    midi_max_combobox_widget: DirectoryComboBox,
    folders_state: u32,
    id: u32,

    // Server only
    // IRConvolver requires block size. This is set on the server after being created, and not set on client at all.
    dry_buffer: Vec<f32>,
    max_buffer_size: usize,
    ir: Option<IRConvolver>,
    sample_rate: Option<f32>,
}

impl Hash for ImpulseResponse {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Serialize for ImpulseResponse {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut ser_map = serializer.serialize_map(Some(2))?;
        ser_map.serialize_entry("id", &self.id)?;
        let mut parameters = self.parameters.clone();
        // If the IR path is in the pedalboard IR directory, store it as a relative path
        if let Some(ir_path) = self.parameters.get("IR").and_then(|p| p.value.as_str()) {
            if let Some(save_dir) = Self::get_save_directory() {
                if let Ok(relative_path) = PathBuf::from(ir_path).strip_prefix(&save_dir) {
                    // Convert relative paths to use forward slashes for cross platform compatibility
                    // Not used for absolute path as they are not intended to be portable
                    let relative_path_converted = forward_slash_path(relative_path);
                    parameters.get_mut("IR").unwrap().value = PedalParameterValue::String(relative_path_converted.to_string_lossy().to_string());
                }
            }
        }
        ser_map.serialize_entry("parameters", &parameters)?;
        ser_map.end()
    }
}

impl<'a> Deserialize<'a> for ImpulseResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        #[derive(Deserialize)]
        struct ImpulseResponseData {
            id: u32,
            parameters: HashMap<String, PedalParameter>,
        }

        let mut helper = ImpulseResponseData::deserialize(deserializer)?;
        let id = helper.id;
        let mut combobox_widget = Self::get_empty_directory_combo_box(id);
        let midi_min_combobox_widget = Self::get_empty_directory_combo_box(egui::Id::new(id).with("midi_min"));
        let midi_max_combobox_widget = Self::get_empty_directory_combo_box(egui::Id::new(id).with("midi_max"));

        let mut model_path = helper.parameters.get("IR")
            .and_then(
                |p| p.value.as_str().and_then(|s| if s == "" { None } else { Some(PathBuf::from(s)) } )
            );

        // If the model path is relative, make it absolute based on save directory
        if let Some(model_path) = model_path.as_mut() {
            if model_path.is_relative() {
                if let Some(save_dir) = Self::get_save_directory() {
                    if let Ok(absolute_path) = dunce::canonicalize(save_dir.join(&model_path)) {
                        *model_path = absolute_path;
                    } else {
                        log::warn!("Failed to canonicalize IR path: {:?}", model_path);
                        *model_path = PathBuf::new();
                    }
                } else {
                    log::warn!("Failed to get save directory for IR path: {:?}", model_path);
                    *model_path = PathBuf::new();
                }
            }
        }

        combobox_widget.set_selection(model_path.as_ref());
        helper.parameters.get_mut("IR").map(|p| {
            if let Some(path) = model_path {
                p.value = PedalParameterValue::String(path.to_string_lossy().to_string());
            } else {
                p.value = PedalParameterValue::String("".to_string());
            }
        });

        Ok(Self {
            ir: None,
            parameters: helper.parameters,
            dry_buffer: vec![0.0; 512],
            combobox_widget,
            midi_min_combobox_widget,
            midi_max_combobox_widget,
            folders_state: 0,
            max_buffer_size: 0,
            id,
            sample_rate: None,
        })
    }
}

impl ImpulseResponse {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();

        let init_ir = r"";
        parameters.insert(
            "IR".to_string(),
            PedalParameter {
                value: PedalParameterValue::String(init_ir.to_string()),
                min: None,
                max: None,
                step: None,
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
            "Active".to_string(),
            PedalParameter {
                value: PedalParameterValue::Bool(true),
                min: None,
                max: None,
                step: None,
            },
        );

        let id = unique_time_id();

        Self {
            ir: None,
            parameters,
            dry_buffer: Vec::new(),
            combobox_widget: Self::get_empty_directory_combo_box(id),
            midi_min_combobox_widget: Self::get_empty_directory_combo_box(egui::Id::new(id).with("midi_min")),
            midi_max_combobox_widget: Self::get_empty_directory_combo_box(egui::Id::new(id).with("midi_max")),
            folders_state: 0,
            max_buffer_size: 0,
            id,
            sample_rate: None,
        }
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = unique_time_id();
        cloned
    }

    fn get_empty_directory_combo_box(id: impl std::hash::Hash) -> DirectoryComboBox {
        DirectoryComboBox::new_from_nodes(vec![])
            .with_id(egui::Id::new("ir_combobox").with(id))
            .with_wrap_mode(egui::TextWrapMode::Truncate)
            .show_extensions(false)
            .select_files_only(true)
            .with_filter(Arc::new(|path: &std::path::Path| {
                if path.is_dir() {
                    true
                } else if let Some(ext) = path.extension() {
                    ext == "wav"
                } else {
                    false
                }
            }))
    }

    /// Ensure max_buffer_size is set before setting the IR.
    pub fn set_ir_convolver<P: AsRef<Path>>(&mut self, ir_path: P, sample_rate: f32) {
        if ir_path.as_ref().as_os_str().is_empty() {
            self.remove_ir();
            return;
        }

        let canon_path = match dunce::canonicalize(ir_path.as_ref()) {
            Ok(p) => p,
            Err(e) => {
                log::error!("Failed to canonicalize IR path {:?}: {}", ir_path.as_ref(), e);
                return;
            }
        };

        let string_path = match canon_path.to_str() {
            Some(s) => s.to_string(),
            None => {
                log::warn!("IR path is not valid unicode");
                return;
            }
        };

        match load_ir(ir_path.as_ref(), sample_rate) {
            Ok(ir) => {
                self.ir = Some(IRConvolver::new(ir.first().expect("IR has no channels").as_slice(), self.max_buffer_size));

                // Update combobox to match new selection (in case it was not set from the combobox itself)
                self.combobox_widget.set_selection(Some(&string_path));

                self.parameters.get_mut("IR").unwrap().value = PedalParameterValue::String(string_path);
            },
            Err(e) => {
                log::error!("Failed to load IR: {}", e);
                return;
            }
        };
    }

    pub fn remove_ir(&mut self) {
        self.parameters.get_mut("IR").unwrap().value = PedalParameterValue::String("".to_string());
        self.combobox_widget.set_selection::<&str>(None);
        self.ir = None;
    }

    pub fn get_save_directory() -> Option<PathBuf> {
        Some(dunce::canonicalize(homedir::my_home().ok()??.join(SAVE_DIR).join(IR_SAVE_PATH)).ok()?)
    }

    /// Update the main pedal value, and midi min and max combobox widgets if the root directories have changed
    fn update_combobox_nodes(&mut self, ui: &mut egui::Ui) {
        // Refresh the list of root directories if it has changed
        let new_root_directories: Option<Vec<egui_directory_combobox::DirectoryNode>> = ui.ctx().memory_mut(|m| {
            let state = m.data.get_temp_mut_or("ir_folders_state".into(), 1u32);
            if *state != self.folders_state {
                self.folders_state = *state;
                m.data.get_temp("ir_folders".into()).as_ref().cloned()
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

    // If parameter is set, we force the combobox to show the value in parameter
    fn show_ir_combobox(&mut self, ui: &mut egui::Ui, parameter: Option<&PedalParameter>, location: ParameterUILocation) -> egui::InnerResponse<Option<PedalParameterValue>> {
        self.update_combobox_nodes(ui);

        let combobox_to_show = match location {
            ParameterUILocation::Pedal => &mut self.combobox_widget,
            ParameterUILocation::ParameterWindow => &mut self.combobox_widget,
            ParameterUILocation::MidiMin => &mut self.midi_min_combobox_widget,
            ParameterUILocation::MidiMax => &mut self.midi_max_combobox_widget
        };

        if let Some(param) = parameter {
            if let PedalParameterValue::String(ir_path) = &param.value {
                if ir_path.is_empty() {
                    combobox_to_show.set_selection::<&str>(None);
                } else {
                    combobox_to_show.set_selection(Some(ir_path));
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
                            log::warn!("Selected IR is not valid unicode");
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

impl PedalTrait for ImpulseResponse {
    fn get_id(&self) -> u32 {
        self.id
    }

    /// If `ir` parameter is set, but `ir` is None, this will set the IR as it is assumed that we are waiting on knowing the max buffer size and sample rate (on server).
    fn set_config(&mut self, buffer_size: usize, sample_rate: u32) {
        self.max_buffer_size = buffer_size;
        self.dry_buffer.resize(buffer_size, 0.0);
        self.sample_rate = Some(sample_rate as f32);

        let ir_path = self.parameters.get("IR").unwrap().value.as_str().unwrap().to_string();
        self.set_ir_convolver(&ir_path, sample_rate as f32);
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        if self.sample_rate.is_none() {
            log::warn!("ImpulseResponse: Call set_config before processing.");
            return;
        }

        if self.ir.is_none() {
            return;
        }

        let dry_wet = self.parameters.get("Dry/Wet").unwrap().value.as_float().unwrap();

        self.dry_buffer.clear();
        self.dry_buffer.extend_from_slice(buffer);

        self.ir.as_mut().unwrap().process(buffer);

        for (i, sample) in buffer.iter_mut().enumerate() {
            *sample = (*sample * dry_wet) + (self.dry_buffer[i] * (1.0 - dry_wet));
        }
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }

    fn set_parameter_value(&mut self, name: &str, value: PedalParameterValue) {
        if name == "IR" {
            // If sample rate is not set we are not on server, so don't need to set the IR convolver.
            let path = value.as_str().unwrap();
            if let Some(sample_rate) = self.sample_rate {
                self.set_ir_convolver(path, sample_rate);
                return;
            }
            if !path.is_empty() {
                self.combobox_widget.set_selection(Some(path));
            } else {
                self.combobox_widget.set_selection::<&str>(None);
            }
        }

        if !self.parameters.get(name).unwrap().is_valid(&value) {
            log::warn!("Attempted to set invalid value for parameter {}: {:?}", name, value);
            return;
        }

        if let Some(param) = self.parameters.get_mut(name) {
            param.value = value;
        } else {
            log::error!("Parameter {} not found", name);
        }
    }

    fn parameter_editor_ui(&mut self, ui: &mut egui::Ui, name: &str, parameter: &PedalParameter, location: ParameterUILocation) -> egui::InnerResponse<Option<PedalParameterValue>> {
        if name == "IR" {
            ui.spacing_mut().combo_width = ui.available_width();
            
            self.show_ir_combobox(ui, Some(parameter), location)
        } else {
            parameter.parameter_editor_ui(ui)
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _message_buffer: &[String]) -> Option<(String,PedalParameterValue)> {
        let pedal_rect = ui.available_rect_before_wrap();

        ui.add(egui::Image::new(include_image!("images/ir.png")));
        
        let mut to_change = None;

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
        
        combo_ui.spacing_mut().combo_width = combo_ui.available_width();
        if let Some(new_model_value) = self.show_ir_combobox(&mut combo_ui, None, ParameterUILocation::Pedal).inner {
            to_change = Some(("IR".to_string(), new_model_value));
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
                    to_change = Some((String::from("IR"), PedalParameterValue::String(s.to_string())));
                } else {
                    log::warn!("Selected IR path is not valid unicode");
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
                    to_change = Some((String::from("IR"), PedalParameterValue::String(s.to_string())));
                } else {
                    log::warn!("Selected IR path is not valid unicode");
                }
            }
        };

        if let Some(value) = pedal_knob(ui, "", self.parameters.get("Dry/Wet").unwrap(), Vec2::new(0.325, 0.037), 0.35) {
            to_change = Some(("Dry/Wet".to_string(), value));
        }

        let active_param = self.get_parameters().get("Active").unwrap().value.as_bool().unwrap();
        if let Some(value) = pedal_switch(ui, active_param, egui::Vec2::new(0.33, 0.72), 0.16) {
            to_change = Some(("Active".to_string(), PedalParameterValue::Bool(value)));
        }

        to_change
    }
}
