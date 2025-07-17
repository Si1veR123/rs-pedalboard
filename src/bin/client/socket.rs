use std::io::Write;
use std::net::TcpStream;
use std::net::Ipv4Addr;

use rs_pedalboard::pedalboard::Pedalboard;
use rs_pedalboard::pedalboard_set::PedalboardSet;
use rs_pedalboard::pedals::Pedal;
use rs_pedalboard::pedals::PedalParameterValue;
use rs_pedalboard::socket_helper::CommandReceiver;

pub struct ClientSocket {
    port: u16,
    stream: Option<TcpStream>,
    command_receiver: CommandReceiver,
    // Commands that have been received but not yet processed
    pub received_commands: Vec<String>,
}

impl ClientSocket {
    pub fn new(port: u16) -> Self {
        ClientSocket {
            port,
            stream: None,
            command_receiver: CommandReceiver::new(),
            received_commands: Vec::new(),
        }
    }

    /// Check if the server is available, but don't maintain a connection.
    pub fn is_server_available(&mut self) -> bool {
        log::info!("Checking if server is available on port {}", self.port);
        TcpStream::connect((Ipv4Addr::LOCALHOST, self.port)).is_ok()
    }

    pub fn connect(&mut self) -> std::io::Result<()> {
        log::info!("Connecting to server on port {}", self.port);
        let stream = TcpStream::connect((Ipv4Addr::LOCALHOST, self.port))?;
        stream.set_nonblocking(true)?;
        self.stream = Some(stream);
        log::info!("Connected to server on port {}", self.port);
        Ok(())
    }

    pub fn is_connected(&self) -> bool {
        self.stream.is_some()
    }

    pub fn send(&mut self, message: &str) {
        if let Some(stream) = &mut self.stream {
            if let Err(e) = stream.write_all(message.as_bytes()) {
                match e.kind() {
                    std::io::ErrorKind::BrokenPipe |
                    std::io::ErrorKind::NotConnected |
                    std::io::ErrorKind::ConnectionReset |
                    std::io::ErrorKind::ConnectionAborted => {
                        log::info!("Connection closed");
                        self.stream = None;
                    },
                    _ => {
                        log::error!("Failed to send message. Closing connection. Error: {}", e);
                        self.stream = None
                    }
                }
            } else {
                if message.len() < 40 || cfg!(feature="log_full_commands") {
                    log::info!("Sent: {:?}", message);
                } else {
                    log::info!("Sent: {:?}...", &message[..40]);
                }
            }
        } else {
            log::warn!("Socket not connected");
        }
    }

    pub fn update_recv(&mut self) -> std::io::Result<()> {
        if let Some(stream) = &mut self.stream {
            match self.command_receiver.receive_commands(stream, &mut self.received_commands) {
                Ok(closed) if closed => {
                    log::info!("Connection closed");
                    self.stream = None;
                },
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionAborted || e.kind() == std::io::ErrorKind::ConnectionReset => {
                    log::info!("Connection closed");
                    self.stream = None;
                },
                Err(e) => return Err(e),
                _ => {}
            }
        }
        Ok(())
    }

    pub fn set_tuner(&mut self, active: bool) {
        let message = format!("tuner {}\n", if active { "on" } else { "off" });
        self.send(&message);
    }

    pub fn set_metronome(&mut self, active: bool, bpm: u32, volume: f32) {
        let message = format!("metronome {} {} {}\n", if active { "on" } else { "off" }, bpm, volume);
        self.send(&message);
    }

    pub fn set_parameter(&mut self, pedalboard_index: usize, pedal_index: usize, name: &str, parameter_value: &PedalParameterValue) {
        let message = format!("setparameter {} {} {} {}\n", pedalboard_index, pedal_index, name, serde_json::to_string(parameter_value).unwrap());
        self.send(&message);
    }

    pub fn move_pedalboard(&mut self, src_index: usize, dest_index: usize) {
        let message = format!("movepedalboard {} {}\n", src_index, dest_index);
        self.send(&message);
    }

    pub fn add_pedalboard(&mut self, pedalboard: &Pedalboard) {
        let message = format!("addpedalboard {}\n", serde_json::to_string(pedalboard).unwrap());
        self.send(&message);
    }

    pub fn delete_pedalboard(&mut self, pedalboard_index: usize) {
        let message = format!("deletepedalboard {}\n", pedalboard_index);
        self.send(&message);
    }

    pub fn add_pedal(&mut self, pedalboard_index: usize, pedal: &Pedal) {
        let message = format!("addpedal {} {}\n", pedalboard_index, serde_json::to_string(pedal).unwrap());
        self.send(&message);
    }

    pub fn delete_pedal(&mut self, pedalboard_index: usize, pedal_index: usize) {
        let message = format!("deletepedal {} {}\n", pedalboard_index, pedal_index);
        self.send(&message);
    }

    pub fn move_pedal(&mut self, pedalboard_index: usize, src_index: usize, dest_index: usize) {
        let message = format!("movepedal {} {} {}\n", pedalboard_index, src_index, dest_index);
        self.send(&message);
    }

    pub fn load_set(&mut self, pedalboard_set: &PedalboardSet) {
        let message = format!("loadset {}\n", serde_json::to_string(pedalboard_set).unwrap());
        self.send(&message);
    }

    pub fn play(&mut self, pedalboard_index: usize) {
        let message = format!("play {}\n", pedalboard_index);
        self.send(&message);
    }

    pub fn master(&mut self, volume: f32) {
        let message = format!("master {}\n", volume);
        self.send(&message);
    }

    pub fn kill(&mut self) {
        log::info!("Sending kill command to server.");
        self.send("kill\n");
        self.stream.take();
        self.command_receiver.reset();
    }
}
