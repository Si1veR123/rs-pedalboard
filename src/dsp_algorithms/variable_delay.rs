use std::collections::VecDeque;

#[derive(Clone)]
pub struct VariableDelayLine {
    pub buffer: VecDeque<f32>
}

impl VariableDelayLine {
    pub fn new(max_delay: usize) -> Self {
        VariableDelayLine {
            buffer: VecDeque::from_iter(std::iter::repeat(0.0).take(max_delay+1)) // add 1 for linear interpolation
        }
    }

    pub fn max_delay(&self) -> f32 {
        (self.buffer.len() - 1) as f32
    }

    pub fn get_sample(&mut self, delay: f32) -> f32 {
        let prev_int_index = (self.buffer.len() - delay.floor() as usize).max(0).min(self.buffer.len() - 1);
        let next_int_index =  (self.buffer.len() - delay.ceil() as usize).max(0).min(self.buffer.len() - 1);
        let prev_value = self.buffer[prev_int_index];
        let next_value = self.buffer[next_int_index];
        let interpolation = prev_value + delay.fract() * (next_value - prev_value);
        interpolation
    }

    pub fn reset(&mut self) {
        self.buffer.iter_mut().for_each(|s| *s = 0.0);
    }
}