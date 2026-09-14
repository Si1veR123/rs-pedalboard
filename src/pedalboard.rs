use crate::{
    pedalboard_set, pedals::{Pedal, PedalParameterValue, PedalTrait}, processor_settings::FloatSettingUpdate, unique_time_id,
};
use serde::{Deserialize, Serialize};
use std::{fmt::Write, hash::Hash};

/// Can uniquely identify a parameter.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ParameterPath {
    pub pedalboard_id: Option<u32>,
    pub pedal_id: u32,
    pub parameter_name: String,
}

impl ParameterPath {
    /// Find the pedalboard that contains the pedal with this path's pedal ID and set
    /// `pedalboard_id`.
    ///
    /// The active pedalboard is preferred, which disambiguates the case where the same pedal ID
    /// appears in several pedalboards. Note that pedal IDs are only unique within a pedalboard,
    /// not across pedalboards (for example, duplicated pedalboards keep their pedal IDs).
    ///
    /// Returns whether the path now points at a pedalboard in the set.
    pub fn resolve_pedalboard_id(
        &mut self,
        pedalboard_set: &pedalboard_set::PedalboardSet,
    ) -> bool {
        if self.pedalboard_id.is_none() {
            let pedal_id = self.pedal_id;
            let contains_pedal =
                |pedalboard: &&Pedalboard| pedalboard.pedals.iter().any(|p| p.get_id() == pedal_id);

            // Prefer the active pedalboard so ambiguous pedal IDs resolve to the board in view.
            let active_pedalboard = pedalboard_set
                .pedalboards
                .get(pedalboard_set.active_pedalboard)
                .filter(|pedalboard| contains_pedal(pedalboard));

            let containing_pedalboard = active_pedalboard.or_else(|| {
                pedalboard_set
                    .pedalboards
                    .iter()
                    .find(|pedalboard| contains_pedal(pedalboard))
            });

            self.pedalboard_id = containing_pedalboard.map(|pedalboard| pedalboard.get_id());
        }

        self.pedalboard_id.is_some()
    }
}

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Pedalboard {
    // This ID is not necessarily unique in a PedalboardSet,
    // however Pedalboards with the same ID should be functionally equal
    // (same pedals, same parameters, same name, etc)
    #[serde(default)]
    id: u32,
    #[serde(default)]
    pub name: String,
    pub pedals: Vec<Pedal>,

    #[serde(skip)]
    prepend_message: String,
    #[serde(skip)]
    pedal_message_buffer: Vec<String>,
}

impl std::fmt::Debug for Pedalboard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Pedalboard {{ id: {}, name: {}, pedals count: {:?} }}",
            self.id,
            self.name,
            self.pedals.len()
        )
    }
}

impl Default for Pedalboard {
    fn default() -> Pedalboard {
        Pedalboard {
            id: unique_time_id(),
            name: String::from("Default Pedalboard"),
            pedals: vec![Pedal::Volume(crate::pedals::Volume::new())],
            prepend_message: String::new(),
            pedal_message_buffer: Vec::with_capacity(12),
        }
    }
}

impl Hash for &Pedalboard {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl Pedalboard {
    /// Has a volume pedal by default
    pub fn new(name: String) -> Pedalboard {
        Self {
            name,
            ..Default::default()
        }
    }

    pub fn clone_with_new_id(&self) -> Self {
        let mut cloned = self.clone();
        cloned.id = unique_time_id();
        cloned
    }

    pub fn from_pedals(name: String, pedals: Vec<Pedal>) -> Pedalboard {
        Pedalboard {
            id: unique_time_id(),
            name,
            pedals,
            ..Default::default()
        }
    }

    pub fn process_audio(&mut self, buffer: &mut [f32], message_buffer: &mut Vec<String>) {
        self.pedals.iter_mut().for_each(|pedal| {
            if !pedal.is_active() {
                return;
            }

            // Clear the message buffer for each pedal
            self.pedal_message_buffer.clear();

            pedal.process_audio(buffer, &mut self.pedal_message_buffer);

            // only build the prefix if the pedal sent a message
            if !self.pedal_message_buffer.is_empty() {
                self.prepend_message.clear();
                if let Err(e) = write!(&mut self.prepend_message, "pedalmsg{} ", pedal.get_id()) {
                    tracing::warn!("Failed to write prepend message: {}", e);
                }

                for message in &mut self.pedal_message_buffer {
                    message.insert_str(0, &self.prepend_message);
                }
            }

            message_buffer.append(&mut self.pedal_message_buffer);
        });
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }

    pub fn sensible_parameter_from_index(&self, index: usize, update: FloatSettingUpdate) -> Option<ParameterPath> {
        // Find a parameter in a fixed 'sensible parameter' order
        
        match update {
            // For FlipFlop update find an 'Active' parameter
            FloatSettingUpdate::FlipFlop => {
                // Find the active parameter of the pedal at given index
                if self.pedals.len() > index {
                    let pedal = &self.pedals[index];
                    if let Some(_parameter) = pedal.get_parameters().get("Active") {
                        return Some(ParameterPath {
                            pedalboard_id: Some(self.id),
                            pedal_id: pedal.get_id(),
                            parameter_name: "Active".to_string(),
                        });
                    }
                }
            }
            // For other updates, find a numerical parameter
            _ => {
                // Order of parameters likely to be of 'high importance'
                let prioritise_parameters = ["Gain", "Drive", "Level", "Dry/Wet", "Tone"];
                let mut priority_parameters_found = 0;

                for pedal in &self.pedals {
                    for param_name in prioritise_parameters.iter() {
                        if let Some(_parameter) = pedal.get_parameters().get(*param_name) {
                            priority_parameters_found += 1;
                            if priority_parameters_found - 1 == index {
                                return Some(ParameterPath {
                                    pedalboard_id: Some(self.id),
                                    pedal_id: pedal.get_id(),
                                    parameter_name: param_name.to_string(),
                                });
                            }
                        }
                    }
                }

                // Fallback to first numerical parameter
                let mut numerical_parameters_found = 0;
                for pedal in &self.pedals {
                    for (param_name, parameter) in pedal.get_parameters() {
                        if matches!(&parameter.value, PedalParameterValue::Float(_) | PedalParameterValue::Int(_)) {
                            numerical_parameters_found += 1;
                            if numerical_parameters_found - 1 == index {
                                return Some(ParameterPath {
                                    pedalboard_id: Some(self.id),
                                    pedal_id: pedal.get_id(),
                                    parameter_name: param_name.clone(),
                                });
                            }
                        }
                    }
                }
            }
        }

        None
    }
}
