use cpal::{traits::{DeviceTrait, HostTrait}, Host, Device};
use regex::Regex;
use std::{fs::File, io::{self, stdin, stdout, Read, Write}, process::{Child, Command, Stdio}};


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

pub fn get_jack_host() -> (Host, Device, Device) {
    let jack_host = cpal::host_from_id(
        cpal::available_hosts()
            .into_iter()
            .find(|id| *id == cpal::HostId::Jack)
            .expect("JACK host not found")
        ).unwrap();

    if jack_host.devices().unwrap().count() == 0 {
        panic!("Failed to initialise JACK client");
    }

    let input_device = jack_host.devices().unwrap()
        .find(|d| d.name().unwrap().contains("in"))
        .expect("No JACK input found on host");
    let output_device = jack_host.devices().unwrap()
        .find(|d| d.name().unwrap().contains("out"))
        .expect("No JACK output found on host");

    (jack_host, input_device, output_device)
}

pub fn kill_jack_servers() {
    log::info!("Killing existing JACK servers");    
    Command::new("pkill").arg("jackd").spawn().expect("Failed to kill any existing JACK servers.").wait().unwrap();
}

pub fn start_jack_server(frames_per_period: usize, periods_per_buffer: usize, input: String, output: String) -> io::Result<Child> {
    kill_jack_servers();

    log::info!("Starting JACK server with: Frames per Period {frames_per_period}, Periods per Buffer {periods_per_buffer}, Input {input}, Output {output}");

    Command::new("jackd")
        .arg("-dalsa")
        .arg("-r48000")
        .arg(format!("-p{frames_per_period}"))
        .arg(format!("-n{periods_per_buffer}"))
        .arg(format!("-C{input}"))
        .arg(format!("-P{output}"))
        .stdout(File::create("jack_server_out.log").expect("Failed to create file for jack server stdout"))
        .stderr(File::create("jack_server_err.log").expect("Failed to create file for jack server stderr"))
        .spawn()
}

pub fn jack_server_wait() {
    log::info!("Starting jack_wait");
    let status = Command::new("jack_wait")
        .arg("-t 10")
        .arg("-w")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .expect("Failed to execute jack_wait");

    if status.code().unwrap() == 1 {
        panic!("jack_wait timeout")
    }

    log::info!("jack_wait completed successfully. JACK server is running.");
}

pub fn stereo_output() {
    log::info!("Connecting output to second playback port.");
    Command::new("jack_connect")
        .arg("cpal_client_out:out_0")
        .arg("system:playback_2")
        .spawn()
        .expect("Failed to connect to second playback port");
}
