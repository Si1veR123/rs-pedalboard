use serde::{Deserialize, Serialize};

use crate::dsp_algorithms::yin::{Yin, freq_to_note};

use super::PedalTrait;


pub struct Tuner {
    yin: Yin,
    params: std::collections::HashMap<String, super::PedalParameter>
}

// PLACEHOLDER
impl Clone for Tuner {
    fn clone(&self) -> Self {
        Self::new()
    }
}

/// PLACEHOLDER SERIALIZATION FOR TESTING
impl Serialize for Tuner {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str("Tuner")
    }
}

impl<'de> Deserialize<'de> for Tuner {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        Ok(Self::new())
    }
}

impl Tuner {
    pub fn new() -> Self {
        // E1
        let min_freq = 40;
        // G4
        let max_freq = 392;
        let threshold = 0.01;

        Self {
            yin: Yin::new(threshold, min_freq, max_freq, 48000),
            params: std::collections::HashMap::new(),
        }
    }
}

impl PedalTrait for Tuner {
    fn process_audio(&mut self, buffer: &mut [f32]) {
        let freq = self.yin.process_buffer(buffer);
        log::info!("Tuner note: {:?}, hz: {}", freq_to_note(freq), freq);
    }

    fn get_parameters(&self) -> &std::collections::HashMap<String, super::PedalParameter> {
        &self.params
    }

    fn get_parameters_mut(&mut self) -> &mut std::collections::HashMap<String, super::PedalParameter> {
        &mut self.params
    }
}
