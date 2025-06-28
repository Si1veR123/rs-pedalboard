use core::panic;
use std::cell::UnsafeCell;
use cpal::{BuildStreamError, InputCallbackInfo, OutputCallbackInfo, StreamConfig, SupportedStreamConfig};
use cpal::{traits::DeviceTrait, Device, Stream};
use crossbeam::channel::{Receiver, Sender};
use ringbuf::traits::Split;
use ringbuf::{traits::Consumer, HeapRb};
use rs_pedalboard::pedalboard_set::PedalboardSet;
#[cfg(target_os = "windows")]
use rs_pedalboard::server_settings::SupportedHost;

use crate::audio_processor::AudioProcessor;
use crate::sample_conversion::*;
use crate::settings::ServerSettings;
use crate::stream_config::{get_output_config_for_device, get_input_config_for_device};

pub fn ring_buffer_size(buffer_size: usize, latency: f32, sample_rate: f32) -> usize {
    let latency_frames = (latency / 1000.0) * sample_rate;
    buffer_size * 2 + latency_frames as usize
}

fn build_input_stream(
    device: &Device,
    stream_config: SupportedStreamConfig,
    buffer_size: usize,
    mut data_callback: impl FnMut(&[f32], &InputCallbackInfo) + Send + 'static
) -> Result<Stream, BuildStreamError> {
    let sample = stream_config.sample_format();
    let config = StreamConfig {
        channels: 1,
        sample_rate: stream_config.sample_rate(),
        buffer_size: cpal::BufferSize::Fixed(buffer_size as u32),
    };
    let err_fn = |err| {
        log::error!("An error occurred on the input stream: {}", err);
    };

    let mut sample_converter_buffer = Vec::with_capacity(buffer_size);

    match sample {
        cpal::SampleFormat::F32 => device.build_input_stream(&config, data_callback, err_fn, None),
        cpal::SampleFormat::I8 => device.build_input_stream(&config, move |data: &[i8], info: &InputCallbackInfo| {
            sample_converter_buffer.resize(data.len(), 0.0);
            convert_i8_to_f32(data, sample_converter_buffer.as_mut());
            data_callback(sample_converter_buffer.as_ref(), info);
        }, err_fn, None),
        cpal::SampleFormat::U8 => device.build_input_stream(&config, move |data: &[u8], info: &InputCallbackInfo| {
            sample_converter_buffer.resize(data.len(), 0.0);
            convert_u8_to_f32(data, sample_converter_buffer.as_mut());
            data_callback(sample_converter_buffer.as_ref(), info);
        }, err_fn, None),
        cpal::SampleFormat::I16 => device.build_input_stream(&config, move |data: &[i16], info: &InputCallbackInfo| {
            sample_converter_buffer.resize(data.len(), 0.0);
            convert_i16_to_f32(data, sample_converter_buffer.as_mut());
            data_callback(sample_converter_buffer.as_ref(), info);
        }, err_fn, None),
        cpal::SampleFormat::U16 => device.build_input_stream(&config, move |data: &[u16], info: &InputCallbackInfo| {
            sample_converter_buffer.resize(data.len(), 0.0);
            convert_u16_to_f32(data, sample_converter_buffer.as_mut());
            data_callback(sample_converter_buffer.as_ref(), info);
        }, err_fn, None),
        cpal::SampleFormat::I32 => device.build_input_stream(&config, move |data: &[i32], info: &InputCallbackInfo| {
            sample_converter_buffer.resize(data.len(), 0.0);
            convert_i32_to_f32(data, sample_converter_buffer.as_mut());
            data_callback(sample_converter_buffer.as_ref(), info);
        }, err_fn, None),
        cpal::SampleFormat::U32 => device.build_input_stream(&config, move |data: &[u32], info: &InputCallbackInfo| {
            sample_converter_buffer.resize(data.len(), 0.0);
            convert_u32_to_f32(data, sample_converter_buffer.as_mut());
            data_callback(sample_converter_buffer.as_ref(), info);
        }, err_fn, None),
        cpal::SampleFormat::I64 => device.build_input_stream(&config, move |data: &[i64], info: &InputCallbackInfo| {
            sample_converter_buffer.resize(data.len(), 0.0);
            convert_i64_to_f32(data, sample_converter_buffer.as_mut());
            data_callback(sample_converter_buffer.as_ref(), info);
        }, err_fn, None),
        cpal::SampleFormat::U64 => device.build_input_stream(&config, move |data: &[u64], info: &InputCallbackInfo| {
            sample_converter_buffer.resize(data.len(), 0.0);
            convert_u64_to_f32(data, sample_converter_buffer.as_mut());
            data_callback(sample_converter_buffer.as_ref(), info);
        }, err_fn, None),
        cpal::SampleFormat::F64 => device.build_input_stream(&config, move |data: &[f64], info: &InputCallbackInfo| {
            sample_converter_buffer.resize(data.len(), 0.0);
            convert_f64_to_f32(data, sample_converter_buffer.as_mut());
            data_callback(sample_converter_buffer.as_ref(), info);
        }, err_fn, None),
        _ => panic!("Unsupported sample format: {}", sample),
    }
}

fn build_output_stream(
    device: &Device,
    stream_config: SupportedStreamConfig,
    stereo: bool,
    buffer_size: usize,
    mut data_callback: impl FnMut(&mut [f32], &OutputCallbackInfo) + Send + 'static
) -> Result<Stream, BuildStreamError> {
    let sample = stream_config.sample_format();
    let config = StreamConfig {
        channels: if stereo { 2 } else { 1 },
        sample_rate: stream_config.sample_rate(),
        buffer_size: cpal::BufferSize::Fixed(buffer_size as u32),
    };
    let err_fn = |err| {
        log::error!("An error occurred on the output stream: {}", err);
    };

    let mut sample_converter_buffer = Vec::with_capacity(buffer_size);

    match sample {
        cpal::SampleFormat::F32 => device.build_output_stream(&config, data_callback, err_fn, None),
        cpal::SampleFormat::I8 => device.build_output_stream(&config, move |data: &mut [i8], info: &OutputCallbackInfo| {
            sample_converter_buffer.resize(data.len(), 0.0);
            data_callback(sample_converter_buffer.as_mut(), info);
            convert_f32_to_i8(sample_converter_buffer.as_ref(), data);
        }, err_fn, None),
        cpal::SampleFormat::U8 => device.build_output_stream(&config, move |data: &mut [u8], info: &OutputCallbackInfo| {
            sample_converter_buffer.resize(data.len(), 0.0);
            data_callback(sample_converter_buffer.as_mut(), info);
            convert_f32_to_u8(sample_converter_buffer.as_ref(), data);
        }, err_fn, None),
        cpal::SampleFormat::I16 => device.build_output_stream(&config, move |data: &mut [i16], info: &OutputCallbackInfo| {
            sample_converter_buffer.resize(data.len(), 0.0);
            data_callback(sample_converter_buffer.as_mut(), info);
            convert_f32_to_i16(sample_converter_buffer.as_ref(), data);
        }, err_fn, None),
        cpal::SampleFormat::U16 => device.build_output_stream(&config, move |data: &mut [u16], info: &OutputCallbackInfo| {
            sample_converter_buffer.resize(data.len(), 0.0);
            data_callback(sample_converter_buffer.as_mut(), info);
            convert_f32_to_u16(sample_converter_buffer.as_ref(), data);
        }, err_fn, None),
        cpal::SampleFormat::I32 => device.build_output_stream(&config, move |data: &mut [i32], info: &OutputCallbackInfo| {
            sample_converter_buffer.resize(data.len(), 0.0);
            data_callback(sample_converter_buffer.as_mut(), info);
            convert_f32_to_i32(sample_converter_buffer.as_ref(), data);
        }, err_fn, None),
        cpal::SampleFormat::U32 => device.build_output_stream(&config, move |data: &mut [u32], info: &OutputCallbackInfo| {
            sample_converter_buffer.resize(data.len(), 0.0);
            data_callback(sample_converter_buffer.as_mut(), info);
            convert_f32_to_u32(sample_converter_buffer.as_ref(), data);
        }, err_fn, None),
        cpal::SampleFormat::I64 => device.build_output_stream(&config, move |data: &mut [i64], info: &OutputCallbackInfo| {
            sample_converter_buffer.resize(data.len(), 0.0);
            data_callback(sample_converter_buffer.as_mut(), info);
            convert_f32_to_i64(sample_converter_buffer.as_ref(), data);
        }, err_fn, None),
        cpal::SampleFormat::U64 => device.build_output_stream(&config, move |data: &mut [u64], info: &OutputCallbackInfo| {
            sample_converter_buffer.resize(data.len(), 0.0);
            data_callback(sample_converter_buffer.as_mut(), info);
            convert_f32_to_u64(sample_converter_buffer.as_ref(), data);
        }, err_fn, None),
        cpal::SampleFormat::F64 => device.build_output_stream(&config, move |data: &mut [f64], info: &OutputCallbackInfo| {
            sample_converter_buffer.resize(data.len(), 0.0);
            data_callback(sample_converter_buffer.as_mut(), info);
            convert_f32_to_f64(sample_converter_buffer.as_ref(), data);
        }, err_fn, None),
        _ => panic!("Unsupported sample format: {}", sample)
    }
}

pub fn create_linked_streams(
    in_device: Device,
    out_device: Device,
    latency: f32,
    buffer_size: usize,
    command_receiver: Receiver<Box<str>>,
    command_sender: Sender<Box<str>>,
    settings: ServerSettings
) -> (Stream, Stream) {
    let ring_buffer_size = ring_buffer_size(buffer_size, latency, 48000.0);
    log::info!("Ring buffer size: {}", ring_buffer_size);
    let ring_buffer: HeapRb<f32> = HeapRb::new(ring_buffer_size);

    let (audio_buffer_writer, mut audio_buffer_reader) = ring_buffer.split();
    let mut maybe_writer = Some(audio_buffer_writer);

    log::info!("Finding a compatible config for input and output devices...");
    let in_config = get_input_config_for_device(&in_device, 48000, buffer_size);
    let out_config = get_output_config_for_device(&out_device, 48000, buffer_size);

    // If the host is ASIO, and output device isn't mono, we must output stereo audio.
    // This is because other hosts (WASAPI, JACK) remap the mono output to stereo outside
    // of this program, but ASIO doesn't.
    let mut stereo_output = false;
    #[cfg(target_os = "windows")]
    if settings.host == SupportedHost::ASIO && out_config.channels() > 1 {
        log::info!("Enabling stereo output for ASIO");
        stereo_output = true;
    }

    log::info!("Input config: {:?}", in_config);
    log::info!("Output config: {:?}", out_config);

    let stream_in = build_input_stream(
        &in_device,
        in_config,
        buffer_size,
        move |data: &[f32], _| {
            thread_local! {
                static INPUT_PROCESSOR: UnsafeCell<Option<AudioProcessor>> = UnsafeCell::new(None);
            }
        
            INPUT_PROCESSOR.with(|ip| {
                // Safety: This only exists on the current thread (no other threads have a reference to it),
                // and this is the only place where a reference is acquired. This is a unique reference.
                let input_processor = unsafe { &mut *ip.get() };
        
                if input_processor.is_none() {
                    *input_processor = Some(AudioProcessor {
                        pedalboard_set: PedalboardSet::default(),
                        command_receiver: command_receiver.clone(),
                        command_sender: command_sender.clone(),
                        writer: maybe_writer.take().expect("Writer moved more than once"),
                        processing_buffer: Vec::with_capacity(buffer_size),
                        master_volume: 1.0,
                        tuner_handle: None,
                        pedal_command_to_client_buffer: Vec::with_capacity(12),
                        settings: settings.clone()
                    });
                }
                
                input_processor.as_mut().unwrap().process_audio(data);
            });
        }
    ).expect("Failed to build input stream");
    log::info!("Input stream built successfully");

    let stream_out = if stereo_output {
        let mut mono_buffer = vec![0.0; buffer_size];
        build_output_stream(
            &out_device,
            out_config,
            stereo_output,
            buffer_size,
            move |data: &mut [f32], _| {
                if data.len() % 2 == 0 {
                    mono_buffer.resize(data.len()/2, 0.0);

                    let read = audio_buffer_reader.pop_slice(&mut mono_buffer);
                    if read != data.len() {
                        log::error!("Failed to provide a full buffer to output device. Input is behind.");
                    };

                    for (i, sample) in mono_buffer.iter().enumerate() {
                        data[i * 2] = *sample;     // Left channel
                        data[i * 2 + 1] = *sample; // Right channel
                    }
                } else {
                    log::error!("Output buffer length is not even, cannot write stereo output.");
                }
            }
        ).expect("Failed to build output stream")
    } else {
        build_output_stream(
            &out_device,
            out_config,
            stereo_output,
            buffer_size,
            move |data: &mut [f32], _| {
                let read = audio_buffer_reader.pop_slice(data);
                if read != data.len() {
                    log::error!("Failed to provide a full buffer to output device. Input is behind.");
                }
            }
        ).expect("Failed to build output stream")
    };

    log::info!("Output stream built successfully");

    (stream_in, stream_out)
}
