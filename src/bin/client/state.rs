use std::{cell::{Cell, RefCell}, collections::HashSet, time::Instant};
use crossbeam::channel::Receiver;
use rs_pedalboard::{pedalboard::Pedalboard, pedals::{Pedal, PedalParameterValue, PedalTrait}, server_settings::ServerSettingsSave};
use crate::{midi::{MidiSettings, MidiState}, saved_pedalboards::SavedPedalboards, settings::{ClientSettings, VolumeNormalizationMode}, socket::{ClientSocket, Command, ParameterPath}};
use eframe::egui;

pub struct State {
    pub pedalboards: SavedPedalboards,
    socket: RefCell<ClientSocket>,

    pub client_settings: RefCell<ClientSettings>,
    pub server_settings: RefCell<ServerSettingsSave>,
    pub midi_state: RefCell<MidiState>,
    pub midi_command_receiver: Receiver<Command>,

    // Utility state
    pub recording_time: Cell<Option<Instant>>,
    pub recording_save_clean: Cell<bool>,
    pub metronome_active: Cell<bool>,
    pub metronome_bpm: Cell<u32>,
    pub metronome_volume: Cell<f32>,
    pub tuner_active: Cell<bool>,
}

impl State {
    /// Get a set of all pedalboard IDs in the active pedalboard stage and in the pedalboard library
    /// 
    /// Requires a lock on active_pedalboardstage and pedalboard_library
    pub fn all_pedalboard_ids(&self) -> HashSet<u32> {
        let mut ids = HashSet::new();

        for pedalboard in self.pedalboards.active_pedalboardstage.borrow().pedalboards.iter() {
            ids.insert(pedalboard.get_id());
        }

        for pedalboard in self.pedalboards.pedalboard_library.borrow().iter() {
            ids.insert(pedalboard.get_id());
        }

        ids
    }

    /// Delete a pedalboard from the active pedalboard stage
    /// 
    /// Requires a lock on active_pedalboardstage and socket
    pub fn remove_pedalboard_from_stage(&self, index: usize, local: bool) {
        let mut pedalboard_set = self.pedalboards.active_pedalboardstage.borrow_mut();

        if pedalboard_set.pedalboards.len() <= 1 {
            log::error!("Cannot remove the last pedalboard from the stage");
            return;
        }

        pedalboard_set.pedalboards.remove(index);

        if pedalboard_set.active_pedalboard > index || pedalboard_set.active_pedalboard == pedalboard_set.pedalboards.len() {
            pedalboard_set.active_pedalboard -= 1;
        }

        if !local {
            let mut socket = self.socket.borrow_mut();
            socket.send(Command::DeletePedalboard(index));
        }
    }

    /// Move a pedalboard in the active pedalboard stage
    /// 
    /// Requires a lock on active_pedalboardstage and socket
    pub fn move_pedalboard(&self, src_index: usize, dest_index: usize, local: bool) {
        let mut pedalboard_set = self.pedalboards.active_pedalboardstage.borrow_mut();
        egui_dnd::utils::shift_vec(src_index, dest_index, &mut pedalboard_set.pedalboards);

        if !local {
            let mut socket = self.socket.borrow_mut();
            socket.send(Command::MovePedalboard(src_index, dest_index));
        }
    }

    /// Add a pedalboard to the active pedalboard stage
    /// 
    /// Requires a lock on active_pedalboardstage and socket
    pub fn add_pedalboard(&self, pedalboard: Pedalboard, local: bool) {
        if !local {
            let mut socket = self.socket.borrow_mut();
            socket.send(Command::AddPedalboard(serde_json::to_string(&pedalboard).unwrap()));
        }

        let mut pedalboard_set = self.pedalboards.active_pedalboardstage.borrow_mut();
        pedalboard_set.pedalboards.push(pedalboard);
    }

    /// Requires a lock on active_pedalboardstage, pedalboard_library, songs_library and socket
    pub fn rename_pedalboard(&self, pedalboard_id: u32, new_name: String) {
        // First rename any matching names in pedalboard library
        let mut pedalboard_library = self.pedalboards.pedalboard_library.borrow_mut();
        for pedalboard in pedalboard_library.iter_mut() {
            if pedalboard.get_id() == pedalboard_id {
                pedalboard.name = new_name.to_string();
            }
        }
    
        // Then rename any matching names in the active pedalboard stage
        let unique_name = self.pedalboards.unique_stage_pedalboard_name(new_name.to_string());
        let mut pedalboard_set = self.pedalboards.active_pedalboardstage.borrow_mut();
        for pedalboard in pedalboard_set.pedalboards.iter_mut() {
            if pedalboard.get_id() == pedalboard_id {
                pedalboard.name = unique_name.clone();
            }
        }
    }

    /// Duplicate pedalboard in stage with same name
    /// 
    /// Requires a lock on active_pedalboardstage and socket
    pub fn duplicate_linked(&self, index: usize) {
        let pedalboard_set = self.pedalboards.active_pedalboardstage.borrow_mut();
        let pedalboard = &pedalboard_set.pedalboards[index];
        let new_pedalboard = pedalboard.clone();
        let src_index = pedalboard_set.pedalboards.len();
        drop(pedalboard_set);

        self.add_pedalboard(new_pedalboard, false);
        self.move_pedalboard(src_index, index+1, false);
    }

    /// Duplicate pedalboard in stage with new name
    /// 
    /// Requires a lock on active_pedalboardstage and socket
    pub fn duplicate_new(&self, index: usize) {
        let pedalboard_set = self.pedalboards.active_pedalboardstage.borrow_mut();
        let pedalboard = &pedalboard_set.pedalboards[index];

        let mut new_pedalboard = pedalboard.clone_with_new_id();
        let src_index = pedalboard_set.pedalboards.len();
        // Have to drop as the unique stage name requires a lock on active pedalboard stage
        drop(pedalboard_set);
        new_pedalboard.name = self.pedalboards.unique_stage_pedalboard_name(new_pedalboard.name.clone());
        
        self.add_pedalboard(new_pedalboard, false);
        self.move_pedalboard(src_index, index+1, false);
    }

    /// Add a pedal to a given pedalboard ID
    /// 
    /// Requires a lock on active_pedalboardstage, pedalboard_library, and socket
    pub fn add_pedal_to_pedalboard(&self, pedalboard_id: u32, pedal: &Pedal, local: bool) {
        // Add in pedalboard library
        let mut pedalboard_library = self.pedalboards.pedalboard_library.borrow_mut();
        for pedalboard in pedalboard_library.iter_mut() {
            if pedalboard.get_id() == pedalboard_id {
                pedalboard.pedals.push(pedal.clone());
                break;
            }
        }

        // Add in all matching pedalboards in active pedalboard stage
        let mut active_pedalboardstage = self.pedalboards.active_pedalboardstage.borrow_mut();
        for pedalboard in active_pedalboardstage.pedalboards.iter_mut() {
            if pedalboard.get_id() == pedalboard_id {
                pedalboard.pedals.push(pedal.clone());
            }
        }

        if !local {
            let mut socket = self.socket.borrow_mut();
            socket.send(Command::AddPedal(pedalboard_id, serde_json::to_string(pedal).unwrap()));
        }
    }

    /// Add a pedal to the active pedalboard and matching pedalboard in library
    /// 
    /// Requires a lock on active_pedalboardstage, pedalboard_library, and socket
    pub fn add_pedal_to_active(&self, pedal: &Pedal, local: bool) {
        let active_pedalboardstage = self.pedalboards.active_pedalboardstage.borrow_mut();
        let active_pedalboard_id = active_pedalboardstage.pedalboards[active_pedalboardstage.active_pedalboard].get_id();
        drop(active_pedalboardstage);
        self.add_pedal_to_pedalboard(active_pedalboard_id, pedal, local);
    }

    /// Move a pedal in the pedalboard stage and in library
    /// 
    /// Requires a lock on active_pedalboardstage, pedalboard_library, and socket
    pub fn move_pedal(&self, pedalboard_id: u32, pedal_id: u32, mut to_index: usize, local: bool) {
        let mut active_pedalboardstage = self.pedalboards.active_pedalboardstage.borrow_mut();
        let mut pedalboard_library = self.pedalboards.pedalboard_library.borrow_mut();

        // Move in all matching pedalboards in active pedalboard stage
        let mut src_index = None;

        for pedalboard in active_pedalboardstage.pedalboards.iter_mut() {
            if pedalboard.get_id() == pedalboard_id {
                if to_index >= pedalboard.pedals.len() + 1 {
                    to_index = pedalboard.pedals.len();
                }

                src_index = Some(pedalboard.pedals.iter().position(|p| p.get_id() == pedal_id).unwrap());
                egui_dnd::utils::shift_vec(src_index.unwrap(), to_index, &mut pedalboard.pedals);
            }
        }

        if src_index.is_none() {
            log::error!("move_pedal: Could not find pedalboard with ID {} in pedalboard library", pedalboard_id);
            return;
        }

        // Move in pedalboard library
        for pedalboard in pedalboard_library.iter_mut() {
            if pedalboard.get_id() == pedalboard_id {
                egui_dnd::utils::shift_vec(src_index.unwrap(), to_index, &mut pedalboard.pedals); 
                break;
            }
        }

        if !local {
            let mut socket = self.socket.borrow_mut();
            socket.send(Command::MovePedal(pedalboard_id, pedal_id, to_index));
        }
    }

    /// Delete a pedal from the pedalboard stage and in library
    /// 
    /// Requires a lock on active_pedalboardstage, pedalboard_library, and socket
    pub fn delete_pedal(&self, pedalboard_id: u32, pedal_id: u32, local: bool) {
        let mut active_pedalboardstage = self.pedalboards.active_pedalboardstage.borrow_mut();
        let mut pedalboard_library = self.pedalboards.pedalboard_library.borrow_mut();

        for pedalboard in pedalboard_library.iter_mut() {
            if pedalboard.get_id() == pedalboard_id {
                pedalboard.pedals.retain(|p| p.get_id() != pedal_id);
                break;
            }
        }

        // Remove in all matching pedalboards in active pedalboard stage
        for pedalboard in active_pedalboardstage.pedalboards.iter_mut() {
            if pedalboard.get_id() == pedalboard_id {
                pedalboard.pedals.retain(|p| p.get_id() != pedal_id);
            }
        }

        if !local {
            let mut socket = self.socket.borrow_mut();
            socket.send(Command::DeletePedal(pedalboard_id, pedal_id));
        }
    }

    /// Set a parameter on all pedalboards, on stage and in library, with the same name
    /// 
    /// Requires a lock on active_pedalboardstage, pedalboard_library and socket
    pub fn set_parameter(&self, pedalboard_id: u32, pedal_id: u32, parameter_name: String, parameter_value: PedalParameterValue, local: bool) {
        // Set parameter on pedalboard stage
        for pedalboard in self.pedalboards.active_pedalboardstage.borrow_mut().pedalboards.iter_mut() {
            if pedalboard.get_id() == pedalboard_id {
                let pedal = pedalboard.pedals.iter_mut().find(|p| p.get_id() == pedal_id).unwrap();
                pedal.set_parameter_value(&parameter_name, parameter_value.clone());
            }
        }

        // Set parameter on pedalboard library
        for pedalboard in self.pedalboards.pedalboard_library.borrow_mut().iter_mut() {
            if pedalboard.get_id() == pedalboard_id {
                let pedal = pedalboard.pedals.iter_mut().find(|p| p.get_id() == pedal_id).unwrap();
                pedal.set_parameter_value(&parameter_name, parameter_value.clone());
            }
        }

        if !local {
            let mut socket = self.socket.borrow_mut();
            socket.send(Command::ParameterUpdate(ParameterPath {
                pedalboard_id,
                pedal_id,
                parameter_name
            }, parameter_value));
        }
    }

    /// Tell the server to load the client's active pedalboard stage
    pub fn load_active_set(&self) {
        let mut socket = self.socket.borrow_mut();
        let active_pedalboardstage = self.pedalboards.active_pedalboardstage.borrow();
        socket.send(Command::LoadSet(serde_json::to_string(&*active_pedalboardstage).unwrap()));
    }

    /// Play a pedalboard from the active stage
    /// 
    /// Requires a lock on active_pedalboardstage and socket
    pub fn play(&self, pedalboard_index: usize, local: bool) {
        let mut active_pedalboardset = self.pedalboards.active_pedalboardstage.borrow_mut();
        active_pedalboardset.set_active_pedalboard(pedalboard_index);

        let new_pedalboard_id = active_pedalboardset.pedalboards[active_pedalboardset.active_pedalboard].get_id();
        self.midi_state.borrow().active_pedalboard_id.store(new_pedalboard_id, std::sync::atomic::Ordering::Relaxed);

        if !local {
            let mut socket = self.socket.borrow_mut();
            socket.send(Command::Play(pedalboard_index));
        }
    }

    /// Update the received messages from the socket thread.
    /// 
    /// Requires a lock on socket.
    pub fn update_socket_responses(&self) {
        let mut socket = self.socket.borrow_mut();
        socket.update_socket_responses();
    }

    /// Get a received command from the server, beginning with the given prefix.
    /// 
    /// Requires a lock on socket
    pub fn get_commands(&self, prefix: &str, into: &mut Vec<String>) {
        let mut socket = self.socket.borrow_mut();

        // TODO: remove cloning with nightly `drain_filter`? 
        socket.received_server_commands.retain(|cmd| {
            if cmd.starts_with(prefix) {
                // Remove the prefix and push the command into the vector
                let cmd_trim = cmd.trim_start_matches(prefix).trim().to_string();
                into.push(cmd_trim);
                false
            } else {
                true
            }
        });
    }

    /// Set whether the tuner is active.
    /// 
    /// Requires a lock on socket.
    pub fn set_tuner_active(&self, active: bool) {
        self.tuner_active.set(active);

        let mut socket = self.socket.borrow_mut();
        socket.send(Command::Tuner(active));
    }

    /// Set the metronome settings.
    /// 
    /// Requires a lock on socket.
    pub fn set_metronome(&self, active: bool, bpm: u32, volume: f32) {
        self.metronome_active.set(active);
        self.metronome_bpm.set(bpm);
        self.metronome_volume.set(volume);

        let mut socket = self.socket.borrow_mut();
        let rounded_volume = (volume * 100.0).round() / 100.0;
        socket.send(Command::Metronome(active, bpm, rounded_volume));
    }

    /// Set whether the volume monitor is active on the server.
    /// 
    /// Requires a lock on socket.
    pub fn set_volume_monitor_active_server(&self, active: bool) {
        let mut socket = self.socket.borrow_mut();
        socket.send(Command::VolumeMonitor(active));
    }

    pub fn set_volume_normalization_server(&self, mode: crate::settings::VolumeNormalizationMode, auto_decay: f32) {
        let mut socket = self.socket.borrow_mut();
        let rounded_auto_decay = (auto_decay * 1000.0).round() / 1000.0;
        
        if matches!(mode, VolumeNormalizationMode::Automatic) {
            socket.send(Command::VolumeNormalization(VolumeNormalizationMode::Automatic, Some(rounded_auto_decay)));
        } else {
            socket.send(Command::VolumeNormalization(mode, None));
        }
    }

    pub fn reset_volume_normalization_peak(&self) {
        let mut socket = self.socket.borrow_mut();
        socket.send(Command::VolumeNormalizationReset);
    }

    pub fn master_in_server(&self, volume: f32) {
        let mut socket = self.socket.borrow_mut();
        let rounded_volume = (volume * 100.0).round() / 100.0;
        socket.send(Command::MasterIn(rounded_volume));
    }

    pub fn master_out_server(&self, volume: f32) {
        let mut socket = self.socket.borrow_mut();
        let rounded_volume = (volume * 100.0).round() / 100.0;
        socket.send(Command::MasterOut(rounded_volume));
    }

    pub fn set_recording(&self, active: bool) {
        let mut socket = self.socket.borrow_mut();
        socket.send(Command::SetRecording(active));
        if active {
            self.recording_time.set(Some(Instant::now()));
        } else {
            self.recording_time.set(None);
        }
    }

    pub fn set_recorder_clean(&self, clean: bool) {
        let mut socket = self.socket.borrow_mut();
        socket.send(Command::RecordClean(clean));
        self.recording_save_clean.set(clean);
    }

    pub fn load_state(egui_ctx: eframe::egui::Context) -> Self {
        let pedalboards = SavedPedalboards::load_or_default();
        let active_pedalboard = pedalboards.active_pedalboardstage.borrow();
        let active_pedalboard_index = active_pedalboard.active_pedalboard;
        let active_pedalboard_id = active_pedalboard.pedalboards[active_pedalboard_index].get_id();
        drop(active_pedalboard);

        let socket = ClientSocket::new(crate::SERVER_PORT);
        let client_settings = ClientSettings::load_or_default();

        // Set NAM folders, IR folders and VST2 in ctx memory so pedals can access
        log::info!("Indexing NAM, IR and VST2 folders...");
        let nam_root_nodes: Vec<_> = client_settings.nam_folders.iter().map(|p| {
            egui_directory_combobox::DirectoryNode::from_path(p)
        }).collect();

        let ir_root_nodes: Vec<_> = client_settings.ir_folders.iter().map(|p| {
            egui_directory_combobox::DirectoryNode::from_path(p)
        }).collect();

        let vst2_root_nodes: Vec<_> = client_settings.vst2_folders.iter().map(|p| {
            egui_directory_combobox::DirectoryNode::from_path(p)
        }).collect();

        egui_ctx.memory_mut(|writer| {
            writer.data.insert_temp(egui::Id::new("nam_folders_state"), 1u32);
            writer.data.insert_temp(egui::Id::new("nam_folders"), nam_root_nodes);
            writer.data.insert_temp(egui::Id::new("ir_folders_state"), 1u32);
            writer.data.insert_temp(egui::Id::new("ir_folders"), ir_root_nodes);
            writer.data.insert_temp(egui::Id::new("vst2_folders_state"), 1u32);
            writer.data.insert_temp(egui::Id::new("vst2_folders"), vst2_root_nodes);
        });

        let server_settings = ServerSettingsSave::load_or_default();
        let midi_settings = MidiSettings::load_or_default();
        let (midi_command_sender, midi_command_receiver) = crossbeam::channel::unbounded();

        let midi_state = MidiState::new(
            midi_settings.clone(),
            egui_ctx.clone(),
            midi_command_sender,
            None,
            active_pedalboard_id
        );

        State {
            pedalboards,
            socket: RefCell::new(socket),
            client_settings: RefCell::new(client_settings),
            server_settings: RefCell::new(server_settings),
            midi_state: RefCell::new(midi_state),
            midi_command_receiver,
            recording_time: Cell::new(None),
            recording_save_clean: Cell::new(true),
            metronome_active: Cell::new(false),
            metronome_bpm: Cell::new(120),
            metronome_volume: Cell::new(0.5),
            tuner_active: Cell::new(false),
        }
    }

    pub fn save_state(&self) -> Result<(), std::io::Error> {
        self.pedalboards.save()?;
        self.client_settings.borrow().save()?;
        self.server_settings.borrow().save()?;
        self.midi_state.borrow().save_settings()?;
        Ok(())
    }

    pub fn connect_to_server(&self) -> Result<(), std::io::Error> {
        let mut socket = self.socket.borrow_mut();
        if !socket.is_connected() {
            socket.connect()?;
            if socket.is_connected() {
                socket.send(Command::RequestSampleRate);

                let new_handle = socket.handle.clone();
                drop(socket);
                let mut midi_state = self.midi_state.borrow_mut();
                midi_state.disconnect_from_all_ports();
                midi_state.set_socket_handle(new_handle);
                midi_state.connect_to_auto_connect_ports();

                let client_settings = self.client_settings.borrow();
                self.set_volume_monitor_active_server(client_settings.show_volume_monitor);
                self.set_volume_normalization_server(client_settings.volume_normalization, client_settings.auto_volume_normalization_decay);
                self.master_in_server(client_settings.input_volume);
                self.load_active_set();
            }
        }
        Ok(())
    }

    /// Requires a lock on socket
    pub fn is_connected(&self) -> bool {
        self.socket.borrow_mut().is_connected()
    }

    /// Requires a lock on socket
    pub fn is_server_available(&self) -> bool {
        self.socket.borrow_mut().is_server_available()
    }

    /// Requires a lock on socket
    pub fn kill_server(&self) {
        let mut socket = self.socket.borrow_mut();
        socket.kill();
    }

    /// Update the state with commands that other threads have sent to the server
    pub fn handle_other_thread_commands(&self) {
        for command in self.midi_command_receiver.try_iter() {
            match command {
                Command::LoadSet(pedalboard_set_json) => {
                    match serde_json::from_str::<rs_pedalboard::pedalboard_set::PedalboardSet>(&pedalboard_set_json) {
                        Ok(pedalboard_set) => {
                            self.pedalboards.active_pedalboardstage.replace(pedalboard_set);
                        },
                        Err(e) => {
                            log::error!("Failed to parse pedalboard set JSON from server: {}", e);
                        }
                    }
                },
                Command::Play(pedalboard_index) => {
                    self.play(pedalboard_index, true);
                },
                Command::NextPedalboard => {
                    let pedalboard_set = self.pedalboards.active_pedalboardstage.borrow();
                    let new_index = (pedalboard_set.active_pedalboard + 1) % pedalboard_set.pedalboards.len();
                    drop(pedalboard_set);
                    self.play(new_index, true);
                },
                Command::PrevPedalboard => {
                    let pedalboard_set = self.pedalboards.active_pedalboardstage.borrow();
                    let new_index = if pedalboard_set.active_pedalboard == 0 {
                        pedalboard_set.pedalboards.len() - 1
                    } else {
                        pedalboard_set.active_pedalboard - 1
                    };
                    drop(pedalboard_set);
                    self.play(new_index, true);
                },
                Command::MovePedal(pedalboard_id, pedal_id, to_index) => {
                    self.move_pedal(pedalboard_id, pedal_id, to_index, true);
                },
                Command::DeletePedal(pedalboard_id, pedal_id) => {
                    self.delete_pedal(pedalboard_id, pedal_id, true);
                },
                Command::MovePedalboard(src_index, dest_index) => {
                    self.move_pedalboard(src_index, dest_index, true);
                },
                Command::DeletePedalboard(index) => {
                    self.remove_pedalboard_from_stage(index, true);
                },
                Command::DeleteActivePedalboard => {
                    let pedalboard_set = self.pedalboards.active_pedalboardstage.borrow();
                    let active_index = pedalboard_set.active_pedalboard;
                    drop(pedalboard_set);
                    self.remove_pedalboard_from_stage(active_index, true);
                },
                Command::AddPedalboard(pedalboard_json) => {
                    match serde_json::from_str::<Pedalboard>(&pedalboard_json) {
                        Ok(pedalboard) => {
                            self.add_pedalboard(pedalboard, true);
                        },
                        Err(e) => {
                            log::error!("Failed to parse pedalboard JSON from other thread: {}", e);
                        }
                    }
                },
                Command::AddPedal(pedalboard_id, pedal_json) => {
                    match serde_json::from_str::<Pedal>(&pedal_json) {
                        Ok(pedal) => {
                            self.add_pedal_to_pedalboard(pedalboard_id, &pedal, true);
                        },
                        Err(e) => {
                            log::error!("Failed to parse pedal JSON from other thread: {}", e);
                        }
                    }
                },
                Command::KillServer => {
                    self.socket.borrow_mut().handle = None;
                },
                Command::MasterIn(vol) => {
                    self.client_settings.borrow_mut().input_volume = vol;
                },
                Command::MasterOut(vol) => {
                    self.client_settings.borrow_mut().output_volume = vol;
                },
                Command::VolumeNormalization(mode, decay) => {
                    let mut client_settings = self.client_settings.borrow_mut();
                    client_settings.volume_normalization = mode;
                    if let Some(d) = decay {
                        client_settings.auto_volume_normalization_decay = d;
                    }
                },
                Command::SetRecording(active) => {
                    if active {
                        self.recording_time.set(Some(Instant::now()));
                    } else {
                        self.recording_time.set(None);
                    }
                },
                Command::ToggleRecording => {
                    let currently_recording = self.recording_time.get().is_some();
                    if currently_recording {
                        self.recording_time.set(None);
                    } else {
                        self.recording_time.set(Some(Instant::now()));
                    }
                },
                Command::RecordClean(clean) => {
                    self.recording_save_clean.set(clean);
                },
                Command::ToggleClean => {
                    let currently_clean = self.recording_save_clean.get();
                    self.recording_save_clean.set(!currently_clean);
                },
                Command::VolumeMonitor(active) => {
                    self.client_settings.borrow_mut().show_volume_monitor = active;
                },
                Command::ToggleVolumeMonitor => {
                    let mut client_settings = self.client_settings.borrow_mut();
                    client_settings.show_volume_monitor = !client_settings.show_volume_monitor;
                },
                Command::Metronome(active, bpm, volume) => {
                    self.metronome_active.set(active);
                    self.metronome_bpm.set(bpm);
                    self.metronome_volume.set(volume);
                },
                Command::ToggleMetronome => {
                    let currently_active = self.metronome_active.get();
                    self.metronome_active.set(!currently_active);
                },
                Command::Tuner(active) => {
                    self.tuner_active.set(active);
                },
                Command::ToggleTuner => {
                    let currently_active = self.tuner_active.get();
                    self.tuner_active.set(!currently_active);
                },
                Command::ParameterUpdate(path, value) => {
                    self.set_parameter(
                        path.pedalboard_id,
                        path.pedal_id,
                        path.parameter_name,
                        value,
                        true
                    );
                },
                Command::VolumeNormalizationReset => {},
                Command::SetMute(mute) => { log::info!("Set mute to {mute}") },
                Command::ToggleMute => { log::info!("Toggled mute") },
                Command::RequestSampleRate => log::error!("Unexpected RequestSampleRate command in other thread commands"),
                Command::ThreadAliveTest => log::error!("Unexpected ThreadAliveTest command in other thread commands"),
                Command::SubscribeToResponses(_) => log::error!("Unexpected SubscribeToResponses command in other thread commands"),
            }
        }
    }
}