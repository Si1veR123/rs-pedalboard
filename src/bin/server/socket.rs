use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::net::Ipv4Addr;

use crossbeam::channel::Sender;

pub struct ServerSocket {
    port: u16,
    command_sender: Sender<Box<str>>,
}

impl ServerSocket {
    pub fn new(port: u16, command_sender: Sender<Box<str>>) -> Self {
        ServerSocket {
            port,
            command_sender,
        }
    }

    pub fn start(&mut self) -> std::io::Result<()> {
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, self.port))?;
        log::info!("Server listening on port {}", self.port);

        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
                    log::info!("New connection: {}", stream.peer_addr()?);
                    // Don't make a new thread as currently only one client is supported
                    self.handle_client(stream);
                }
                Err(e) => log::error!("Connection failed: {}", e),
            }
        }

        Ok(())
    }

    fn handle_client(&mut self, mut stream: TcpStream) {
        let mut buffer = [0; 2048];
        loop {
            match stream.read(&mut buffer) {
                Ok(0) => break, // Connection closed
                Ok(n) => {
                    if let Ok(received_str) = std::str::from_utf8(&buffer[..n]) {
                        log::info!("Received: {:?}", received_str);
                        
                        if self.command_sender.send(received_str.into()).is_err() {
                            log::error!("Failed to send command to audio thread");
                            break;
                        }
                    } else {
                        log::error!("Received invalid UTF-8 string");
                        continue;
                    }
                },
                Err(e) => {
                    log::error!("Failed to read from socket: {}", e);
                    break;
                }
            }
        }
    }
}
