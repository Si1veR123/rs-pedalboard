//! Feeds back the input stream directly into the output stream.
//!
//! Assumes that the input and output devices can use the same stream configuration and that they
//! support the f32 sample format.
//!
//! Uses a delay of `LATENCY_MS` milliseconds in case the default input and output streams are not
//! precisely synchronised.

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use ringbuf::{
    traits::{Consumer, Producer, Split},
    HeapRb,
};

const LATENCY: f32 = 50.0;

fn main() {
    let host = cpal::host_from_id(cpal::available_hosts()
        .into_iter()
        .find(|id| *id == cpal::HostId::Asio)
        .unwrap()).expect("asio host unavailable");

    // Find devices.
    let input_device = host.default_input_device().unwrap();
    let output_device = host.default_output_device().unwrap();

    println!("Using input device: \"{}\"", input_device.name().unwrap());
    println!("Using output device: \"{}\"", output_device.name().unwrap());

    // We'll try and use the same configuration between streams to keep it simple.
    let config: cpal::StreamConfig = input_device.default_input_config().unwrap().into();

    // Create a delay in case the input and output devices aren't synced.
    let latency_frames = (LATENCY / 1_000.0) * config.sample_rate.0 as f32;
    let latency_samples = latency_frames as usize * config.channels as usize;

    // The buffer to share samples
    let ring = HeapRb::<i32>::new(latency_samples * 2);
    let (mut producer, mut consumer) = ring.split();

    // Fill the samples with 0.0 equal to the length of the delay.
    for _ in 0..latency_samples {
        // The ring buffer has twice as much space as necessary to add latency here,
        // so this should never fail
        producer.try_push(0).unwrap();
    }

    let input_data_fn = move |data: &[i32], _: &cpal::InputCallbackInfo| {
        let mut output_fell_behind = false;
        for &sample in data {
            if producer.try_push(sample).is_err() {
                output_fell_behind = true;
            }
        }
        if output_fell_behind {
            eprintln!("output stream fell behind: try increasing latency");
        }
    };

    let output_data_fn = move |data: &mut [i32], _: &cpal::OutputCallbackInfo| {
        let mut input_fell_behind = false;
        for sample in data {
            *sample = match consumer.try_pop() {
                Some(s) => s,
                None => {
                    input_fell_behind = true;
                    0
                }
            };
        }
        if input_fell_behind {
            eprintln!("input stream fell behind: try increasing latency");
        }
    };

    // Build streams.
    println!(
        "Attempting to build both streams with i32 samples and `{:?}`.",
        config
    );
    input_device.supported_input_configs().unwrap().for_each(|c| {
        println!("Supported input config: {:?}", c);
    });
    output_device.supported_output_configs().unwrap().for_each(|c| {
        println!("Supported output config: {:?}", c);
    });

    let input_stream = input_device.build_input_stream(&config, input_data_fn, err_fn, None).unwrap();
    let output_stream = output_device.build_output_stream(&config, output_data_fn, err_fn, None).unwrap();
    println!("Successfully built streams.");

    // Play the streams.
    println!(
        "Starting the input and output streams with `{}` milliseconds of latency.",
        LATENCY
    );
    input_stream.play().unwrap();
    output_stream.play().unwrap();

    // Run for 3 seconds before closing.
    println!("Playing for 3 seconds... ");
    std::thread::sleep(std::time::Duration::from_secs(3));
    drop(input_stream);
    drop(output_stream);
    println!("Done!");
}

fn err_fn(err: cpal::StreamError) {
    eprintln!("an error occurred on stream: {}", err);
}