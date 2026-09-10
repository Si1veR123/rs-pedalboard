use std::{collections::HashMap, hash::Hash};

use eframe::egui::{self, include_image, Button, Color32, Image};
use serde::{Deserialize, Serialize};

use super::{eq_background, gain_knob};
use crate::{
    dsp_algorithms::biquad::BiquadFilter,
    pedals::{PedalParameter, PedalParameterValue, PedalTrait},
    unique_time_id,
};

const MAX_GAIN_DB: f32 = 15.0;
const HIGH_PASS_FREQ: f32 = 80.0;
const LOW_SHELF_FREQ: f32 = 150.0;
const MID_FREQ: f32 = 1000.0;
const HIGH_SHELF_FREQ: f32 = 4500.0;
const LOW_PASS_FREQ: f32 = 9000.0;

#[derive(Serialize, Deserialize, Clone)]
pub struct SimpleEq {
    parameters: HashMap<String, PedalParameter>,
    #[serde(skip)]
    filters: Option<Vec<BiquadFilter>>,
    #[serde(skip)]
    sample_rate: f32,
    id: u32,
}

impl Hash for SimpleEq {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl SimpleEq {
    pub fn new() -> Self {
        let mut parameters = HashMap::new();
        for (name, value) in [("Low Gain", 0.0), ("Mid Gain", 0.0), ("High Gain", 0.0)] {
            parameters.insert(
                name.to_string(),
                PedalParameter {
                    value: PedalParameterValue::Float(value),
                    min: Some(PedalParameterValue::Float(-MAX_GAIN_DB)),
                    max: Some(PedalParameterValue::Float(MAX_GAIN_DB)),
                    step: Some(PedalParameterValue::Float(0.1)),
                },
            );
        }

        parameters.insert(
            "High Pass".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(HIGH_PASS_FREQ),
                min: Some(PedalParameterValue::Float(20.0)),
                max: Some(PedalParameterValue::Float(2000.0)),
                step: Some(PedalParameterValue::Float(1.0)),
            },
        );
        parameters.insert(
            "High Pass Enabled".to_string(),
            PedalParameter {
                value: PedalParameterValue::Bool(true),
                min: None,
                max: None,
                step: None,
            },
        );
        parameters.insert(
            "Low Pass".to_string(),
            PedalParameter {
                value: PedalParameterValue::Float(LOW_PASS_FREQ),
                min: Some(PedalParameterValue::Float(2000.0)),
                max: Some(PedalParameterValue::Float(20000.0)),
                step: Some(PedalParameterValue::Float(1.0)),
            },
        );
        parameters.insert(
            "Low Pass Enabled".to_string(),
            PedalParameter {
                value: PedalParameterValue::Bool(true),
                min: None,
                max: None,
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

        Self {
            parameters,
            filters: None,
            sample_rate: 48000.0,
            id: unique_time_id(),
        }
    }

    fn parameter_float(&self, name: &str) -> f32 {
        self.parameters[name].value.as_float().unwrap()
    }

    fn parameter_bool(&self, name: &str) -> bool {
        self.parameters
            .get(name)
            .and_then(|parameter| parameter.value.as_bool())
            .unwrap_or(true)
    }

    fn build_filters(&self, sample_rate: f32) -> Vec<BiquadFilter> {
        let nyquist = sample_rate * 0.49;
        let high_pass = self.parameter_float("High Pass").min(nyquist);
        let low_pass = self
            .parameter_float("Low Pass")
            .min(nyquist)
            .max(high_pass + 1.0);

        let mut filters = Vec::with_capacity(5);
        if self.parameter_bool("High Pass Enabled") {
            filters.push(BiquadFilter::high_pass(high_pass, sample_rate, 0.707));
        }
        filters.extend([
            BiquadFilter::low_shelf(
                LOW_SHELF_FREQ.min(nyquist),
                sample_rate,
                0.707,
                self.parameter_float("Low Gain"),
            ),
            BiquadFilter::peaking(
                MID_FREQ.min(nyquist),
                sample_rate,
                0.707,
                self.parameter_float("Mid Gain"),
            ),
            BiquadFilter::high_shelf(
                HIGH_SHELF_FREQ.min(nyquist),
                sample_rate,
                0.707,
                self.parameter_float("High Gain"),
            ),
        ]);
        if self.parameter_bool("Low Pass Enabled") {
            filters.push(BiquadFilter::low_pass(low_pass, sample_rate, 0.707));
        }
        filters
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = unique_time_id();
        cloned.filters = None;
        cloned
    }
}

impl PedalTrait for SimpleEq {
    fn process_audio(&mut self, buffer: &mut [f32], _message_buffer: &mut Vec<String>) {
        if self.filters.is_none() {
            self.filters = Some(self.build_filters(self.sample_rate));
        }

        for sample in buffer.iter_mut() {
            for filter in self.filters.as_mut().unwrap() {
                *sample = filter.process(*sample);
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
        if !self.parameters.contains_key(name)
            && matches!(name, "High Pass Enabled" | "Low Pass Enabled")
        {
            self.parameters.insert(
                name.to_string(),
                PedalParameter {
                    value: PedalParameterValue::Bool(true),
                    min: None,
                    max: None,
                    step: None,
                },
            );
        }

        if let Some(parameter) = self.parameters.get_mut(name) {
            if parameter.is_valid(&value) {
                parameter.value = value;
                self.filters = None;
            }
        }
    }

    fn set_config(&mut self, _buffer_size: usize, sample_rate: u32) {
        self.sample_rate = sample_rate as f32;
        self.filters = Some(self.build_filters(self.sample_rate));
    }

    fn get_id(&self) -> u32 {
        self.id
    }

    fn ui(
        &mut self,
        ui: &mut egui::Ui,
        _message_buffer: &[String],
    ) -> Option<(String, PedalParameterValue)> {
        let pedal_rect = ui.available_rect_before_wrap();
        eq_background(ui);
        let mut changed = None;
        let pedal_size = pedal_rect.size();
        let high_pass_enabled = self.parameter_bool("High Pass Enabled");
        let low_pass_enabled = self.parameter_bool("Low Pass Enabled");

        ui.horizontal(|ui| {
            ui.style_mut().visuals.widgets.inactive.weak_bg_fill = Color32::from_black_alpha(50);
            ui.style_mut().visuals.widgets.hovered.weak_bg_fill = Color32::from_black_alpha(80);
            ui.columns_const(|[left, _, right]| {
                for (column, path, enabled, name, label) in [
                    (
                        left,
                        include_image!("../images/eq/low_shelf.png"),
                        high_pass_enabled,
                        "High Pass",
                        "High Pass Enabled",
                    ),
                    (
                        right,
                        include_image!("../images/eq/high_shelf.png"),
                        low_pass_enabled,
                        "Low Pass",
                        "Low Pass Enabled",
                    ),
                ] {
                    if column
                        .centered_and_justified(|ui| {
                            ui.add(
                                Button::image(Image::new(path).max_width(pedal_size.x * 0.15))
                                    .corner_radius(3.0)
                                    .selected(enabled),
                            )
                            .on_hover_text(name)
                            .clicked()
                        })
                        .inner
                    {
                        changed = Some((label.to_string(), PedalParameterValue::Bool(!enabled)));
                    }
                }
            });
        });

        ui.add_space(4.0);
        ui.allocate_ui_with_layout(
            egui::Vec2::new(ui.available_width(), ui.available_height()),
            egui::Layout::top_down(egui::Align::Center),
            |ui| {
                let knob_width = (pedal_size.x / 8.0).min(140.0);
                ui.columns(5, |columns| {
                    for (column, (name, enabled)) in columns.iter_mut().zip([
                        ("High Pass", high_pass_enabled),
                        ("Low Gain", true),
                        ("Mid Gain", true),
                        ("High Gain", true),
                        ("Low Pass", low_pass_enabled),
                    ]) {
                        column.vertical_centered(|ui| {
                            if enabled {
                                if let Some(value) = gain_knob(
                                    ui,
                                    self.parameters.get(name).unwrap(),
                                    knob_width,
                                ) {
                                    changed = Some((
                                        name.to_string(),
                                        PedalParameterValue::Float(value),
                                    ));
                                }
                            }
                        });
                    }
                });
            },
        );

        changed
    }
}
