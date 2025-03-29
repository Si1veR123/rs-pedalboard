mod socket;
use std::thread;

use socket::ClientSocket;
use rs_pedalboard::pedals::PedalParameterValue;

fn main() {
    let mut socket = ClientSocket::new(29475);
    socket.connect().expect("Failed to connect to server");

    let command = format!("setparameter 0 0 max_depth {}", serde_json::to_string(&PedalParameterValue::Float(50.0)).unwrap());
    log::info!("Sending command: {}", command);
    socket.send(&command).expect("Failed to send command");
}