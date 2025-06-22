use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::net::Ipv4Addr;

use crossbeam::channel::{Sender, Receiver};
use rs_pedalboard::socket_helper::CommandReceiver;

pub struct ServerSocket {
    port: u16,
    command_sender: Sender<Box<str>>,
    command_receiver: Receiver<Box<str>>,
    command_receive_helper: CommandReceiver
}

impl ServerSocket {
    pub fn new(port: u16, command_sender: Sender<Box<str>>, command_receiver: Receiver<Box<str>>) -> Self {
        ServerSocket {
            port,
            command_sender,
            command_receiver,
            command_receive_helper: CommandReceiver::new()
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
        let mut buffer = Vec::new();
        stream.set_nonblocking(true).expect("Failed to set non-blocking");

        loop {
            match self.command_receive_helper.receive_commands(&mut stream, &mut buffer) {
                Ok(true) => break, // Connection closed
                Ok(false) => {
                    for command in buffer.drain(..) {
                        if self.command_sender.send(command.into()).is_err() {
                            log::error!("Failed to send command to audio thread");
                            break;
                        }
                    }
                },
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {}
                Err(e) if e.kind() == std::io::ErrorKind::ConnectionAborted || e.kind() == std::io::ErrorKind::ConnectionReset => {
                    log::info!("Client closed connection");
                    break;
                },
                Err(e) => {
                    log::error!("Error receiving commands: {}", e);
                    break;
                }
            }

            // Send any commands that have been received
            while let Ok(command) = self.command_receiver.try_recv() {
                match stream.write_all(command.as_bytes()) {
                    Ok(_) => {},
                    Err(e) if e.kind() == std::io::ErrorKind::BrokenPipe ||
                              e.kind() == std::io::ErrorKind::ConnectionReset ||
                              e.kind() == std::io::ErrorKind::ConnectionAborted => {
                        log::info!("Client disconnected");
                        break;
                    },
                    Err(e) => {
                        log::error!("Failed to send command to client: {}", e);
                        break;
                    }
                }
                if command.len() <= 20 {
                    log::info!("Sent command: {:?}", command);
                } else {
                    log::info!("Sent command: {:?}...", &command[..20]);
                }
            }

            std::thread::sleep(std::time::Duration::from_millis(50)); // Avoid busy waiting
        }
        self.command_sender.send("disconnect".into()).expect("Failed to send disconnect command");
        stream.set_nonblocking(false).expect("Failed to restore blocking");
    }
}
