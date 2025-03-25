use super::{Pedal, PedalParameter};
use std::collections::HashMap;

pub struct BypassPedal(HashMap<String, PedalParameter>);

impl Default for BypassPedal {
    fn default() -> BypassPedal {
        BypassPedal(HashMap::new())
    }
}

impl Pedal for BypassPedal {
    fn process_audio(&mut self, buffer: &mut [f32]) {
        // Do nothing
    }

    fn get_properties(&self) -> &HashMap<String, PedalParameter> {
        &self.0
    }

    fn get_properties_mut(&mut self) -> &mut HashMap<String, PedalParameter> {
        &mut self.0
    }
}
