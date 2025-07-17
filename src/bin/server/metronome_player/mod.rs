use std::io::Cursor;
use hound::{WavReader, SampleFormat};

pub struct MetronomePlayer {
    pub bpm: u32,
    pub volume: f32,
    pub sample_rate: usize,
    click_audio: Vec<f32>,
    // Current position, where 0 is the start of the click sound
    // and the maximum value is the 'samples per beat'-1
    current_position: usize,
}

impl MetronomePlayer {
    pub fn load_click_from_bytes(data: &[u8]) -> Vec<f32> {
        let cursor = Cursor::new(data);
        let reader = WavReader::new(cursor).expect("Failed to create WAV reader");
        let spec = reader.spec();
    
        let samples: Vec<f32> = match spec.sample_format {
            SampleFormat::Float => {
                reader
                    .into_samples::<f32>()
                    .map(|s| s.expect("Failed to read sample"))
                    .collect()
            }
            SampleFormat::Int => {
                match spec.bits_per_sample {
                    8 => reader
                        .into_samples::<i8>()
                        .map(|s| s.expect("Failed to read sample") as f32 / i8::MAX as f32)
                        .collect(),
                    16 => reader
                        .into_samples::<i16>()
                        .map(|s| s.expect("Failed to read sample") as f32 / i16::MAX as f32)
                        .collect(),
                    24 => reader
                        .into_samples::<i32>()
                        .map(|s| {
                            let sample = s.expect("Failed to read sample");
                            // 24-bit WAVs are stored in 32-bit ints, so we scale accordingly
                            (sample >> 8) as f32 / (1 << 23) as f32
                        })
                        .collect(),
                    32 => reader
                        .into_samples::<i32>()
                        .map(|s| s.expect("Failed to read sample") as f32 / i32::MAX as f32)
                        .collect(),
                    other => panic!("Unsupported bit depth: {}", other),
                }
            }
        };
    
        samples
    }

    pub fn new(bpm: u32, volume: f32, sample_rate: usize) -> Self {
        let click_audio = MetronomePlayer::load_click_from_bytes(include_bytes!("click_trim.wav"));
        MetronomePlayer {
            bpm,
            volume,
            sample_rate,
            click_audio,
            current_position: 0,
        }
    }

    fn samples_per_beat(&self) -> usize {
        let seconds_per_beat = 60.0 / self.bpm as f32;
        (seconds_per_beat * self.sample_rate as f32) as usize
    }

    pub fn add_to_buffer(&mut self, buffer: &mut [f32]) {
        // Clamp current_position in case parameters have changed
        self.current_position = self.current_position.min(self.samples_per_beat()-1);

        for sample in buffer.iter_mut() {
            if self.current_position < self.click_audio.len() {
                // Write the click sound
                *sample += self.click_audio[self.current_position] * self.volume;
            }

            // Move to the next position
            self.current_position += 1;

            // If we reached the end of the beat, reset to the start
            if self.current_position >= self.samples_per_beat() {
                self.current_position = 0;
            }
        }
    }
}
