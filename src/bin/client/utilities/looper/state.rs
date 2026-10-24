use rs_pedalboard::looper_state::{LooperInformation, LooperSettings};

#[derive(Clone, Debug)]
pub(crate) struct LooperMultitrackClientState {
    pub(crate) name: String,
    pub(crate) track_count: usize,
    pub(crate) tracks_playing: Vec<usize>,
    pub(crate) track_recording: Option<usize>,
    pub(crate) selected_track: Option<usize>,
}

impl LooperMultitrackClientState {
    pub fn new(name: String, track_count: usize) -> Self {
        Self {
            name,
            track_count,
            tracks_playing: Vec::new(),
            track_recording: None,
            selected_track: None,
        }
    }
}

#[derive(Clone, Default, Debug)]
pub struct LooperClientState {
    looper_information: Option<LooperInformation>,
    active_multitrack: Option<LooperMultitrackClientState>,
}

impl LooperClientState {
    pub fn new() -> Self {
        Self {
            looper_information: None,
            active_multitrack: None,
        }
    }

    pub fn set_looper_information(&mut self, looper_information: LooperInformation) {
        self.looper_information = Some(looper_information);
    }

    pub fn remove_active_multitrack(&mut self) {
        self.active_multitrack = None;
    }

    pub fn set_active_multitrack(&mut self, name: String) {
        // Get track count from looper information
        let track_count = self
            .looper_information
            .as_ref()
            .and_then(|info| {
                info.multitracks_information
                    .iter()
                    .find(|mt| mt.name == name)
                    .map(|mt| mt.track_count)
            })
            .unwrap_or(0);
        self.active_multitrack = Some(LooperMultitrackClientState::new(name, track_count));
    }

    pub(crate) fn add_multitrack(&mut self, name: String) {
        if let Some(info) = &mut self.looper_information {
            if info
                .multitracks_information
                .iter()
                .any(|item| item.name == name)
            {
                return;
            }

            info.multitracks_information
                .push(rs_pedalboard::looper_state::MultitrackInformation {
                    name: name.clone(),
                    track_count: 0,
                });
            self.active_multitrack = Some(LooperMultitrackClientState::new(name, 0));
        }
    }

    pub(crate) fn delete_multitrack(&mut self, name: &str) {
        if let Some(info) = &mut self.looper_information {
            info.multitracks_information
                .retain(|item| item.name != name);
        }
        if self
            .active_multitrack
            .as_ref()
            .is_some_and(|active| active.name == name)
        {
            self.active_multitrack = None;
        }
    }

    pub(crate) fn add_track_to_active_multitrack(&mut self) {
        if let Some(active) = &mut self.active_multitrack {
            active.track_count += 1;
            if let Some(info) = &mut self.looper_information {
                if let Some(item) = info
                    .multitracks_information
                    .iter_mut()
                    .find(|item| item.name == active.name)
                {
                    item.track_count = active.track_count;
                }
            }
        }
    }

    pub fn play_all_tracks(&mut self) {
        if let Some(active_multitrack) = &mut self.active_multitrack {
            active_multitrack.tracks_playing = (0..active_multitrack.track_count).collect();
        }
    }

    pub fn stop_all_tracks(&mut self) {
        if let Some(active_multitrack) = &mut self.active_multitrack {
            active_multitrack.tracks_playing.clear();
        }
    }

    pub fn remove_selected_track(&mut self) {
        if let Some(active) = &mut self.active_multitrack {
            if let Some(index) = active.selected_track {
                if index < active.track_count {
                    active.track_count -= 1;
                    active.tracks_playing.retain(|&track| track != index);
                    active.selected_track = None;
                }
            }
        }
    }

    pub fn clear_selected_track(&mut self) {
        if let Some(active) = &mut self.active_multitrack {
            if let Some(index) = active.selected_track {
                active.tracks_playing.retain(|&track| track != index);
            }
        }
    }

    pub fn play_track(&mut self, track_index: usize) {
        if let Some(active_multitrack) = &mut self.active_multitrack {
            if !active_multitrack.tracks_playing.contains(&track_index) {
                active_multitrack.tracks_playing.push(track_index);
            }
        }
    }

    pub fn stop_track(&mut self, track_index: usize) {
        if let Some(active_multitrack) = &mut self.active_multitrack {
            active_multitrack
                .tracks_playing
                .retain(|&i| i != track_index);
        }
    }

    pub fn set_selected_track(&mut self, track_index: Option<usize>) {
        if let Some(active_multitrack) = &mut self.active_multitrack {
            active_multitrack.selected_track = track_index;
        }
    }

    pub fn set_recording_track(&mut self, track_index: Option<usize>) {
        if let Some(active_multitrack) = &mut self.active_multitrack {
            active_multitrack.track_recording = track_index;
        }
    }

    pub fn stop_recording(&mut self) {
        if let Some(active_multitrack) = &mut self.active_multitrack {
            active_multitrack.track_recording = None;
        }
    }

    pub fn play_selected_track(&mut self) {
        if let Some(active_multitrack) = &mut self.active_multitrack {
            if let Some(selected_track) = active_multitrack.selected_track {
                if !active_multitrack.tracks_playing.contains(&selected_track) {
                    active_multitrack.tracks_playing.push(selected_track);
                }
            }
        }
    }

    pub fn stop_selected_track(&mut self) {
        if let Some(active_multitrack) = &mut self.active_multitrack {
            if let Some(selected_track) = active_multitrack.selected_track {
                active_multitrack
                    .tracks_playing
                    .retain(|&i| i != selected_track);
            }
        }
    }

    pub fn set_selected_track_recording(&mut self) {
        if let Some(active_multitrack) = &mut self.active_multitrack {
            if let Some(selected_track) = active_multitrack.selected_track {
                active_multitrack.track_recording = Some(selected_track);
            }
        }
    }

    pub fn get_looper_information(&self) -> Option<&LooperInformation> {
        self.looper_information.as_ref()
    }

    pub(crate) fn active_multitrack(&self) -> Option<&LooperMultitrackClientState> {
        self.active_multitrack.as_ref()
    }

    pub fn prev_active_multitrack(&mut self) {
        if let Some(active_multitrack) = &self.active_multitrack {
            if let Some(looper_information) = &self.looper_information {
                let multitrack_names = &looper_information.multitracks_information;
                if let Some(current_index) = multitrack_names
                    .iter()
                    .position(|mt_info| mt_info.name == active_multitrack.name)
                {
                    let prev_index = if current_index == 0 {
                        multitrack_names.len() - 1
                    } else {
                        current_index - 1
                    };
                    let prev_name = &multitrack_names[prev_index].name.clone();
                    self.set_active_multitrack(prev_name.clone());
                }
            }
        }
    }

    pub fn next_active_multitrack(&mut self) {
        if let Some(active_multitrack) = &self.active_multitrack {
            if let Some(looper_information) = &self.looper_information {
                let multitrack_names = &looper_information.multitracks_information;
                if let Some(current_index) = multitrack_names
                    .iter()
                    .position(|mt_info| mt_info.name == active_multitrack.name)
                {
                    let next_index = (current_index + 1) % multitrack_names.len();
                    let next_name = &multitrack_names[next_index].name.clone();
                    self.set_active_multitrack(next_name.clone());
                }
            }
        }
    }

    pub fn next_selected_track(&mut self) {
        if let Some(active_multitrack) = &mut self.active_multitrack {
            if let Some(selected_track) = active_multitrack.selected_track {
                let next_track = (selected_track + 1) % active_multitrack.track_count;
                active_multitrack.selected_track = Some(next_track);
            } else {
                active_multitrack.selected_track = Some(0);
            }
        }
    }

    pub fn prev_selected_track(&mut self) {
        if let Some(active_multitrack) = &mut self.active_multitrack {
            if let Some(selected_track) = active_multitrack.selected_track {
                let prev_track = if selected_track == 0 {
                    active_multitrack.track_count - 1
                } else {
                    selected_track - 1
                };
                active_multitrack.selected_track = Some(prev_track);
            } else {
                active_multitrack.selected_track = Some(0);
            }
        }
    }

    pub fn start_recording_selected_track(&mut self) {
        if let Some(active_multitrack) = &mut self.active_multitrack {
            if let Some(selected_track) = active_multitrack.selected_track {
                active_multitrack.track_recording = Some(selected_track);
            }
        }
    }

    pub fn set_settings(&mut self, settings: LooperSettings) {
        if let Some(looper_information) = &mut self.looper_information {
            looper_information.settings = settings;
        }
    }

    pub fn save_settings(&self) {
        if let Some(looper_information) = &self.looper_information {
            looper_information
                .settings
                .save_to_disk()
                .unwrap_or_else(|e| {
                    tracing::error!("Failed to save looper settings: {}", e);
                });
        }
    }

    pub fn toggle_play_selected_track(&mut self) {
        if let Some(active_multitrack) = &mut self.active_multitrack {
            if let Some(selected_track) = active_multitrack.selected_track {
                if active_multitrack.tracks_playing.contains(&selected_track) {
                    active_multitrack
                        .tracks_playing
                        .retain(|&i| i != selected_track);
                } else {
                    active_multitrack.tracks_playing.push(selected_track);
                }
            }
        }
    }

    pub fn toggle_record_selected_track(&mut self) {
        if let Some(active_multitrack) = &mut self.active_multitrack {
            if let Some(selected_track) = active_multitrack.selected_track {
                if active_multitrack.track_recording == Some(selected_track) {
                    active_multitrack.track_recording = None;
                } else {
                    active_multitrack.track_recording = Some(selected_track);
                }
            }
        }
    }
}
