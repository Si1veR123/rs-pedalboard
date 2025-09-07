use crate::pedalboard::Pedalboard;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PedalboardSet {
    pub pedalboards: Vec<Pedalboard>,
    pub active_pedalboard: usize
}

impl Default for PedalboardSet {
    fn default() -> PedalboardSet {
        PedalboardSet {
            pedalboards: vec![Pedalboard::default()],
            active_pedalboard: 0
        }
    }
}

#[derive(Debug)]
pub struct EmptyPedalboardSetError;
impl std::fmt::Display for EmptyPedalboardSetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Pedalboard set is empty")
    }
}

impl PedalboardSet {
    pub fn from_pedalboards(pedalboards: Vec<Pedalboard>) -> Result<PedalboardSet, EmptyPedalboardSetError> {
        if pedalboards.is_empty() {
            return Err(EmptyPedalboardSetError);
        }

        Ok(PedalboardSet {
            pedalboards,
            active_pedalboard: 0
        })
    }

    /// Return true if it was removed
    pub fn remove_pedalboard(&mut self, index: usize) -> bool {
        if index < self.pedalboards.len() {
            if self.pedalboards.len() > 1 {
                self.pedalboards.remove(index);
                if self.active_pedalboard >= self.pedalboards.len() {
                    self.active_pedalboard = self.pedalboards.len() - 1;
                }
                true
            } else {
                log::error!("Cannot remove the last pedalboard");
                false
            }
        } else {
            log::error!("Pedalboard index out of bounds");
            false
        }
    }

    pub fn set_active_pedalboard(&mut self, index: usize) {
        if index < self.pedalboards.len() {
            self.active_pedalboard = index;
        } else {
            log::error!("Pedalboard index out of bounds");
        }
    }

    pub fn process_audio(&mut self, buffer: &mut [f32], message_buffer: &mut Vec<String>) {
        if self.pedalboards.is_empty() {
            return;
        }

        if self.active_pedalboard < self.pedalboards.len() {
            self.pedalboards[self.active_pedalboard].process_audio(buffer, message_buffer);
        } else {
            log::error!("Pedalboard index out of bounds");
            self.active_pedalboard = 0;
        }
    }
}
