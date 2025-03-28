use serde::{Deserialize, Serialize};
use crate::pedals::{Pedal, PedalTrait};


#[derive(Serialize, Deserialize)]
pub struct Pedalboard {
    pub pedals: Vec<Pedal>
}

impl Default for Pedalboard {
    fn default() -> Pedalboard {
        Pedalboard {
            pedals: Vec::new()
        }
    }
}

impl Pedalboard {
    pub fn new() -> Pedalboard {
        Self::default()
    }

    pub fn from_pedals(pedals: Vec<Pedal>) -> Pedalboard {
        Pedalboard { pedals }
    }

    pub fn process_audio(&mut self, buffer: &mut [f32]) {
        self.pedals.iter_mut().for_each(|pedal| pedal.process_audio(buffer));
    }
}
