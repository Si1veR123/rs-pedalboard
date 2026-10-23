use rs_pedalboard::processor_api::{resample_audio, save_wav, load_wav};
use rs_pedalboard::pedalboard::Pedalboard;
use rs_pedalboard::looper_state::{LOOPER_AUDIO_SAVE_DIR, LooperInformation, LooperSettings, LooperSync};

const TAIL_MIN_RMS: f32 = 0.001; // Minimum RMS for a tail to be considered significant



pub fn quantize(base_track_length_samples: usize, track_length_samples: usize, maximum_fractionals: usize) -> usize {
    if base_track_length_samples <= 0 || track_length_samples <= 0 || base_track_length_samples == track_length_samples {
        return base_track_length_samples;
    }

    if track_length_samples < base_track_length_samples {
        // Find the closest fractional multiple of the base track length to the track length
        let raw_fraction = base_track_length_samples as f32 / track_length_samples as f32;
        let clamped_fraction = raw_fraction.clamp(1.0, maximum_fractionals as f32);
        (base_track_length_samples as f32 / clamped_fraction.round()) as usize
    } else {
        // Find the closest multiple of the base track length to the track length
        let raw_multiple = track_length_samples as f32 / base_track_length_samples as f32;
        let clamped_multiple = raw_multiple.clamp(1.0, maximum_fractionals as f32);
        (base_track_length_samples as f32 * clamped_multiple.round()) as usize
    }
}

#[derive(Debug, Clone)]
enum TrackPlaybackState {
    Active {
        started_at: usize,
        stopping: bool, // stop once a full track has been played
        first_playback: bool
    },
    WritingTail {
        tail_started_at: usize
    },
    Stopped
}

impl Default for TrackPlaybackState {
    fn default() -> Self {
        Self::Stopped
    }
}

pub struct LooperTrack {
    raw_audio: Vec<f32>,
    // Delay/Reverb tail that is not part of the main loop, but should be played out after the loop ends
    tail: Option<Vec<f32>>,
    sample_rate: u32,
    playback_state: TrackPlaybackState
}

impl LooperTrack {
    pub fn new(raw_audio: Vec<f32>, tail: Option<Vec<f32>>, sample_rate: u32) -> Self {
        Self {
            raw_audio,
            sample_rate,
            tail,
            playback_state: TrackPlaybackState::default()
        }
    }

    pub fn get_length_samples(&self) -> usize {
        self.raw_audio.len()
    }

    pub fn get_length_seconds(&self) -> f32 {
        self.raw_audio.len() as f32 / self.sample_rate as f32
    }

    pub fn resampled(&mut self, new_sample_rate: u32) -> Result<Self, String> {
        let channels = vec![self.raw_audio.clone()];
        let mut resampled_channels = resample_audio(&channels, self.sample_rate as f32, new_sample_rate as f32)?;

        let resampled_tail = if let Some(tail) = self.tail.as_ref() {
            let channels = vec![tail.clone()];
            let mut resampled_tail_channels = resample_audio(&channels, self.sample_rate as f32, new_sample_rate as f32)?;
            Some(resampled_tail_channels.pop().expect("Resampled tail should have a channel"))
        } else {
            None
        };
        Ok(Self::new(resampled_channels.pop().expect("Resampled output should have a channel"), resampled_tail, new_sample_rate))
    }

    pub fn save_to_file(&self, path: &std::path::Path) -> Result<(), String> {
        save_wav(path, &self.raw_audio, self.sample_rate as f32)?;
        
        if let Some(tail) = &self.tail {
            let tail_path = path.with_file_name(format!("{}_tail.wav", path.file_stem().unwrap().to_string_lossy()));
            save_wav(&tail_path, tail, self.sample_rate as f32)?;
        }
        Ok(())
    }

    pub fn load_from_file(path: &std::path::Path, sample_rate: u32) -> Result<Self, String> {
        let mut channels = load_wav(path, sample_rate as f32, false)?;
        let raw_audio = channels.pop().ok_or_else(|| "No audio channels found in file".to_string())?;

        let tail_path = path.with_file_name(format!("{}_tail.wav", path.file_stem().unwrap().to_string_lossy()));
        let tail = if tail_path.exists() {
            let mut tail_channels = load_wav(&tail_path, sample_rate as f32, false)?;
            Some(tail_channels.pop().ok_or_else(|| "No audio channels found in tail file".to_string())?)
        } else {
            None
        };

        let track = Self::new(raw_audio, tail, sample_rate);

        Ok(track)
    }

    pub fn push_audio(&mut self, audio: &[f32]) {
        self.raw_audio.extend_from_slice(audio);
    }

    pub fn start_playback(&mut self, playback_position: usize) {
        if self.raw_audio.is_empty() {
            return;
        }

        let next_quantised_start = (playback_position as f32 / self.get_length_samples() as f32).ceil() as usize * self.get_length_samples();
        self.playback_state = TrackPlaybackState::Active { started_at: next_quantised_start, stopping: false, first_playback: true };
    }

    pub fn stop_playback(&mut self) {
        match self.playback_state {
            TrackPlaybackState::Active { started_at, stopping, first_playback: _ } => {
                if !stopping {
                    self.playback_state = TrackPlaybackState::Active { started_at, stopping: true, first_playback: false };
                }
            },
            _ => {}
        }
    }

    pub fn snap_to_length(&mut self, snap_to_samples: usize, tail: Option<Vec<f32>>) {
        // Strategy:
        // If the snapped length is shorter than the current length, we will push the extra samples into the start of the tail (if it exists), or discard them if not.
        // If the snapped length is longer than the current length, we will pull samples from the start of the tail (if it exists) to fill up to the snapped length, or pad with silence if not.
        
        if snap_to_samples < self.raw_audio.len() {
            // Push extra samples from the end of the track into the start of the tail
            if let Some(mut tail) = tail {
                let tail_to_add = self.raw_audio.drain(snap_to_samples..);
                tail.splice(0..0, tail_to_add);
                tail.truncate(snap_to_samples);
                self.tail = Some(tail);
            } else {
                self.raw_audio.truncate(snap_to_samples);
            }
        } else if snap_to_samples > self.raw_audio.len() {
            let tail_samples_to_add = snap_to_samples - self.raw_audio.len();
            if let Some(mut tail) = tail {
                // Add samples from the start of the tail to fill up to the snapped length
                let tail_to_add = tail.drain(..tail_samples_to_add.min(tail.len()));
                self.raw_audio.extend(tail_to_add);
                self.raw_audio.resize(snap_to_samples, 0.0);

                if !tail.is_empty() {
                    self.tail = Some(tail);
                }
            } else {
                self.raw_audio.resize(snap_to_samples, 0.0);
            }
        }
    }

    fn sum_samples_to_buffer(buffer: &mut [f32], samples: &[f32]) {
        for (b, s) in buffer.iter_mut().zip(samples.iter()) {
            *b += *s;
        }
    }

    pub fn add_track_to_buffer(&mut self, mut buffer: &mut [f32], mut playback_position: usize) {
        match self.playback_state {
            TrackPlaybackState::Active { started_at, stopping, first_playback } => {
                let track_length_samples = self.raw_audio.len();
                let buffer_length = buffer.len();
                let original_playback_position = playback_position;

                if playback_position < started_at {
                    // Track has not started yet
                    if playback_position + buffer_length <= started_at {
                        // Track will not start during this buffer, so do nothing
                        if stopping {
                            self.playback_state = TrackPlaybackState::Stopped;
                        } else {
                            return;
                        }
                    } else {
                        // Crop the buffer and change the playback position to the start of the track
                        let crop_length = started_at - playback_position;
                        buffer = &mut buffer[crop_length.min(buffer_length)..];
                        playback_position = started_at;
                    }
                }

                let mut track_playback_position = (playback_position - started_at) % track_length_samples;
                let mut ready_to_stop = false;
                for buffer_chunk in buffer.chunks_mut(track_length_samples) {
                    // Sum from the track playback position to end of track or end of buffer
                    let samples_to_copy = (track_length_samples - track_playback_position).min(buffer_chunk.len());
                    if samples_to_copy > 0 {
                        let track_slice = &self.raw_audio[track_playback_position..track_playback_position + samples_to_copy];
                        let buffer_slice = &mut buffer_chunk[..samples_to_copy];
                        Self::sum_samples_to_buffer(buffer_slice, track_slice);

                        // If this is not the first time this track has played, we add tail
                        if !first_playback {
                            if let Some(tail) = &self.tail {
                                let tail_length_samples = tail.len();
                                let tail_slice = &tail[track_playback_position..(track_playback_position + samples_to_copy).min(tail_length_samples)];
                                Self::sum_samples_to_buffer(&mut buffer_slice[..tail_slice.len()], tail_slice);
                            }
                        }

                        playback_position += samples_to_copy;
                        track_playback_position = (track_playback_position + samples_to_copy) % track_length_samples;

                        if track_playback_position == 0 {
                            // If we have looped back to the start of the track, we are no longer in the first playback
                            self.playback_state = TrackPlaybackState::Active { started_at, stopping, first_playback: false };
                        }
                    }
                    
                    if stopping && track_playback_position == 0 {
                        ready_to_stop = true;
                        break;
                    } else {
                        // Sum from the start of the track to the end of the buffer chunk
                        let remaining_samples = buffer_chunk.len() - samples_to_copy;
                        if remaining_samples > 0 {
                            let track_slice = &self.raw_audio[..remaining_samples];
                            let buffer_slice = &mut buffer_chunk[samples_to_copy..];
                            Self::sum_samples_to_buffer(buffer_slice, track_slice);

                            // Again, add tail
                            if !first_playback {
                                if let Some(tail) = &self.tail {
                                    let tail_length_samples = tail.len();
                                    let tail_slice = &tail[..remaining_samples.min(tail_length_samples)];
                                    Self::sum_samples_to_buffer(&mut buffer_slice[..tail_slice.len()], tail_slice);
                                }
                            }

                            playback_position += remaining_samples;
                            track_playback_position = remaining_samples;
                        }
                    }

                }

                if ready_to_stop {
                    if self.tail.is_some() {
                        self.playback_state = TrackPlaybackState::WritingTail { tail_started_at: playback_position };
                        // Maybe a little hacky, but we want to start writing the tail in the rest of the buffer
                        let written_samples = playback_position - original_playback_position;
                        let buffer_len = buffer.len();

                        // If we still have space in the buffer, write the tail to it
                        if !(written_samples >= buffer_len) {
                            self.add_track_to_buffer(&mut buffer[written_samples..], playback_position);
                        }
                        return;
                    } else {
                        self.playback_state = TrackPlaybackState::Stopped;
                    }
                }
            }
            TrackPlaybackState::WritingTail { tail_started_at } => {
                // track has finished playing, but we are still writing out the tail
                if let Some(tail) = &self.tail {
                    let tail_length_samples = tail.len();
                    let buffer_length = buffer.len();

                    let tail_start_index = playback_position.saturating_sub(tail_started_at);
                    let samples_to_copy = tail_length_samples.saturating_sub(tail_start_index).min(buffer_length);

                    if samples_to_copy > 0 {
                        let tail_slice = &tail[tail_start_index..tail_start_index + samples_to_copy];
                        let buffer_slice = &mut buffer[..samples_to_copy];
                        Self::sum_samples_to_buffer(buffer_slice, tail_slice);
                    }

                    // If tail is completely written, stop playback
                    if tail_start_index + samples_to_copy >= tail_length_samples {
                        self.playback_state = TrackPlaybackState::Stopped;
                    }
                } else {
                    // No tail, so we just stop playback
                    self.playback_state = TrackPlaybackState::Stopped;
                }
            },
            TrackPlaybackState::Stopped => {
                // Do nothing, track is stopped
            }
        }
    }

    pub fn reverse_effect(&mut self) {
        self.raw_audio.reverse();
        if let Some(tail) = &mut self.tail {
            tail.reverse();
        }
    }

    pub fn half_speed_effect(&mut self) {
        let channels = vec![self.raw_audio.clone()];
        let mut resampled_channels = resample_audio(&channels, self.sample_rate as f32, self.sample_rate as f32 / 2.0).expect("Resampling should not fail");
        self.raw_audio = resampled_channels.pop().expect("Resampled output should have a channel");
        if let Some(tail) = &mut self.tail {
            let channels = vec![tail.clone()];
            let mut resampled_channels = resample_audio(&channels, self.sample_rate as f32, self.sample_rate as f32 / 2.0).expect("Resampling should not fail");
            *tail = resampled_channels.pop().expect("Resampled output should have a channel");
        }
    }
}


#[derive(Debug, Clone)]
enum MultitrackPlaybackState {
    Playing(usize),
    Stopped
}

pub struct LooperMultitrack {
    pub name: String,
    pub tracks: Vec<LooperTrack>,
    pub recording_track: Option<usize>,
    pub sample_rate: u32,
    playback: MultitrackPlaybackState,
}

impl LooperMultitrack {
    pub fn new(name: String, sample_rate: u32) -> Self {
        Self {
            name,
            tracks: Vec::new(),
            recording_track: None,
            sample_rate,
            playback: MultitrackPlaybackState::Stopped
        }
    }

    pub fn new_and_save_to_disk(name: String, sample_rate: u32) -> Result<Self, String> {
        let multitrack = Self::new(name, sample_rate);
        multitrack.save_to_directory()?;
        Ok(multitrack)
    }

    fn tail_is_significant(tail: &[f32]) -> bool {
        let rms = (tail.iter().map(|&sample| sample * sample).sum::<f32>() / tail.len() as f32).sqrt();
        rms >= TAIL_MIN_RMS
    }

    fn capture_tail(&self, pedalboard: &mut Pedalboard, buffer_size: usize, max_tail_samples: usize) -> Option<Vec<f32>> {
        if !pedalboard.has_time_based_pedal() {
            return None;
        }

        // Preallocate 1 second
        let mut tail_buffer = Vec::with_capacity(self.sample_rate as usize * 1);
        let mut processing_buffer = vec![0.0f32; buffer_size];
        let mut null_message_buffer = Vec::new();

        loop {
            if tail_buffer.len() >= max_tail_samples {
                break;
            }

            pedalboard.process_audio(&mut processing_buffer, &mut null_message_buffer);

            if !Self::tail_is_significant(&processing_buffer) {
                break;
            }

            tail_buffer.extend_from_slice(&processing_buffer);
            processing_buffer.fill(0.0);
        }

        if tail_buffer.is_empty() {
            None
        } else {
            tail_buffer.truncate(max_tail_samples);
            Some(tail_buffer)
        }
    }

    /// Track with shortest length in samples, ignoring the track at `ignore` index if provided
    fn get_base_track_length(&self, ignore: Option<usize>) -> Option<usize> {
        self.tracks.iter().enumerate()
            .filter(|(i, _)| Some(*i) != ignore)
            .map(|(_, track)| track.raw_audio.len())
            .filter(|length| *length > 0)
            .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
    }

    pub fn start_recording(&mut self, to: usize) {
        if to >= self.tracks.len() {
            self.tracks.resize_with(to + 1, || LooperTrack::new(Vec::new(), None, self.sample_rate));
        }
        self.recording_track = Some(to);
    }

    fn stop_recording(&mut self, pedalboard: &mut Pedalboard, buffer_size: usize, quantize_length_samples: usize) {
        let recording_track = self.recording_track.take();

        if let Some(recording_track) = recording_track {
            let tail = self.capture_tail(pedalboard, buffer_size, quantize_length_samples);
            self.tracks[recording_track].snap_to_length(quantize_length_samples, tail);
        }
    }

    pub fn stop_recording_dynamic_snap(&mut self, pedalboard: &mut Pedalboard, buffer_size: usize, maximum_fractionals: usize) {
        if let Some(recording_track) = self.recording_track {
            let quantized_length_samples = if let Some(base_track_length) = self.get_base_track_length(Some(recording_track)) {
                quantize(base_track_length, self.tracks[recording_track].get_length_samples(), maximum_fractionals)
            } else {
                self.tracks[recording_track].get_length_samples()
            };

            self.stop_recording(pedalboard, buffer_size, quantized_length_samples);
        }
    }

    pub fn stop_recording_bpm_snap(&mut self, pedalboard: &mut Pedalboard, buffer_size: usize, looper_sync: &LooperSync) {
        if let Some(recording_track) = self.recording_track {
            let recording_samples = self.tracks[recording_track].get_length_samples();
            let quantized_length_samples = looper_sync.quantize(recording_samples, self.sample_rate as f32);
            self.stop_recording(pedalboard, buffer_size, quantized_length_samples);
        }
    }

    pub fn push_audio_to_recording_track(&mut self, audio: &[f32]) {
        if let Some(recording_track) = self.recording_track {
            self.tracks[recording_track].push_audio(audio);
        }
    }

    pub fn add_tracks_to_buffer(&mut self, buffer: &mut [f32]) {
        if !matches!(self.playback, MultitrackPlaybackState::Stopped) && self.tracks.iter().any(|track| !matches!(track.playback_state, TrackPlaybackState::Stopped)) {
            for track in &mut self.tracks {
                match &mut self.playback {
                    MultitrackPlaybackState::Playing(position) => {
                        track.add_track_to_buffer(buffer, *position);
                        *position += buffer.len();
                    },
                    MultitrackPlaybackState::Stopped => {}
                }
            }
        } else {
            // All tracks are stopped, so stop multitrack playback
            self.playback = MultitrackPlaybackState::Stopped;
        }
    }

    pub fn play_all(&mut self) {
        match self.playback {
            MultitrackPlaybackState::Playing(position) => {
                for track in &mut self.tracks {
                    if !matches!(track.playback_state, TrackPlaybackState::Active { .. }) {
                        track.start_playback(position);
                    }
                }
            },
            MultitrackPlaybackState::Stopped => {
                self.playback = MultitrackPlaybackState::Playing(0);
                for track in &mut self.tracks {
                    track.start_playback(0);
                }
            }
        }
    }

    pub fn play_track(&mut self, track_index: usize) {
        if track_index >= self.tracks.len() {
            return;
        }

        match self.playback {
            MultitrackPlaybackState::Playing(position) => {
                if !matches!(self.tracks[track_index].playback_state, TrackPlaybackState::Active { .. }) {
                    self.tracks[track_index].start_playback(position);
                }
            },
            MultitrackPlaybackState::Stopped => {
                self.playback = MultitrackPlaybackState::Playing(0);
                self.tracks[track_index].start_playback(0);
            }
        }
    }

    pub fn stop_all(&mut self) {
        for track in &mut self.tracks {
            track.stop_playback();
        }
    }

    pub fn stop_track(&mut self, track_index: usize) {
        if track_index >= self.tracks.len() {
            return;
        }

        self.tracks[track_index].stop_playback();
    }

    pub fn save_to_directory(&self) -> Result<(), String> {
        let dir_path = std::path::Path::new(LOOPER_AUDIO_SAVE_DIR).join(&self.name);
        std::fs::create_dir_all(&dir_path).map_err(|e| format!("Failed to create directory: {}", e))?;

        for (i, track) in self.tracks.iter().enumerate() {
            let track_path = dir_path.join(format!("track_{}.wav", i));
            track.save_to_file(&track_path)?;
        }

        Ok(())
    }

    pub fn load_from_directory(name: &str, sample_rate: u32) -> Result<Self, String> {
        let dir_path = std::path::Path::new(LOOPER_AUDIO_SAVE_DIR).join(name);
        if !dir_path.exists() {
            return Err(format!("Directory {} does not exist", dir_path.display()));
        }

        let mut tracks = Vec::new();
        for entry in std::fs::read_dir(&dir_path).map_err(|e| format!("Failed to read directory: {}", e))? {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();
            if path.extension().and_then(|s| s.to_str()) == Some("wav") && !path.file_name().and_then(|s| s.to_str()).unwrap_or("").ends_with("_tail.wav") {
                let track = LooperTrack::load_from_file(&path, sample_rate)?;
                tracks.push(track);
            }
        }

        Ok(Self {
            name: name.to_string(),
            tracks,
            recording_track: None,
            sample_rate,
            playback: MultitrackPlaybackState::Stopped
        })
    }
}

pub struct LooperProcessor {
    // store multitracks by name, load when needed
    pub looper_information: LooperInformation,
    pub active_multitrack: Option<LooperMultitrack>,
    selected_track_index: Option<usize>,
    sample_rate: u32
}

impl Default for LooperProcessor {
    fn default() -> Self {
        Self {
            looper_information: LooperInformation::default(),
            active_multitrack: None,
            selected_track_index: None,
            sample_rate: 44100
        }
    }
}

impl LooperProcessor {
    pub fn load(sample_rate: u32) -> Result<Self, String> {
        let looper_information = LooperInformation::from_disk()?;
        Ok(Self {
            looper_information,
            active_multitrack: None,
            selected_track_index: None,
            sample_rate
        })
    }

    pub fn start_recording(&mut self, to: usize) {
        if let Some(active_multitrack) = &mut self.active_multitrack {
            active_multitrack.start_recording(to);
        }
    }

    pub fn stop_recording(&mut self, pedalboard: &mut Pedalboard, buffer_size: usize) {
        if let Some(active_multitrack) = &mut self.active_multitrack {
            if active_multitrack.tracks.len() > 1 {
                // If there are multiple tracks, snap according to multiples of the shortest track length
                active_multitrack.stop_recording_dynamic_snap(pedalboard, buffer_size, self.looper_information.settings.maximum_fractionals);
            } else {
                // This is the first track, so if we have a looper sync, snap to the nearest BPM
                if let Some(looper_sync) = &self.looper_information.settings.looper_sync {
                    active_multitrack.stop_recording_bpm_snap(pedalboard, buffer_size, looper_sync);
                } else {
                    active_multitrack.stop_recording_dynamic_snap(pedalboard, buffer_size, self.looper_information.settings.maximum_fractionals);
                }
            }

            if let Err(e) = active_multitrack.save_to_directory() {
                tracing::error!("Failed to save multitrack: {}", e);
            }
        }
    }

    pub fn push_audio_to_recording_track(&mut self, audio: &[f32]) {
        if let Some(active_multitrack) = &mut self.active_multitrack {
            active_multitrack.push_audio_to_recording_track(audio);
        }
    }

    pub fn add_tracks_to_buffer(&mut self, buffer: &mut [f32]) {
        if let Some(active_multitrack) = &mut self.active_multitrack {
            active_multitrack.add_tracks_to_buffer(buffer);
        }
    }

    pub fn play_all(&mut self) {
        if let Some(active_multitrack) = &mut self.active_multitrack {
            active_multitrack.play_all();
        }
    }

    pub fn play_track(&mut self, track_index: usize) {
        if let Some(active_multitrack) = &mut self.active_multitrack {
            active_multitrack.play_track(track_index);
        }
    }

    pub fn stop_all(&mut self) {
        if let Some(active_multitrack) = &mut self.active_multitrack {
            active_multitrack.stop_all();
        }
    }

    pub fn stop_track(&mut self, track_index: usize) {
        if let Some(active_multitrack) = &mut self.active_multitrack {
            active_multitrack.stop_track(track_index);
        }
    }

    pub fn set_active_multitrack(&mut self, name: &str) -> Result<(), String> {
        let multitrack = match LooperMultitrack::load_from_directory(name, self.sample_rate) {
            Ok(multitrack) => multitrack,
            Err(_) => LooperMultitrack::new_and_save_to_disk(name.to_string(), self.sample_rate)?
        };
        self.active_multitrack = Some(multitrack);
        Ok(())
    }

    pub fn handle_client_subcommand(&mut self, subcommand_and_args: &str, client_command_buffer: &mut Vec<String>, pedalboard: &mut Pedalboard, buffer_size: usize) {
        let subcommand_parts: Vec<&str> = subcommand_and_args.split('|').collect();
        let subcommand = match subcommand_parts.get(0) {
            Some(cmd) => *cmd,
            None => {
                tracing::warn!("Empty looper subcommand received");
                return;
            }
        };

        match subcommand {
            "getinfo" => {
                let info_json = serde_json::to_string(&self.looper_information).unwrap_or_else(|e| format!("Failed to serialize looper information: {}", e));
                client_command_buffer.push(format!("looperinfo {}", info_json));
                // assume if client is asking for getinfo, it has started a new session, so reset the active multitrack to None
                self.active_multitrack = None;
                self.selected_track_index = None;
            },
            "setactivemultitrack" => {
                if let Some(name) = subcommand_parts.get(1) {
                    match self.set_active_multitrack(name) {
                        Ok(_) => client_command_buffer.push(format!("popup:Active multitrack set to {}", name)),
                        Err(e) => client_command_buffer.push(format!("popup:Failed to set active multitrack: {}", e)),
                    }
                } else {
                    tracing::warn!("No multitrack name provided for setactivemultitrack command");
                }
            },
            "playall" => {
                self.play_all();
            },
            "playtrack" => {
                if let Some(track_index_str) = subcommand_parts.get(1) {
                    if let Ok(track_index) = track_index_str.parse::<usize>() {
                        self.play_track(track_index);
                    } else {
                        tracing::warn!("Invalid track index provided for playtrack command: {}", track_index_str);
                    }
                } else {
                    tracing::warn!("No track index provided for playtrack command");
                }
            },
            "stopall" => {
                self.stop_all();
            },
            "stoptrack" => {
                if let Some(track_index_str) = subcommand_parts.get(1) {
                    if let Ok(track_index) = track_index_str.parse::<usize>() {
                        self.stop_track(track_index);
                    } else {
                        tracing::warn!("Invalid track index provided for stoptrack command: {}", track_index_str);
                    }
                } else {
                    tracing::warn!("No track index provided for stoptrack command");
                }
            },
            "startrecording" => {
                if let Some(track_index_str) = subcommand_parts.get(1) {
                    if let Ok(track_index) = track_index_str.parse::<usize>() {
                        self.start_recording(track_index);
                    } else {
                        tracing::warn!("Invalid track index provided for startrecording command: {}", track_index_str);
                    }
                } else {
                    tracing::warn!("No track index provided for startrecording command");
                }
            },
            "stoprecording" => {
                self.stop_recording(pedalboard, buffer_size);
            },
            "reverseeffect" => {
                if let Some(track_index_str) = subcommand_parts.get(1) {
                    if let Ok(track_index) = track_index_str.parse::<usize>() {
                        if let Some(active_multitrack) = &mut self.active_multitrack {
                            if track_index < active_multitrack.tracks.len() {
                                active_multitrack.tracks[track_index].reverse_effect();
                            } else {
                                tracing::warn!("Invalid track index provided for reverseeffect command: {}", track_index);
                            }
                        } else {
                            tracing::warn!("No active multitrack to apply reverseeffect command");
                        }
                    } else {
                        tracing::warn!("Invalid track index provided for reverseeffect command: {}", track_index_str);
                    }
                } else {
                    tracing::warn!("No track index provided for reverseeffect command");
                }
            },
            "sloweffect" => {
                if let Some(track_index_str) = subcommand_parts.get(1) {
                    if let Ok(track_index) = track_index_str.parse::<usize>() {
                        if let Some(active_multitrack) = &mut self.active_multitrack {
                            if track_index < active_multitrack.tracks.len() {
                                active_multitrack.tracks[track_index].half_speed_effect();
                            } else {
                                tracing::warn!("Invalid track index provided for sloweffect command: {}", track_index);
                            }
                        } else {
                            tracing::warn!("No active multitrack to apply sloweffect command");
                        }
                    } else {
                        tracing::warn!("Invalid track index provided for sloweffect command: {}", track_index_str);
                    }
                } else {
                    tracing::warn!("No track index provided for sloweffect command");
                }
            },
            "setsettings" => {
                if let Some(settings_json) = subcommand_parts.get(1) {
                    match serde_json::from_str::<LooperSettings>(settings_json) {
                        Ok(new_settings) => {
                            self.looper_information.settings = new_settings;
                        },
                        Err(e) => {
                            tracing::warn!("Failed to parse looper settings JSON: {}", e);
                        }
                    }
                } else {
                    tracing::warn!("No settings JSON provided for setsettings command");
                }
            },
            "prevselectedtrack" => {
                if let Some(active_multitrack) = &mut self.active_multitrack {
                    if let Some(selected_index) = self.selected_track_index {
                        if selected_index > 0 {
                            self.selected_track_index = Some(selected_index - 1);
                        }
                    } else {
                        self.selected_track_index = Some(0);
                    }
                } else {
                    tracing::warn!("No active multitrack to select previous track");
                }
            },
            "nextselectedtrack" => {
                if let Some(active_multitrack) = &mut self.active_multitrack {
                    if let Some(selected_index) = self.selected_track_index {
                        if selected_index + 1 < active_multitrack.tracks.len() {
                            self.selected_track_index = Some(selected_index + 1);
                        }
                    } else {
                        self.selected_track_index = Some(0);
                    }
                } else {
                    tracing::warn!("No active multitrack to select next track");
                }
            },
            "playselectedtrack" => {
                if let Some(active_multitrack) = &mut self.active_multitrack {
                    if let Some(selected_index) = self.selected_track_index {
                        active_multitrack.play_track(selected_index);
                    } else {
                        tracing::warn!("No track selected to play");
                    }
                } else {
                    tracing::warn!("No active multitrack to play selected track");
                }
            },
            "stopselectedtrack" => {
                if let Some(active_multitrack) = &mut self.active_multitrack {
                    if let Some(selected_index) = self.selected_track_index {
                        active_multitrack.stop_track(selected_index);
                    } else {
                        tracing::warn!("No track selected to stop");
                    }
                } else {
                    tracing::warn!("No active multitrack to stop selected track");
                }
            },
            "startrecordingselectedtrack" => {
                if let Some(active_multitrack) = &mut self.active_multitrack {
                    if let Some(selected_index) = self.selected_track_index {
                        active_multitrack.start_recording(selected_index);
                    } else {
                        tracing::warn!("No track selected to start recording");
                    }
                } else {
                    tracing::warn!("No active multitrack to start recording selected track");
                }
            },
            "prevactivemultitrack" => {
                if let Some(current_multitrack) = &self.active_multitrack {
                    let multitrack_names = &self.looper_information.multitracks_information;
                    if let Some(current_index) = multitrack_names.iter().position(|mt_info| mt_info.name == current_multitrack.name) {
                        let prev_index = if current_index == 0 {
                            multitrack_names.len() - 1
                        } else {
                            current_index - 1
                        };
                        let prev_name = &multitrack_names[prev_index].name.clone();
                        if let Err(e) = self.set_active_multitrack(prev_name) {
                            tracing::warn!("Failed to set previous active multitrack: {}", e);
                        }
                    }
                }
            },
            "nextactivemultitrack" => {
                if let Some(current_multitrack) = &self.active_multitrack {
                    let multitrack_names = &self.looper_information.multitracks_information;
                    if let Some(current_index) = multitrack_names.iter().position(|mt_info| mt_info.name == current_multitrack.name) {
                        let next_index = (current_index + 1) % multitrack_names.len();
                        let next_name = &multitrack_names[next_index].name.clone();
                        if let Err(e) = self.set_active_multitrack(next_name) {
                            tracing::warn!("Failed to set next active multitrack: {}", e);
                        }
                    }
                }
            },
            _ => tracing::warn!("Unknown looper subcommand: {}", subcommand),
        }
    }
}
