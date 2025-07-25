use std::{sync::{atomic::AtomicBool, Arc}, time::Instant};
use crossbeam::channel::{Receiver, Sender};
use ringbuf::{traits::{Producer, Split}, HeapProd, HeapRb};

use rs_pedalboard::{
    pedalboard_set::PedalboardSet,
    pedals::{Pedal, PedalTrait},
    dsp_algorithms::yin::Yin,
    DEFAULT_VOLUME_MONITOR_UPDATE_RATE
};

use crate::{
    metronome_player::MetronomePlayer,
    settings::ServerSettings,
    volume_monitor::PeakVolumeMonitor, volume_normalization::{PeakNormalizer}
};

pub struct AudioProcessor {
    pub pedalboard_set: PedalboardSet,
    pub command_receiver: Receiver<Box<str>>,
    pub command_sender: Sender<Box<str>>,
    pub writer: HeapProd<f32>,
    pub processing_buffer: Vec<f32>,
    pub pedal_command_to_client_buffer: Vec<String>,
    pub master_in_volume: f32,
    pub master_out_volume: f32,
    pub settings: ServerSettings,
    // If tuner is enabled, this will contain the writer to the tuner buffer,
    // a receiver for frequency updates, and a kill flag
    pub tuner_handle: Option<(HeapProd<f32>, Receiver<f32>, Arc<AtomicBool>)>,
    // Enabled?, metronome
    pub metronome: (bool, MetronomePlayer),
    // Enabled?, last sent, last value both 0?, input volume monitor, output volume monitor
    pub volume_monitor: (bool, Instant, bool, PeakVolumeMonitor, PeakVolumeMonitor),
    pub volume_normalizer: Option<PeakNormalizer>
}

impl AudioProcessor {
    pub fn process_audio(&mut self, data: &[f32]) {
        self.processing_buffer.clear();
        self.processing_buffer.extend_from_slice(data);
        self.pedal_command_to_client_buffer.clear();

        // Volume Normalization
        if let Some(normalizer) = &mut self.volume_normalizer {
            normalizer.process_buffer(&mut self.processing_buffer);
        } else {
            self.processing_buffer.iter_mut().for_each(|sample| *sample *= self.master_in_volume);
        }

        // Update input volume monitor
        self.volume_monitor.3.add_samples(&self.processing_buffer);

        if data.iter().all(|&sample| sample == 0.0) {
            log::debug!("Buffer is silent, skipping processing.");
        } else if let Some((tuner_writer, frequency_channel_recv, _kill)) = &mut self.tuner_handle {
            // Tuner
            tuner_writer.push_slice(data);
            
            if !frequency_channel_recv.is_empty() {
                match frequency_channel_recv.recv() {
                    Ok(frequency) => {
                        let command = format!("tuner {:.2}\n", frequency);
                        if self.command_sender.send(command.into()).is_err() {
                            log::error!("Failed to send tuner command to client");
                        }
                    },
                    Err(e) => {
                        log::error!("Failed to receive frequency from tuner: {}", e);
                    }
                }
            }
        } else {
            // Main pedal audio processing

            // In case data is larger than buffer_size and pedals may only expect buffer_size or less,
            // we process the data in chunks of FRAMES_PER_PERIOD.
            for i in 0..(data.len() as f32 / self.settings.frames_per_period as f32).ceil() as usize {
                let start = i * self.settings.frames_per_period;
                let mut end = start + self.settings.frames_per_period;
                end = end.min(self.processing_buffer.len());
                let frame = &mut self.processing_buffer[start..end];
                self.pedalboard_set.process_audio(frame, &mut self.pedal_command_to_client_buffer);
            }

            self.processing_buffer.iter_mut().for_each(|sample| *sample *= self.master_out_volume);   
        }

        // Update output volume monitor
        self.volume_monitor.4.add_samples(&self.processing_buffer);

        // Add metronome click
        if self.metronome.0 {
            self.metronome.1.add_to_buffer(&mut self.processing_buffer);
        }

        let written = self.writer.push_slice(&self.processing_buffer);
        if written != self.processing_buffer.len() {
            // XRun occurred
            if let Err(e) = self.command_sender.send("xrun\n".into()) {
                log::error!("Failed to send xrun command: {}", e);
            }
            log::error!("Failed to write all processed data. Output is behind.")
        }

        // Send volume monitor to client
        if self.volume_monitor.0 {
            if Instant::now().duration_since(self.volume_monitor.1) >= DEFAULT_VOLUME_MONITOR_UPDATE_RATE {
                self.volume_monitor.1 = Instant::now();

                let in_peak = self.volume_monitor.3.take_peak();
                let out_peak = self.volume_monitor.4.take_peak();

                let epsilon = 5e-4;
                let both_zero = in_peak.abs() < epsilon && out_peak.abs() < epsilon;

                // Prevent sending multiple consecutive zeros
                if !(both_zero && self.volume_monitor.2) {
                    let command = format!("volumemonitor {:.3} {:.3}\n", in_peak, out_peak); 
                    if self.command_sender.send(command.into()).is_err() {
                        log::error!("Failed to send volume monitor command to client");
                    }
                }

                self.volume_monitor.2 = both_zero;
            }
        }

        // Send any commands from pedals to client
        for mut command in self.pedal_command_to_client_buffer.drain(..) {
            command.push('\n');
            if self.command_sender.send(command.into()).is_err() {
                log::error!("Failed to send pedal command to client");
            }
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
            "kill" => {
                log::info!("Received kill command, shutting down server.");
                std::process::exit(0);
            },
            "disconnect" => {
                // The client has disconnected, stop tuner if it is running
                if let Some((_, _, k)) = self.tuner_handle.take() {
                    k.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            },
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
                pedal.set_config(self.settings.frames_per_period, 48000);
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
                let mut pedalboardset: PedalboardSet = serde_json::from_str(&pedalboardset_stringified).ok()?;

                // Call set_config on every pedal
                for pedalboard in &mut pedalboardset.pedalboards {
                    for pedal in &mut pedalboard.pedals {
                        pedal.set_config(self.settings.frames_per_period, 48000);
                    }
                }

                self.pedalboard_set = pedalboardset;
            },
            "play" => {
                let pedalboard_index = words.next()?.parse::<usize>().ok()?;
                self.pedalboard_set.set_active_pedalboard(pedalboard_index);
            },
            "masterin" => {
                let volume = words.next()?.parse::<f32>().ok()?;
                self.master_in_volume = volume;
            },
            "masterout" => {
                let volume = words.next()?.parse::<f32>().ok()?;
                self.master_out_volume = volume.clamp(0.0, 1.0);
            },
            "tuner" => {
                let enable_str = words.next()?;
                match enable_str {
                    "on" => {
                        let buffer_size = Yin::minimum_buffer_length(48000, self.settings.tuner_min_freq, self.settings.tuner_periods);
                        let (tuner_writer, tuner_reader) = HeapRb::new(buffer_size).split();
                        let (frequency_channel_send, frequency_channel_recv) = crossbeam::channel::bounded(1);
                        let yin = Yin::new(
                            0.2,
                            self.settings.tuner_min_freq,
                            self.settings.tuner_max_freq,
                            48000,
                            self.settings.tuner_periods,
                            tuner_reader,
                        );
                        let kill = Arc::new(AtomicBool::new(false));
                        crate::tuner::start_tuner(yin, kill.clone(), frequency_channel_send);
                        self.tuner_handle = Some((tuner_writer, frequency_channel_recv, kill));
                    },
                    "off" => {
                        if let Some((_, _, kill)) = self.tuner_handle.take() {
                            kill.store(true, std::sync::atomic::Ordering::Relaxed);
                        }
                    },
                    _ => {
                        log::error!("Invalid value for tuner command: expected 'on' or 'off'");
                        return None;
                    }
                }
            },
            "metronome" => {
                let enable_str = words.next()?;
                let bpm = words.next()?.parse::<u32>().ok()?;
                let volume = words.next()?.parse::<f32>().ok()?;
                match enable_str {
                    "on" => {
                        self.metronome.0 = true;
                    },
                    "off" => {
                        self.metronome.0 = false;
                    },
                    _ => {
                        log::error!("Invalid value for metronome command: expected 'on' or 'off'");
                        return None;
                    }
                }

                self.metronome.1.bpm = bpm;
                self.metronome.1.volume = volume.clamp(0.0, 1.0);
            },
            "volumemonitor" => {
                let enable_str = words.next()?;
                match enable_str {
                    "on" => {
                        self.volume_monitor.0 = true;
                    },
                    "off" => {
                        self.volume_monitor.0 = false;
                        self.volume_monitor.3.reset();
                    },
                    _ => {
                        log::error!("Invalid value for volumemonitor command: expected 'on' or 'off'");
                        return None;
                    }
                }
            },
            "volumenormalization" => {
                let mode = words.next()?;
                match mode {
                    "off" => {
                        self.volume_normalizer = None;
                    },
                    "manual" => {
                        self.volume_normalizer = Some(PeakNormalizer::new(0.95, 1.0, self.settings.frames_per_period, 48000));
                    },
                    "automatic" => {
                        let decay = words.next()?.parse::<f32>().ok()?.clamp(0.01, 1.0);
                        self.volume_normalizer = Some(PeakNormalizer::new(0.95, decay, self.settings.frames_per_period, 48000));
                    },
                    "reset" => {
                        if let Some(normalizer) = &mut self.volume_normalizer {
                            normalizer.reset();
                        } else {
                            log::warn!("Volume normalizer is not enabled, cannot reset");
                        }
                    },
                    _ => {
                        log::error!("Invalid value for volumenormalization command: expected 'off', 'manual', 'automatic' or 'reset'");
                        return None;
                    }
                }
            }
            _ => return None
        }

        Some(())
    }
}