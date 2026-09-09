use std::collections::VecDeque;

#[derive(Clone)]
pub struct VariableDelayLine {
    pub buffer: VecDeque<f32>,
}

impl VariableDelayLine {
    pub fn new(max_delay: usize) -> Self {
        VariableDelayLine {
            buffer: VecDeque::from_iter(std::iter::repeat(0.0).take(max_delay + 1)), // add 1 for linear interpolation
        }
    }

    pub fn max_delay(&self) -> f32 {
        (self.buffer.len() - 1) as f32
    }

    pub fn get_sample(&mut self, delay: f32) -> f32 {
        let delay = delay.clamp(0.0, self.max_delay());
        let prev_int_index = delay.floor() as usize;
        let next_int_index = delay.ceil() as usize;
        let prev_value = self.buffer[prev_int_index];
        let next_value = self.buffer[next_int_index];
        let interpolation = prev_value + delay.fract() * (next_value - prev_value);
        interpolation
    }

    pub fn reset(&mut self) {
        self.buffer.iter_mut().for_each(|s| *s = 0.0);
    }
}

#[cfg(test)]
mod tests {
    use super::VariableDelayLine;

    #[test]
    fn reads_from_newest_sample() {
        let mut delay = VariableDelayLine::new(2);
        for value in [3.0, 2.0, 1.0] {
            delay.buffer.push_front(value);
            delay.buffer.pop_back();
        }

        assert_eq!(delay.get_sample(0.0), 1.0);
        assert_eq!(delay.get_sample(1.0), 2.0);
        assert_eq!(delay.get_sample(2.0), 3.0);
        assert_eq!(delay.get_sample(1.5), 2.5);
    }
}
