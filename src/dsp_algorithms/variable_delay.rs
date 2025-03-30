use std::collections::VecDeque;

#[derive(Clone)]
pub struct VariableDelay {
    pub buffer: VecDeque<f32>
}

impl VariableDelay {
    pub fn new(max_delay: usize) -> Self {
        VariableDelay {
            buffer: VecDeque::from_iter(std::iter::repeat(0.0).take(max_delay))
        }
    }

    pub fn process_sample(&mut self, sample: f32, delay: usize) -> f32 {
        self.buffer.pop_front();
        self.buffer.push_back(sample);

        let index = (self.buffer.len() - delay).max(0).min(self.buffer.len() - 1);

        self.buffer[index]
    }
}
