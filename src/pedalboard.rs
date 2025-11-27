use serde::{Deserialize, Serialize};
use crate::{pedals::{Pedal, PedalTrait}, unique_time_id};
use std::{fmt::Write, hash::Hash};

/// Can uniquely identify a parameter.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ParameterPath {
    pub pedalboard_id: u32,
    pub pedal_id: u32,
    pub parameter_name: String,
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
        write!(f, "Pedalboard {{ id: {}, name: {}, pedals count: {:?} }}", self.id, self.name, self.pedals.len())
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
            self.prepend_message.clear();
            // Each message from a pedal will be preprended with "pedalmsg<id> "
            if let Err(e) = write!(&mut self.prepend_message, "pedalmsg{} ", pedal.get_id()) {
                tracing::warn!("Failed to write prepend message: {}", e);
            }

            pedal.process_audio(buffer, &mut self.pedal_message_buffer);

            for message in &mut self.pedal_message_buffer {
                message.insert_str(0, &self.prepend_message);
            }

            message_buffer.append(&mut self.pedal_message_buffer);
        });
    }

    pub fn get_id(&self) -> u32 {
        self.id
    }
}
