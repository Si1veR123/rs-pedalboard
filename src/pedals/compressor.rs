// make threshold 0-1
// add soft knee
use std::collections::HashMap;
use std::hash::Hash;
use std::time::{Duration, Instant};
use crate::pedals::ui::pedal_switch;
use crate::DEFAULT_REFRESH_DURATION;

use super::{PedalTrait, PedalParameter, PedalParameterValue};
use super::ui::pedal_knob;
use eframe::egui::{self, include_image, UiBuilder, Vec2};
use serde::{ser::SerializeMap, Deserialize, Serialize};

const ENVELOPE_UPDATE_RATE: Duration = Duration::from_millis(100);
const EPS: f32 = 1e-8;

#[derive(Clone)]
pub struct Compressor {
    parameters: HashMap<String, PedalParameter>,
    sample_rate: Option<f32>,
    envelope: f32,

    // Client only, used for smoothing
    current_envelope: f32,

    // Server only
    envelope_last_sent_time: Instant,
    envelope_last_sent_value: f32,

    id: u32,
}

impl Serialize for Compressor {
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

impl<'de> Deserialize<'de> for Compressor {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct CompressorData {
            id: u32,
            parameters: HashMap<String, PedalParameter>,
        }

        let helper = CompressorData::deserialize(deserializer)?;
        Ok(Compressor {
            id: helper.id,
            parameters: helper.parameters,
            sample_rate: None,
            envelope: 0.0,
            current_envelope: 0.0,
            envelope_last_sent_time: Instant::now(),
            envelope_last_sent_value: 0.0,
        })
    }
}

impl Hash for Compressor {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Compressor {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();

        parameters.insert(
            "Threshold".into(),
            PedalParameter {
                value: PedalParameterValue::Float(-6.0),
                min: Some(PedalParameterValue::Float(-30.0)),
                max: Some(PedalParameterValue::Float(0.0)),
                step: None,
            },
        );
        parameters.insert("Attack".into(), PedalParameter {
            value: PedalParameterValue::Float(10.0),
            min: Some(PedalParameterValue::Float(1.0)),
            max: Some(PedalParameterValue::Float(50.0)),
            step: None,
        });
        parameters.insert("Release".into(), PedalParameter {
            value: PedalParameterValue::Float(100.0),
            min: Some(PedalParameterValue::Float(5.0)),
            max: Some(PedalParameterValue::Float(300.0)),
            step: None,
        });
        parameters.insert("Level".into(), PedalParameter {
            value: PedalParameterValue::Float(1.0),
            min: Some(PedalParameterValue::Float(0.0)),
            max: Some(PedalParameterValue::Float(5.0)),
            step: None,
        });
        parameters.insert("Ratio".into(), PedalParameter {
            value: PedalParameterValue::Float(5.0),
            min: Some(PedalParameterValue::Float(1.0)),
            max: Some(PedalParameterValue::Float(20.0)),
            step: None,
        });
        parameters.insert("Dry/Wet".into(), PedalParameter {
            value: PedalParameterValue::Float(1.0),
            min: Some(PedalParameterValue::Float(0.0)),
            max: Some(PedalParameterValue::Float(1.0)),
            step: None,
        });
        parameters.insert(
            "Soft Knee".into(),
            PedalParameter {
                value: PedalParameterValue::Float(0.0),
                min: Some(PedalParameterValue::Float(0.0)),
                max: Some(PedalParameterValue::Float(12.0)),
                step: None,
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

        Compressor {
            parameters,
            envelope: 0.0,
            current_envelope: 0.0,
            sample_rate: None,
            envelope_last_sent_time: Instant::now(),
            envelope_last_sent_value: 0.0,
            id: crate::unique_time_id(),
        }
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = crate::unique_time_id();
        cloned
    }
}

impl PedalTrait for Compressor {
    fn get_id(&self) -> u32 {
        self.id
    }

    fn set_config(&mut self, _buffer_size: usize, sample_rate: u32) {
        self.sample_rate = Some(sample_rate as f32);
    }

    fn process_audio(&mut self, buffer: &mut [f32], messages: &mut Vec<String>) {
        let sample_rate = match self.sample_rate {
            Some(rate) => rate,
            None => {
                tracing::warn!("Compressor: Call set_config before processing.");
                return;
            }
        };

        let threshold_db = self.parameters["Threshold"].value.as_float().unwrap();
        let attack = self.parameters["Attack"].value.as_float().unwrap();
        let release = self.parameters["Release"].value.as_float().unwrap();
        let level = self.parameters["Level"].value.as_float().unwrap();
        let ratio = self.parameters["Ratio"].value.as_float().unwrap();
        let blend = self.parameters["Dry/Wet"].value.as_float().unwrap();
        let soft_knee_db = self.parameters["Soft Knee"].value.as_float().unwrap();

        // Sample rate independent
        let attack_coeff = (-1.0 / (attack / 1000.0 * sample_rate)).exp();
        let release_coeff = (-1.0 / (release / 1000.0 * sample_rate)).exp();

        for sample in buffer.iter_mut() {
            // Envelope follower
            self.envelope = if sample.abs() > self.envelope {
                attack_coeff * (self.envelope - sample.abs()) + sample.abs()
            } else {
                release_coeff * (self.envelope - sample.abs()) + sample.abs()
            };

            // Convert envelope to dB
            let env_lin = (self.envelope).max(EPS);
            let env_db = 20.0 * env_lin.log10();

            let knee_start = threshold_db - soft_knee_db / 2.0;
            let knee_end = threshold_db + soft_knee_db / 2.0;

            // Compression gain
            let out_db = if env_db <= knee_start {
                env_db
            } else if env_db >= knee_end {
                threshold_db + (env_db - threshold_db) / ratio
            } else {
                let delta = env_db - knee_start;
                env_db + ((1.0 / ratio - 1.0) * delta * delta) / (2.0 * soft_knee_db.max(EPS))
            };

            // gain in dB to apply = out_db - env_db (usually <= 0)
            let gain_db = out_db - env_db;
            let gain_lin = 10f32.powf(gain_db / 20.0);

            let compressed_sample = *sample * gain_lin * level;

            // Blend dry + compressed
            *sample = *sample * (1.0 - blend) + compressed_sample * blend;
        }

        // Send envelope to client
        if self.envelope_last_sent_time.elapsed() >= ENVELOPE_UPDATE_RATE {
            let envelope_round = (self.envelope * 100.0).round() / 100.0;

            if (envelope_round - self.envelope_last_sent_value).abs() >= 0.005 {
                messages.push(format!("{:?}", envelope_round));
                self.envelope_last_sent_value = envelope_round;
            }
            
            self.envelope_last_sent_time = Instant::now();
        }
    }

    fn get_parameters(&self) -> &HashMap<String, PedalParameter> {
        &self.parameters
    }

    fn get_parameters_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.parameters
    }
    
    fn set_parameter_value(&mut self,name: &str,value:PedalParameterValue){
        let parameters = self.get_parameters_mut();
        if let Some(parameter) = parameters.get_mut(name){
            if parameter.is_valid(&value){
                parameter.value = value;
            } else {
                tracing::warn!("Attempted to set invalid value for parameter {}: {:?}",name,value);
            }
        }
    }
    
    fn ui(&mut self, ui: &mut eframe::egui::Ui, message_buffer: &[String]) -> Option<(String,PedalParameterValue)> {
        ui.ctx().request_repaint_after(DEFAULT_REFRESH_DURATION);

        if message_buffer.len() > 0 {
            // Update envelope from message buffer
            if let Ok(envelope) = message_buffer[0].parse::<f32>() {
                self.envelope = envelope;
            } else {
                tracing::warn!("Compressor: Invalid envelope value in message buffer: {}", message_buffer[0]);
            }
        }

        // Smooth current_envelope to envelope
        let smoothing_factor = 0.5; // based on refresh rate and envelope update rate
        self.current_envelope = self.current_envelope * (1.0 - smoothing_factor) + self.envelope * smoothing_factor;

        let pedal_rect = ui.available_rect_before_wrap();
        ui.add(egui::Image::new(include_image!("images/compressor_bg.png")));

        let mut to_change = None;
        let ratio_param = self.get_parameters().get("Ratio").unwrap();
        if let Some(value) = pedal_knob(ui, "", "Ratio", ratio_param, egui::Vec2::new(0.0625, 0.03), 0.25, self.id) {
            to_change = Some(("Ratio".to_string(), value));
        }

        let threshold_param = self.get_parameters().get("Threshold").unwrap();
        if let Some(value) = pedal_knob(ui, "", "Threshold", threshold_param, egui::Vec2::new(0.375, 0.014), 0.25, self.id) {
            to_change = Some(("Threshold".to_string(), value));
        }

        let level_param = self.get_parameters().get("Level").unwrap();
        if let Some(value) = pedal_knob(ui, "", "Level", level_param, egui::Vec2::new(0.6875, 0.03), 0.25, self.id) {
            to_change = Some(("Level".to_string(), value));
        }

        let attack_param = self.get_parameters().get("Attack").unwrap();
        if let Some(value) = pedal_knob(ui, "", "Attack", attack_param, egui::Vec2::new(0.09, 0.207), 0.2, self.id) {
            to_change = Some(("Attack".to_string(), value));
        }

        let release_param = self.get_parameters().get("Release").unwrap();
        if let Some(value) = pedal_knob(ui, "", "Release", release_param, egui::Vec2::new(0.3, 0.207), 0.2, self.id) {
            to_change = Some(("Release".to_string(), value));
        }

        let soft_knee_param = self.get_parameters().get("Soft Knee").unwrap();
        if let Some(value) = pedal_knob(ui, "", "Soft Knee", soft_knee_param, egui::Vec2::new(0.50, 0.207), 0.2, self.id) {
            to_change = Some(("Soft Knee".to_string(), value));
        }

        let dry_wet_param = self.get_parameters().get("Dry/Wet").unwrap();
        if let Some(value) = pedal_knob(ui, "", "Dry/Wet", dry_wet_param, egui::Vec2::new(0.71, 0.207), 0.2, self.id) {
            to_change = Some(("Dry/Wet".to_string(), value));
        }


        let compressor_graph_rect = egui::Rect::from_min_size(
            pedal_rect.min + Vec2::new(0.2*pedal_rect.width(), 0.36*pedal_rect.height()),
            Vec2::new(0.6*pedal_rect.width(), 0.365*pedal_rect.height())
        );
        let mut graph_ui = ui.new_child(UiBuilder::new().max_rect(compressor_graph_rect));

        draw_compressor_graph(
            &mut graph_ui,
            20.0*self.current_envelope.log10(), // dB conversion
            self.parameters["Threshold"].value.as_float().unwrap(),
            self.parameters["Ratio"].value.as_float().unwrap(),
            self.parameters["Soft Knee"].value.as_float().unwrap(),
        );

        let active_param = self.get_parameters().get("Active").unwrap().value.as_bool().unwrap();
        if let Some(value) = pedal_switch(ui, active_param, egui::Vec2::new(0.363, 0.77), 0.12) {
            to_change = Some(("Active".to_string(), PedalParameterValue::Bool(value)));
        }

        to_change
    }
}

/// Parameters are in db
fn draw_compressor_graph(
    ui: &mut egui::Ui,
    envelope: f32,
    threshold: f32,
    ratio: f32,
    soft_knee: f32,
) {
    let available = ui.available_rect_before_wrap();
    let size = available.width().min(available.height());

    // Center the square inside the available rect
    let x_offset = (available.width() - size) / 2.0;
    let y_offset = (available.height() - size) / 2.0;

    let graph_rect = egui::Rect::from_min_size(
        egui::pos2(available.min.x + x_offset, available.min.y + y_offset),
        egui::vec2(size, size),
    );
    ui.allocate_rect(graph_rect, egui::Sense::hover());

    ui.painter().rect_filled(graph_rect, 2.0, egui::Color32::from_black_alpha(50));

    let mut points = vec![];

    let step = 0.01;
    let knee_start_db = threshold - soft_knee / 2.0;
    let knee_end_db = threshold + soft_knee / 2.0;

    // Apply compressor to a dB value
    let x_db_to_y_db = |x_db: f32| -> f32 {
        if x_db <= knee_start_db {
            x_db
        } else if x_db >= knee_end_db {
            threshold + (x_db - threshold) / ratio
        } else {
            let delta = x_db - knee_start_db;
            x_db + ((1.0 / ratio - 1.0) * delta * delta) / (2.0 * soft_knee.max(EPS))
        }
    };

    // x_db goes from -30 dB to 0 dB
    for x_db in (0..=100).map(|i| (i as f32 * step * 30.0)-30.0) {
        let y_db = x_db_to_y_db(x_db);

        // convert back to linear for plotting
        let x = (x_db + 30.0) / 30.0;
        let y = (y_db + 30.0) / 30.0;

        let screen_x = graph_rect.left() + x * graph_rect.width();
        let screen_y = graph_rect.bottom() - y * graph_rect.height();
        points.push(egui::pos2(screen_x, screen_y));
    }

    // Draw compression curve
    ui.painter().add(egui::Shape::line(
        points,
        egui::Stroke::new(1.0, egui::Color32::WHITE),
    ));

    // Draw envelope indicator
    let envelope_y_db = x_db_to_y_db(envelope);
    let envelope_x_scaled = ((envelope + 30.0) / 30.0).clamp(0.0, 1.0);
    let envelope_y_scaled = ((envelope_y_db + 30.0) / 30.0).clamp(0.0, 1.0);
    let envelope_screen_x = graph_rect.left() + envelope_x_scaled * graph_rect.width();
    let envelope_screen_y = graph_rect.bottom() - envelope_y_scaled * graph_rect.height();
    let pos = egui::pos2(envelope_screen_x, envelope_screen_y);

    ui.painter().add(egui::Shape::circle_filled(pos, 3.0, egui::Color32::RED));
}