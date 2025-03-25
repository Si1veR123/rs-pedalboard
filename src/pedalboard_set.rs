use crate::pedalboard::Pedalboard;

pub struct PedalboardSet {
    pub pedalboards: Vec<Pedalboard>,
    pub active_pedalboard: usize
}

impl Default for PedalboardSet {
    fn default() -> PedalboardSet {
        PedalboardSet {
            pedalboards: Vec::new(),
            active_pedalboard: 0
        }
    }
}

impl PedalboardSet {
    pub fn set_active_pedalboard(&mut self, index: usize) {
        if index < self.pedalboards.len() {
            self.active_pedalboard = index;
        } else {
            log::error!("Pedalboard index out of bounds");
        }
    }

    pub fn process_audio(&mut self, buffer: &mut [f32]) {
        if self.pedalboards.is_empty() {
            return;
        }

        if self.active_pedalboard < self.pedalboards.len() {
            self.pedalboards[self.active_pedalboard].process_audio(buffer);
        } else {
            log::error!("Pedalboard index out of bounds");
            self.active_pedalboard = 0;
        }
    }
}
