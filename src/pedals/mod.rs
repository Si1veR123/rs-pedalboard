pub mod bypass_pedal;

use std::collections::HashMap;

pub trait Pedal {
    fn process_audio(&mut self, buffer: &mut [f32]);

    fn get_properties(&self) -> &HashMap<String, f32>;
    fn get_properties_mut(&mut self) -> &mut HashMap<String, f32>;

    fn update_property(&mut self, key: &str, value: f32) {
        if self.get_properties().contains_key(key) {
            self.get_properties_mut().insert(key.to_string(), value);
        }
    }
    fn get_property(&self, key: &str) -> f32 {
        match self.get_properties().get(key) {
            Some(value) => *value,
            None => 0.0,
        }
    }

    fn get_functions(&self) -> &[String] {
        &[]
    }
    fn activate_function(&mut self, _function: &str) {
        // Do nothing by default
    }
}
