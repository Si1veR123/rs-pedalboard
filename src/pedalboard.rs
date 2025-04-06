use serde::{Deserialize, Serialize};
use crate::pedals::{Pedal, PedalTrait};


#[derive(Serialize, Deserialize, Clone)]
pub struct Pedalboard {
    pub name: String,
    pub pedals: Vec<Pedal>
}

impl Default for Pedalboard {
    fn default() -> Pedalboard {
        Pedalboard {
            name: String::from("New Pedalboard"),
            pedals: vec![Pedal::PitchShift(crate::pedals::PitchShift::new())]
        }
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
