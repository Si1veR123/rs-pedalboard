use std::io::Read;
use std::net::{TcpListener, TcpStream};
use std::net::Ipv4Addr;

use crossbeam::channel::Sender;

pub struct TcpServer {
    port: u16,
    command_sender: Sender<Box<[u8]>>,
}

impl TcpServer {
    pub fn new(port: u16, command_sender: Sender<Box<[u8]>>) -> Self {
        TcpServer {
            port,
            command_sender: command_sender,
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
                    let received = buffer[..n].to_vec().into_boxed_slice();
                    log::info!("Received: {:?}", received);
                        
                    if self.command_sender.send(received).is_err() {
                        log::error!("Failed to send command to audio thread");
                        break;
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
