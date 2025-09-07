use std::cell::RefCell;
use rs_pedalboard::{pedalboard::Pedalboard, pedals::{Pedal, PedalParameterValue, PedalTrait}, server_settings::ServerSettingsSave};
use crate::{midi::{MidiSettings, MidiState}, saved_pedalboards::SavedPedalboards, settings::ClientSettings, socket::{ClientSocket, ParameterPath}};
use eframe::egui;

pub struct State {
    pub pedalboards: SavedPedalboards,
    socket: RefCell<ClientSocket>,

    pub client_settings: RefCell<ClientSettings>,
    pub server_settings: RefCell<ServerSettingsSave>,
    pub midi_state: RefCell<MidiState>
}

impl State {
    /// Delete a pedalboard from the active pedalboard stage
    /// 
    /// Requires a lock on active_pedalboardstage and socket
    pub fn remove_pedalboard_from_stage(&self, index: usize) {
        let mut pedalboard_set = self.pedalboards.active_pedalboardstage.borrow_mut();

        if pedalboard_set.pedalboards.len() <= 1 {
            log::error!("Cannot remove the last pedalboard from the stage");
            return;
        }

        pedalboard_set.pedalboards.remove(index);

        if pedalboard_set.active_pedalboard > index || pedalboard_set.active_pedalboard == pedalboard_set.pedalboards.len() {
            pedalboard_set.active_pedalboard -= 1;
        }

        self.socket.borrow_mut().send_delete_pedalboard(index);
    }

    /// Move a pedalboard in the active pedalboard stage
    /// 
    /// Requires a lock on active_pedalboardstage and socket
    pub fn move_pedalboard(&self, src_index: usize, dest_index: usize) {
        let mut pedalboard_set = self.pedalboards.active_pedalboardstage.borrow_mut();
        egui_dnd::utils::shift_vec(src_index, dest_index, &mut pedalboard_set.pedalboards);

        self.socket.borrow_mut().send_move_pedalboard(src_index, dest_index);
    }

    /// Add a pedalboard to the active pedalboard stage
    /// 
    /// Requires a lock on active_pedalboardstage and socket
    pub fn add_pedalboard(&self, pedalboard: Pedalboard) {
        let mut pedalboard_set = self.pedalboards.active_pedalboardstage.borrow_mut();
        let mut socket = self.socket.borrow_mut();

        socket.send_add_pedalboard(serde_json::to_string(&pedalboard).unwrap());

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

        self.add_pedalboard(new_pedalboard);
        self.move_pedalboard(src_index, index+1);
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
        
        self.add_pedalboard(new_pedalboard);
        self.move_pedalboard(src_index, index+1);
    }

    /// Add a pedal to the active pedalboard and matching pedalboard in library
    /// 
    /// Requires a lock on active_pedalboardstage, pedalboard_library, and socket
    pub fn add_pedal(&self, pedal: &Pedal) {
        let mut socket = self.socket.borrow_mut();

        let mut active_pedalboardstage = self.pedalboards.active_pedalboardstage.borrow_mut();
        let active_pedalboard_id = active_pedalboardstage.pedalboards[active_pedalboardstage.active_pedalboard].get_id();
        
        // Add in pedalboard library
        let mut pedalboard_library = self.pedalboards.pedalboard_library.borrow_mut();
        for pedalboard in pedalboard_library.iter_mut() {
            if pedalboard.get_id() == active_pedalboard_id {
                pedalboard.pedals.push(pedal.clone());
                break;
            }
        }

        // Add in all matching pedalboards in active pedalboard stage
        for pedalboard in active_pedalboardstage.pedalboards.iter_mut() {
            if pedalboard.get_id() == active_pedalboard_id {
                pedalboard.pedals.push(pedal.clone());
            }
        }

        socket.send_add_pedal(active_pedalboard_id, serde_json::to_string(pedal).unwrap());
    }

    /// Move a pedal in the pedalboard stage and in library
    /// 
    /// Requires a lock on active_pedalboardstage, pedalboard_library, and socket
    pub fn move_pedal(&self, pedalboard_id: u32, pedal_id: u32, mut to_index: usize) {
        let mut active_pedalboardstage = self.pedalboards.active_pedalboardstage.borrow_mut();
        let mut pedalboard_library = self.pedalboards.pedalboard_library.borrow_mut();

        // Move in all matching pedalboards in active pedalboard stage
        let mut src_index = None;

        let mut socket = self.socket.borrow_mut();
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

        socket.send_move_pedal(pedalboard_id, pedal_id, to_index);
    }

    /// Delete a pedal from the pedalboard stage and in library
    /// 
    /// Requires a lock on active_pedalboardstage, pedalboard_library, and socket
    pub fn delete_pedal(&self, pedalboard_id: u32, pedal_id: u32) {
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

        let mut socket = self.socket.borrow_mut();
        socket.send_delete_pedal(pedalboard_id, pedal_id);
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
            socket.send_parameter_update(pedalboard_id, pedal_id, parameter_name.to_string(), parameter_value.clone());
        }
    }

    /// Tell the server to load the client's active pedalboard stage
    pub fn load_active_set(&self) {
        let mut socket = self.socket.borrow_mut();
        let active_pedalboardstage = self.pedalboards.active_pedalboardstage.borrow();
        let message = format!("loadset|{}\n", serde_json::to_string(&*active_pedalboardstage).unwrap());
        socket.send(message);
    }

    /// Play a pedalboard from the active stage
    /// 
    /// Requires a lock on active_pedalboardstage and socket
    pub fn play(&self, pedalboard_index: usize) {
        let mut socket = self.socket.borrow_mut();
        let message = format!("play|{}\n", pedalboard_index);
        socket.send(message);
        self.pedalboards.active_pedalboardstage.borrow_mut().set_active_pedalboard(pedalboard_index);
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
        socket.received_commands.retain(|cmd| {
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

    /// Set whether the tuner is active on the server.
    /// 
    /// Requires a lock on socket.
    pub fn set_tuner_active_server(&self, active: bool) {
        let mut socket = self.socket.borrow_mut();
        let message = format!("tuner|{}\n", if active { "on" } else { "off" });
        socket.send(message);
    }

    /// Set the metronome settings on the server.
    /// 
    /// Requires a lock on socket.
    pub fn set_metronome_server(&self, active: bool, bpm: u32, volume: f32) {
        let mut socket = self.socket.borrow_mut();
        let rounded_volume = (volume * 100.0).round() / 100.0;
        let message = format!("metronome|{} {} {}\n", if active { "on" } else { "off" }, bpm, rounded_volume);
        socket.send(message);
    }

    /// Set whether the volume monitor is active on the server.
    /// 
    /// Requires a lock on socket.
    pub fn set_volume_monitor_active_server(&self, active: bool) {
        let mut socket = self.socket.borrow_mut();
        let message = format!("volumemonitor|{}\n", if active { "on" } else { "off" });
        socket.send(message);
    }

    pub fn set_volume_normalization_server(&self, mode: crate::settings::VolumeNormalizationMode, auto_decay: f32) {
        let mut socket = self.socket.borrow_mut();
        let rounded_auto_decay = (auto_decay * 1000.0).round() / 1000.0;
        match mode {
            crate::settings::VolumeNormalizationMode::None => socket.send("volumenormalization|none\n".to_string()),
            crate::settings::VolumeNormalizationMode::Manual => socket.send("volumenormalization|manual\n".to_string()),
            crate::settings::VolumeNormalizationMode::Automatic => socket.send(format!("volumenormalization|automatic {}\n", rounded_auto_decay)),
        };
    }

    pub fn reset_volume_normalization_peak(&self) {
        let mut socket = self.socket.borrow_mut();
        socket.send("volumenormalization|reset\n".to_string());
    }

    pub fn master_in_server(&self, volume: f32) {
        let mut socket = self.socket.borrow_mut();
        let rounded_volume = (volume * 100.0).round() / 100.0;
        let message = format!("masterin|{}\n", rounded_volume);
        socket.send(message);
    }

    pub fn master_out_server(&self, volume: f32) {
        let mut socket = self.socket.borrow_mut();
        let rounded_volume = (volume * 100.0).round() / 100.0;
        let message = format!("masterout|{}\n", rounded_volume);
        socket.send(message);
    }

    pub fn start_recording_server(&self) {
        let mut socket = self.socket.borrow_mut();
        socket.send("startrecording\n".to_string());
    }

    pub fn stop_recording_server(&self) {
        let mut socket = self.socket.borrow_mut();
        socket.send("stoprecording\n".to_string());
    }

    pub fn set_recorder_clean_server(&self, clean: bool) {
        let mut socket = self.socket.borrow_mut();
        let message = format!("recordclean|{}\n", if clean { "on" } else { "off" });
        socket.send(message);
    }

    pub fn load_state(egui_ctx: eframe::egui::Context) -> Self {
        let pedalboards = SavedPedalboards::load_or_default();
        let socket = ClientSocket::new(crate::SERVER_PORT);
        let client_settings = ClientSettings::load_or_default();

        // Set NAM folders, IR folders and VST2 in ctx memory so pedals can access
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

        State {
            pedalboards,
            socket: RefCell::new(socket),
            client_settings: RefCell::new(client_settings),
            server_settings: RefCell::new(server_settings),
            midi_state: RefCell::new(MidiState::new(midi_settings, egui_ctx))
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
                drop(socket);
                let client_settings = self.client_settings.borrow();
                self.set_volume_monitor_active_server(client_settings.show_volume_monitor);
                self.set_volume_normalization_server(client_settings.volume_normalization, client_settings.auto_volume_normalization_decay);
                self.master_in_server(client_settings.input_volume);
                self.load_active_set();

                self.socket.borrow_mut().send("requestsr\n".to_string());
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

    /// Apply parameter updates that other threads have sent to the server (oscillators, etc)
    /// 
    /// Requires a lock on socket, active_pedalboardstage
    pub fn apply_parameter_updates(&self) {
        let mut socket = self.socket.borrow_mut();
        for (ParameterPath { pedalboard_id, pedal_id, parameter_name }, new_value) in socket.parameter_updates.drain() {
            self.set_parameter(pedalboard_id, pedal_id, parameter_name, new_value, true);
        }
    }

    pub fn default_with_context(egui_ctx: eframe::egui::Context) -> Self {
        let socket = ClientSocket::new(crate::SERVER_PORT);
        let midi_state = MidiState::new(MidiSettings::default(), egui_ctx);
        Self {
            pedalboards: SavedPedalboards::default(),
            socket: RefCell::new(socket),
            client_settings: RefCell::new(ClientSettings::default()),
            server_settings: RefCell::new(ServerSettingsSave::default()),
            midi_state: RefCell::new(midi_state)
        }
    }
}