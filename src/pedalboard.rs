use crate::pedals::Pedal;


pub struct Pedalboard {
    pub pedals: Vec<Box<dyn Pedal>>
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

    pub fn from_pedals(pedals: Vec<Box<dyn Pedal>>) -> Pedalboard {
        Pedalboard { pedals }
    }

    pub fn process_audio(&mut self, buffer: &mut [f32]) {
        self.pedals.iter_mut().for_each(|pedal| pedal.process_audio(buffer));
    }
}
