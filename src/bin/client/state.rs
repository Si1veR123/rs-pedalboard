use std::cell::RefCell;
use rs_pedalboard::{pedalboard::Pedalboard, pedalboard_set::PedalboardSet, pedals::{Pedal, PedalParameterValue, PedalTrait}, server_settings::ServerSettingsSave};
use crate::{saved_pedalboards::SavedPedalboards, settings::ClientSettings, socket::ClientSocket};

pub struct State {
    pub pedalboards: SavedPedalboards,
    pub socket: RefCell<ClientSocket>,

    pub client_settings: RefCell<ClientSettings>,
    pub server_settings: RefCell<ServerSettingsSave>
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

        self.socket.borrow_mut().delete_pedalboard(index);
    }

    /// Move a pedalboard in the active pedalboard stage
    /// 
    /// Requires a lock on active_pedalboardstage and socket
    pub fn move_pedalboard(&self, src_index: usize, dest_index: usize) {
        let mut pedalboard_set = self.pedalboards.active_pedalboardstage.borrow_mut();
        egui_dnd::utils::shift_vec(src_index, dest_index, &mut pedalboard_set.pedalboards);

        self.socket.borrow_mut().move_pedalboard(src_index, dest_index);
    }

    /// Add a pedalboard to the active pedalboard stage
    /// 
    /// Requires a lock on active_pedalboardstage and socket
    pub fn add_pedalboard(&self, pedalboard: Pedalboard) {
        let mut pedalboard_set = self.pedalboards.active_pedalboardstage.borrow_mut();
        let mut socket = self.socket.borrow_mut();

        socket.add_pedalboard(&pedalboard);
        pedalboard_set.pedalboards.push(pedalboard);
    }

    /// Duplicate pedalboard in stage with same name
    /// 
    /// Requires a lock on active_pedalboardstage and socket
    pub fn duplicate_linked(&self, index: usize) {
        let mut pedalboard_set = self.pedalboards.active_pedalboardstage.borrow_mut();
        let pedalboard = &pedalboard_set.pedalboards[index];
        let new_pedalboard = pedalboard.clone();

        let mut socket = self.socket.borrow_mut();
        socket.add_pedalboard(&pedalboard);
        socket.move_pedalboard(pedalboard_set.pedalboards.len()-1, index+1);

        pedalboard_set.pedalboards.insert(index+1, new_pedalboard);
    }

    /// Duplicate pedalboard in stage with new name
    /// 
    /// Requires a lock on active_pedalboardstage and socket
    pub fn duplicate_new(&self, index: usize) {
        let pedalboard_set = self.pedalboards.active_pedalboardstage.borrow_mut();
        let pedalboard = &pedalboard_set.pedalboards[index];

        let mut new_pedalboard = pedalboard.clone();
        // Have to drop as the unique stage name requires a lock on active pedalboard stage
        drop(pedalboard_set);
        new_pedalboard.name = self.pedalboards.unique_stage_pedalboard_name(new_pedalboard.name.clone());
        // Reborrow
        let mut pedalboard_set = self.pedalboards.active_pedalboardstage.borrow_mut();
        let pedalboard = &pedalboard_set.pedalboards[index];

        let mut socket = self.socket.borrow_mut();
        socket.add_pedalboard(&pedalboard);
        socket.move_pedalboard(pedalboard_set.pedalboards.len()-1, index+1);

        pedalboard_set.pedalboards.insert(index+1, new_pedalboard);
    }

    /// Add a pedal to the active pedalboard and matching pedalboard in library
    /// 
    /// Requires a lock on active_pedalboardstage, pedalboard_library, and socket
    pub fn add_pedal(&self, pedal: &Pedal) {
        let mut socket = self.socket.borrow_mut();

        let mut active_pedalboardstage = self.pedalboards.active_pedalboardstage.borrow_mut();
        let active_pedalboard_name = active_pedalboardstage.pedalboards[active_pedalboardstage.active_pedalboard].name.clone();
        
        // Add in pedalboard library
        let mut pedalboard_library = self.pedalboards.pedalboard_library.borrow_mut();
        for pedalboard in pedalboard_library.iter_mut() {
            if pedalboard.name == *active_pedalboard_name {
                pedalboard.pedals.push(pedal.clone());
                break;
            }
        }

        // Add in all matching pedalboards in active pedalboard stage
        for (i, pedalboard) in active_pedalboardstage.pedalboards.iter_mut().enumerate() {
            if pedalboard.name == *active_pedalboard_name {
                pedalboard.pedals.push(pedal.clone());
                socket.add_pedal(i, &pedal);
            }
        }
    }

    /// Move a pedal in the pedalboard stage and in library
    /// 
    /// Requires a lock on active_pedalboardstage, pedalboard_library, and socket
    pub fn move_pedal(&self, pedalboard_index: usize, src_index: usize, dest_index: usize) {
        let mut active_pedalboardstage = self.pedalboards.active_pedalboardstage.borrow_mut();
        let active_pedalboard_name = active_pedalboardstage.pedalboards[pedalboard_index].name.clone();
        let mut pedalboard_library = self.pedalboards.pedalboard_library.borrow_mut();

        for pedalboard in pedalboard_library.iter_mut() {
            if pedalboard.name == *active_pedalboard_name {
                egui_dnd::utils::shift_vec(src_index, dest_index, &mut pedalboard.pedals);
                break;
            }
        }

        // Move in all matching pedalboards in active pedalboard stage
        for (i, pedalboard) in active_pedalboardstage.pedalboards.iter_mut().enumerate() {
            if pedalboard.name == *active_pedalboard_name {
                egui_dnd::utils::shift_vec(src_index, dest_index, &mut pedalboard.pedals);
                self.socket.borrow_mut().move_pedal(i, src_index, dest_index);
            }
        }
    }

    /// Delete a pedal from the pedalboard stage and in library
    /// 
    /// Requires a lock on active_pedalboardstage, pedalboard_library, and socket
    pub fn delete_pedal(&self, pedalboard_index: usize, pedal_index: usize) {
        let mut active_pedalboardstage = self.pedalboards.active_pedalboardstage.borrow_mut();
        let active_pedalboard_name = active_pedalboardstage.pedalboards[pedalboard_index].name.clone();
        let mut pedalboard_library = self.pedalboards.pedalboard_library.borrow_mut();

        for pedalboard in pedalboard_library.iter_mut() {
            if pedalboard.name == *active_pedalboard_name {
                pedalboard.pedals.remove(pedal_index);
                break;
            }
        }

        // Remove in all matching pedalboards in active pedalboard stage
        for (i, pedalboard) in active_pedalboardstage.pedalboards.iter_mut().enumerate() {
            if pedalboard.name == *active_pedalboard_name {
                pedalboard.pedals.remove(pedal_index);
                self.socket.borrow_mut().delete_pedal(i, pedal_index);
            }
        }
    }

    /// Set a parameter on all pedalboards, on stage and in library, with the same name
    /// 
    /// Requires a lock on active_pedalboardstage, pedalboard_library and socket
    pub fn set_parameter(&self, pedalboard_name: &str, pedal_index: usize, name: &str, parameter_value: &PedalParameterValue) {
        let mut socket = self.socket.borrow_mut();

        // Set parameter on pedalboard stage
        for (i, pedalboard) in self.pedalboards.active_pedalboardstage.borrow_mut().pedalboards.iter_mut().enumerate() {
            if pedalboard.name == pedalboard_name {
                socket.set_parameter(i, pedal_index, name, parameter_value);
                pedalboard.pedals[pedal_index].set_parameter_value(name, parameter_value.clone());
            }
        }

        // Set parameter on pedalboard library
        for pedalboard in self.pedalboards.pedalboard_library.borrow_mut().iter_mut() {
            if pedalboard.name == pedalboard_name {
                pedalboard.pedals[pedal_index].set_parameter_value(name, parameter_value.clone());
            }
        }
    }

    /// Load a given pedalboard set to active stage
    /// 
    /// Requires a lock on active_pedalboardstage and socket
    pub fn load_set(&self, pedalboard_set: PedalboardSet) {
        let mut socket = self.socket.borrow_mut();
        socket.load_set(&pedalboard_set);

        *self.pedalboards.active_pedalboardstage.borrow_mut() = pedalboard_set;
    }

    /// Tell the server to load the client's active pedalboard stage
    pub fn load_active_set(&self) {
        let mut socket = self.socket.borrow_mut();
        let active_pedalboardstage = self.pedalboards.active_pedalboardstage.borrow();
        socket.load_set(&active_pedalboardstage);
    }

    /// Play a pedalboard from the active stage
    /// 
    /// Requires a lock on active_pedalboardstage and socket
    pub fn play(&self, pedalboard_index: usize) {
        let mut socket = self.socket.borrow_mut();
        socket.play(pedalboard_index);
        self.pedalboards.active_pedalboardstage.borrow_mut().set_active_pedalboard(pedalboard_index);
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
        socket.set_tuner(active);
    }

    /// Set the metronome settings on the server.
    /// 
    /// Requires a lock on socket.
    pub fn set_metronome_server(&self, active: bool, bpm: u32, volume: f32) {
        let mut socket = self.socket.borrow_mut();
        let rounded_volume = (volume * 100.0).round() / 100.0;
        socket.set_metronome(active, bpm, rounded_volume);
    }

    /// Set whether the volume monitor is active on the server.
    /// 
    /// Requires a lock on socket.
    pub fn set_volume_monitor_active_server(&self, active: bool) {
        let mut socket = self.socket.borrow_mut();
        socket.set_volume_monitor(active);
    }

    pub fn set_volume_normalization_server(&self, mode: crate::settings::VolumeNormalizationMode, auto_decay: f32) {
        let mut socket = self.socket.borrow_mut();
        let rounded_auto_decay = (auto_decay * 100.0).round() / 100.0;
        socket.set_volume_normalization(mode, rounded_auto_decay);
    }

    pub fn load_state() -> Result<State, std::io::Error> {
        let pedalboards = SavedPedalboards::load_or_default()?;
        let socket = ClientSocket::new(crate::SERVER_PORT);
        let client_settings = ClientSettings::load_or_default()?;
        let server_settings = ServerSettingsSave::load_or_default()?;
        
        Ok(State {
            pedalboards,
            socket: RefCell::new(socket),
            client_settings: RefCell::new(client_settings),
            server_settings: RefCell::new(server_settings),
        })
    }

    pub fn save_state(&self) -> Result<(), std::io::Error> {
        self.pedalboards.save()?;
        self.client_settings.borrow().save()?;
        self.server_settings.borrow().save()?;
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
                self.load_active_set();
            }
        }
        Ok(())
    }
}

impl Default for State {
    fn default() -> Self {
        let socket = ClientSocket::new(crate::SERVER_PORT);

        State {
            pedalboards: SavedPedalboards::default(),
            socket: RefCell::new(socket),
            client_settings: RefCell::new(ClientSettings::default()),
            server_settings: RefCell::new(ServerSettingsSave::default()),
        }
    }
}