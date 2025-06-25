use std::path::PathBuf;
use std::collections::HashMap;
use std::hash::Hash;

use crate::dsp_algorithms::impluse_response::{IRConvolver, load_ir};
use crate::{unique_time_id, SAVE_DIR};
use serde::{ser::SerializeMap, Deserialize, Serialize};
use eframe::egui::{self, include_image, Color32, Layout, RichText, UiBuilder, Vec2};

use super::{ui::pedal_knob, PedalParameter, PedalParameterValue, PedalTrait};

const IR_SAVE_PATH: &str = r"IR";

pub struct ImpulseResponse {
    // IRConvolver requires block size. This is set on the server after being created, and not set on client at all.
    ir: Option<IRConvolver>,
    max_buffer_size: usize,
    parameters: HashMap<String, PedalParameter>,

    dry_buffer: Vec<f32>,
    saved_ir_files: Vec<PathBuf>,
    // Used to generate a unique ID for the drop down menu
    id: usize
}

impl Clone for ImpulseResponse {
    fn clone(&self) -> Self {
        Self {
            ir: self.ir.clone(),
            max_buffer_size: self.max_buffer_size,
            parameters: self.parameters.clone(),
            dry_buffer: Vec::new(),
            saved_ir_files: Self::saved_ir_files(),
            id: unique_time_id()
        }
    }
}

impl Hash for ImpulseResponse {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.parameters.values().map(|p| &p.value).for_each(|v| v.hash(state));
    }
}

impl Serialize for ImpulseResponse {
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

impl<'a> Deserialize<'a> for ImpulseResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'a>,
    {
        // The ir convolver is set when the config is set on the server, as it requires the max buffer size.
        let parameters = HashMap::<String, PedalParameter>::deserialize(deserializer)?;

        let mut ir = Self::new();
        ir.parameters = parameters;

        Ok(ir)
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

        Self {
            ir: None,
            parameters,
            dry_buffer: Vec::new(),
            saved_ir_files: Self::saved_ir_files(),
            max_buffer_size: 0,
            id: unique_time_id()
        }
    }

    /// Ensure max_buffer_size is set before setting the IR.
    pub fn set_ir(&mut self, ir_path: &str) {
        if ir_path.is_empty() {
            self.remove_ir();
            return;
        }

        match load_ir(ir_path) {
            Ok(ir) => {
                self.parameters.get_mut("ir").unwrap().value = PedalParameterValue::String(ir_path.to_string());
                self.ir = Some(IRConvolver::new(&ir, self.max_buffer_size));
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

    pub fn saved_ir_files() -> Vec<PathBuf> {
        let mut files = Vec::new();
        if let Some(dir) = Self::get_save_directory() {
            if dir.exists() {
                for entry in std::fs::read_dir(dir).unwrap() {
                    let entry = entry.unwrap();
                    if entry.path().extension().map_or(false, |ext| ext == "wav") {
                        files.push(entry.path());
                    }
                }
            } else {
                std::fs::create_dir_all(&dir).unwrap();
            }
        } else {
            log::error!("Failed to get IR save directory");
        }
        files
    }
}

impl PedalTrait for ImpulseResponse {
    /// If `ir` parameter is set, but `ir` is None, this will set the IR as it is assumed that we are waiting on knowing the max buffer size.
    fn set_config(&mut self, buffer_size: usize, _sample_rate: usize) {
        self.max_buffer_size = buffer_size;

        let ir_path = self.parameters.get("ir").unwrap().value.as_str().unwrap().to_string();
        if !ir_path.is_empty() && self.ir.is_none() {
            self.set_ir(&ir_path);
        } else {
            self.ir = None;
        }
    }

    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        let ir = match self.ir {
            Some(ref mut ir) => ir,
            None => return,
        };

        let dry_wet = self.parameters.get("dry_wet").unwrap().value.as_float().unwrap();

        self.dry_buffer.clear();
        self.dry_buffer.extend_from_slice(buffer);

        ir.process(buffer);

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

    fn set_parameter_value(&mut self, name: &str, value:PedalParameterValue) {
        if !self.parameters.get(name).unwrap().is_valid(&value) {
            log::warn!("Attempted to set invalid value for parameter {}: {:?}", name, value);
            return;
        }

        if let Some(param) = self.parameters.get_mut(name) {
            if name == "ir" {
                self.set_ir(value.as_str().unwrap());
            } else {
                param.value = value;
            }
        } else {
            log::error!("Parameter {} not found", name);
        }
    }

    fn ui(&mut self, ui: &mut egui::Ui, _message_buffer: &[String]) -> Option<(String,PedalParameterValue)> {
        let mut img_ui = ui.new_child(
            UiBuilder::new()
                .max_rect(ui.available_rect_before_wrap())
        );

        img_ui.add(egui::Image::new(include_image!("images/ir_pedal_gradient.png")).tint(Color32::from_rgb(70, 70, 70)));

        let selected = PathBuf::from(self.parameters.get("ir").unwrap().value.as_str().unwrap());

        let selected_file_name = selected.file_name().unwrap_or_default().to_string_lossy();
        let mut selected_str = selected.to_string_lossy().to_string();
        let old = selected_str.clone();
        
        let mut knob_to_change = None;

        ui.allocate_ui_with_layout(Vec2::new(ui.available_width()*0.95, ui.available_height()), Layout::top_down(egui::Align::Center), |ui| {
            ui.add_space(33.0);
            
            ui.label(egui::RichText::new("Impulse\nResponse").size(19.0));

            ui.add_space(5.0);

            egui::ComboBox::from_id_salt(self.id)
                .selected_text(selected_file_name)
                .width(ui.available_width())
                .wrap_mode(egui::TextWrapMode::Truncate)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut selected_str, String::new(), "Empty");
                    for file in &self.saved_ir_files {
                        let name = file.file_name().unwrap().to_string_lossy();

                        ui.selectable_value(&mut selected_str, file.to_string_lossy().to_string(), &name[..name.len()-4]); // remove .wav extension
                    }
                });

            ui.add_space(5.0);

            ui.allocate_ui_with_layout(Vec2::new(ui.available_width(), ui.available_width()*0.25), Layout::left_to_right(egui::Align::Center), |ui| {
                if let Some(value) = pedal_knob(ui, RichText::new("Dry/Wet").color(Color32::WHITE).size(8.0), self.parameters.get("dry_wet").unwrap(), Vec2::new(0.325, 0.0), 0.35) {
                    knob_to_change = Some(("dry_wet".to_string(), value));
                }
            });
        });

        if selected_str != old {
            Some((String::from("ir"), PedalParameterValue::String(selected_str)))
        } else {
            if let Some(to_change) = knob_to_change {
                Some(to_change)
            } else {
                None
            }
        }
    }
}
