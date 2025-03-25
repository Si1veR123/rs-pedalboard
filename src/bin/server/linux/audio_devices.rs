use std::{fs::File, io::Read};
use regex::Regex;
use crate::device_select::device_select_menu;

fn get_hw_devices() -> Vec<String> {
    let mut sound_cards = String::new();
    File::open("/proc/asound/cards")
        .expect("Failed to open sound cards file")
        .read_to_string(&mut sound_cards)
        .expect("Failed to read sound cards");

    let re = Regex::new(r"\[(.*)\]").unwrap();
    re.captures_iter(&sound_cards).map(|c| {
        let (_full, [name]) = c.extract();
        format!("hw:{}", name.trim())
    }).collect()
}

pub fn io_device_selector() -> (String, String) {
    let devices = get_hw_devices();

    println!("Input Devices:");
    let input_device = device_select_menu(&devices);
    
    println!("Output Devices:");
    let output_device = device_select_menu(&devices);

    log::info!("Selected ALSA Devices: Input {input_device}, Output {output_device}");

    (input_device, output_device)
}