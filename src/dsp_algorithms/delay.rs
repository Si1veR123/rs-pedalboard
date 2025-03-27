use std::collections::VecDeque;

pub struct Delay {
    pub buffer: VecDeque<f32>
}

impl Delay {
    pub fn new(delay_length_samples: usize) -> Self {
        Delay {
            buffer: VecDeque::from_iter(std::iter::repeat(0.0).take(delay_length_samples))
        }
    }

    pub fn process_buffer(&mut self, buffer: &mut [f32]) {
        for sample in buffer {
            let delayed_sample = self.buffer.pop_front().unwrap();
            self.buffer.push_back(*sample);
            *sample = delayed_sample;
        }
    }
}
