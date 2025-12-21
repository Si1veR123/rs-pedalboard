use crate::dsp_algorithms::impluse_response::load_wav;
use crate::pedalboard::Pedalboard;
use crate::pedals::PedalTrait;

pub const PROCESSING_BUFFER_SIZE: usize = 1024;

fn save_wav<P: AsRef<std::path::Path>>(wav_path: P, buffer: &[f32], sample_rate: f32) -> Result<(), String> {
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: sample_rate as u32,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = hound::WavWriter::create(wav_path, spec)
        .map_err(|e| e.to_string())?;

    for &sample in buffer {
        writer.write_sample(sample)
            .map_err(|e| e.to_string())?;
    }

    writer.finalize()
        .map_err(|e| e.to_string())?;

    Ok(())
}

pub fn process_audio_file(src_path: &std::path::Path, to_path: &std::path::Path, pedalboard: &mut Pedalboard, sample_rate: f32, normalise: bool) -> Result<f32, String> {
    let mut pedal_command_to_client_buffer: Vec<String> = Vec::new();

    for pedal in &mut pedalboard.pedals {
        pedal.set_config(PROCESSING_BUFFER_SIZE, sample_rate as u32);
    }

    let mut processing_buffer = match load_wav(src_path, sample_rate, false) {
        Ok(channels) => {
            // Average down to mono
            let num_channels = channels.len();
            let num_samples = channels[0].len();
            let mut mono_buffer = vec![0.0f32; num_samples];
            for channel in &channels {
                for (i, &sample) in channel.iter().enumerate() {
                    mono_buffer[i] += sample;
                }
            }
            for sample in &mut mono_buffer {
                *sample /= num_channels as f32;
            }
            mono_buffer
        },
        Err(e) => {
            return Err(e);
        }
    };

    for i in 0..(processing_buffer.len() as f32 / PROCESSING_BUFFER_SIZE as f32).ceil() as usize {
        let start = i * PROCESSING_BUFFER_SIZE;
        let mut end = start + PROCESSING_BUFFER_SIZE;
        end = end.min(processing_buffer.len());
        let frame = &mut processing_buffer[start..end];
        pedalboard.process_audio(frame, &mut pedal_command_to_client_buffer);
    }

    let peak_level = processing_buffer.iter().cloned().fold(f32::MIN, f32::max).abs();

    if normalise && peak_level > 0.0 {
        let normalisation_factor = 1.0 / peak_level;
        for sample in &mut processing_buffer {
            *sample *= normalisation_factor;
        }
    }

    // Save processed buffer to output file
    if let Err(e) = save_wav(to_path, &processing_buffer, sample_rate) {
        return Err(e);
    }

    Ok(peak_level)
}