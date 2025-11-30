use std::io::{stdin, stdout, Write};

pub fn device_select_menu(devices: &[String]) -> String {
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
