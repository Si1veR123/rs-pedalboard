use std::collections::VecDeque;

#[derive(Clone)]
pub struct VariableDelayLine {
    pub buffer: VecDeque<f32>
}

impl VariableDelayLine {
    pub fn new(max_delay: usize) -> Self {
        VariableDelayLine {
            buffer: VecDeque::from_iter(std::iter::repeat(0.0).take(max_delay))
        }
    }

    // TODO: subsample delay with interpolation?
    pub fn get_sample(&mut self, delay: usize) -> f32 {
        let index = (self.buffer.len() - delay).max(0).min(self.buffer.len() - 1);

        self.buffer[index]
    }
}
