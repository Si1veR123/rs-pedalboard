use std::{fs::File, io::{stdin, stdout, Read, Write}};
use regex::Regex;

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

fn device_select_menu(devices: &[String]) -> String {
    let mut input_buf = String::new();

    for (i, device) in devices.iter().enumerate() {
        println!("{}: {}", i, device);
    }
    print!("Select a device: ");
    stdout().flush().expect("Failed to flush stdout");
    stdin().read_line(&mut input_buf).expect("Failed to read stdin");

    devices.get(
        input_buf.trim().parse::<usize>().expect("Failed to parse device index")
    ).expect("Invalid index").clone()
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