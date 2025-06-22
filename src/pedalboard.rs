use serde::{Deserialize, Serialize};
use crate::pedals::{Pedal, PedalTrait};
use std::{fmt::Write, hash::Hash};


#[derive(Serialize, Deserialize, Clone)]
pub struct Pedalboard {
    pub name: String,
    pub pedals: Vec<Pedal>,

    prepend_message: String,
    pedal_message_buffer: Vec<String>,
}

impl Default for Pedalboard {
    fn default() -> Pedalboard {
        Pedalboard {
            name: String::from("Default Pedalboard"),
            pedals: vec![Pedal::Volume(crate::pedals::Volume::new())],
            prepend_message: String::new(),
            pedal_message_buffer: Vec::with_capacity(12),
        }
    }
}

impl Hash for &Pedalboard {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl Pedalboard {
    /// Has a volume pedal by default
    pub fn new(name: String) -> Pedalboard {
        Self {
            name,
            pedals: vec![Pedal::Volume(crate::pedals::Volume::new())],
            ..Default::default()
        }
    }

    pub fn from_pedals(name: String, pedals: Vec<Pedal>) -> Pedalboard {
        Pedalboard {
            name,
            pedals,
            ..Default::default()
        }
    }

    pub fn process_audio(&mut self, buffer: &mut [f32], message_buffer: &mut Vec<String>) {
        self.pedals.iter_mut().enumerate().for_each(|(i, pedal)| {
            // Clear the message buffer for each pedal
            self.pedal_message_buffer.clear();
            self.prepend_message.clear();
            // Each message from a pedal will be preprended with "pedalmsg<i> "
            if let Err(e) = write!(&mut self.prepend_message, "pedalmsg{} ", i) {
                log::warn!("Failed to write prepend message: {}", e);
            }

            pedal.process_audio(buffer, &mut self.pedal_message_buffer);

            for message in &mut self.pedal_message_buffer {
                message.insert_str(0, &self.prepend_message);
            }

            message_buffer.append(&mut self.pedal_message_buffer);
        });
    }
}
