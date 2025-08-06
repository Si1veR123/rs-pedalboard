use std::io::Write;
use std::net::TcpStream;
use std::net::Ipv4Addr;

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
        log::info!("Attempting to connect to server on port {}", self.port);
        let stream = match TcpStream::connect((Ipv4Addr::LOCALHOST, self.port)) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Failed to connect to server: {}", e);
                return Err(e);
            }
        };
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

    pub fn kill(&mut self) {
        log::info!("Sending kill command to server.");
        
        self.send("kill\n");
        if let Some(stream) = &mut self.stream {
            let _ = stream.flush();
            let _ = stream.shutdown(std::net::Shutdown::Both);
        }
        self.stream.take();
        self.command_receiver.reset();
    }
}
