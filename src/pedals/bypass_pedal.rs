use super::Pedal;
use std::collections::HashMap;

pub struct BypassPedal(HashMap<String, f32>);

impl Default for BypassPedal {
    fn default() -> BypassPedal {
        BypassPedal(HashMap::new())
    }
}

impl Pedal for BypassPedal {
    fn process_audio(&mut self, _buffer: &mut [f32]) {
        // Do nothing
    }

    fn get_properties(&self) -> &HashMap<String, f32> {
        &self.0
    }

    fn get_properties_mut(&mut self) -> &mut HashMap<String, f32> {
        &mut self.0
    }
}
