use std::io::Write;
use std::net::TcpStream;
use std::net::Ipv4Addr;
use std::time::Duration;

use crossbeam::channel::TryRecvError;
use crossbeam::channel::{Sender, Receiver};
use rs_pedalboard::socket_helper::CommandReceiver;

pub const RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);

/// Manages a handle to a client socket thread, when connected.
pub struct ClientSocket {
    port: u16,
    pub received_commands: Vec<String>,
    handle: Option<ClientSocketThreadHandle>,
}

impl ClientSocket {
    pub fn new(port: u16) -> Self {
        ClientSocket {
            port,
            handle: None,
            received_commands: Vec::new(),
        }
    }

    pub fn connect(&mut self) -> std::io::Result<()> {
        if self.is_connected() {
            log::info!("Already connected to server on port {}", self.port);
            return Ok(());
        }

        match ClientSocketThread::new_thread(self.port) {
            Ok(handle) => {
                self.handle = Some(handle);
                Ok(())
            }
            Err(e) => {
                Err(e)
            }
        }
    }

    pub fn is_connected(&mut self) -> bool {
        if let Some(handle) = &self.handle {
            if handle.is_connected() {
                true
            } else {
                self.handle = None;
                false
            }
        } else {
            false
        }
    }

    pub fn send(&mut self, message: String) {
        if let Some(handle) = &self.handle {
            if handle.send(message) {
                self.handle = None;
            }
        } else {
            log::error!("Cannot send message, not connected to server");
        }
    }

    pub fn update_recv(&mut self) {
        if let Some(handle) = &self.handle {
            if handle.all_server_messages(&mut self.received_commands) {
                self.handle = None;
            }
        }
    }

    pub fn kill(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.kill();
            self.received_commands.clear();
        }
    }

    pub fn is_server_available(&mut self) -> bool {
        if self.is_connected() {
            true
        } else {
            TcpStream::connect((Ipv4Addr::LOCALHOST, self.port)).is_ok()
        }
    }
}

#[derive(Debug, Clone)]
pub enum SocketThreadMessage {
    Send(String),
    ThreadAliveTest,
    KillServer
}

/// Handle doesn't ID any messages. This means if there are multiple handles waiting on a response at the same time, they could get mixed up.
/// This isn't a problem in this case as only the UI thread requires responses.
#[derive(Clone)]
pub struct ClientSocketThreadHandle {
    message_sender: Sender<SocketThreadMessage>,
    response_receiver: Receiver<String>
}

impl ClientSocketThreadHandle {
    pub fn new(message_sender: Sender<SocketThreadMessage>, response_receiver: Receiver<String>) -> Self {
        ClientSocketThreadHandle {
            message_sender,
            response_receiver
        }
    }

    /// Returns true if closed
    pub fn send(&self, message: String) -> bool {
        log::debug!("Sending message to socket thread: {:?}", message);
        match self.message_sender.send(SocketThreadMessage::Send(message)) {
            Ok(_) => false,
            Err(_) => {
                log::error!("Failed to send message to socket thread");
                true
            }
        }
    }

    pub fn kill(&self) {
        if self.message_sender.send(SocketThreadMessage::KillServer).is_err() {
            log::error!("Failed to send kill command");
        }
    }

    /// Returns true if closed
    pub fn all_server_messages(&self, into: &mut Vec<String>) -> bool {
        loop {
            match self.response_receiver.try_recv() {
                Ok(command) => {
                    into.push(command);
                },
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => return true,
            }
        }
        false
    }

    pub fn is_connected(&self) -> bool {
        match self.message_sender.send(SocketThreadMessage::ThreadAliveTest) {
            Ok(_) => true,
            Err(_) => false
        }
    }
}

pub struct ClientSocketThread {
    stream: TcpStream,
    command_receiver: CommandReceiver,
    // Commands that have been received but not yet sent to receivers
    pub received_commands: Vec<String>,
}

impl ClientSocketThread {
    pub fn new_thread(port: u16) -> std::io::Result<ClientSocketThreadHandle> {
        let (message_sender, message_receiver) = crossbeam::channel::unbounded();
        let (response_sender, response_receiver) = crossbeam::channel::unbounded();

        let (
            connected_status_oneshot_sender,
            connected_status_oneshot_receiver
        ) = crossbeam::channel::bounded(0);

        std::thread::spawn(move || {
            log::info!("Attempting to connect to server on port {}", port);
            let stream = match TcpStream::connect((Ipv4Addr::LOCALHOST, port)) {
                Ok(s) => s,
                Err(e) => {
                    let _ = connected_status_oneshot_sender.send(Err(e));
                    return;
                }
            };
            if let Err(e) = stream.set_nonblocking(true) {
                let _ = connected_status_oneshot_sender.send(Err(e));
                return;
            }
            log::info!("Connected to server on port {}", port);
            let _ = connected_status_oneshot_sender.send(Ok(()));

            let mut client_socket = ClientSocketThread {
                stream,
                command_receiver: CommandReceiver::new(),
                received_commands: Vec::new(),
            };

            'main: loop {
                'message_receiver: loop {
                    match message_receiver.try_recv() {
                        Ok(SocketThreadMessage::Send(message)) => {
                            match client_socket.stream.write_all(message.as_bytes()) {
                                Ok(()) => {
                                    if message.len() < 40 || cfg!(feature="log_full_commands") {
                                        log::info!("Sent: {:?}", message);
                                    } else {
                                        log::info!("Sent: {:?}...", &message[..40]);
                                    }
                                }
                                Err(e) =>  {
                                    match e.kind() {
                                        std::io::ErrorKind::BrokenPipe |
                                        std::io::ErrorKind::NotConnected |
                                        std::io::ErrorKind::ConnectionReset |
                                        std::io::ErrorKind::ConnectionAborted => {
                                            log::info!("Connection closed");
                                        },
                                        _ => {
                                            log::error!("Failed to send message. Closing connection. Error: {}", e);
                                        }
                                    };
                                    break 'main;
                                }
                            }
                        },
                        Ok(SocketThreadMessage::KillServer) => {
                            log::info!("Sending kill command to server.");
                            if let Err(e) = client_socket.stream.write_all(b"kill\n") {
                                log::error!("Failed to send kill command: {}", e);
                            }
                            let _ = client_socket.stream.flush();
                            let _ = client_socket.stream.shutdown(std::net::Shutdown::Both);
                            break 'main;
                        },
                        Ok(SocketThreadMessage::ThreadAliveTest) => { }, // Ignore thread alive tests. Only used to check if channel is connected.
                        Err(TryRecvError::Disconnected) => break 'main,
                        Err(TryRecvError::Empty) => { break 'message_receiver }
                    }
                };

                match client_socket.command_receiver.receive_commands(&mut client_socket.stream, &mut client_socket.received_commands) {
                    Ok(closed) if closed => {
                        log::info!("Connection closed");
                        break;
                    },
                    Err(e) if e.kind() == std::io::ErrorKind::ConnectionAborted || e.kind() == std::io::ErrorKind::ConnectionReset => {
                        log::info!("Connection closed");
                        break;
                    },
                    Err(e) => {
                        log::error!("Error receiving commands: {}", e);
                        break;
                    },
                    _ => {}
                }

                for command in client_socket.received_commands.drain(..) {
                    if response_sender.send(command).is_err() {
                        log::error!("Failed to send command to response channel");
                    }
                }

                std::thread::sleep(Duration::from_millis(10));
            }
        });

        match connected_status_oneshot_receiver.recv_timeout(RESPONSE_TIMEOUT) {
            Ok(Ok(_)) => {
                Ok(ClientSocketThreadHandle::new(message_sender, response_receiver))
            },
            Ok(Err(e)) => {
                Err(e)
            }
            Err(e) => {
                Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    format!("Failed to connect to server within timeout: {}", e)
                ))
            }
        }
    }
}
