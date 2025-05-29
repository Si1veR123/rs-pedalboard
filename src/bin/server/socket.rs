use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::net::Ipv4Addr;

use crossbeam::channel::{Sender, Receiver};

pub struct ServerSocket {
    port: u16,
    command_sender: Sender<Box<str>>,
    command_receiver: Receiver<Box<str>>,
}

impl ServerSocket {
    pub fn new(port: u16, command_sender: Sender<Box<str>>, command_receiver: Receiver<Box<str>>) -> Self {
        ServerSocket {
            port,
            command_sender,
            command_receiver,
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

    /// Returns true if closed
    fn read_to_newline(&mut self, stream: &mut TcpStream, buffer: &mut Vec<u8>) -> std::io::Result<bool> {
        let mut chunk_buffer = [0; 256];
        buffer.clear();
        loop {
            match stream.read(&mut chunk_buffer) {
                Ok(0) => return Ok(true), // Connection closed
                Ok(n) => {
                    buffer.extend_from_slice(&chunk_buffer[..n]);
                    if let Some(pos) = buffer.iter().position(|&b| b == b'\n') {
                        buffer.truncate(pos + 1); // Keep the newline character
                        break;
                    }
                }
                Err(e) => return Err(e),
            }
        }
        Ok(false) // Connection still open
    }

    fn handle_client(&mut self, mut stream: TcpStream) {
        let mut buffer = Vec::new();
        stream.set_nonblocking(true).expect("Failed to set non-blocking");

        loop {
            match self.read_to_newline(&mut stream, &mut buffer) {
                Ok(true) => break, // Connection closed
                Ok(false) => {
                    if let Ok(received_str) = std::str::from_utf8(buffer.as_slice()) {
                        log::info!("Received: {:?}", received_str);
                        
                        if self.command_sender.send(received_str.into()).is_err() {
                            log::error!("Failed to send command to audio thread");
                            break;
                        }
                    } else {
                        log::error!("Received invalid UTF-8 string");
                    }
                },
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) => {
                    log::error!("Failed to read from socket: {}", e);
                    break;
                }
            }

            // Send any commands that have been received
            while let Ok(command) = self.command_receiver.try_recv() {
                if let Err(e) = stream.write_all(command.as_bytes()) {
                    log::error!("Failed to send command to client: {}", e);
                    break;
                }
                log::info!("Sent command: {:?}", command);
            }
        }

        stream.set_nonblocking(false).expect("Failed to restore blocking");
    }
}
