use rs_pedalboard::pedalboard::Pedalboard;

use crate::State;


pub fn unique_pedalboard_name(mut name: String, state: &State) -> String {
    let mut i = 1;
    while state.active_pedalboardset.borrow().pedalboards.iter()
        .chain(state.pedalboard_library.borrow().iter())
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

// TODO: ensure that when a pedalboard is removed, it is removed from songs too
pub fn remove_pedalboard() {
    unimplemented!();
}
