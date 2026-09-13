use crate::{dsp_algorithms::oscillator::Oscillator, pedals::PedalTrait};
use eframe::egui;
use serde::{Deserialize, Serialize};
use std::hash::Hash;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub enum PedalParameterValue {
    Float(f32),
    String(String),
    Bool(bool),
    Int(i16),
    Oscillator(Oscillator),
}

impl Hash for PedalParameterValue {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            PedalParameterValue::Float(value) => value.to_bits().hash(state),
            PedalParameterValue::String(value) => value.hash(state),
            PedalParameterValue::Bool(value) => value.hash(state),
            PedalParameterValue::Int(value) => value.hash(state),
            PedalParameterValue::Oscillator(osc) => osc.hash(state),
        }
    }
}

impl PedalParameterValue {
    pub fn as_float(&self) -> Option<f32> {
        match self {
            PedalParameterValue::Float(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            PedalParameterValue::String(value) => Some(value),
            _ => None,
        }
    }

    pub fn as_bool(&self) -> Option<bool> {
        match self {
            PedalParameterValue::Bool(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i16> {
        match self {
            PedalParameterValue::Int(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_oscillator(&self) -> Option<&Oscillator> {
        match self {
            PedalParameterValue::Oscillator(osc) => Some(osc),
            _ => None,
        }
    }

    pub fn as_oscillator_mut(&mut self) -> Option<&mut Oscillator> {
        match self {
            PedalParameterValue::Oscillator(osc) => Some(osc),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PedalParameter {
    pub value: PedalParameterValue,
    // min and max are used for floats and ints
    #[serde(skip_serializing_if = "Option::is_none")]
    pub min: Option<PedalParameterValue>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max: Option<PedalParameterValue>,
    // For floats only
    #[serde(skip_serializing_if = "Option::is_none")]
    pub step: Option<PedalParameterValue>,
}

impl PedalParameter {
    pub fn is_valid(&self, value: &PedalParameterValue) -> bool {
        match value {
            PedalParameterValue::Float(value) => {
                if let Some(PedalParameterValue::Float(min)) = self.min {
                    if *value < min {
                        return false;
                    }
                }
                if let Some(PedalParameterValue::Float(max)) = self.max {
                    if *value > max {
                        return false;
                    }
                }

                // Don't validate float step, but it can be used for hinting to UI

                true
            }
            PedalParameterValue::Int(value) => {
                if let Some(PedalParameterValue::Int(min)) = self.min {
                    if *value < min {
                        return false;
                    }
                }
                if let Some(PedalParameterValue::Int(max)) = self.max {
                    if *value > max {
                        return false;
                    }
                }
                true
            }
            _ => true,
        }
    }

    /// The range that can be used for relative / flip-flop updates.
    ///
    /// Float, Int and Oscillator parameters store their range in `min`/`max`.
    /// Bool parameters have no stored range, so their range is `false..=true`.
    /// Returns `None` for parameters that cannot be updated without a preset range (strings etc.).
    pub fn to_range(&self) -> Option<PedalParameterRange> {
        if let (Some(min), Some(max)) = (self.min.clone(), self.max.clone()) {
            return Some(PedalParameterRange { min, max });
        }

        match self.value {
            PedalParameterValue::Bool(_) => Some(PedalParameterRange {
                min: PedalParameterValue::Bool(false),
                max: PedalParameterValue::Bool(true),
            }),
            _ => None,
        }
    }

    pub fn int_to_float(&self) -> Self {
        if let PedalParameterValue::Int(value) = self.value {
            let new_parameter = PedalParameter {
                value: PedalParameterValue::Float(value as f32),
                min: Some(PedalParameterValue::Float(
                    self.min.clone().unwrap().as_int().unwrap() as f32,
                )),
                max: Some(PedalParameterValue::Float(
                    self.max.clone().unwrap().as_int().unwrap() as f32,
                )),
                step: None,
            };
            new_parameter
        } else {
            panic!("PedalParameter::int_to_float called on non-int parameter");
        }
    }

    pub fn float_to_int(&self) -> Self {
        if let PedalParameterValue::Float(value) = self.value {
            let new_parameter = PedalParameter {
                value: PedalParameterValue::Int(value as i16),
                min: Some(PedalParameterValue::Int(
                    self.min.clone().unwrap().as_float().unwrap() as i16,
                )),
                max: Some(PedalParameterValue::Int(
                    self.max.clone().unwrap().as_float().unwrap() as i16,
                )),
                step: None,
            };
            new_parameter
        } else {
            panic!("PedalParameter::float_to_int called on non-float parameter");
        }
    }

    pub fn parameter_editor_ui(
        &self,
        ui: &mut egui::Ui,
    ) -> egui::InnerResponse<Option<PedalParameterValue>> {
        let width = ui.available_width() * 0.8;
        let mut to_change = None;

        let response = match self.value {
            PedalParameterValue::Float(mut f) => {
                let init_value = f;
                let min = self.min.clone().unwrap().as_float().unwrap_or(0.0);
                let max = self.max.clone().unwrap().as_float().unwrap_or(1.0);
                let response = ui.add(egui::Slider::new(&mut f, min..=max).max_decimals(2));
                f = f.clamp(min, max);
                if f != init_value {
                    to_change = Some(PedalParameterValue::Float(f));
                }
                response
            }
            PedalParameterValue::Bool(mut b) => {
                let init_value = b;
                let response = ui.checkbox(&mut b, "");
                if b != init_value {
                    to_change = Some(PedalParameterValue::Bool(b));
                }
                response
            }
            PedalParameterValue::Int(mut i) => {
                let init_value = i;
                let min = self.min.clone().unwrap().as_int().unwrap_or(0);
                let max = self.max.clone().unwrap().as_int().unwrap_or(100);
                let response = ui.add(egui::Slider::new(&mut i, min..=max));

                if i != init_value {
                    to_change = Some(PedalParameterValue::Int(i));
                }

                response
            }
            PedalParameterValue::Oscillator(_) => {
                let inner_response =
                    super::ui::oscillator_selection_window(ui, &self, width, false);
                if let Some(oscillator) = inner_response.inner {
                    to_change = Some(PedalParameterValue::Oscillator(oscillator));
                }
                inner_response.response
            }

            PedalParameterValue::String(_) => {
                let mut text = self.value.as_str().unwrap().to_string();
                let response = ui.text_edit_singleline(&mut text);
                if response.changed() {
                    to_change = Some(PedalParameterValue::String(text));
                };
                response
            }
        };

        egui::InnerResponse::new(to_change, response)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PedalParameterRange {
    pub min: PedalParameterValue,
    pub max: PedalParameterValue,
}

impl PedalParameterRange {
    pub fn parameter_from_interp(&self, value: f32) -> PedalParameterValue {
        match self.min {
            PedalParameterValue::Float(min) => {
                let max = self.max.as_float().unwrap_or(min);
                PedalParameterValue::Float(min + (max - min) * value)
            }
            PedalParameterValue::Int(min) => {
                let max = self.max.as_int().unwrap_or(min);
                PedalParameterValue::Int(min + ((max - min) as f32 * value).round() as i16)
            }
            PedalParameterValue::Bool(_)
            | PedalParameterValue::Oscillator(_)
            | PedalParameterValue::String(_) => {
                if value >= 0.5 {
                    self.max.clone()
                } else {
                    self.min.clone()
                }
            }
        }
    }
}

/// Represents a change to a pedal parameter.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ParameterUpdate {
    // Float value is -1-1, where 0 is no change and 1 is full change to max value
    Relative(f32, Option<PedalParameterRange>),
    Absolute(PedalParameterValue),
    FlipFlop(Option<PedalParameterRange>),
}

impl ParameterUpdate {
    pub fn fill_missing_range(&mut self, parameter: &PedalParameter) {
        let range = match self {
            ParameterUpdate::Relative(_, range) | ParameterUpdate::FlipFlop(range) => range,
            ParameterUpdate::Absolute(_) => return,
        };

        if range.is_none() {
            *range = parameter.to_range();
        }
    }

    /// If this update carries an oscillator, make sure it uses the given sample rate.
    /// Used by the processor so oscillators sent from the client are correct for the processor's sample rate.
    pub fn set_oscillator_sample_rate(&mut self, sample_rate: f32) {
        if let ParameterUpdate::Absolute(PedalParameterValue::Oscillator(oscillator)) = self {
            oscillator.set_sample_rate(sample_rate);
        }
    }

    pub fn apply_to_pedal(&mut self, pedal: &mut dyn PedalTrait, parameter_name: &str) {
        if let Some(parameter) = pedal.get_parameters().get(parameter_name) {
            if let Some(new_value) = self.apply_to_parameter(parameter) {
                pedal.set_parameter_value(parameter_name, new_value);
            }
        } else {
            tracing::warn!(
                "ParameterUpdate::apply_to_pedal called with non-existent parameter name: {}",
                parameter_name
            );
        }
    }

    /// Applies this update to the given parameter, returning the new value.
    /// Returns `None` if the parameter cannot be updated (missing range, invalid value, or no change).
    pub fn apply_to_parameter(
        &mut self,
        parameter: &PedalParameter,
    ) -> Option<PedalParameterValue> {
        self.fill_missing_range(parameter);

        match self {
            ParameterUpdate::Relative(delta_frac, Some(range)) => {
                if *delta_frac == 0.0 {
                    return None;
                }

                match &parameter.value {
                    PedalParameterValue::Float(value) => {
                        let min = range.min.as_float().expect("Min value must be a float");
                        let max = range.max.as_float().expect("Max value must be a float");
                        let delta = (max - min) * *delta_frac;
                        Some(PedalParameterValue::Float((*value + delta).clamp(min, max)))
                    }
                    PedalParameterValue::Oscillator(oscillator) => {
                        let min = range.min.as_float().expect("Min value must be a float");
                        let max = range.max.as_float().expect("Max value must be a float");
                        let delta = (max - min) * *delta_frac;
                        let mut oscillator = oscillator.clone();
                        oscillator
                            .set_frequency((oscillator.get_frequency() + delta).clamp(min, max));
                        Some(PedalParameterValue::Oscillator(oscillator))
                    }
                    PedalParameterValue::Int(value) => {
                        let min = range.min.as_int().expect("Min value must be an int");
                        let max = range.max.as_int().expect("Max value must be an int");
                        // Round away from zero so increments and decrements both move by at least one step
                        let magnitude =
                            ((max - min) as f32 * delta_frac.abs()).ceil().max(1.0) as i16;
                        let delta = magnitude * delta_frac.signum() as i16;
                        Some(PedalParameterValue::Int((*value + delta).clamp(min, max)))
                    }
                    PedalParameterValue::Bool(_) | PedalParameterValue::String(_) => {
                        if *delta_frac > 0.0 {
                            Some(range.max.clone())
                        } else {
                            Some(range.min.clone())
                        }
                    }
                }
            }
            ParameterUpdate::Absolute(value) => {
                if parameter.is_valid(value) {
                    Some(value.clone())
                } else {
                    None
                }
            }
            ParameterUpdate::FlipFlop(Some(range)) => match &parameter.value {
                PedalParameterValue::Float(value) => {
                    let min = range.min.as_float().expect("Min value must be a float");
                    let max = range.max.as_float().expect("Max value must be a float");
                    let is_high = (max - min != 0.0) && (*value - min) / (max - min) >= 0.5;
                    Some(PedalParameterValue::Float(if is_high { min } else { max }))
                }
                PedalParameterValue::Oscillator(oscillator) => {
                    let min = range.min.as_float().expect("Min value must be a float");
                    let max = range.max.as_float().expect("Max value must be a float");
                    let frequency = oscillator.get_frequency();
                    let is_high = (max - min != 0.0) && (frequency - min) / (max - min) >= 0.5;
                    let mut oscillator = oscillator.clone();
                    oscillator.set_frequency(if is_high { min } else { max });
                    Some(PedalParameterValue::Oscillator(oscillator))
                }
                PedalParameterValue::Int(value) => {
                    let min = range.min.as_int().expect("Min value must be an int");
                    let max = range.max.as_int().expect("Max value must be an int");
                    let is_high = max != min && ((*value - min) as f32 / (max - min) as f32) >= 0.5;
                    Some(PedalParameterValue::Int(if is_high { min } else { max }))
                }
                PedalParameterValue::Bool(_) | PedalParameterValue::String(_) => {
                    if parameter.value == range.min {
                        Some(range.max.clone())
                    } else {
                        Some(range.min.clone())
                    }
                }
            },
            // Branches may not match if the range is not provided and a parameter doesn't have a range.
            // This isn't an error, but some parameters cannot be updated without preset ranges (string fields etc.)
            _ => {
                tracing::info!(
                    "ParameterUpdate::apply_to_parameter called with missing range for parameter {:?}",
                    parameter
                );
                None
            }
        }
    }
}

pub enum ParameterUILocation {
    Pedal,
    ParameterWindow,
    MidiMin,
    MidiMax,
}
