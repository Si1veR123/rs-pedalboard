use std::path::Path;

use rubato::{Resampler, SincFixedIn, SincInterpolationParameters};

use crate::pedalboard::Pedalboard;
use crate::pedals::PedalTrait;

pub const PROCESSING_BUFFER_SIZE: usize = 1024;

pub fn load_wav<P: AsRef<Path>>(wav_path: P, sample_rate: f32, normalise: bool) -> Result<Vec<Vec<f32>>, String> {
    let mut reader = hound::WavReader::open(wav_path.as_ref()).map_err(
        |e| format!("Failed to open WAV file '{}': {}", wav_path.as_ref().display(), e)
    )?;
    
    let spec = reader.spec();
    if spec.bits_per_sample > 32 {
        return Err("WAV file has more than 32 bits per sample. This is not supported.".into());
    }

    let resample_ratio = sample_rate / spec.sample_rate as f32;

    let float_samples = match spec.sample_format {
        hound::SampleFormat::Float => {
            let ir_samples: Result<Vec<f32>, _> = reader.into_samples().collect();
            ir_samples.map_err(|e| e.to_string())
        },
        hound::SampleFormat::Int => {
            let max_amplitude = (1i64 << (spec.bits_per_sample - 1)) as f32;
            let ir_samples: Result<Vec<f32>, _> = reader.samples::<i32>()
                .map(|s| s.and_then(|s| Ok(s as f32 / max_amplitude)))
                .collect();
            ir_samples.map_err(|e| e.to_string())
        }
    };

    float_samples.and_then(|float_samples| {
        let num_channels = spec.channels as usize;

        // Deinterleave samples into a Vec of channels
        let mut channels: Vec<Vec<f32>> = vec![Vec::with_capacity(float_samples.len() / num_channels); num_channels];
        for frame in float_samples.chunks_exact(num_channels) {
            for (i, &sample) in frame.iter().enumerate() {
                channels[i].push(sample);
            }
        }

        let mut resampled_channels;
        if resample_ratio == 1.0 {
            resampled_channels = channels;
        } else {
            let mut resampler = SincFixedIn::<f32>::new(
                resample_ratio as f64,
                1.0,
                SincInterpolationParameters {
                    sinc_len: 512,
                    f_cutoff: 0.9,
                    oversampling_factor: 128,
                    interpolation: rubato::SincInterpolationType::Linear,
                    window: rubato::WindowFunction::BlackmanHarris2,
                },
                float_samples.len() / num_channels,
                num_channels,
            ).map_err(|e| e.to_string())?;

            let channel_refs: Vec<&[f32]> = channels.iter().map(|ch| ch.as_slice()).collect();

            resampled_channels = resampler.process(&channel_refs, None)
                .map_err(|e| e.to_string())?;
        }

        if normalise {
            // Normalize WAV by RMS
            let rms = resampled_channels
                .iter()
                .flat_map(|ch| ch.iter())
                .map(|&x| x * x)
                .sum::<f32>()
                .sqrt();

            if rms > 1e-12 {
                let scale = 1.0 / rms;
                for ch in &mut resampled_channels {
                    for s in ch.iter_mut() {
                        *s *= scale;
                    }
                }
            }
        }
        Ok(resampled_channels)
    })
}

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

pub fn process_audio(audio: &mut [f32], pedalboard: &mut Pedalboard, sample_rate: f32, normalise: bool) {
    let mut pedal_command_to_client_buffer: Vec<String> = Vec::new();

    for pedal in &mut pedalboard.pedals {
        pedal.set_config(PROCESSING_BUFFER_SIZE, sample_rate as u32);
    }

    for i in 0..(audio.len() as f32 / PROCESSING_BUFFER_SIZE as f32).ceil() as usize {
        let start = i * PROCESSING_BUFFER_SIZE;
        let mut end = start + PROCESSING_BUFFER_SIZE;
        end = end.min(audio.len());
        let frame = &mut audio[start..end];
        pedalboard.process_audio(frame, &mut pedal_command_to_client_buffer);
    }

    let peak_level = audio.iter().cloned().fold(f32::MIN, f32::max).abs();

    if normalise && peak_level > 0.0 {
        let normalisation_factor = 1.0 / peak_level;
        for sample in audio {
            *sample *= normalisation_factor;
        }
    }
}

pub fn process_audio_file(src_path: &std::path::Path, pedalboard: &mut Pedalboard, sample_rate: f32, normalise: bool) -> Result<Vec<f32>, String> {
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

    process_audio(&mut processing_buffer, pedalboard, sample_rate, normalise);

    Ok(processing_buffer)
}

pub fn process_audio_file_and_save(src_path: &std::path::Path, to_path: &std::path::Path, pedalboard: &mut Pedalboard, sample_rate: f32, normalise: bool) -> Result<(), String> {
    let buffer = process_audio_file(src_path, pedalboard, sample_rate, normalise)?;

    // Save processed buffer to output file
    if let Err(e) = save_wav(to_path, &buffer, sample_rate) {
        return Err(e);
    }

    Ok(())
}