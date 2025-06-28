use core::panic;
use std::cell::UnsafeCell;
use cpal::{BuildStreamError, InputCallbackInfo, OutputCallbackInfo, StreamConfig, SupportedStreamConfig};
use cpal::{traits::DeviceTrait, Device, Stream};
use crossbeam::channel::{Receiver, Sender};
use ringbuf::traits::Split;
use ringbuf::{traits::Consumer, HeapRb};
use rs_pedalboard::pedalboard_set::PedalboardSet;

use crate::audio_processor::AudioProcessor;
use crate::sample_conversion::SampleConverter;
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

    let mut sample_converter = SampleConverter::new();

    match sample {
        cpal::SampleFormat::F32 => device.build_input_stream(&config, data_callback, err_fn, None),
        cpal::SampleFormat::I8 => device.build_input_stream(&config, move |data: &[i8], info: &InputCallbackInfo| {
            sample_converter.convert_i8(data);
            data_callback(sample_converter.as_ref(), info);
        }, err_fn, None),
        cpal::SampleFormat::U8 => device.build_input_stream(&config, move |data: &[u8], info: &InputCallbackInfo| {
            sample_converter.convert_u8(data);
            data_callback(sample_converter.as_ref(), info);
        }, err_fn, None),
        cpal::SampleFormat::I16 => device.build_input_stream(&config, move |data: &[i16], info: &InputCallbackInfo| {
            sample_converter.convert_i16(data);
            data_callback(sample_converter.as_ref(), info);
        }, err_fn, None),
        cpal::SampleFormat::U16 => device.build_input_stream(&config, move |data: &[u16], info: &InputCallbackInfo| {
            sample_converter.convert_u16(data);
            data_callback(sample_converter.as_ref(), info);
        }, err_fn, None),
        cpal::SampleFormat::I32 => device.build_input_stream(&config, move |data: &[i32], info: &InputCallbackInfo| {
            sample_converter.convert_i32(data);
            data_callback(sample_converter.as_ref(), info);
        }, err_fn, None),
        cpal::SampleFormat::U32 => device.build_input_stream(&config, move |data: &[u32], info: &InputCallbackInfo| {
            sample_converter.convert_u32(data);
            data_callback(sample_converter.as_ref(), info);
        }, err_fn, None),
        cpal::SampleFormat::I64 => device.build_input_stream(&config, move |data: &[i64], info: &InputCallbackInfo| {
            sample_converter.convert_i64(data);
            data_callback(sample_converter.as_ref(), info);
        }, err_fn, None),
        cpal::SampleFormat::U64 => device.build_input_stream(&config, move |data: &[u64], info: &InputCallbackInfo| {
            sample_converter.convert_u64(data);
            data_callback(sample_converter.as_ref(), info);
        }, err_fn, None),
        cpal::SampleFormat::F64 => device.build_input_stream(&config, move |data: &[f64], info: &InputCallbackInfo| {
            sample_converter.convert_f64(data);
            data_callback(sample_converter.as_ref(), info);
        }, err_fn, None),
        _ => panic!("Unsupported sample format: {}", sample),
    }
}

fn build_output_stream(
    device: &Device,
    stream_config: SupportedStreamConfig,
    buffer_size: usize,
    mut data_callback: impl FnMut(&mut [f32], &OutputCallbackInfo) + Send + 'static
) -> Result<Stream, BuildStreamError> {
    let sample = stream_config.sample_format();
    let config = StreamConfig {
        channels: 1,
        sample_rate: stream_config.sample_rate(),
        buffer_size: cpal::BufferSize::Fixed(buffer_size as u32),
    };
    let err_fn = |err| {
        log::error!("An error occurred on the output stream: {}", err);
    };

    let mut sample_converter = SampleConverter::new();

    match sample {
        cpal::SampleFormat::F32 => device.build_output_stream(&config, data_callback, err_fn, None),
        cpal::SampleFormat::I8 => device.build_output_stream(&config, move |data: &mut [i8], info: &OutputCallbackInfo| {
            sample_converter.convert_i8(data);
            data_callback(sample_converter.as_mut(), info);
        }, err_fn, None),
        cpal::SampleFormat::U8 => device.build_output_stream(&config, move |data: &mut [u8], info: &OutputCallbackInfo| {
            sample_converter.convert_u8(data);
            data_callback(sample_converter.as_mut(), info);
        }, err_fn, None),
        cpal::SampleFormat::I16 => device.build_output_stream(&config, move |data: &mut [i16], info: &OutputCallbackInfo| {
            sample_converter.convert_i16(data);
            data_callback(sample_converter.as_mut(), info);
        }, err_fn, None),
        cpal::SampleFormat::U16 => device.build_output_stream(&config, move |data: &mut [u16], info: &OutputCallbackInfo| {
            sample_converter.convert_u16(data);
            data_callback(sample_converter.as_mut(), info);
        }, err_fn, None),
        cpal::SampleFormat::I32 => device.build_output_stream(&config, move |data: &mut [i32], info: &OutputCallbackInfo| {
            sample_converter.convert_i32(data);
            data_callback(sample_converter.as_mut(), info);
        }, err_fn, None),
        cpal::SampleFormat::U32 => device.build_output_stream(&config, move |data: &mut [u32], info: &OutputCallbackInfo| {
            sample_converter.convert_u32(data);
            data_callback(sample_converter.as_mut(), info);
        }, err_fn, None),
        cpal::SampleFormat::I64 => device.build_output_stream(&config, move |data: &mut [i64], info: &OutputCallbackInfo| {
            sample_converter.convert_i64(data);
            data_callback(sample_converter.as_mut(), info);
        }, err_fn, None),
        cpal::SampleFormat::U64 => device.build_output_stream(&config, move |data: &mut [u64], info: &OutputCallbackInfo| {
            sample_converter.convert_u64(data);
            data_callback(sample_converter.as_mut(), info);
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
                log::info!("Processing audio data with input processor");
                input_processor.as_mut().unwrap().process_audio(data);
            });
        }
    ).expect("Failed to build input stream");

    let stream_out = build_output_stream(
        &out_device,
        out_config,
        buffer_size,
        move |data: &mut [f32], _| {
            let read = audio_buffer_reader.pop_slice(data);
            if read != data.len() {
                log::error!("Failed to provide a full buffer to output device. Input is behind.");
            }
        }
    ).expect("Failed to build output stream");

    (stream_in, stream_out)
}
