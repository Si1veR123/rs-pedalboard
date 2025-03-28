use cpal::{traits::{DeviceTrait, HostTrait}, Device, Host};
use super::device_select::device_select_menu;

fn find_device_by_name(host: &Host, name: &str) -> Option<Device> {
    host.devices().expect("Failed to get devices")
        .find(|d| d.name().unwrap() == name)
}

pub fn get_windows_host() -> Host{
    cpal::host_from_id(
        cpal::available_hosts()
            .into_iter()
            .find(|id| *id == cpal::HostId::Wasapi)
            .expect("Wasapi host not found")
    ).unwrap()
}

pub fn setup() -> (Host, Device, Device) {
    let wasapi_host = get_windows_host();

    let input_devices: Vec<String> = wasapi_host.input_devices().expect("Failed to get input devices")
        .map(|d| d.name().unwrap())
        .collect();

    let output_devices: Vec<String> = wasapi_host.output_devices().expect("Failed to get output devices")
        .map(|d| d.name().unwrap())
        .collect();

    println!("Input Devices:");
    let input_device_string = device_select_menu(&input_devices);
    let input_device = find_device_by_name(&wasapi_host, &input_device_string).unwrap();

    println!("Output Devices:");
    let output_device_string = device_select_menu(&output_devices);
    let output_device = find_device_by_name(&wasapi_host, &output_device_string).unwrap();

    (wasapi_host, input_device, output_device)
}

pub fn after_setup() {

}
