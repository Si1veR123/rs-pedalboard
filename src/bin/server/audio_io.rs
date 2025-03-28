use cpal::{traits::DeviceTrait, Device, Stream, StreamConfig};
use crossbeam::channel::Receiver;
use ringbuf::{traits::{Consumer, Producer, Split}, HeapProd, HeapRb};
use rs_pedalboard::{pedalboard_set::PedalboardSet, pedals::PedalTrait};

pub fn ring_buffer_size(buffer_size: usize, latency: f32, sample_rate: f32) -> usize {
    let latency_frames = (latency / 1000.0) * sample_rate;
    buffer_size * 2 + latency_frames as usize
}

pub fn create_linked_streams(
    in_device: Device,
    out_device: Device,
    pedalboard_set: PedalboardSet,
    latency: f32,
    buffer_size: usize,
    command_receiver: Receiver<Box<str>>
) -> (Stream, Stream) {
    let ring_buffer_size = ring_buffer_size(buffer_size, latency, 48000.0);
    log::info!("Ring buffer size: {}", ring_buffer_size);
    let ring_buffer: HeapRb<f32> = HeapRb::new(ring_buffer_size);
    let (audio_buffer_writer, mut audio_buffer_reader) = ring_buffer.split();

    let mut input_processor = InputProcessor {
        pedalboard_set,
        command_receiver,
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


struct InputProcessor {
    pedalboard_set: PedalboardSet,
    command_receiver: Receiver<Box<str>>,
    writer: HeapProd<f32>,
    processing_buffer: Vec<f32>
}

impl InputProcessor {
    fn process_audio(&mut self, data: &[f32]) {
        self.processing_buffer.clear();
        self.processing_buffer.extend_from_slice(data);

        self.pedalboard_set.process_audio(&mut self.processing_buffer);

        let written = self.writer.push_slice(&self.processing_buffer);
        if written != self.processing_buffer.len() {
            log::error!("Failed to write all processed data. Output is behind.")
        }

        while let Ok(command) = self.command_receiver.try_recv() {
            self.handle_command(command);
        }
    }

    /// setparameter <pedalboard index> <pedal index> <parameter name> <parameter value>
    /// movepedalboard <src index> <dest index>
    /// addpedalboard <pedalboard stringified>
    /// deletepedalboard <pedalboard index>
    /// addpedal <pedalboard index> <pedal index> <pedal stringified>
    /// deletepedal <pedalboard index> <pedal index>
    /// movepedal <pedalboard index> <src index> <dest index>
    /// loadset <pedalboardset stringified>
    /// play <pedalboard index>
    /// master <volume 0-1>
    fn handle_command(&mut self, command: Box<str>) -> Option<()> {
        let mut words = command.split_whitespace();
        let command_name = words.next()?;

        match command_name {
            "setparameter" => {
                let pedalboard_index = words.next()?.parse::<usize>().ok()?;
                let pedal_index = words.next()?.parse::<usize>().ok()?;
                let parameter_name = words.next()?;

                let pedalboard_ser_start_index = parameter_name.as_ptr() as usize + parameter_name.len() - command.as_ptr() as usize;
                let parameter_value = serde_json::from_str(&command[pedalboard_ser_start_index..]).ok()?;
                self.pedalboard_set.pedalboards.get_mut(pedalboard_index)?
                    .pedals.get_mut(pedal_index)?
                    .set_parameter_value(parameter_name, parameter_value);
            },
            "movepedalboard" => {
                let src_index = words.next()?.parse::<usize>().ok()?;
                let dest_index = words.next()?.parse::<usize>().ok()?;

                let pedalboard = self.pedalboard_set.pedalboards.remove(src_index);
                self.pedalboard_set.pedalboards.insert(dest_index, pedalboard);
            }
            _ => return None
        }

        Some(())
    }
}