use cpal::{traits::{DeviceTrait, HostTrait}, Host, Device};
use std::{fs::File, io::{self, stdin, stdout, Write}, process::{Child, Command}};

fn get_alsa_host() -> Option<Host> {
    cpal::host_from_id(
        cpal::available_hosts()
            .into_iter()
            .find(|id| *id == cpal::HostId::Alsa)?
    ).ok()
}

/// Convert an alsa device name such as 'hw:CARD=CODEC,DEV=0' to 'hw:CODEC'
fn device_name_from_alsa_name(alsa_name: String) -> Option<String> {
    if alsa_name.starts_with("hw:CARD") {
        let comma_index = alsa_name.find(',')?;
        Some(format!("hw:{}", &alsa_name[8..comma_index]))
    } else {
        None
    }
}

fn get_alsa_input_strings() -> Vec<String> {
    let alsa = get_alsa_host().expect("Failed to get ALSA host");
    alsa.input_devices().unwrap()
        .filter_map(|d| d.name().ok())
        .filter_map(|name| device_name_from_alsa_name(name))
        .collect()
}

fn get_alsa_output_strings() -> Vec<String> {
    let alsa = get_alsa_host().expect("Failed to get ALSA host");
    alsa.output_devices().unwrap()
        .filter_map(|d| d.name().ok())
        .filter_map(|name| device_name_from_alsa_name(name))
        .collect()
}

fn device_select_menu(mut devices: Vec<String>) -> String {
    let mut input_buf = String::new();

    for (i, device) in devices.iter().enumerate() {
        println!("{}: {}", i, device);
    }
    print!("Select a device: ");
    stdout().flush().expect("Failed to flush stdout");
    stdin().read_line(&mut input_buf).expect("Failed to read stdin");
    devices.remove(input_buf.trim().parse::<usize>().expect("Failed to parse device index"))
}

pub fn io_device_selector() -> (String, String) {
    let input_devices = get_alsa_input_strings();
    let output_devices = get_alsa_output_strings();

    println!("Input Devices:");
    let input_device = device_select_menu(input_devices);
    
    println!("Output Devices:");
    let output_device = device_select_menu(output_devices);

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

pub fn start_jack_server(frames_per_period: usize, periods_per_buffer: usize, input: String, output: String) -> io::Result<Child> {
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
    let code = Command::new("jack_wait")
        .arg("-t 10")
        .arg("-w")
        .stdout(File::create("jack_wait_out.log").expect("Failed to create file for jack wait stdout"))
        .stderr(File::create("jack_wait_err.log").expect("Failed to create file for jack wait stderr"))
        .spawn()
        .expect("Failed to execute jack_wait")
        .wait()
        .expect("Failed to wait");

    if code.code().unwrap() == 1 {
        panic!("jack_wait timeout")
    }

    log::info!("jack_wait completed successfully. JACK server is running.");
}

pub fn stereo_output() {
    Command::new("jack_connect")
        .arg("cpal_client_out:out_0")
        .arg("system:playback_2")
        .spawn()
        .expect("Failed to connect to second output port");
    }
