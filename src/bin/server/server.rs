mod jack_init;

use std::io::{stdin, stdout, Write};
use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, Device, Host, Stream, StreamConfig};
use ringbuf::{traits::{ring_buffer, Consumer, Producer, Split}, HeapCons, HeapProd, HeapRb};
use rs_pedalboard::pedalboard_set::{self, PedalboardSet};

use simplelog::*;

// Frames=Samples for mono channel
// This is the number of samples provided to JACK callbacks
const JACK_FRAMES_PER_PERIOD: usize = 256;
const JACK_PERIODS_PER_BUFFER: usize = 3;
const RING_BUFFER_LATENCY_MS: f32 = 3.0;

pub fn ring_buffer_size(buffer_size: usize, latency: f32, sample_rate: f32) -> usize {
    let latency_frames = (latency / 1000.0) * sample_rate;
    buffer_size * 2 + latency_frames as usize
}

pub fn create_linked_streams(in_device: Device, out_device: Device, pedalboard_set: PedalboardSet<f32>, latency: f32, buffer_size: usize) -> (Stream, Stream) {
    let ring_buffer_size = ring_buffer_size(buffer_size, latency, 48000.0);
    log::info!("Ring buffer size: {}", ring_buffer_size);
    let ring_buffer: HeapRb<f32> = HeapRb::new(ring_buffer_size);
    let (audio_buffer_writer, mut audio_buffer_reader) = ring_buffer.split();

    let mut input_processor = InputProcessor {
        pedalboard_set,
        writer: audio_buffer_writer,
        processing_buffer: Vec::with_capacity(buffer_size as usize)
    };

    let config = StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(48000),
        buffer_size: cpal::BufferSize::Fixed(buffer_size as u32)
    };

    let stream_in = in_device.build_input_stream(&config, move |data: &[f32], _| {
        input_processor.process_audio(data);
    }, move |err| {
        log::error!("An error occurred on the input stream: {}", err);
    },
    None).expect("Failed to build input stream");

    let stream_out = out_device.build_output_stream(&config, move |data: &mut [f32], _| {
        let read = audio_buffer_reader.pop_slice(data);
        if read != data.len() {
            log::error!("Failed to provide a full buffer to output device. Input is behind.");
        }
    }, move |err| {
        log::error!("An error occurred on the output stream: {}", err);
    }, None).expect("Failed to build output stream");

    (stream_in, stream_out)
}

struct InputProcessor<T> {
    pedalboard_set: PedalboardSet<T>,
    writer: HeapProd<T>,
    processing_buffer: Vec<T>
}

impl<T: Copy> InputProcessor<T> {
    fn process_audio(&mut self, data: &[T]) {
        self.processing_buffer.clear();
        self.processing_buffer.extend_from_slice(data);

        self.pedalboard_set.process_audio(&mut self.processing_buffer);

        let written = self.writer.push_slice(&self.processing_buffer);
        if written != self.processing_buffer.len() {
            log::error!("Failed to write all processed data. Output is behind.")
        }
    }
}

fn main() {
    CombinedLogger::init(
        vec![
            TermLogger::new(LevelFilter::Info, Config::default(), TerminalMode::Mixed, ColorChoice::Auto),
            WriteLogger::new(LevelFilter::Info, Config::default(), std::fs::File::create("pedalboard-server.log").expect("Failed to create log file")),
        ]
    ).expect("Failed to start logging");
    log::info!("Started logging...");

    let (in_device, out_device) = jack_init::io_device_selector();
    
    jack_init::start_jack_server(JACK_FRAMES_PER_PERIOD, JACK_PERIODS_PER_BUFFER, in_device, out_device).expect("Failed to start JACK server");
    jack_init::jack_server_wait();

    let (jack_host, jack_input, jack_output) = jack_init::get_jack_host();
    jack_init::stereo_output();

    let pedalboard_set = PedalboardSet::default();

    let (in_stream, out_stream) = create_linked_streams(jack_input, jack_output, pedalboard_set, RING_BUFFER_LATENCY_MS, JACK_FRAMES_PER_PERIOD);

    in_stream.play().expect("Failed to play input stream");
    out_stream.play().expect("Failed to play output stream");

    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
