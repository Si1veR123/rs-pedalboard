use serde_json;

use rs_pedalboard::pedals::info::Info;

fn main() {
    let info = Info::pedal_defaults();

    let json_str = serde_json::to_string_pretty(&info).expect("Failed to serialize info to JSON");

    match std::env::args().nth(1) {
        Some(path) => std::fs::write(path, json_str).expect("Failed to write info JSON to file"),
        None => println!("{}", json_str),
    }
}
