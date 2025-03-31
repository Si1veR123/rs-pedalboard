use std::{cell::RefCell, collections::HashMap};
use rs_pedalboard::{pedalboard::Pedalboard, pedalboard_set::PedalboardSet};

pub struct State {
    pub active_pedalboardset: RefCell<PedalboardSet>,
    pub pedalboard_library: RefCell<Vec<Pedalboard>>,
    pub songs_library: RefCell<HashMap<String, Vec<String>>>
}

impl State {
    pub fn rename_pedalboard(&self, to_rename: &str, new_name: &str) {
        // First rename any matching names in active pedalboardset
        let mut active_pedalboardset = self.active_pedalboardset.borrow_mut();
        for pedalboard in active_pedalboardset.pedalboards.iter_mut() {
            if pedalboard.name == to_rename {
                pedalboard.name = new_name.to_string();
            }
        }
    
        // Then rename any matching names in pedalboard library
        let mut pedalboard_library = self.pedalboard_library.borrow_mut();
        for pedalboard in pedalboard_library.iter_mut() {
            if pedalboard.name == to_rename {
                pedalboard.name = new_name.to_string();
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
    
    pub fn unique_pedalboard_name(&self, mut name: String) -> String {
        let mut i = 1;
        while self.active_pedalboardset.borrow().pedalboards.iter()
            .chain(self.pedalboard_library.borrow().iter())
            .any(|pedalboard| pedalboard.name == name)
        {
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

    /// Delete a pedalboard from the pedalboard library
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
}

impl Default for State {
    fn default() -> Self {
        State {
            active_pedalboardset: RefCell::new(PedalboardSet::default()),
            pedalboard_library: RefCell::new(Vec::new()),
            songs_library: RefCell::new(HashMap::new()),
        }
    }
}