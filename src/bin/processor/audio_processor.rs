use crossbeam::channel::Receiver;
use ringbuf::{
    traits::{Producer, Split},
    HeapProd, HeapRb,
};
use smol::channel::{Receiver as SmolReceiver, Sender as SmolSender};
use std::{
    sync::{atomic::AtomicBool, Arc},
    time::Instant,
};

use rs_pedalboard::{
    dsp::{
        eq::Equalizer, resampler::Resampler, yin::Yin, ConvertSampleDbLinear, MAXIMUM_DB_INCREASE,
        MINIMUM_DB,
    },
    pedalboard::Pedalboard,
    pedalboard_set::PedalboardSet,
    pedals::{parameters::ParameterUpdate, GraphicEqSettings, Pedal, PedalTrait},
    processor_settings::FloatSettingUpdate,
    MuteStatus, DEFAULT_VOLUME_MONITOR_UPDATE_RATE,
};

use crate::{
    looper_processor::LooperProcessor, metronome_player::MetronomePlayer,
    recording::RecordingHandle, settings::ProcessorSettings, volume_monitor::PeakVolumeMonitor,
    volume_normalization::PeakNormalizer,
};

pub struct AudioProcessor {
    pub pedalboard_set: PedalboardSet,
    pub command_receiver: SmolReceiver<Box<str>>,
    pub command_sender: SmolSender<Box<str>>,
    pub writer: HeapProd<f32>,
    pub data_buffer: Vec<f32>,
    pub processing_buffer: Vec<f32>,
    pub command_to_client_buffer: Vec<String>,

    pub master_in_volume_db: f32,
    pub master_out_volume_db: f32,
    pub mute_status: MuteStatus,

    pub settings: ProcessorSettings,

    // If tuner is enabled, this will contain the writer to the tuner buffer,
    // a receiver for frequency updates, and a kill flag
    pub tuner_handle: Option<(HeapProd<f32>, Receiver<f32>, Arc<AtomicBool>)>,
    // Enabled?, metronome
    pub metronome: (bool, MetronomePlayer),
    // None if it failed to initialize
    pub looper: Option<LooperProcessor>,

    // Enabled?, last sent time, last sent values, input volume monitor, output volume monitor
    pub volume_monitor: (
        bool,
        Instant,
        (f32, f32),
        PeakVolumeMonitor,
        PeakVolumeMonitor,
    ),
    pub volume_normalizer: Option<PeakNormalizer>,

    pub global_input_eq: Option<Equalizer>,
    pub global_output_eq: Option<Equalizer>,

    pub processing_sample_rate: u32,
    pub resamplers: Option<(Resampler, Resampler)>,

    pub recording: RecordingHandle,
}

impl AudioProcessor {
    #[tracing::instrument(level = "trace", skip_all)]
    pub fn process_audio(&mut self, data: &[f32]) {
        self.recording.tick();
        self.data_buffer.clear();
        self.data_buffer.extend_from_slice(data);
        self.command_to_client_buffer.clear();

        self.write_clean_data_to_recording(data);

        // Volume Normalization
        if let Some(normalizer) = &mut self.volume_normalizer {
            normalizer.process_buffer(&mut self.data_buffer);
        } else {
            self.apply_master_in();
        }

        // Update input volume monitor
        self.volume_monitor.3.add_samples(&self.data_buffer);

        // Skip audio processing if output is muted
        if self.mute_status.is_muted() {
            self.volume_monitor.4.reset();
            self.data_buffer.fill(0.0);
            self.write_data_buffer_to_output();
            self.send_volume_monitor_to_client();
            self.handle_all_commands();
            self.send_commands_to_client();
            return;
        }

        self.processing_buffer.clear();
        self.upsample_into_processing_buffer();

        if self.tuner_handle.is_some() {
            self.handle_tuner();
        }

        // If looper is playing or recording, we need to process the audio
        let looper_is_active = self.looper
            .as_ref()
            .map(|l| l.is_playing() || l.is_recording())
            .unwrap_or(false);

        if !self.tuner_handle.is_some() || looper_is_active {
            self.apply_pedalboard_processing();
        }

        self.downsample_into_data_buffer();
        self.write_processed_data_to_recording();
        self.looper_pass();
        self.apply_master_out();

        // Update output volume monitor
        self.volume_monitor.4.add_samples(&self.data_buffer);

        // Add metronome click
        if self.metronome.0 {
            self.metronome
                .1
                .add_to_buffer(&mut self.data_buffer, self.master_out_volume_db);
        }

        self.write_data_buffer_to_output();

        self.send_volume_monitor_to_client();
        self.handle_all_commands();
        self.send_commands_to_client();
    }

    fn looper_pass(&mut self) {
        if let Some(looper) = &mut self.looper {
            looper.push_audio_to_recording_track(&self.data_buffer);
            looper.add_tracks_to_buffer(&mut self.data_buffer);
        }
    }

    fn handle_tuner(&mut self) {
        if let Some((tuner_writer, frequency_channel_recv, _kill)) = &mut self.tuner_handle {
            // Tuner
            tuner_writer.push_slice(self.data_buffer.as_slice());

            if !frequency_channel_recv.is_empty() {
                match frequency_channel_recv.recv() {
                    Ok(frequency) => {
                        let command = format!("tuner {:.2}\n", frequency);
                        if self.command_sender.try_send(command.into()).is_err() {
                            tracing::error!("Failed to send tuner command to client");
                        }
                    }
                    Err(e) => {
                        tracing::error!("Failed to receive frequency from tuner: {}", e);
                    }
                }
            }
        }
    }

    fn apply_pedalboard_processing(&mut self) {
        // we process the data in chunks of FRAMES_PER_PERIOD
        if let Some(input_eq) = &mut self.global_input_eq {
            self.processing_buffer
                .iter_mut()
                .for_each(|sample| *sample = input_eq.process(*sample));
        }

        for i in 0..(self.processing_buffer.len() as f32 / self.settings.frames_per_period as f32)
            .ceil() as usize
        {
            let start = i * self.settings.frames_per_period;
            let mut end = start + self.settings.frames_per_period;
            end = end.min(self.processing_buffer.len());
            let frame = &mut self.processing_buffer[start..end];
            self.pedalboard_set
                .process_audio(frame, &mut self.command_to_client_buffer);
        }

        if let Some(output_eq) = &mut self.global_output_eq {
            self.processing_buffer
                .iter_mut()
                .for_each(|sample| *sample = output_eq.process(*sample));
        }
    }

    fn upsample_into_processing_buffer(&mut self) {
        self.processing_buffer.clear();
        if let Some((upsampler, _)) = &mut self.resamplers {
            self.processing_buffer.resize(
                upsampler.upsample_output_buffer_size(self.data_buffer.len()),
                0.0,
            );
            upsampler.upsample(&self.data_buffer, self.processing_buffer.as_mut_slice());
        } else {
            self.processing_buffer.extend_from_slice(&self.data_buffer);
        }
    }

    fn downsample_into_data_buffer(&mut self) {
        if let Some((_, downsampler)) = &mut self.resamplers {
            downsampler.downsample(&self.processing_buffer, self.data_buffer.as_mut_slice());
        } else {
            self.data_buffer.clear();
            self.data_buffer.extend_from_slice(&self.processing_buffer);
        }
    }

    fn apply_master_in(&mut self) {
        let master_in_volume_linear = f32::db_to_linear(self.master_in_volume_db);
        self.data_buffer
            .iter_mut()
            .for_each(|sample| *sample *= master_in_volume_linear);
    }

    fn apply_master_out(&mut self) {
        let master_out_volume_linear = f32::db_to_linear(self.master_out_volume_db);
        self.data_buffer
            .iter_mut()
            .for_each(|sample| *sample *= master_out_volume_linear);
    }

    fn write_clean_data_to_recording(&mut self, data: &[f32]) {
        if self.recording.is_recording() {
            if let Some(producer) = self.recording.clean_recording_producer() {
                let written = producer.push_slice(data);
                if written != data.len() {
                    tracing::warn!(
                        "RecordingHandle: Recording ring buffer full, dropping samples."
                    );
                }
            }
        }
    }

    fn write_processed_data_to_recording(&mut self) {
        if self.recording.is_recording() {
            if let Some(producer) = self.recording.recording_producer() {
                let written = producer.push_slice(&self.data_buffer);
                if written != self.data_buffer.len() {
                    tracing::warn!(
                        "RecordingHandle: Recording ring buffer full, dropping samples."
                    );
                }
            }
        }
    }

    fn write_data_buffer_to_output(&mut self) {
        let written = self.writer.push_slice(&self.data_buffer);
        if written != self.data_buffer.len() {
            // XRun occurred
            if let Err(e) = self.command_sender.try_send("xrun\n".into()) {
                tracing::error!("Failed to send xrun command: {}", e);
            }
            tracing::warn!("Failed to write all processed data. Output is behind.")
        }
    }

    fn send_volume_monitor_to_client(&mut self) {
        if self.volume_monitor.0 {
            if Instant::now().duration_since(self.volume_monitor.1)
                >= DEFAULT_VOLUME_MONITOR_UPDATE_RATE
            {
                self.volume_monitor.1 = Instant::now();

                let in_peak = self.volume_monitor.3.take_peak_db();
                let out_peak = self.volume_monitor.4.take_peak_db();

                // Prevent sending multiple consecutive same values
                let eps = 5e-3;
                if !((self.volume_monitor.2 .0 - in_peak).abs() < eps
                    && (self.volume_monitor.2 .1 - out_peak).abs() < eps)
                {
                    let command = format!("volumemonitor {:.3} {:.3}\n", in_peak, out_peak);
                    if self.command_sender.try_send(command.into()).is_err() {
                        tracing::error!("Failed to send volume monitor command to client");
                    }
                }

                self.volume_monitor.2 = (in_peak, out_peak);
            }
        }
    }

    fn send_commands_to_client(&mut self) {
        for mut command in self.command_to_client_buffer.drain(..) {
            command.push('\n');
            if let Err(e) = self.command_sender.try_send(command.into()) {
                tracing::error!("Failed to send command to client: {}", e);
            }
        }
    }

    fn handle_all_commands(&mut self) {
        while let Ok(command) = self.command_receiver.try_recv() {
            if let Err(e) = self.handle_command(command) {
                tracing::error!("Failed to handle command: {e}");
            }
        }
    }

    #[tracing::instrument(level = "trace", skip(self))]
    fn handle_command(&mut self, command: Box<str>) -> Result<(), String> {
        let mut arguments = command.split('|');
        let command_name = arguments
            .next()
            .ok_or_else(|| "No command name found".to_string())?;

        match command_name {
            "kill" => {
                tracing::info!("Received kill command, shutting down processor.");
                std::process::exit(0);
            }
            "disconnect" => {
                // The client has disconnected, stop tuner if it is running
                if let Some((_, _, k)) = self.tuner_handle.take() {
                    k.store(true, std::sync::atomic::Ordering::Relaxed);
                }
            }
            "setparameter" => {
                let pedalboard_id = arguments
                    .next()
                    .ok_or_else(|| "setparameter: Failed to get pedalboard ID".to_string())?
                    .parse::<u32>()
                    .map_err(|e| format!("setparameter: Failed to parse pedalboard ID: {e}"))?;
                let pedal_id = arguments
                    .next()
                    .ok_or_else(|| "setparameter: Failed to get pedal ID".to_string())?
                    .parse::<u32>()
                    .map_err(|e| format!("setparameter: Failed to parse pedal ID: {e}"))?;
                let parameter_name = arguments
                    .next()
                    .ok_or_else(|| "setparameter: Failed to get parameter name".to_string())?;

                let parameter_update_ser_first_word = arguments
                    .next()
                    .ok_or_else(|| "setparameter: Failed to get parameter update".to_string())?;
                let parameter_update_ser_start =
                    parameter_update_ser_first_word.as_ptr() as usize - command.as_ptr() as usize;
                let parameter_update_str = &command[parameter_update_ser_start..];
                let mut parameter_update: ParameterUpdate =
                    serde_json::from_str(parameter_update_str).map_err(|e| {
                        format!(
                            "setparameter: Failed to deserialize parameter update: {}",
                            e
                        )
                    })?;

                // If the update is an oscillator, we must change the sample rate to whatever the processor is using
                parameter_update.set_oscillator_sample_rate(self.processing_sample_rate as f32);

                for pedalboard in self
                    .pedalboard_set
                    .pedalboards
                    .iter_mut()
                    .filter(|pedalboard| pedalboard.get_id() == pedalboard_id)
                {
                    let pedal = pedalboard
                        .pedals
                        .iter_mut()
                        .find(|pedal| pedal.get_id() == pedal_id)
                        .ok_or_else(|| {
                            format!(
                                "setparameter: Pedal with ID {} not found in pedalboard {}",
                                pedal_id, pedalboard_id
                            )
                        })?;
                    parameter_update.apply_to_pedal(pedal, parameter_name);
                }
            }
            "movepedalboard" => {
                let src_index = arguments
                    .next()
                    .ok_or_else(|| "movepedalboard: Failed to get source index".to_string())?
                    .parse::<usize>()
                    .map_err(|e| format!("movepedalboard: Failed to parse source index: {e}"))?;
                let dest_index = arguments
                    .next()
                    .ok_or_else(|| "movepedalboard: Failed to get destination index".to_string())?
                    .parse::<usize>()
                    .map_err(|e| {
                        format!("movepedalboard: Failed to parse destination index: {e}")
                    })?;

                let pedalboard = self.pedalboard_set.pedalboards.remove(src_index);

                let shifted_dest_index = if dest_index > src_index {
                    dest_index - 1
                } else {
                    dest_index
                };

                self.pedalboard_set
                    .pedalboards
                    .insert(shifted_dest_index, pedalboard);
            }
            "addpedalboard" => {
                let pedalboard_stringified = &command[command_name.len() + 1..];
                let mut pedalboard: Pedalboard = serde_json::from_str(&pedalboard_stringified)
                    .map_err(|e| {
                        format!("addpedalboard: Failed to deserialize pedalboard: {}", e)
                    })?;

                for pedal in &mut pedalboard.pedals {
                    pedal.set_config(self.settings.frames_per_period, self.processing_sample_rate);
                }

                self.pedalboard_set.pedalboards.push(pedalboard);
            }
            "deletepedalboard" => {
                let pedalboard_index_str = arguments.next().ok_or_else(|| {
                    "deletepedalboard: Failed to get pedalboard index".to_string()
                })?;

                let pedalboard_index = if pedalboard_index_str == "active" {
                    self.pedalboard_set.active_pedalboard
                } else {
                    pedalboard_index_str.parse::<usize>().map_err(|e| {
                        format!("deletepedalboard: Failed to parse pedalboard index: {e}")
                    })?
                };
                self.pedalboard_set.pedalboards.remove(pedalboard_index);
            }
            "addpedal" => {
                let pedalboard_id = arguments
                    .next()
                    .ok_or_else(|| "addpedal: Failed to get pedalboard ID".to_string())?
                    .parse::<u32>()
                    .map_err(|e| format!("addpedal: Failed to parse pedalboard ID: {e}"))?;

                let pedal_ser_start = arguments
                    .next()
                    .ok_or_else(|| "addpedal: Failed to get pedal".to_string())?;
                let pedalboard_ser_start_index =
                    pedal_ser_start.as_ptr() as usize - command.as_ptr() as usize;
                let pedal_stringified = &command[pedalboard_ser_start_index..];

                let mut pedal: Pedal = serde_json::from_str(&pedal_stringified)
                    .map_err(|e| format!("addpedal: Failed to deserialize pedal: {}", e))?;
                pedal.set_config(self.settings.frames_per_period, self.processing_sample_rate);

                for pedalboard in self.pedalboard_set.pedalboards.iter_mut() {
                    if pedalboard.get_id() == pedalboard_id {
                        pedalboard.pedals.push(pedal.clone());
                    }
                }
            }
            "deletepedal" => {
                let pedalboard_id = arguments
                    .next()
                    .ok_or_else(|| "deletepedal: Failed to get pedalboard ID".to_string())?
                    .parse::<u32>()
                    .map_err(|e| format!("deletepedal: Failed to parse pedalboard ID: {e}"))?;
                let pedal_id = arguments
                    .next()
                    .ok_or_else(|| "deletepedal: Failed to get pedal ID".to_string())?
                    .parse::<u32>()
                    .map_err(|e| format!("deletepedal: Failed to parse pedal ID: {e}"))?;

                for pedalboard in self
                    .pedalboard_set
                    .pedalboards
                    .iter_mut()
                    .filter(|pedalboard| pedalboard.get_id() == pedalboard_id)
                {
                    pedalboard.pedals.retain(|p| p.get_id() != pedal_id);
                }
            }
            "movepedal" => {
                let pedalboard_id = arguments
                    .next()
                    .ok_or_else(|| "movepedal: Failed to get pedalboard ID".to_string())?
                    .parse::<u32>()
                    .map_err(|e| format!("movepedal: Failed to parse pedalboard ID: {e}"))?;
                let pedal_id = arguments
                    .next()
                    .ok_or_else(|| "movepedal: Failed to get pedal ID".to_string())?
                    .parse::<usize>()
                    .map_err(|e| format!("movepedal: Failed to parse pedal ID: {e}"))?;
                let dest_index = arguments
                    .next()
                    .ok_or_else(|| "movepedal: Failed to get destination index".to_string())?
                    .parse::<usize>()
                    .map_err(|e| format!("movepedal: Failed to parse destination index: {e}"))?;

                for pedalboard in &mut self.pedalboard_set.pedalboards {
                    if pedalboard.get_id() == pedalboard_id {
                        let pedal_index = pedalboard
                            .pedals
                            .iter()
                            .position(|p| p.get_id() as usize == pedal_id)
                            .ok_or_else(|| {
                                format!(
                                    "movepedal: Pedal with ID {} not found in pedalboard {}",
                                    pedal_id, pedalboard_id
                                )
                            })?;
                        let pedal = pedalboard.pedals.remove(pedal_index);

                        let shifted_dest_index = if dest_index > pedal_index {
                            dest_index - 1
                        } else {
                            dest_index
                        };

                        pedalboard.pedals.insert(shifted_dest_index, pedal.clone());
                    }
                }
            }
            "loadset" => {
                let pedalboardset_stringified = &command[command_name.len() + 1..];
                let mut pedalboardset: PedalboardSet =
                    serde_json::from_str(&pedalboardset_stringified)
                        .ok()
                        .ok_or_else(|| {
                            "loadset: Failed to deserialize pedalboard set".to_string()
                        })?;

                // Call set_config on every pedal
                for pedalboard in &mut pedalboardset.pedalboards {
                    for pedal in &mut pedalboard.pedals {
                        pedal.set_config(
                            self.settings.frames_per_period,
                            self.processing_sample_rate,
                        );
                    }
                }

                self.pedalboard_set = pedalboardset;
            }
            "play" => {
                let pedalboard_index = arguments
                    .next()
                    .ok_or_else(|| "play: Failed to get pedalboard index".to_string())?
                    .parse::<usize>()
                    .map_err(|e| format!("play: Failed to parse pedalboard index: {e}"))?;
                self.pedalboard_set.set_active_pedalboard(pedalboard_index);
            }
            "nextpedalboard" => {
                self.pedalboard_set.active_pedalboard = (self.pedalboard_set.active_pedalboard + 1)
                    % self.pedalboard_set.pedalboards.len();
            }
            "prevpedalboard" => {
                if self.pedalboard_set.active_pedalboard == 0 {
                    self.pedalboard_set.active_pedalboard =
                        self.pedalboard_set.pedalboards.len() - 1;
                } else {
                    self.pedalboard_set.active_pedalboard -= 1;
                }
            }
            "masterin" => {
                let volume_update_ser = arguments
                    .next()
                    .ok_or_else(|| "masterin: Failed to get volume update".to_string())?;
                let volume_update: FloatSettingUpdate = serde_json::from_str(volume_update_ser)
                    .map_err(|e| format!("masterin: Failed to deserialize volume update: {e}"))?;
                self.master_in_volume_db = volume_update.apply(
                    self.master_in_volume_db,
                    Some((MINIMUM_DB, MAXIMUM_DB_INCREASE)),
                );
            }
            "masterout" => {
                let volume_update_ser = arguments
                    .next()
                    .ok_or_else(|| "masterout: Failed to get volume update".to_string())?;
                let volume_update: FloatSettingUpdate = serde_json::from_str(volume_update_ser)
                    .map_err(|e| format!("masterout: Failed to deserialize volume update: {e}"))?;
                self.master_out_volume_db =
                    volume_update.apply(self.master_out_volume_db, Some((MINIMUM_DB, 0.0)));
            }
            "masterindb" => {
                let volume_db_str = arguments
                    .next()
                    .ok_or_else(|| "masterindb: Failed to get volume db".to_string())?;
                let volume_db = volume_db_str
                    .parse::<f32>()
                    .map_err(|e| format!("masterindb: Failed to parse volume db: {e}"))?;
                self.master_in_volume_db = volume_db.clamp(MINIMUM_DB, MAXIMUM_DB_INCREASE);
            }
            "masteroutdb" => {
                let volume_db_str = arguments
                    .next()
                    .ok_or_else(|| "masteroutdb: Failed to get volume db".to_string())?;
                let volume_db = volume_db_str
                    .parse::<f32>()
                    .map_err(|e| format!("masteroutdb: Failed to parse volume db: {e}"))?;
                self.master_out_volume_db = volume_db.clamp(MINIMUM_DB, 0.0);
            }
            "mutestatus" => {
                let mute_status_str = arguments
                    .next()
                    .ok_or_else(|| "mutestatus has no argument".to_string())?;
                let mute_status: MuteStatus = serde_json::from_str(mute_status_str)
                    .map_err(|e| format!("Failed to deserialize MuteStatus: {}", e))?;

                self.mute_status = mute_status;
            }
            "mutetoggle" => {
                self.mute_status = self.mute_status.toggled();
            }
            "tuner" => {
                let enable_str = arguments
                    .next()
                    .ok_or_else(|| "tuner: Failed to get on/off/toggle".to_string())?;
                let mut enable = self.tuner_handle.is_some();
                match enable_str {
                    "on" => {
                        enable = true;
                    }
                    "off" => {
                        enable = false;
                    }
                    "toggle" => {
                        enable = !enable;
                    }
                    _ => {
                        return Err(
                            "Invalid value for tuner command, expected on/off/toggle".to_string()
                        );
                    }
                }

                if enable {
                    let buffer_size = Yin::minimum_buffer_length(
                        self.processing_sample_rate,
                        self.settings.tuner_min_freq,
                        self.settings.tuner_periods,
                    );
                    let (tuner_writer, tuner_reader) = HeapRb::new(buffer_size).split();
                    let (frequency_channel_send, frequency_channel_recv) =
                        crossbeam::channel::bounded(1);
                    let yin = Yin::new(
                        0.2,
                        self.settings.tuner_min_freq,
                        self.settings.tuner_max_freq,
                        self.processing_sample_rate,
                        self.settings.tuner_periods,
                        tuner_reader,
                    );
                    let kill = Arc::new(AtomicBool::new(false));
                    if let Err(e) =
                        crate::tuner::start_tuner(yin, kill.clone(), frequency_channel_send)
                    {
                        return Err(format!("Failed to start tuner thread: {}", e));
                    } else {
                        self.tuner_handle = Some((tuner_writer, frequency_channel_recv, kill));
                    }
                } else {
                    if let Some((_, _, kill)) = self.tuner_handle.take() {
                        kill.store(true, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            }
            "metronome" => {
                let enable_str = arguments
                    .next()
                    .ok_or_else(|| "metronome: Failed to get on/off/toggle".to_string())?;
                match enable_str {
                    "on" => {
                        self.metronome.0 = true;
                    }
                    "off" => {
                        self.metronome.0 = false;
                    }
                    "toggle" => {
                        self.metronome.0 = !self.metronome.0;
                    }
                    _ => {
                        return Err(
                            "Invalid value for metronome command, expected on/off/toggle"
                                .to_string(),
                        );
                    }
                }

                if let Some(bpm) = arguments.next().and_then(|s| s.parse::<u32>().ok()) {
                    self.metronome.1.bpm = bpm;
                }
                if let Some(volume) = arguments.next().and_then(|s| s.parse::<f32>().ok()) {
                    self.metronome.1.volume = volume.clamp(0.0, 1.0);
                }
            }
            "volumemonitor" => {
                let enable_str = arguments
                    .next()
                    .ok_or_else(|| "volumemonitor: Failed to get on/off/toggle".to_string())?;
                match enable_str {
                    "on" => {
                        self.volume_monitor.0 = true;
                    }
                    "off" => {
                        self.volume_monitor.0 = false;
                        self.volume_monitor.3.reset();
                    }
                    "toggle" => {
                        self.volume_monitor.0 = !self.volume_monitor.0;
                        self.volume_monitor.3.reset();
                    }
                    _ => {
                        return Err(
                            "Invalid value for volumemonitor command, expected on/off/toggle"
                                .to_string(),
                        );
                    }
                }
            }
            "volumenormalization" => {
                let mode = arguments
                    .next()
                    .ok_or_else(|| "volumenormalization: No arguments given")?;
                match mode {
                    "none" => {
                        self.volume_normalizer = None;
                    }
                    "automatic" => {
                        self.volume_normalizer = Some(PeakNormalizer::new(
                            0.95,
                            1.0,
                            self.settings.frames_per_period,
                            self.processing_sample_rate,
                        ));
                    }
                    "automaticdecaying" => {
                        let decay = arguments
                            .next()
                            .ok_or_else(|| {
                                "volumenormalization: Failed to get decay value".to_string()
                            })?
                            .parse::<f32>()
                            .ok()
                            .ok_or_else(|| {
                                "volumenormalization: Failed to parse decay value".to_string()
                            })?
                            .clamp(0.01, 1.0);
                        self.volume_normalizer = Some(PeakNormalizer::new(
                            0.95,
                            decay,
                            self.settings.frames_per_period,
                            self.processing_sample_rate,
                        ));
                    }
                    "reset" => {
                        if let Some(normalizer) = &mut self.volume_normalizer {
                            normalizer.reset();
                        } else {
                            tracing::warn!("Volume normalizer is not enabled, cannot reset");
                        }
                    }
                    _ => {
                        return Err("Invalid value for volumenormalization command, expected none/automatic/automaticdecaying/reset".to_string());
                    }
                }
            }
            "setinputeq" => {
                self.global_input_eq =
                    parse_eq_command(&command, "setinputeq", self.processing_sample_rate)?;
            }
            "setoutputeq" => {
                self.global_output_eq =
                    parse_eq_command(&command, "setoutputeq", self.processing_sample_rate)?;
            }
            "requestsr" => {
                self.command_sender
                    .try_send(format!("sr {}\n", self.processing_sample_rate).into())
                    .map_err(|e| format!("Failed to send sample rate response: {}", e))?;
            }
            "recording" => {
                let enable_str = arguments
                    .next()
                    .ok_or_else(|| "recording: Failed to get on/off/toggle".to_string())?;
                match enable_str {
                    "on" => {
                        self.recording.start_recording();
                    }
                    "off" => {
                        self.recording.stop_recording();
                    }
                    "toggle" => {
                        if self.recording.is_recording() {
                            self.recording.stop_recording();
                        } else {
                            self.recording.start_recording();
                        }
                    }
                    _ => {
                        return Err(
                            "Invalid value for recording command: expected 'on' or 'off'"
                                .to_string(),
                        );
                    }
                }
            }
            "recordclean" => {
                let enable_str = arguments
                    .next()
                    .ok_or_else(|| "recordclean: Failed to get on/off/toggle".to_string())?;
                match enable_str {
                    "on" => {
                        self.recording.set_clean(true);
                    }
                    "off" => {
                        self.recording.set_clean(false);
                    }
                    "toggle" => {
                        self.recording.set_clean(!self.recording.is_clean());
                    }
                    _ => {
                        return Err(
                            "Invalid value for recordclean command: expected 'on' or 'off'"
                                .to_string(),
                        );
                    }
                }
            }
            "setrecordingdir" => {
                let dir_str = &command[command_name.len() + 1..];
                let dir_path = std::path::PathBuf::from(dir_str);
                if dir_path.is_dir() {
                    self.settings.recording_dir = dir_path;
                } else {
                    return Err(format!(
                        "Invalid directory for setrecordingdir command: {dir_path:?}"
                    ));
                }
            }
            "looper" => {
                if let Some(looper) = &mut self.looper {
                    if command.len() <= command_name.len() + 1 {
                        return Err("looper: No subcommand given".to_string());
                    }
                    let subcommand = &command[command_name.len() + 1..];

                    let pedalboard = self
                        .pedalboard_set
                        .pedalboards
                        .get_mut(self.pedalboard_set.active_pedalboard)
                        .ok_or_else(|| {
                            format!(
                                "looper: Active pedalboard index {} is out of bounds",
                                self.pedalboard_set.active_pedalboard
                            )
                        })?;

                    looper.handle_client_subcommand(
                        subcommand,
                        &mut self.command_to_client_buffer,
                        pedalboard,
                        self.settings.frames_per_period,
                        self.processing_sample_rate
                    );
                } else {
                    tracing::warn!("Looper is not enabled, cannot handle looper command");
                }
            }
            _ => return Err(format!("Unknown command: {}", command_name)),
        }

        Ok(())
    }
}

/// The EQ a global EQ command applies, or `None` when it applies no EQ at all.
///
/// The EQ is the rest of the command rather than an argument, so a name containing a separator
/// character cannot be mistaken for another argument. It is built for the rate audio is processed
/// at, which is higher than the device's when the processor is upsampling.
fn parse_eq_command(
    command: &str,
    command_name: &str,
    sample_rate: u32,
) -> Result<Option<Equalizer>, String> {
    let eq_str = command
        .strip_prefix(&format!("{command_name}|"))
        .ok_or_else(|| format!("{command_name}: No EQ was given"))?;

    if eq_str == "none" {
        return Ok(None);
    }

    let eq: GraphicEqSettings = serde_json::from_str(eq_str)
        .map_err(|e| format!("{command_name}: Failed to deserialize global EQ: {e}"))?;

    Ok(Some(eq.build_eq(sample_rate as f32)))
}
