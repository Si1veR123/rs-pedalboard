use std::io::{stdin, stdout, Write};
use cpal::{traits::{DeviceTrait, HostTrait, StreamTrait}, Device, Host, Stream, StreamConfig};
use ringbuf::{traits::{ring_buffer, Consumer, Producer, Split}, HeapCons, HeapProd, HeapRb};
use rs_pedalboard::pedalboard_set::{self, PedalboardSet};


pub fn device_selector(devices: &[Device]) -> Device {
    let mut input_buf = String::new();

    for (i, device) in devices.iter().enumerate() {
        println!("{}: {}", i, device.name().unwrap());
    }
    print!("Select a device: ");
    stdout().flush().expect("Failed to flush stdout");
    stdin().read_line(&mut input_buf).expect("Failed to read stdin");
    devices[input_buf.trim().parse::<usize>().expect("Failed to parse device index")].clone()
}


pub fn io_device_selector(host: &Host) -> (Device, Device) {
    let in_devices = Vec::from_iter(host.input_devices().expect("Failed to get input devices"));
    let out_devices = Vec::from_iter(host.output_devices().expect("Failed to get output devices"));

    println!("Input devices:");
    let in_device = device_selector(&in_devices);

    println!("Output devices:");
    let out_device = device_selector(&out_devices);

    (in_device, out_device)
}

pub fn ring_buffer_size(buffer_size: usize, latency: f32, sample_rate: f32) -> usize {
    let latency_frames = (latency / 1000.0) * sample_rate;
    buffer_size * 2 + latency_frames as usize
}

pub fn create_linked_streams(in_device: Device, out_device: Device, pedalboard_set: PedalboardSet, latency: f32, buffer_size: usize) -> (Stream, Stream) {
    let ring_buffer: HeapRb<f32> = HeapRb::new(ring_buffer_size(buffer_size, latency, 48000.0));
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
            log::error!("Failed to read all data from the ring buffer");
        }
    }, move |err| {
        log::error!("An error occurred on the output stream: {}", err);
    }, None).expect("Failed to build output stream");

    (stream_in, stream_out)
}

struct InputProcessor {
    pedalboard_set: PedalboardSet,
    writer: HeapProd<f32>,
    processing_buffer: Vec<f32>
}

impl InputProcessor {
    fn process_audio(&mut self, data: &[f32]) {
        self.processing_buffer.clear();
        self.processing_buffer.extend_from_slice(data);

        self.pedalboard_set.process_audio(&mut self.processing_buffer);

        self.writer.push_slice(&self.processing_buffer);
    }
}

fn main() {
    let jack_host = cpal::host_from_id(
        cpal::available_hosts()
            .into_iter()
            .find(|id| *id == cpal::HostId::Jack)
            .expect("JACK host not found")
        ).unwrap();

    let (in_device, out_device) = io_device_selector(&jack_host);
    let pedalboard_set = PedalboardSet::default();
    let (in_stream, out_stream) = create_linked_streams(in_device, out_device, pedalboard_set, 20.0, 512);

    in_stream.play().expect("Failed to play input stream");
    out_stream.play().expect("Failed to play output stream");

    loop {
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}
