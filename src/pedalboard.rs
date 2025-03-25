use crate::pedals::Pedal;


pub struct Pedalboard<T> {
    pub pedals: Vec<Box<dyn Pedal<T>>>
}

impl<T> Default for Pedalboard<T> {
    fn default() -> Pedalboard<T> {
        Pedalboard {
            pedals: Vec::new()
        }
    }
}

impl<T> Pedalboard<T> {
    pub fn new() -> Pedalboard<T> {
        Self::default()
    }

    pub fn process_audio(&mut self, buffer: &mut [T]) {
        self.pedals.iter_mut().for_each(|pedal| pedal.process_audio(buffer));
    }
}
