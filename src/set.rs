use log::error;

use crate::pedalboard::Pedalboard;


pub struct Set {
    pub pedalboards: Vec<Pedalboard>,
    active_pedalboard: usize
}

impl Default for Set {
    fn default() -> Set {
        Set {
            pedalboards: Vec::new(),
            active_pedalboard: 0
        }
    }
}

impl Set {
    pub fn process_audio(&mut self, buffer: &mut [f32]) {
        let pedalboard = self.pedalboards.get_mut(self.active_pedalboard);
        if let Some(pedalboard) = pedalboard {
            pedalboard.process_audio(buffer);
        } else if self.pedalboards.len() > 0 {
            error!("Active pedalboard not found, defaulting to 0");
            self.active_pedalboard = 0;
            self.pedalboards[0].process_audio(buffer);
        }
    }
}
