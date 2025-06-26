use std::{cell::RefCell, collections::HashMap};
use rs_pedalboard::{pedalboard::Pedalboard, pedalboard_set::PedalboardSet, pedals::{Pedal, PedalParameterValue, PedalTrait}};
use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};
use crate::socket::ClientSocket;
use crate::SAVE_DIR;

const SAVE_NAME: &str = "state.json";

pub struct State {
    pub active_pedalboardstage: RefCell<PedalboardSet>,
    pub pedalboard_library: RefCell<Vec<Pedalboard>>,
    pub songs_library: RefCell<HashMap<String, Vec<String>>>,
    pub socket: RefCell<ClientSocket>
}

impl Serialize for State {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut state = serializer.serialize_struct("State", 3)?;
        state.serialize_field("active_pedalboardstage", &*self.active_pedalboardstage.borrow())?;
        state.serialize_field("pedalboard_library", &*self.pedalboard_library.borrow())?;
        state.serialize_field("songs_library", &*self.songs_library.borrow())?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for State {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: serde::Deserializer<'de> {
        #[derive(Deserialize)]
        struct StateData {
            active_pedalboardstage: PedalboardSet,
            pedalboard_library: Vec<Pedalboard>,
            songs_library: HashMap<String, Vec<String>>,
        }

        let data = StateData::deserialize(deserializer)?;
        let mut socket = ClientSocket::new(crate::SERVER_PORT);
        let _ = socket.connect();

        Ok(State {
            active_pedalboardstage: RefCell::new(data.active_pedalboardstage),
            pedalboard_library: RefCell::new(data.pedalboard_library),
            songs_library: RefCell::new(data.songs_library),
            socket: RefCell::new(socket)
        })
    }
}

impl State {
    /// Requires a lock on active_pedalboardstage, pedalboard_library, and songs_library
    pub fn rename_pedalboard(&self, to_rename: &str, new_name: &str) {
        // First rename any matching names in pedalboard library
        let mut pedalboard_library = self.pedalboard_library.borrow_mut();
        for pedalboard in pedalboard_library.iter_mut() {
            if pedalboard.name == to_rename {
                pedalboard.name = new_name.to_string();
            }
        }
    
        // Then rename any matching names in the active pedalboard stage
        let unique_name = self.unique_stage_pedalboard_name(new_name.to_string());
        let mut pedalboard_set = self.active_pedalboardstage.borrow_mut();
        for pedalboard in pedalboard_set.pedalboards.iter_mut() {
            if pedalboard.name == to_rename {
                pedalboard.name = unique_name.clone();
            }
        }

        // Finally rename any matching names in songs
        let mut songs = self.songs_library.borrow_mut();
        for (_, pedalboards) in songs.iter_mut() {
            for pedalboard in pedalboards.iter_mut() {
                if pedalboard == to_rename {
                    *pedalboard = new_name.to_string();
                }
            }
        }
    }
    
    fn unique_name(mut name: String, pedalboards: &[Pedalboard]) -> String {
        name.truncate(25);

        let mut i = 1;
        while pedalboards.iter().any(|pedalboard| pedalboard.name == name) {
            if i == 1 {
                name.push_str("_1");
            } else {
                name.pop();
                name.push_str(&i.to_string());
            }
            
            i += 1;
        }
        name
    }

    /// Requires a lock on active_pedalboardstage
    pub fn unique_stage_pedalboard_name(&self, name: String) -> String {
        Self::unique_name(name, &self.active_pedalboardstage.borrow().pedalboards)
    }

    /// Requires a lock on pedalboard_library
    pub fn unique_library_pedalboard_name(&self, name: String) -> String {
        Self::unique_name(name, &self.pedalboard_library.borrow())
    }

    /// Delete a pedalboard from the pedalboard library
    /// 
    /// Requires a lock on pedalboard_library and songs_library
    pub fn delete_pedalboard(&self, name: &str) {
        let mut pedalboard_library = self.pedalboard_library.borrow_mut();
        if let Some(index) = pedalboard_library.iter().position(|pedalboard| pedalboard.name == name) {
            pedalboard_library.remove(index);
        }

        // Remove the pedalboard from any songs
        let mut songs = self.songs_library.borrow_mut();
        for (_, pedalboards) in songs.iter_mut() {
            if let Some(index) = pedalboards.iter().position(|pedalboard| pedalboard == name) {
                pedalboards.remove(index);
            }
        }
    }

    /// Delete a pedalboard from the active pedalboard stage
    /// 
    /// Requires a lock on active_pedalboardstage and socket
    pub fn remove_pedalboard_from_stage(&self, index: usize) {
        let mut pedalboard_set = self.active_pedalboardstage.borrow_mut();

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
        let mut pedalboard_set = self.active_pedalboardstage.borrow_mut();
        egui_dnd::utils::shift_vec(src_index, dest_index, &mut pedalboard_set.pedalboards);

        self.socket.borrow_mut().move_pedalboard(src_index, dest_index);
    }

    /// Add a pedalboard to the active pedalboard stage
    /// 
    /// Requires a lock on active_pedalboardstage and socket
    pub fn add_pedalboard(&self, pedalboard: Pedalboard) {
        let mut pedalboard_set = self.active_pedalboardstage.borrow_mut();
        let mut socket = self.socket.borrow_mut();

        socket.add_pedalboard(&pedalboard);
        pedalboard_set.pedalboards.push(pedalboard);
    }

    /// Save the current pedalboard stage to a song
    /// 
    /// Requires a lock on active_pedalboardstage, pedalboard_library, and songs_library
    pub fn save_to_song(&self, song_name: String) {
        let active_pedalboards = &self.active_pedalboardstage.borrow().pedalboards;
        let mut pedalboard_library = self.pedalboard_library.borrow_mut();

        for pedalboard in active_pedalboards.iter() {
            let pedalboard_in_library = pedalboard_library.iter_mut().find(|library_pedalboard| library_pedalboard.name == pedalboard.name);
            if pedalboard_in_library.is_none() {
                pedalboard_library.push(pedalboard.clone());
            }
        }

        self.songs_library.borrow_mut().insert(song_name, active_pedalboards.iter().map(|pedalboard| pedalboard.name.clone()).collect());
    }

    /// Duplicate pedalboard in stage with same name
    /// 
    /// Requires a lock on active_pedalboardstage and socket
    pub fn duplicate_linked(&self, index: usize) {
        let mut pedalboard_set = self.active_pedalboardstage.borrow_mut();
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
        let pedalboard_set = self.active_pedalboardstage.borrow_mut();
        let pedalboard = &pedalboard_set.pedalboards[index];

        let mut new_pedalboard = pedalboard.clone();
        // Have to drop as the unique stage name requires a lock on active pedalboard stage
        drop(pedalboard_set);
        new_pedalboard.name = self.unique_stage_pedalboard_name(new_pedalboard.name.clone());
        // Reborrow
        let mut pedalboard_set = self.active_pedalboardstage.borrow_mut();
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

        let mut active_pedalboardstage = self.active_pedalboardstage.borrow_mut();
        let active_pedalboard_name = active_pedalboardstage.pedalboards[active_pedalboardstage.active_pedalboard].name.clone();
        
        // Add in pedalboard library
        let mut pedalboard_library = self.pedalboard_library.borrow_mut();
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
        let mut active_pedalboardstage = self.active_pedalboardstage.borrow_mut();
        let active_pedalboard_name = active_pedalboardstage.pedalboards[pedalboard_index].name.clone();
        let mut pedalboard_library = self.pedalboard_library.borrow_mut();

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
        let mut active_pedalboardstage = self.active_pedalboardstage.borrow_mut();
        let active_pedalboard_name = active_pedalboardstage.pedalboards[pedalboard_index].name.clone();
        let mut pedalboard_library = self.pedalboard_library.borrow_mut();

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
        for (i, pedalboard) in self.active_pedalboardstage.borrow_mut().pedalboards.iter_mut().enumerate() {
            if pedalboard.name == pedalboard_name {
                socket.set_parameter(i, pedal_index, name, parameter_value);
                pedalboard.pedals[pedal_index].set_parameter_value(name, parameter_value.clone());
            }
        }

        // Set parameter on pedalboard library
        for pedalboard in self.pedalboard_library.borrow_mut().iter_mut() {
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

        *self.active_pedalboardstage.borrow_mut() = pedalboard_set;
    }

    /// Tell the server to load the client's active pedalboard stage
    pub fn load_active_set(&self) {
        let mut socket = self.socket.borrow_mut();
        let active_pedalboardstage = self.active_pedalboardstage.borrow();
        socket.load_set(&active_pedalboardstage);
    }

    /// Play a pedalboard from the active stage
    /// 
    /// Requires a lock on active_pedalboardstage and socket
    pub fn play(&self, pedalboard_index: usize) {
        let mut socket = self.socket.borrow_mut();
        socket.play(pedalboard_index);
        self.active_pedalboardstage.borrow_mut().set_active_pedalboard(pedalboard_index);
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

    /// Save the entire state into the save file
    /// 
    /// Requires a lock on active_pedalboardstage, pedalboard_library, and songs_library
    pub fn save(&self) -> Result<(), std::io::Error> {
        let stringified = serde_json::to_string(self).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let dir_path = homedir::my_home().map_err(
            |e| std::io::Error::new(std::io::ErrorKind::Other, e)
        )?.unwrap().join(SAVE_DIR);

        if !dir_path.exists() {
            std::fs::create_dir_all(&dir_path)?;
        }
        let file_path = dir_path.join(SAVE_NAME);

        std::fs::write(file_path, stringified)
    }

    /// Load the state from the save file
    pub fn load() -> Result<State, std::io::Error> {
        let file_path = homedir::my_home().map_err(
            |e| std::io::Error::new(std::io::ErrorKind::Other, e)
        )?.unwrap().join(SAVE_DIR).join(SAVE_NAME);

        if !file_path.exists() {
            return Ok(State::default());
        }

        let stringified = std::fs::read_to_string(file_path)?;
        let state: State = serde_json::from_str(&stringified).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;

        // Attempt to load set if it is connected to server, or ignore and continue
        let _ = state.socket.borrow_mut().load_set(&state.active_pedalboardstage.borrow());
        Ok(state)
    }
}

impl Default for State {
    fn default() -> Self {
        let mut socket = ClientSocket::new(crate::SERVER_PORT);
        let _ = socket.connect();

        State {
            active_pedalboardstage: RefCell::new(PedalboardSet::default()),
            pedalboard_library: RefCell::new(Vec::new()),
            songs_library: RefCell::new(HashMap::new()),
            socket: RefCell::new(socket)
        }
    }
}