use std::{cell::RefCell, collections::HashMap};
use rs_pedalboard::{pedalboard::Pedalboard, pedalboard_set::PedalboardSet, pedals::{PedalParameterValue, PedalTrait}};
use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};
use crate::socket::ClientSocket;

const SAVE_NAME: &str = "state.json";

// TODO: all socket calls should be a method of the state, rather than directly calling the socket
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
        Ok(State {
            active_pedalboardstage: RefCell::new(data.active_pedalboardstage),
            pedalboard_library: RefCell::new(data.pedalboard_library),
            songs_library: RefCell::new(data.songs_library),
            socket: RefCell::new(ClientSocket::new(crate::SERVER_PORT))
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
        pedalboard_set.pedalboards.remove(index);

        self.socket.borrow_mut().delete_pedalboard(index).expect("Failed to delete pedalboard from socket");
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
        socket.add_pedalboard(&pedalboard).expect("Failed to add pedalboard");
        socket.move_pedalboard(pedalboard_set.pedalboards.len()-1, index+1).expect("Failed to move pedalboard");

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
        socket.add_pedalboard(&pedalboard).expect("Failed to add pedalboard");
        socket.move_pedalboard(pedalboard_set.pedalboards.len()-1, index+1).expect("Failed to move pedalboard");

        pedalboard_set.pedalboards.insert(index+1, new_pedalboard);
    }

    /// Set a parameter on all pedalboards, on stage and in library, with the same name
    /// 
    /// Requires a lock on active_pedalboardstage, pedalboard_library and socket
    pub fn set_parameter(&self, pedalboard_name: &str, pedal_index: usize, name: &str, parameter_value: &PedalParameterValue) {
        let mut socket = self.socket.borrow_mut();

        // Set parameter on pedalboard stage
        for (i, pedalboard) in self.active_pedalboardstage.borrow_mut().pedalboards.iter_mut().enumerate() {
            if pedalboard.name == pedalboard_name {
                socket.set_parameter(i, pedal_index, name, parameter_value).expect("Failed to set parameter");
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

    pub fn save(&self) -> Result<(), std::io::Error> {
        let stringified = serde_json::to_string(self).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
        let file_path = homedir::my_home().map_err(
            |e| std::io::Error::new(std::io::ErrorKind::Other, e)
        )?.unwrap().join(SAVE_NAME);

        std::fs::write(file_path, stringified)
    }

    pub fn load() -> Result<State, std::io::Error> {
        let file_path = homedir::my_home().map_err(
            |e| std::io::Error::new(std::io::ErrorKind::Other, e)
        )?.unwrap().join(SAVE_NAME);

        if !file_path.exists() {
            return Ok(State::default());
        }

        let stringified = std::fs::read_to_string(file_path)?;
        let state: State = serde_json::from_str(&stringified).map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
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