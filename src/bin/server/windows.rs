use cpal::{traits::{DeviceTrait, HostTrait}, Device, Host};
use crate::ServerSettings;
use super::device_select::device_select_menu;
use rs_pedalboard::audio_devices::{get_input_devices, get_output_devices, get_host};

fn find_device_by_name(host: &Host, name: &str) -> Option<Device> {
    host.devices().expect("Failed to get devices")
        .find(|d| d.name().unwrap() == name)
}

pub fn setup(input: Option<&str>, output: Option<&str>, _args: &ServerSettings) -> (Host, Device, Device) {
    let wasapi_host = get_host().expect("Failed to get WASAPI host");

    let input_device = match input {
        Some(name) => find_device_by_name(&wasapi_host, name).expect("Input device not found"),
        None => {
            let input_devices = get_input_devices(Some(&wasapi_host)).expect("Failed to get input devices");
            println!("Input Devices:");
            let input_device_string = device_select_menu(&input_devices);
            find_device_by_name(&wasapi_host, &input_device_string).unwrap()
        }
    };

    let output_device = match output {
        Some(name) => find_device_by_name(&wasapi_host, name).expect("Output device not found"),
        None => {
            let output_devices: Vec<String> = get_output_devices(Some(&wasapi_host)).expect("Failed to get output devices");

            println!("Output Devices:");
            let output_device_string = device_select_menu(&output_devices);
            find_device_by_name(&wasapi_host, &output_device_string).unwrap()
        }
    };

    (wasapi_host, input_device, output_device)
}

pub fn after_setup() {

}
