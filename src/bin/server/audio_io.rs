use std::{cell::UnsafeCell, time::Instant};

use cpal::{traits::DeviceTrait, Device, Stream, StreamConfig};
use crossbeam::channel::{Receiver, Sender};
use ringbuf::{traits::{Consumer, Producer, Split}, HeapProd, HeapRb};
use rs_pedalboard::{pedalboard_set::PedalboardSet, pedals::{Pedal, PedalTrait}, dsp_algorithms::yin::Yin};

use crate::constants;

pub fn ring_buffer_size(buffer_size: usize, latency: f32, sample_rate: f32) -> usize {
    let latency_frames = (latency / 1000.0) * sample_rate;
    buffer_size * 2 + latency_frames as usize
}

pub fn create_linked_streams(
    in_device: Device,
    out_device: Device,
    latency: f32,
    buffer_size: usize,
    command_receiver: Receiver<Box<str>>,
    command_sender: Sender<Box<str>>
) -> (Stream, Stream) {
    let ring_buffer_size = ring_buffer_size(buffer_size, latency, 48000.0);
    log::info!("Ring buffer size: {}", ring_buffer_size);
    let ring_buffer: HeapRb<f32> = HeapRb::new(ring_buffer_size);

    let (audio_buffer_writer, mut audio_buffer_reader) = ring_buffer.split();
    let mut maybe_writer = Some(audio_buffer_writer);

    let config = StreamConfig {
        channels: 1,
        sample_rate: cpal::SampleRate(48000),
        buffer_size: cpal::BufferSize::Fixed(buffer_size as u32)
    };

    let stream_in = in_device.build_input_stream(
        &config,
        {
            move |data: &[f32], _| {
                thread_local! {
                    static INPUT_PROCESSOR: UnsafeCell<Option<InputProcessor>> = UnsafeCell::new(None);
                }

                INPUT_PROCESSOR.with(|ip| {
                    let input_processor = unsafe { &mut *ip.get() };

                    if input_processor.is_none() {
                        *input_processor = Some(InputProcessor {
                            pedalboard_set: PedalboardSet::default(),
                            command_receiver: command_receiver.clone(),
                            command_sender: command_sender.clone(),
                            writer: maybe_writer.take().expect("Writer moved more than once"),
                            processing_buffer: Vec::with_capacity(buffer_size),
                            master_volume: 1.0,
                            tuner_enabled: false,
                            tuner_last_sent: Instant::now(),
                            tuner: Yin::new(0.1, 40, 1300, 48000),
                        });
                    }

                    input_processor.as_mut().unwrap().process_audio(data);
                });
            }
        },
        move |err| {
            log::error!("An error occurred on the input stream: {}", err);
        },
        None,
    ).expect("Failed to build input stream");

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


struct InputProcessor {
    pedalboard_set: PedalboardSet,
    command_receiver: Receiver<Box<str>>,
    command_sender: Sender<Box<str>>,
    writer: HeapProd<f32>,
    processing_buffer: Vec<f32>,
    master_volume: f32,

    tuner_enabled: bool,
    tuner_last_sent: Instant, 
    tuner: Yin
}

impl InputProcessor {
    fn process_audio(&mut self, data: &[f32]) {
        
        if self.tuner_enabled {
            self.tuner.push_to_buffer(data);
            if Instant::now().duration_since(self.tuner_last_sent).as_millis() >= 50 {
                let frequency = self.tuner.process_buffer();
                self.tuner_last_sent = Instant::now();
                log::debug!("Tuner frequency: {:.2} Hz", frequency);
                let command = format!("tuner {:.2}\n", frequency);
                if self.command_sender.send(command.into()).is_err() {
                    log::error!("Failed to send tuner command");
                }
            }
        }

        self.processing_buffer.clear();
        self.processing_buffer.extend_from_slice(data);

        // In case data is larger than buffer_size and pedals may only expect buffer_size or less
        for i in 0..(data.len() as f32 / constants::FRAMES_PER_PERIOD as f32).ceil() as usize {
            let start = i * constants::FRAMES_PER_PERIOD;
            let mut end = start + constants::FRAMES_PER_PERIOD;
            end = end.min(self.processing_buffer.len());
            let frame = &mut self.processing_buffer[start..end];
            self.pedalboard_set.process_audio(frame);
        }

        self.processing_buffer.iter_mut().for_each(|sample| *sample *= self.master_volume);

        let written = self.writer.push_slice(&self.processing_buffer);
        if written != self.processing_buffer.len() {
            log::error!("Failed to write all processed data. Output is behind.")
        }

        // Handle commands that have been received
        while let Ok(command) = self.command_receiver.try_recv() {
            if self.handle_command(command).is_none() {
                log::error!("Failed to handle command");
            }
        }
    }

    fn handle_command(&mut self, command: Box<str>) -> Option<()> {
        let mut words = command.split_whitespace();
        let command_name = words.next()?;

        match command_name {
            "setparameter" => {
                let pedalboard_index = words.next()?.parse::<usize>().ok()?;
                let pedal_index = words.next()?.parse::<usize>().ok()?;
                let parameter_name = words.next()?;

                let pedalboard_ser_start_index = parameter_name.as_ptr() as usize + parameter_name.len() - command.as_ptr() as usize;
                let parameter_value = serde_json::from_str(&command[pedalboard_ser_start_index + 1..]).ok()?;
                self.pedalboard_set.pedalboards.get_mut(pedalboard_index)?
                    .pedals.get_mut(pedal_index)?
                    .set_parameter_value(parameter_name, parameter_value);
            },
            "movepedalboard" => {
                let src_index = words.next()?.parse::<usize>().ok()?;
                let dest_index = words.next()?.parse::<usize>().ok()?;

                let pedalboard = self.pedalboard_set.pedalboards.remove(src_index);

                let shifted_dest_index = if dest_index > src_index {
                    dest_index - 1
                } else {
                    dest_index
                };

                self.pedalboard_set.pedalboards.insert(shifted_dest_index, pedalboard);
            },
            "addpedalboard" => {
                let pedalboard_stringified = &command[command_name.len() + 1..];
                let pedalboard = serde_json::from_str(&pedalboard_stringified).ok()?;
                self.pedalboard_set.pedalboards.push(pedalboard);
            },
            "deletepedalboard" => {
                let pedalboard_index = words.next()?.parse::<usize>().ok()?;
                self.pedalboard_set.pedalboards.remove(pedalboard_index);
            },
            "addpedal" => {
                let pedalboard_index_str = words.next()?;
                let pedalboard_index = pedalboard_index_str.parse::<usize>().ok()?;

                let pedalboard_ser_start_index = pedalboard_index_str.as_ptr() as usize + pedalboard_index_str.len() - command.as_ptr() as usize;
                let pedal_stringified = &command[pedalboard_ser_start_index + 1..];
                
                let mut pedal: Pedal = serde_json::from_str(&pedal_stringified).ok()?;
                pedal.set_config(super::constants::FRAMES_PER_PERIOD, 48000);
                self.pedalboard_set.pedalboards.get_mut(pedalboard_index)?
                    .pedals.push(pedal);
            },
            "deletepedal" => {
                let pedalboard_index = words.next()?.parse::<usize>().ok()?;
                let pedal_index = words.next()?.parse::<usize>().ok()?;
                self.pedalboard_set.pedalboards.get_mut(pedalboard_index)?
                    .pedals.remove(pedal_index);
            },
            "movepedal" => {
                let pedalboard_index = words.next()?.parse::<usize>().ok()?;
                let src_index = words.next()?.parse::<usize>().ok()?;
                let dest_index = words.next()?.parse::<usize>().ok()?;

                let pedal = self.pedalboard_set.pedalboards.get_mut(pedalboard_index)?
                    .pedals.remove(src_index);

                let shifted_dest_index = if dest_index > src_index {
                    dest_index - 1
                } else {
                    dest_index
                };

                self.pedalboard_set.pedalboards.get_mut(pedalboard_index)?
                    .pedals.insert(shifted_dest_index, pedal);
            },
            "loadset" => {
                let pedalboardset_stringified = &command[command_name.len() + 1..];
                let pedalboardset = serde_json::from_str(&pedalboardset_stringified).ok()?;
                self.pedalboard_set = pedalboardset;
            },
            "play" => {
                let pedalboard_index = words.next()?.parse::<usize>().ok()?;
                self.pedalboard_set.set_active_pedalboard(pedalboard_index);
            },
            "master" => {
                let volume = words.next()?.parse::<f32>().ok()?;
                self.master_volume = volume;
            },
            "tuner" => {
                let enable_str = words.next()?;
                match enable_str {
                    "on" => {
                        self.tuner_enabled = true;
                        log::info!("Tuner enabled");
                    },
                    "off" => {
                        self.tuner_enabled = false;
                        log::info!("Tuner disabled");
                    },
                    _ => {
                        log::error!("Invalid value for tuner command: expected 'on' or 'off'");
                        return None;
                    }
                }
            },
            _ => return None
        }

        Some(())
    }
}