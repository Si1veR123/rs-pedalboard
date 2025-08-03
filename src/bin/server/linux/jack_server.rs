use cpal::{traits::{DeviceTrait, HostTrait}, Host, Device};
use std::{fs::File, io, process::{Child, Command, Stdio}};
use rs_pedalboard::audio_devices::get_host;

pub fn get_jack_host() -> (Host, Device, Device) {
    let jack_host = get_host().expect("Failed to get JACK host");

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

pub fn start_jack_server(frames_per_period: usize, periods_per_buffer: usize, sample_rate: u32, input: String, output: String) -> io::Result<Child> {
    kill_jack_servers();
    jack_server_wait(false);
    std::thread::sleep(std::time::Duration::from_millis(1000));

    log::info!("Starting JACK server with: Frames per Period {frames_per_period}, Periods per Buffer {periods_per_buffer}, Input {input}, Output {output}");

    Command::new("jackd")
        .arg("-dalsa")
        .arg(format!("-r{sample_rate}"))
        .arg(format!("-p{frames_per_period}"))
        .arg(format!("-n{periods_per_buffer}"))
        .arg(format!("-C{input}"))
        .arg(format!("-P{output}"))
        .stdout(File::create("jack_server_out.log").expect("Failed to create file for jack server stdout"))
        .stderr(File::create("jack_server_err.log").expect("Failed to create file for jack server stderr"))
        .spawn()
}

pub fn jack_server_wait(wait_until_open: bool) {
    log::info!("Starting jack_wait. Waiting until open={wait_until_open}");

    let mut command = Command::new("jack_wait");
    command.arg("-t").arg("10");

    if !wait_until_open {
        command.arg("-q")
    } else {
        command.arg("-w")
    };

    let status = command
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
