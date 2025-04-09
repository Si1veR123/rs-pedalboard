use serde::{Deserialize, Serialize};
use crate::pedals::{Pedal, PedalTrait};
use std::hash::Hash;


#[derive(Serialize, Deserialize, Clone)]
pub struct Pedalboard {
    pub name: String,
    pub pedals: Vec<Pedal>
}

impl Default for Pedalboard {
    fn default() -> Pedalboard {
        Pedalboard {
            name: String::from("Default Pedalboard"),
            pedals: vec![Pedal::Tuner(crate::pedals::Tuner::new())]
        }
    }
}

impl Hash for &Pedalboard {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.name.hash(state);
    }
}

impl Pedalboard {
    pub fn new(name: String) -> Pedalboard {
        Self {
            name,
            pedals: Vec::new()
        }
    }

    pub fn from_pedals(name: String, pedals: Vec<Pedal>) -> Pedalboard {
        Pedalboard { name, pedals }
    }

    pub fn process_audio(&mut self, buffer: &mut [f32]) {
        self.pedals.iter_mut().for_each(|pedal| pedal.process_audio(buffer));
    }
}
