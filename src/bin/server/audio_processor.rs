use std::{sync::{atomic::AtomicBool, Arc}, time::Instant};
use smol::channel::{Receiver as SmolReceiver, Sender as SmolSender};
use crossbeam::channel::Receiver;
use ringbuf::{traits::{Producer, Split}, HeapProd, HeapRb};

use rs_pedalboard::{
    dsp_algorithms::{resampler::Resampler, yin::Yin}, pedalboard::Pedalboard, pedalboard_set::PedalboardSet, pedals::{Pedal, PedalParameterValue, PedalTrait}, DEFAULT_VOLUME_MONITOR_UPDATE_RATE
};

use crate::{
    metronome_player::MetronomePlayer, recording::RecordingHandle, settings::ServerSettings, volume_monitor::PeakVolumeMonitor, volume_normalization::PeakNormalizer
};

pub struct AudioProcessor {
    pub pedalboard_set: PedalboardSet,
    pub command_receiver: SmolReceiver<Box<str>>,
    pub command_sender: SmolSender<Box<str>>,
    pub writer: HeapProd<f32>,
    pub data_buffer: Vec<f32>,
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
    // Enabled?, last sent time, last sent values, input volume monitor, output volume monitor
    pub volume_monitor: (bool, Instant, (f32, f32), PeakVolumeMonitor, PeakVolumeMonitor),
    pub volume_normalizer: Option<PeakNormalizer>,
    pub processing_sample_rate: u32,
    pub resamplers: Option<(Resampler, Resampler)>,
    pub recording: RecordingHandle
}

impl AudioProcessor {
    pub fn process_audio(&mut self, data: &[f32]) {
        self.recording.tick();
        if self.recording.is_recording() {
            if let Some(producer) = self.recording.clean_recording_producer() {
                let written = producer.push_slice(data);
                if written != data.len() {
                    log::warn!("RecordingHandle: Recording ring buffer full, dropping samples.");
                }
            }
        }

        self.data_buffer.clear();
        self.data_buffer.extend_from_slice(data);
        self.pedal_command_to_client_buffer.clear();

        // Volume Normalization
        if let Some(normalizer) = &mut self.volume_normalizer {
            normalizer.process_buffer(&mut self.data_buffer);
        } else {
            self.data_buffer.iter_mut().for_each(|sample| *sample *= self.master_in_volume);
        }

        // Update input volume monitor
        self.volume_monitor.3.add_samples(&self.data_buffer);
        
        // Upsample, if needed, into processing buffer
        self.processing_buffer.clear();
        if let Some((upsampler, _)) = &mut self.resamplers {
            self.processing_buffer.resize(upsampler.upsample_output_buffer_size(self.data_buffer.len()), 0.0);
            upsampler.upsample(&self.data_buffer, self.processing_buffer.as_mut_slice());
        } else {
            self.processing_buffer.extend_from_slice(&self.data_buffer);
        }

        if self.data_buffer.iter().all(|&sample| sample == 0.0) {
            log::debug!("Buffer is silent, skipping processing.");
        } else if let Some((tuner_writer, frequency_channel_recv, _kill)) = &mut self.tuner_handle {
            // Tuner
            tuner_writer.push_slice(self.data_buffer.as_slice());
            
            if !frequency_channel_recv.is_empty() {
                match frequency_channel_recv.recv() {
                    Ok(frequency) => {
                        let command = format!("tuner {:.2}\n", frequency);
                        if self.command_sender.try_send(command.into()).is_err() {
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
            // we process the data in chunks of FRAMES_PER_PERIOD
            for i in 0..(self.processing_buffer.len() as f32 / self.settings.frames_per_period as f32).ceil() as usize {
                let start = i * self.settings.frames_per_period;
                let mut end = start + self.settings.frames_per_period;
                end = end.min(self.processing_buffer.len());
                let frame = &mut self.processing_buffer[start..end];
                self.pedalboard_set.process_audio(frame, &mut self.pedal_command_to_client_buffer);
            }

            self.processing_buffer.iter_mut().for_each(|sample| *sample *= self.master_out_volume);   
        }

        // Downsample, if needed, back into data buffer
        if let Some((_, downsampler)) = &mut self.resamplers {
            downsampler.downsample(&self.processing_buffer, self.data_buffer.as_mut_slice());
        } else {
            self.data_buffer.clear();
            self.data_buffer.extend_from_slice(&self.processing_buffer);
        }

        if self.recording.is_recording() {
            if let Some(producer) = self.recording.recording_producer() {
                let written = producer.push_slice(&self.data_buffer);
                if written != self.data_buffer.len() {
                    log::warn!("RecordingHandle: Recording ring buffer full, dropping samples.");
                }
            }
        }

        // Update output volume monitor
        self.volume_monitor.4.add_samples(&self.data_buffer);

        // Add metronome click
        if self.metronome.0 {
            self.metronome.1.add_to_buffer(&mut self.data_buffer);
        }

        let written = self.writer.push_slice(&self.data_buffer);
        if written != self.data_buffer.len() {
            // XRun occurred
            if let Err(e) = self.command_sender.try_send("xrun\n".into()) {
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

                let in_peak_round = (in_peak * 1000.0).round() / 1000.0;
                let out_peak_round = (out_peak * 1000.0).round() / 1000.0;

                // Prevent sending multiple consecutive same values
                let eps = 5e-3;
                if !((self.volume_monitor.2.0 - in_peak_round).abs() < eps && (self.volume_monitor.2.1 - out_peak_round).abs() < eps) {
                    let command = format!("volumemonitor {} {}\n", in_peak_round, out_peak_round); 
                    if self.command_sender.try_send(command.into()).is_err() {
                        log::error!("Failed to send volume monitor command to client");
                    }
                }

                self.volume_monitor.2 = (in_peak_round, out_peak_round);
            }
        }

        // Send any commands from pedals to client
        for mut command in self.pedal_command_to_client_buffer.drain(..) {
            command.push('\n');
            if self.command_sender.try_send(command.into()).is_err() {
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
        let mut arguments = command.split('|');
        let command_name = arguments.next()?;

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
                let pedalboard_id = arguments.next()?.parse::<u32>().ok()?;
                let pedal_id = arguments.next()?.parse::<u32>().ok()?;
                let parameter_name = arguments.next()?;

                let pedal_parameter_ser_start = arguments.next()?.as_ptr() as usize - command.as_ptr() as usize;
                let pedal_parameter_str = &command[pedal_parameter_ser_start..];
                let mut parameter_value: PedalParameterValue = serde_json::from_str(&pedal_parameter_str).ok()?;

                // If the parameter is an oscillator, we must change the sample rate to whatever the server is using
                if let Some(oscillator) = parameter_value.as_oscillator_mut() {
                    oscillator.set_sample_rate(self.processing_sample_rate as f32);
                }

                for pedalboard in self.pedalboard_set.pedalboards.iter_mut().filter(|pedalboard| pedalboard.get_id() == pedalboard_id) {
                    pedalboard.pedals.iter_mut().find(|pedal| pedal.get_id() == pedal_id)?
                        .set_parameter_value(parameter_name, parameter_value.clone());
                }
            },
            "movepedalboard" => {
                let src_index = arguments.next()?.parse::<usize>().ok()?;
                let dest_index = arguments.next()?.parse::<usize>().ok()?;

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
                let mut pedalboard: Pedalboard = serde_json::from_str(&pedalboard_stringified).ok()?;

                for pedal in &mut pedalboard.pedals {
                    pedal.set_config(self.settings.frames_per_period, self.processing_sample_rate);
                }

                self.pedalboard_set.pedalboards.push(pedalboard);
            },
            "deletepedalboard" => {
                let pedalboard_index = arguments.next()?.parse::<usize>().ok()?;
                self.pedalboard_set.pedalboards.remove(pedalboard_index);
            },
            "addpedal" => {
                let pedalboard_id = arguments.next()?.parse::<u32>().ok()?;

                let pedal_ser_start = arguments.next()?;
                let pedalboard_ser_start_index = pedal_ser_start.as_ptr() as usize - command.as_ptr() as usize;
                let pedal_stringified = &command[pedalboard_ser_start_index..];
                
                let mut pedal: Pedal = serde_json::from_str(&pedal_stringified).ok()?;
                pedal.set_config(self.settings.frames_per_period, self.processing_sample_rate);

                for pedalboard in self.pedalboard_set.pedalboards.iter_mut() {
                    if pedalboard.get_id() == pedalboard_id {
                        pedalboard.pedals.push(pedal.clone());
                    }
                }
            },
            "deletepedal" => {
                let pedalboard_id = arguments.next()?.parse::<u32>().ok()?;
                let pedal_id = arguments.next()?.parse::<u32>().ok()?;
                
                for pedalboard in self.pedalboard_set.pedalboards.iter_mut().filter(|pedalboard| pedalboard.get_id() == pedalboard_id) {
                    pedalboard.pedals.retain(|p| p.get_id() != pedal_id);
                }
            },
            "movepedal" => {
                let pedalboard_id = arguments.next()?.parse::<u32>().ok()?;
                let pedal_id = arguments.next()?.parse::<usize>().ok()?;
                let dest_index = arguments.next()?.parse::<usize>().ok()?;

                for pedalboard in &mut self.pedalboard_set.pedalboards {
                    if pedalboard.get_id() == pedalboard_id {
                        let pedal_index = pedalboard.pedals.iter().position(|p| p.get_id() as usize == pedal_id)?;
                        let pedal = pedalboard.pedals.remove(pedal_index);

                        let shifted_dest_index = if dest_index > pedal_index {
                            dest_index - 1
                        } else {
                            dest_index
                        };

                        pedalboard.pedals.insert(shifted_dest_index, pedal.clone());
                    }
                }
            },
            "loadset" => {
                let pedalboardset_stringified = &command[command_name.len() + 1..];
                let mut pedalboardset: PedalboardSet = serde_json::from_str(&pedalboardset_stringified).ok()?;

                // Call set_config on every pedal
                for pedalboard in &mut pedalboardset.pedalboards {
                    for pedal in &mut pedalboard.pedals {
                        pedal.set_config(self.settings.frames_per_period, self.processing_sample_rate);
                    }
                }

                self.pedalboard_set = pedalboardset;
            },
            "play" => {
                let pedalboard_index = arguments.next()?.parse::<usize>().ok()?;
                self.pedalboard_set.set_active_pedalboard(pedalboard_index);
            },
            "masterin" => {
                let volume = arguments.next()?.parse::<f32>().ok()?;
                self.master_in_volume = volume;
            },
            "masterout" => {
                let volume = arguments.next()?.parse::<f32>().ok()?;
                self.master_out_volume = volume.clamp(0.0, 1.0);
            },
            "tuner" => {
                let enable_str = arguments.next()?;
                match enable_str {
                    "on" => {
                        let buffer_size = Yin::minimum_buffer_length(self.processing_sample_rate, self.settings.tuner_min_freq, self.settings.tuner_periods);
                        let (tuner_writer, tuner_reader) = HeapRb::new(buffer_size).split();
                        let (frequency_channel_send, frequency_channel_recv) = crossbeam::channel::bounded(1);
                        let yin = Yin::new(
                            0.2,
                            self.settings.tuner_min_freq,
                            self.settings.tuner_max_freq,
                            self.processing_sample_rate,
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
                let enable_str = arguments.next()?;
                let bpm = arguments.next()?.parse::<u32>().ok()?;
                let volume = arguments.next()?.parse::<f32>().ok()?;
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
                let enable_str = arguments.next()?;
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
                let mode = arguments.next()?;
                match mode {
                    "none" => {
                        self.volume_normalizer = None;
                    },
                    "manual" => {
                        self.volume_normalizer = Some(PeakNormalizer::new(0.95, 1.0, self.settings.frames_per_period, self.processing_sample_rate));
                    },
                    "automatic" => {
                        let decay = arguments.next()?.parse::<f32>().ok()?.clamp(0.01, 1.0);
                        self.volume_normalizer = Some(PeakNormalizer::new(0.95, decay, self.settings.frames_per_period, self.processing_sample_rate));
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
            },
            "requestsr" => {
                if self.command_sender.try_send(format!("sr {}\n", self.processing_sample_rate).into()).is_err() {
                    log::error!("Failed to send sample rate to client");
                }
            },
            "startrecording" => {
                self.recording.start_recording();
            },
            "stoprecording" => {
                self.recording.stop_recording();
            },
            "recordclean" => {
                let enable_str = arguments.next()?;
                match enable_str {
                    "on" => {
                        self.recording.set_clean(true);
                    },
                    "off" => {
                        self.recording.set_clean(false);
                    },
                    _ => {
                        log::error!("Invalid value for recordclean command: expected 'on' or 'off'");
                        return None;
                    }
                }
            },
            "setrecordingdir" => {
                let dir_str = &command[command_name.len() + 1..];
                let dir_path = std::path::PathBuf::from(dir_str);
                if dir_path.is_dir() {
                    self.settings.recording_dir = dir_path;
                } else {
                    log::error!("Invalid directory for setrecordingdir command: {dir_path:?}");
                }
            },
            _ => return None
        }

        Some(())
    }
}