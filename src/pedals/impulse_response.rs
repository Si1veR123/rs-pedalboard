use std::path::{Path, PathBuf};
use std::collections::HashMap;
use std::hash::Hash;
use std::sync::Arc;

use crate::dsp_algorithms::impluse_response::{IRConvolver, load_ir};
use crate::pedals::ui::{pedal_switch, sideways_arrow};
use crate::{unique_time_id, SAVE_DIR};
use egui_directory_combobox::{DirectoryComboBox, DirectoryNode};
use serde::{ser::SerializeMap, Deserialize, Serialize};
use eframe::egui::{self, include_image, Vec2};

use super::{ui::pedal_knob, PedalParameter, PedalParameterValue, PedalTrait};

const IR_SAVE_PATH: &str = r"IR";

#[derive(Clone)]
pub struct ImpulseResponse {
    parameters: HashMap<String, PedalParameter>,

    combobox_widget: DirectoryComboBox,
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
        ser_map.serialize_entry("parameters", &self.parameters)?;
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

        let helper = ImpulseResponseData::deserialize(deserializer)?;
        let id = helper.id;
        let combobox_widget = Self::get_empty_directory_combo_box(id);

        Ok(Self {
            ir: None,
            parameters: helper.parameters,
            dry_buffer: vec![0.0; 512],
            combobox_widget,
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
            "ir".to_string(),
            PedalParameter {
                value: PedalParameterValue::String(init_ir.to_string()),
                min: None,
                max: None,
                step: None,
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
            "active".to_string(),
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
        let string_path = match ir_path.as_ref().to_str() {
            Some(s) => s.to_string(),
            None => {
                log::warn!("IR path is not valid unicode");
                return;
            }
        };

        match load_ir(ir_path.as_ref(), sample_rate) {
            Ok(ir) => {
                self.ir = Some(IRConvolver::new(ir.first().expect("IR has no channels").as_slice(), self.max_buffer_size));
                self.parameters.get_mut("ir").unwrap().value = PedalParameterValue::String(string_path);
            },
            Err(e) => {
                log::error!("Failed to load IR: {}", e);
                return;
            }
        };
    }

    pub fn remove_ir(&mut self) {
        self.parameters.get_mut("ir").unwrap().value = PedalParameterValue::String("".to_string());
        self.ir = None;
    }

    pub fn get_save_directory() -> Option<PathBuf> {
        Some(homedir::my_home().ok()??.join(SAVE_DIR).join(IR_SAVE_PATH))
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

        let ir_path = self.parameters.get("ir").unwrap().value.as_str().unwrap().to_string();
        if !ir_path.is_empty() {
            self.set_ir_convolver(&ir_path, sample_rate as f32);
        } else {
            self.ir = None;
        }
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        if self.sample_rate.is_none() {
            log::warn!("ImpulseResponse: Call set_config before processing.");
            return;
        }

        if self.ir.is_none() {
            return;
        }

        let dry_wet = self.parameters.get("dry_wet").unwrap().value.as_float().unwrap();

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
        if name == "ir" {
            // If sample rate is not set we are not on server, so don't need to set the IR convolver.
            let path = value.as_str().unwrap();
            if let Some(sample_rate) = self.sample_rate {
                if path.is_empty() {
                    self.remove_ir();
                } else {
                    self.set_ir_convolver(path, sample_rate);
                }
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

    fn ui(&mut self, ui: &mut egui::Ui, _message_buffer: &[String]) -> Option<(String,PedalParameterValue)> {
        // Refresh the list of root directories if it has changed
        let new_root_directories: Option<Vec<DirectoryNode>> = ui.ctx().memory_mut(|m| {
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
                roots.push(DirectoryNode::from_path(&main_save_dir));
            } else {
                log::warn!("Failed to get main save directory");
            }
            self.combobox_widget = Self::get_empty_directory_combo_box(self.id);
            let ir_path = self.parameters.get("ir").unwrap().value.as_str().unwrap();
            self.combobox_widget.set_selection(match ir_path {
                s if s.is_empty() => None,
                s => Some(s)
            });

            // If there is only one root directory, use its children as the roots
            if roots.len() == 1 {
                match roots.pop().unwrap() {
                    DirectoryNode::Directory(_, children) => {
                        self.combobox_widget.roots = children;
                    },
                    _ => self.combobox_widget.roots = roots
                }
            } else {
                self.combobox_widget.roots = roots;
            }
        }
        
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
        
        let old = self.combobox_widget.selected().map(|p| p.to_path_buf());
        combo_ui.spacing_mut().combo_width = combo_ui.available_width();
        combo_ui.add_sized(Vec2::new(combo_ui.available_width(), 15.0), &mut self.combobox_widget);
        if old.as_ref().map(|p| p.as_path()) != self.combobox_widget.selected() {
            match self.combobox_widget.selected() {
                Some(path) => {
                    match path.to_str() {
                        Some(s) => {
                            let selected_str = s.to_string();
                            to_change = Some((String::from("ir"), PedalParameterValue::String(selected_str)));
                        },
                        None => {
                            log::warn!("Selected IR path is not valid unicode");
                        }
                    }
                },
                None => {
                    to_change = Some((String::from("ir"), PedalParameterValue::String("".to_string())));
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
                    to_change = Some((String::from("ir"), PedalParameterValue::String(s.to_string())));
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
                    to_change = Some((String::from("ir"), PedalParameterValue::String(s.to_string())));
                } else {
                    log::warn!("Selected IR path is not valid unicode");
                }
            }
        };

        if let Some(value) = pedal_knob(ui, "", self.parameters.get("dry_wet").unwrap(), Vec2::new(0.325, 0.037), 0.35) {
            to_change = Some(("dry_wet".to_string(), value));
        }

        let active_param = self.get_parameters().get("active").unwrap().value.as_bool().unwrap();
        if let Some(value) = pedal_switch(ui, active_param, egui::Vec2::new(0.33, 0.72), 0.16) {
            to_change = Some(("active".to_string(), PedalParameterValue::Bool(value)));
        }

        to_change
    }
}
