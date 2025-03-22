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

    pub fn process_audio(&mut self, buffer: &mut [f32]) {
        for pedal in &mut self.pedals {
            pedal.process_audio(buffer);
        }
    }
}
