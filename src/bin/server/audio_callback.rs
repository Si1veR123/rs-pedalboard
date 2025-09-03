use core::panic;
use std::cell::UnsafeCell;
use std::time::Instant;
use cpal::{InputCallbackInfo, OutputCallbackInfo, StreamConfig, SupportedStreamConfig};
use cpal::{traits::DeviceTrait, Device, Stream};
use smol::channel::{Receiver, Sender};
use ringbuf::traits::Split;
use ringbuf::{traits::Consumer, HeapRb};
use rs_pedalboard::pedalboard_set::PedalboardSet;
use rs_pedalboard::dsp_algorithms::resampler::Resampler;

use crate::audio_processor::AudioProcessor;
use crate::metronome_player::MetronomePlayer;
use crate::sample_conversion::*;
use crate::settings::ServerSettings;
use crate::stream_config::get_compatible_configs;
use crate::volume_monitor::PeakVolumeMonitor;

pub fn ring_buffer_size(buffer_size: usize, latency: f32, sample_rate: f32) -> usize {
    let latency_frames = (latency / 1000.0) * sample_rate;
    buffer_size * 2 + latency_frames as usize
}

fn clip_f32_samples(samples: &mut [f32]) -> bool {
    let mut clipped = false;
    for sample in samples.iter_mut() {
        if *sample < -1.0 {
            *sample = -1.0;
            clipped = true;
        } else if *sample > 1.0 {
            *sample = 1.0;
            clipped = true;
        }
    }
    clipped
}

fn handle_clipped_f32_samples(samples: &mut [f32], command_sender: &Sender<Box<str>>) {
    if clip_f32_samples(samples) {
        log::warn!("Output samples clipped");
        if let Err(e) = command_sender.try_send("clipped\n".into()) {
            log::error!("Failed to send clipped command: {}", e);
        }
    }
}

fn build_input_stream(
    device: &Device,
    stream_configs: &[SupportedStreamConfig],
    buffer_size: usize,
    mut data_callback: impl FnMut(&[f32], &InputCallbackInfo, cpal::ChannelCount) + Send + 'static
) -> Option<Stream> {
    let mut working_config = None;
    for supported_config in stream_configs {
        let sample_format = supported_config.sample_format();
        let config = StreamConfig {
            channels: supported_config.channels(),
            sample_rate: supported_config.sample_rate(),
            buffer_size: cpal::BufferSize::Fixed(buffer_size as u32),
        };

        log::info!("Attempting to build test input stream with config: {:?}, format: {:?}", config, sample_format);

        let stream_result = match sample_format {
            cpal::SampleFormat::F32 => device.build_input_stream(&config, |_: &[f32], _| {} , |_| {}, None),
            cpal::SampleFormat::I8 => device.build_input_stream(&config, |_: &[i8], _| {} , |_| {}, None),
            cpal::SampleFormat::U8 => device.build_input_stream(&config, |_: &[u8], _| {} , |_| {}, None),
            cpal::SampleFormat::I16 => device.build_input_stream(&config, |_: &[i16], _| {} , |_| {}, None),
            cpal::SampleFormat::U16 => device.build_input_stream(&config, |_: &[u16], _| {} , |_| {}, None),
            cpal::SampleFormat::I32 => device.build_input_stream(&config, |_: &[i32], _| {} , |_| {}, None),
            cpal::SampleFormat::U32 => device.build_input_stream(&config, |_: &[u32], _| {} , |_| {}, None),
            cpal::SampleFormat::I64 => device.build_input_stream(&config, |_: &[i64], _| {} , |_| {}, None),
            cpal::SampleFormat::U64 => device.build_input_stream(&config, |_: &[u64], _| {} , |_| {}, None),
            cpal::SampleFormat::F64 => device.build_input_stream(&config, |_: &[f64], _| {} , |_| {}, None),
            _ => panic!("Unsupported sample format: {}", sample_format),
        };

        match stream_result {
            Ok(_stream) => {
                log::info!("Successfully built test input stream with config");
                working_config =  Some((config, sample_format));
                break;
            },
            Err(e) => {
                log::warn!("Failed to build test input stream, error: {}", e);
            }
        }
    }

    if let Some((config, sample_format)) = working_config {
        log::info!("Building input stream with config: {:?}, format {:?}", config, sample_format);

        let err_fn = |err| {
            log::error!("An error occurred on the input stream: {}", err);
        };
        let mut sample_converter_buffer = Vec::with_capacity(buffer_size);

        let stream_result = match sample_format {
            cpal::SampleFormat::F32 => device.build_input_stream(&config, move |data: &[f32], info: &InputCallbackInfo| {
                data_callback(data.as_ref(), info, config.channels);
            }, err_fn, None),
            cpal::SampleFormat::I8 => device.build_input_stream(&config, move |data: &[i8], info: &InputCallbackInfo| {
                sample_converter_buffer.resize(data.len(), 0.0);
                convert_i8_to_f32(data, sample_converter_buffer.as_mut());
                data_callback(sample_converter_buffer.as_ref(), info, config.channels);
            }, err_fn, None),
            cpal::SampleFormat::U8 => device.build_input_stream(&config, move |data: &[u8], info: &InputCallbackInfo| {
                sample_converter_buffer.resize(data.len(), 0.0);
                convert_u8_to_f32(data, sample_converter_buffer.as_mut());
                data_callback(sample_converter_buffer.as_ref(), info, config.channels);
            }, err_fn, None),
            cpal::SampleFormat::I16 => device.build_input_stream(&config, move |data: &[i16], info: &InputCallbackInfo| {
                sample_converter_buffer.resize(data.len(), 0.0);
                convert_i16_to_f32(data, sample_converter_buffer.as_mut());
                data_callback(sample_converter_buffer.as_ref(), info, config.channels);
            }, err_fn, None),
            cpal::SampleFormat::U16 => device.build_input_stream(&config, move |data: &[u16], info: &InputCallbackInfo| {
                sample_converter_buffer.resize(data.len(), 0.0);
                convert_u16_to_f32(data, sample_converter_buffer.as_mut());
                data_callback(sample_converter_buffer.as_ref(), info, config.channels);
            }, err_fn, None),
            cpal::SampleFormat::I32 => device.build_input_stream(&config, move |data: &[i32], info: &InputCallbackInfo| {
                sample_converter_buffer.resize(data.len(), 0.0);
                convert_i32_to_f32(data, sample_converter_buffer.as_mut());
                data_callback(sample_converter_buffer.as_ref(), info, config.channels);
            }, err_fn, None),
            cpal::SampleFormat::U32 => device.build_input_stream(&config, move |data: &[u32], info: &InputCallbackInfo| {
                sample_converter_buffer.resize(data.len(), 0.0);
                convert_u32_to_f32(data, sample_converter_buffer.as_mut());
                data_callback(sample_converter_buffer.as_ref(), info, config.channels);
            }, err_fn, None),
            cpal::SampleFormat::I64 => device.build_input_stream(&config, move |data: &[i64], info: &InputCallbackInfo| {
                sample_converter_buffer.resize(data.len(), 0.0);
                convert_i64_to_f32(data, sample_converter_buffer.as_mut());
                data_callback(sample_converter_buffer.as_ref(), info, config.channels);
            }, err_fn, None),
            cpal::SampleFormat::U64 => device.build_input_stream(&config, move |data: &[u64], info: &InputCallbackInfo| {
                sample_converter_buffer.resize(data.len(), 0.0);
                convert_u64_to_f32(data, sample_converter_buffer.as_mut());
                data_callback(sample_converter_buffer.as_ref(), info, config.channels);
            }, err_fn, None),
            cpal::SampleFormat::F64 => device.build_input_stream(&config, move |data: &[f64], info: &InputCallbackInfo| {
                sample_converter_buffer.resize(data.len(), 0.0);
                convert_f64_to_f32(data, sample_converter_buffer.as_mut());
                data_callback(sample_converter_buffer.as_ref(), info, config.channels);
            }, err_fn, None),
            _ => panic!("Unsupported sample format: {}", sample_format),
        };

        match stream_result {
            Ok(stream) => {
                log::info!("Successfully built input stream");
                return Some(stream);
            },
            Err(e) => {
                log::error!("Failed to build input stream, error {}", e);
                return None;
            }
        }
    } else {
        log::error!("No working input config found");
        return None;
    }
}

fn build_output_stream(
    device: &Device,
    stream_configs: &[SupportedStreamConfig],
    buffer_size: usize,
    command_sender: Sender<Box<str>>,
    mut data_callback: impl FnMut(&mut [f32], &OutputCallbackInfo, cpal::ChannelCount) + Send + 'static
) -> Option<Stream> {
    let mut working_config = None;
    for supported_config in stream_configs {
        let sample_format = supported_config.sample_format();
        let config = StreamConfig {
            channels: supported_config.channels(),
            sample_rate: supported_config.sample_rate(),
            buffer_size: cpal::BufferSize::Fixed(buffer_size as u32),
        };

        log::info!("Attempting to build test output stream with config: {:?}, format: {:?}", config, sample_format);

        let stream_result = match sample_format {
            cpal::SampleFormat::F32 => device.build_output_stream(&config, |_: &mut [f32], _| {}, |_| {}, None),
            cpal::SampleFormat::I8 => device.build_output_stream(&config, |_: &mut [i8], _| {}, |_| {}, None),
            cpal::SampleFormat::U8 => device.build_output_stream(&config, |_: &mut [u8], _| {}, |_| {}, None),
            cpal::SampleFormat::I16 => device.build_output_stream(&config, |_: &mut [i16], _| {}, |_| {}, None),
            cpal::SampleFormat::U16 => device.build_output_stream(&config, |_: &mut [u16], _| {}, |_| {}, None),
            cpal::SampleFormat::I32 => device.build_output_stream(&config, |_: &mut [i32], _| {}, |_| {}, None),
            cpal::SampleFormat::U32 => device.build_output_stream(&config, |_: &mut [u32], _| {}, |_| {}, None),
            cpal::SampleFormat::I64 => device.build_output_stream(&config, |_: &mut [i64], _| {}, |_| {}, None),
            cpal::SampleFormat::U64 => device.build_output_stream(&config, |_: &mut [u64], _| {}, |_| {}, None),
            cpal::SampleFormat::F64 => device.build_output_stream(&config, |_: &mut [f64], _| {}, |_| {}, None),
            _ => panic!("Unsupported sample format: {}", sample_format),
        };

        match stream_result {
            Ok(_stream) => {
                log::info!("Successfully built test output stream with config");
                working_config = Some((config, sample_format));
                break;
            },
            Err(e) => {
                log::warn!("Failed to build test output stream, error: {}", e);
            }
        }
    }

    if let Some((config, sample_format)) = working_config {
        log::info!("Building output stream with config: {:?}, format {:?}", config, sample_format);

        let mut sample_converter_buffer = Vec::with_capacity(buffer_size);

        let err_fn = |err| {
            log::error!("An error occurred on the output stream: {}", err);
        };

        let stream_result = match sample_format {
            cpal::SampleFormat::F32 => device.build_output_stream(&config, move |data: &mut [f32], info: &OutputCallbackInfo| {
                data_callback(data, info, config.channels);
                handle_clipped_f32_samples(data.as_mut(), &command_sender);
            }, err_fn, None),
            cpal::SampleFormat::I8 => device.build_output_stream(&config, move |data: &mut [i8], info: &OutputCallbackInfo| {
                sample_converter_buffer.resize(data.len(), 0.0);
                data_callback(sample_converter_buffer.as_mut(), info, config.channels);
                handle_clipped_f32_samples(sample_converter_buffer.as_mut(), &command_sender);
                convert_f32_to_i8(sample_converter_buffer.as_ref(), data);
            }, err_fn, None),
            cpal::SampleFormat::U8 => device.build_output_stream(&config, move |data: &mut [u8], info: &OutputCallbackInfo| {
                sample_converter_buffer.resize(data.len(), 0.0);
                data_callback(sample_converter_buffer.as_mut(), info, config.channels);
                handle_clipped_f32_samples(sample_converter_buffer.as_mut(), &command_sender);
                convert_f32_to_u8(sample_converter_buffer.as_ref(), data);
            }, err_fn, None),
            cpal::SampleFormat::I16 => device.build_output_stream(&config, move |data: &mut [i16], info: &OutputCallbackInfo| {
                sample_converter_buffer.resize(data.len(), 0.0);
                data_callback(sample_converter_buffer.as_mut(), info, config.channels);
                handle_clipped_f32_samples(sample_converter_buffer.as_mut(), &command_sender);
                convert_f32_to_i16(sample_converter_buffer.as_ref(), data);
            }, err_fn, None),
            cpal::SampleFormat::U16 => device.build_output_stream(&config, move |data: &mut [u16], info: &OutputCallbackInfo| {
                sample_converter_buffer.resize(data.len(), 0.0);
                data_callback(sample_converter_buffer.as_mut(), info, config.channels);
                handle_clipped_f32_samples(sample_converter_buffer.as_mut(), &command_sender);
                convert_f32_to_u16(sample_converter_buffer.as_ref(), data);
            }, err_fn, None),
            cpal::SampleFormat::I32 => device.build_output_stream(&config, move |data: &mut [i32], info: &OutputCallbackInfo| {
                sample_converter_buffer.resize(data.len(), 0.0);
                data_callback(sample_converter_buffer.as_mut(), info, config.channels);
                handle_clipped_f32_samples(sample_converter_buffer.as_mut(), &command_sender);
                convert_f32_to_i32(sample_converter_buffer.as_ref(), data);
            }, err_fn, None),
            cpal::SampleFormat::U32 => device.build_output_stream(&config, move |data: &mut [u32], info: &OutputCallbackInfo| {
                sample_converter_buffer.resize(data.len(), 0.0);
                data_callback(sample_converter_buffer.as_mut(), info, config.channels);
                handle_clipped_f32_samples(sample_converter_buffer.as_mut(), &command_sender);
                convert_f32_to_u32(sample_converter_buffer.as_ref(), data);
            }, err_fn, None),
            cpal::SampleFormat::I64 => device.build_output_stream(&config, move |data: &mut [i64], info: &OutputCallbackInfo| {
                sample_converter_buffer.resize(data.len(), 0.0);
                data_callback(sample_converter_buffer.as_mut(), info, config.channels);
                handle_clipped_f32_samples(sample_converter_buffer.as_mut(), &command_sender);
                convert_f32_to_i64(sample_converter_buffer.as_ref(), data);
            }, err_fn, None),
            cpal::SampleFormat::U64 => device.build_output_stream(&config, move |data: &mut [u64], info: &OutputCallbackInfo| {
                sample_converter_buffer.resize(data.len(), 0.0);
                data_callback(sample_converter_buffer.as_mut(), info, config.channels);
                handle_clipped_f32_samples(sample_converter_buffer.as_mut(), &command_sender);
                convert_f32_to_u64(sample_converter_buffer.as_ref(), data);
            }, err_fn, None),
            cpal::SampleFormat::F64 => device.build_output_stream(&config, move |data: &mut [f64], info: &OutputCallbackInfo| {
                sample_converter_buffer.resize(data.len(), 0.0);
                data_callback(sample_converter_buffer.as_mut(), info, config.channels);
                handle_clipped_f32_samples(sample_converter_buffer.as_mut(), &command_sender);
                convert_f32_to_f64(sample_converter_buffer.as_ref(), data);
            }, err_fn, None),
            _ => panic!("Unsupported sample format: {}", sample_format),
        };

        match stream_result {
            Ok(stream) => {
                log::info!("Successfully built output stream");
                return Some(stream);
            },
            Err(e) => {
                log::error!("Failed to build output stream, error {}", e);
                return None;
            }
        }
    } else {
        log::error!("No working output config found");
        return None;
    }
}

pub fn create_linked_streams(
    in_device: Device,
    out_device: Device,
    command_receiver: Receiver<Box<str>>,
    command_sender: Sender<Box<str>>,
    settings: ServerSettings
) -> (Stream, Stream) {
    let in_command_sender = command_sender.clone();

    log::info!("Finding a compatible config for input and output devices...");
    let (in_configs, out_configs) = get_compatible_configs(
        &in_device,
        &out_device,
        settings.preferred_sample_rate,
        settings.frames_per_period
    );

    if in_configs.is_empty() || out_configs.is_empty() {
        panic!("No valid configs found. Change sample rate or buffer size.");
    }

    let used_sample_rate = in_configs[0].sample_rate().0;
    let processing_sample_rate = used_sample_rate * (1 << settings.upsample_passes);

    let ring_buffer_size = ring_buffer_size(settings.frames_per_period, settings.buffer_latency, processing_sample_rate as f32);
    log::info!("Ring buffer size: {}", ring_buffer_size);
    let ring_buffer: HeapRb<f32> = HeapRb::new(ring_buffer_size);

    let (audio_buffer_writer, mut audio_buffer_reader) = ring_buffer.split();
    let mut maybe_writer = Some(audio_buffer_writer);

    let mut input_stream_running = false;
    let settings_clone = settings.clone();

    let mut mono_buffer = vec![0.0; settings.frames_per_period];
    let stream_in = build_input_stream(
        &in_device,
        &in_configs,
        settings.frames_per_period,
        move |data: &[f32], _, channel_count| {
            let channel_count = channel_count as usize;

            // Average into mono buffer if needed
            if channel_count > 1 {
                let frame_count = data.len() / channel_count as usize;
                mono_buffer.resize(frame_count, 0.0);
                for i in 0..frame_count {
                    let mut sum = 0.0;
                    for ch in 0..channel_count {
                        sum += data[i * channel_count as usize + ch as usize];
                    }
                    mono_buffer[i] = sum / channel_count as f32;
                }
            } else {
                mono_buffer.resize(data.len(), 0.0);
                mono_buffer.copy_from_slice(data);
            }

            if !input_stream_running {
                log::info!("Input stream started. Received {} samples.", data.len());
                input_stream_running = true;
            }

            thread_local! {
                static INPUT_PROCESSOR: UnsafeCell<Option<AudioProcessor>> = UnsafeCell::new(None);
            }
        
            INPUT_PROCESSOR.with(|ip| {
                // Safety: This only exists on the current thread (no other threads have a reference to it),
                // and this is the only place where a reference is acquired. This is a unique reference.
                let input_processor = unsafe { &mut *ip.get() };
        
                if input_processor.is_none() {
                    let resamplers = if settings_clone.upsample_passes > 0 {
                        let max_block = settings_clone.frames_per_period << settings_clone.upsample_passes;
                        Some((
                            Resampler::new(settings_clone.upsample_passes as usize, max_block),
                            Resampler::new(settings_clone.upsample_passes as usize, max_block)
                        ))
                    } else {
                        None
                    };

                    *input_processor = Some(AudioProcessor {
                        pedalboard_set: PedalboardSet::default(),
                        command_receiver: command_receiver.clone(),
                        command_sender: in_command_sender.clone(),
                        writer: maybe_writer.take().expect("Writer moved more than once"),
                        data_buffer: Vec::with_capacity(data.len()),
                        processing_buffer: Vec::with_capacity(data.len() << settings_clone.upsample_passes),
                        master_in_volume: 1.0,
                        master_out_volume: 1.0,
                        tuner_handle: None,
                        pedal_command_to_client_buffer: Vec::with_capacity(12),
                        settings: settings_clone.clone(),
                        metronome: (false, MetronomePlayer::new(120, 0.5, used_sample_rate)),
                        volume_monitor: (false, Instant::now(), (0.0, 0.0), PeakVolumeMonitor::new(), PeakVolumeMonitor::new()),
                        volume_normalizer: None,
                        processing_sample_rate,
                        resamplers
                    });
                }
                
                input_processor.as_mut().unwrap().process_audio(&mono_buffer);
            });
        }
    ).expect("Failed to build input stream");

    let mut output_stream_running = false;
    let mut mono_buffer = vec![0.0; settings.frames_per_period];
    
    let stream_out = build_output_stream(
        &out_device,
        &out_configs,
        settings.frames_per_period,
        command_sender.clone(),
        move |data: &mut [f32], _, channel_count| {
            let channel_count = channel_count as usize;

            if !output_stream_running {
                log::info!("Output stream started. Received {} samples.", data.len());
                output_stream_running = true;
            }

            if data.len() % channel_count == 0 {
                let frame_count = data.len() / channel_count as usize;
                mono_buffer.resize(frame_count, 0.0);

                let read = audio_buffer_reader.pop_slice(&mut mono_buffer);
                if read != frame_count {
                    if let Err(e) = command_sender.try_send("xrun\n".into()) {
                        log::error!("Failed to send xrun command: {}", e);
                    }
                    log::error!("Failed to provide a full buffer to output device. Input is behind.");
                };

                for (i, sample) in mono_buffer.iter().enumerate() {
                    for ch in 0..channel_count {
                        data[i * channel_count + ch] = *sample;
                    }
                }
            } else {
                log::error!("Output buffer length doesn't match channel count.");
            }
        }
    ).expect("Failed to build output stream");

    (stream_in, stream_out)
}
