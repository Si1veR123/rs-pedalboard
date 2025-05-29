use std::io::Read;
use std::io::Write;
use std::net::TcpStream;
use std::net::Ipv4Addr;

use rs_pedalboard::pedalboard::Pedalboard;
use rs_pedalboard::pedalboard_set::PedalboardSet;
use rs_pedalboard::pedals::Pedal;
use rs_pedalboard::pedals::PedalParameterValue;

pub struct ClientSocket {
    port: u16,
    stream: Option<TcpStream>,
    // Commands that have been received but not yet processed
    pub received_commands: Vec<String>,
}

impl ClientSocket {
    pub fn new(port: u16) -> Self {
        ClientSocket {
            port,
            stream: None,
            received_commands: Vec::new(),
        }
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

    pub fn send(&mut self, message: &str) -> std::io::Result<()> {
        if let Some(stream) = &mut self.stream {
            stream.write_all(message.as_bytes())?;
            log::info!("Sent: {:?}", message);
        } else {
            log::error!("Socket not connected");
        }
        
        Ok(())
    }

    pub fn update_recv(&mut self) -> std::io::Result<()> {
        if let Some(stream) = &mut self.stream {
            let mut buffer = [0u8; 1024];
            loop {
                // Stream is non-blocking
                match stream.read(&mut buffer) {
                    Ok(0) => break, // No more data
                    Ok(n) => {
                        let data = String::from_utf8_lossy(&buffer[..n]);
                        for line in data.lines() {
                            log::info!("Received: {:?}", line);
                            self.received_commands.push(line.to_string());
                        }
                    }
                    Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
                    Err(e) => return Err(e),
                }
            }
        }
        Ok(())
    }

    pub fn set_tuner(&mut self, active: bool) -> std::io::Result<()> {
        let message = format!("tuner {}\n", if active { "on" } else { "off" });
        self.send(&message)
    }

    pub fn set_parameter(&mut self, pedalboard_index: usize, pedal_index: usize, name: &str, parameter_value: &PedalParameterValue) -> std::io::Result<()> {
        let message = format!("setparameter {} {} {} {}\n", pedalboard_index, pedal_index, name, serde_json::to_string(parameter_value).unwrap());
        self.send(&message)
    }

    pub fn move_pedalboard(&mut self, src_index: usize, dest_index: usize) -> std::io::Result<()> {
        let message = format!("movepedalboard {} {}\n", src_index, dest_index);
        self.send(&message)
    }

    pub fn add_pedalboard(&mut self, pedalboard: &Pedalboard) -> std::io::Result<()> {
        let message = format!("addpedalboard {}\n", serde_json::to_string(pedalboard).unwrap());
        self.send(&message)
    }

    pub fn delete_pedalboard(&mut self, pedalboard_index: usize) -> std::io::Result<()> {
        let message = format!("deletepedalboard {}\n", pedalboard_index);
        self.send(&message)
    }

    pub fn add_pedal(&mut self, pedalboard_index: usize, pedal: &Pedal) -> std::io::Result<()> {
        let message = format!("addpedal {} {}\n", pedalboard_index, serde_json::to_string(pedal).unwrap());
        self.send(&message)
    }

    pub fn delete_pedal(&mut self, pedalboard_index: usize, pedal_index: usize) -> std::io::Result<()> {
        let message = format!("deletepedal {} {}\n", pedalboard_index, pedal_index);
        self.send(&message)
    }

    pub fn move_pedal(&mut self, pedalboard_index: usize, src_index: usize, dest_index: usize) -> std::io::Result<()> {
        let message = format!("movepedal {} {} {}\n", pedalboard_index, src_index, dest_index);
        self.send(&message)
    }

    pub fn load_set(&mut self, pedalboard_set: &PedalboardSet) -> std::io::Result<()> {
        let message = format!("loadset {}\n", serde_json::to_string(pedalboard_set).unwrap());
        self.send(&message)
    }

    pub fn play(&mut self, pedalboard_index: usize) -> std::io::Result<()> {
        let message = format!("play {}\n", pedalboard_index);
        self.send(&message)
    }

    pub fn master(&mut self, volume: f32) -> std::io::Result<()> {
        let message = format!("master {}\n", volume);
        self.send(&message)
    }
}
