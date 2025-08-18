use std::{cell::RefCell, collections::HashMap};

use rs_pedalboard::{pedalboard::Pedalboard, pedalboard_set::PedalboardSet, SAVE_DIR};
use serde::{ser::SerializeStruct, Deserialize, Serialize, Serializer};

const SAVE_NAME: &str = "pedalboards.json";

pub struct SavedPedalboards {
    pub active_pedalboardstage: RefCell<PedalboardSet>,
    pub pedalboard_library: RefCell<Vec<Pedalboard>>,
    pub songs_library: RefCell<HashMap<String, Vec<String>>>,
}

impl Serialize for SavedPedalboards {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error> where S: Serializer {
        let mut state = serializer.serialize_struct("SavedPedalboards", 3)?;
        state.serialize_field("active_pedalboardstage", &*self.active_pedalboardstage.borrow())?;
        state.serialize_field("pedalboard_library", &*self.pedalboard_library.borrow())?;
        state.serialize_field("songs_library", &*self.songs_library.borrow())?;
        state.end()
    }
}

impl<'de> Deserialize<'de> for SavedPedalboards {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error> where D: serde::Deserializer<'de> {
        #[derive(Deserialize)]
        struct SavedPedalboardsData {
            active_pedalboardstage: PedalboardSet,
            pedalboard_library: Vec<Pedalboard>,
            songs_library: HashMap<String, Vec<String>>,
        }

        let data = SavedPedalboardsData::deserialize(deserializer)?;

        Ok(SavedPedalboards {
            active_pedalboardstage: RefCell::new(data.active_pedalboardstage),
            pedalboard_library: RefCell::new(data.pedalboard_library),
            songs_library: RefCell::new(data.songs_library)
        })
    }
}

impl Default for SavedPedalboards {
    fn default() -> Self {
        SavedPedalboards {
            active_pedalboardstage: RefCell::new(PedalboardSet::default()),
            pedalboard_library: RefCell::new(Vec::new()),
            songs_library: RefCell::new(HashMap::new()),
        }
    }
}

impl SavedPedalboards {
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

    /// Save the pedalboard library into the save file
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

    /// Load the pedalboard library from the save file, or default
    pub fn load_or_default() -> Self {
        let file_path = match homedir::my_home() {
            Ok(Some(home)) => home.join(SAVE_DIR).join(SAVE_NAME),
            Ok(None) => {
                log::error!("Failed to resolve home directory, using default");
                return Self::default();
            }
            Err(e) => {
                log::error!("Error resolving home directory: {e}, using default");
                return Self::default();
            }
        };

        if !file_path.exists() {
            log::info!("Pedalboard save file not found, using default");
            return Self::default();
        }

        match std::fs::read_to_string(&file_path) {
            Ok(stringified) => match serde_json::from_str::<Self>(&stringified) {
                Ok(state) => state,
                Err(e) => {
                    log::error!("Failed to parse save file {:?}: {e}, using default", file_path);
                    Self::default()
                }
            },
            Err(e) => {
                log::error!("Failed to read save file {:?}: {e}, using default", file_path);
                Self::default()
            }
        }
    }
}
