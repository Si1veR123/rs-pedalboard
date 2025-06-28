use cpal::{traits::{DeviceTrait, HostTrait}, Device, Host};
use crate::ServerSettings;
use super::device_select::device_select_menu;
use rs_pedalboard::audio_devices::{get_input_devices, get_output_devices};

fn find_device_by_name(host: &Host, name: &str) -> Option<Device> {
    host.devices().expect("Failed to get devices")
        .find(|d| d.name().unwrap() == name)
}

pub fn setup(input: Option<&str>, output: Option<&str>, args: &ServerSettings) -> (Host, Device, Device) {
    let host_id = args.host.into();

    if !cpal::available_hosts().contains(&host_id) {
        panic!("Host {:?} is not available on this platform", host_id);
    }

    let windows_host = cpal::host_from_id(host_id).expect("Failed to get host from ID");

    let input_device = match input {
        Some(name) => find_device_by_name(&windows_host, name).expect("Input device not found"),
        None => {
            let input_devices = get_input_devices(Some(&windows_host)).expect("Failed to get input devices");
            println!("Input Devices:");
            let input_device_string = device_select_menu(&input_devices);
            find_device_by_name(&windows_host, &input_device_string).unwrap()
        }
    };

    let output_device = match output {
        Some(name) => find_device_by_name(&windows_host, name).expect("Output device not found"),
        None => {
            let output_devices: Vec<String> = get_output_devices(Some(&windows_host)).expect("Failed to get output devices");

            println!("Output Devices:");
            let output_device_string = device_select_menu(&output_devices);
            find_device_by_name(&windows_host, &output_device_string).unwrap()
        }
    };

    (windows_host, input_device, output_device)
}

pub fn after_setup() {

}
