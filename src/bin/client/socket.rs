use std::collections::HashMap;
use std::time::Duration;

use futures::{pin_mut, select, FutureExt};
use ringbuf::traits::{Consumer, Split};
use smol::channel::{Receiver, Sender, TryRecvError};
use smol::io::{AsyncWriteExt, AsyncWrite};
use smol::net::{TcpStream, Ipv4Addr};

use rs_pedalboard::pedals::PedalParameterValue;
use rs_pedalboard::socket_helper::CommandReceiver;

pub const RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);

/// Manages a handle to a client socket thread, when connected.
pub struct ClientSocket {
    port: u16,
    socket_thread_responses: Vec<SocketThreadResponse>,
    pub received_commands: Vec<String>,
    pub parameter_updates: HashMap<(usize, usize, String), PedalParameterValue>,
    handle: Option<ClientSocketThreadHandle>,
}

impl ClientSocket {
    pub fn new(port: u16) -> Self {
        ClientSocket {
            port,
            handle: None,
            received_commands: Vec::new(),
            socket_thread_responses: Vec::new(),
            parameter_updates: HashMap::new(),
        }
    }

    pub fn connect(&mut self) -> std::io::Result<()> {
        if self.is_connected() {
            log::info!("Already connected to server on port {}", self.port);
            return Ok(());
        }

        match new_client_socket_thread(self.port) {
            Ok(handle) => {
                self.handle = Some(handle);
                Ok(())
            }
            Err(e) => {
                Err(e)
            }
        }
    }

    /// Check if socket is connected to the server.
    /// Note that this does not actually check the connection status, it only checks if the handle is present.
    /// If the connection is closed, this will not update until a method such as `is_connected_test`, `send`, `update_socket_responses` is called.
    pub fn is_connected(&self) -> bool {
        self.handle.is_some()
    }

    /// Check if socket is connected to the server.
    /// This will actually check the connection status by sending a dummy message over the socket thread channel.
    /// Often `is_connected` is sufficient if other methods that use channels are called frequently enough.
    #[allow(dead_code)]
    pub fn is_connected_test(&mut self) -> bool {
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
        }
    }

    pub fn send_parameter_update(&mut self, pedalboard_index: usize, pedal_index: usize, parameter_name: String, value: PedalParameterValue) {
        if let Some(handle) = &self.handle {
            if handle.send_parameter_update(pedalboard_index, pedal_index, parameter_name, value) {
                self.handle = None;
            }
        }
    }

    pub fn update_socket_responses(&mut self) {
        if let Some(handle) = &self.handle {
            if handle.all_socket_responses(&mut self.socket_thread_responses) {
                self.handle = None;
            } else {
                for response in self.socket_thread_responses.drain(..) {
                    match response {
                        SocketThreadResponse::Command(command) => {
                            self.received_commands.push(command);
                        },
                        SocketThreadResponse::ParameterUpdate(pedalboard_index, pedal_index, parameter_name, value) => {
                            self.parameter_updates.insert((pedalboard_index, pedal_index, parameter_name), value);
                        },
                    }
                }
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
            std::net::TcpStream::connect((Ipv4Addr::LOCALHOST, self.port)).is_ok()
        }
    }
}

#[derive(Debug, Clone)]
pub enum SocketThreadMessage {
    Send(String),
    // pedalboard index, pedal index, parameter name, value
    ParameterUpdate(usize, usize, String, PedalParameterValue),
    ThreadAliveTest,
    KillServer,
    SubscribeToResponses(Sender<SocketThreadResponse>)
}

#[derive(Clone, Debug)]
pub enum SocketThreadResponse {
    Command(String),
    ParameterUpdate(usize, usize, String, PedalParameterValue),
}

pub struct ClientSocketThreadHandle {
    message_sender: Sender<SocketThreadMessage>,
    response_receiver: Receiver<SocketThreadResponse>
}

impl ClientSocketThreadHandle {
    pub fn new(message_sender: Sender<SocketThreadMessage>, response_receiver: Receiver<SocketThreadResponse>) -> Self {
        ClientSocketThreadHandle {
            message_sender,
            response_receiver
        }
    }

    /// Returns true if closed
    pub fn send(&self, message: String) -> bool {
        match smol::block_on(
            self.message_sender.send(SocketThreadMessage::Send(message))
        ) {
            Ok(_) => false,
            Err(_) => {
                log::error!("Failed to send message to socket thread");
                true
            }
        }
    }

    /// Sends a parameter update to the server.
    /// 
    /// Returns true if closed.
    pub fn send_parameter_update(&self, pedalboard_index: usize, pedal_index: usize, parameter_name: String, value: PedalParameterValue) -> bool {
        match smol::block_on(
            self.message_sender.send(SocketThreadMessage::ParameterUpdate(pedalboard_index, pedal_index, parameter_name, value))
        ) {
            Ok(_) => false,
            Err(_) => {
                log::error!("Failed to send message to socket thread");
                true
            }
        }
    }

    pub fn kill(&self) {
        if smol::block_on(self.message_sender.send(SocketThreadMessage::KillServer)).is_err() {
            log::error!("Failed to send kill command");
        }
    }

    /// Returns true if closed
    pub fn all_socket_responses(&self, into: &mut Vec<SocketThreadResponse>) -> bool {
        loop {
            match self.response_receiver.try_recv() {
                Ok(command) => {
                    into.push(command);
                },
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Closed) => return true,
            }
        }
        false
    }

    pub fn is_connected(&self) -> bool {
        match smol::block_on(self.message_sender.send(SocketThreadMessage::ThreadAliveTest)) {
            Ok(_) => true,
            Err(_) => false
        }
    }
}

impl Clone for ClientSocketThreadHandle {
    fn clone(&self) -> Self {
        let (new_sender, new_receiver) = smol::channel::unbounded();
        let _ = smol::block_on(self.message_sender.send(SocketThreadMessage::SubscribeToResponses(new_sender)));
        ClientSocketThreadHandle {
            message_sender: self.message_sender.clone(),
            response_receiver: new_receiver
        }
    }
}

pub fn new_client_socket_thread(port: u16) -> std::io::Result<ClientSocketThreadHandle> {
    let (message_sender, message_receiver) = smol::channel::unbounded();
    let (response_sender, response_receiver) = smol::channel::unbounded();

    let (
        connected_status_oneshot_sender,
        connected_status_oneshot_receiver
    ) = crossbeam::channel::bounded(0);

    std::thread::spawn(move || {
        smol::block_on(async {
            log::info!("Attempting to connect to server on port {}", port);
            let stream = match TcpStream::connect((Ipv4Addr::LOCALHOST, port)).await {
                Ok(s) => s,
                Err(e) => {
                    let _ = connected_status_oneshot_sender.send(Err(e));
                    return;
                }
            };

            log::info!("Connected to server on port {}", port);
            match connected_status_oneshot_sender.send(Ok(())) {
                Ok(_) => client_socket_event_loop(stream, message_receiver, vec![response_sender]).await,
                Err(e) => {
                    log::error!("Failed to send connection status: {}", e);
                    return;
                }
            }
        });
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

async fn send_to_all<T: Clone>(
    senders: &[Sender<T>],
    message: T,
) -> bool {
    for sender in senders {
        if sender.send(message.clone()).await.is_err() {
            log::error!("Failed to send message to one of the response channels");
            return true;
        }
    }
    false
}

async fn client_socket_event_loop(
    stream: TcpStream,
    message_receiver: Receiver<SocketThreadMessage>,
    mut response_senders: Vec<Sender<SocketThreadResponse>>,
) {
    let mut command_receiver = CommandReceiver::new();
    // 128 is large but it is only storing String, which is small
    let (mut received_commands_writer, mut received_commands_reader) = ringbuf::HeapRb::new(128).split();
    
    let (mut stream_reader, mut stream_writer) = smol::io::split(stream);

    loop {
        let socket_fut = command_receiver.receive_commands_async(&mut stream_reader, &mut received_commands_writer).fuse();
        let msg_fut = message_receiver.recv().fuse();

        pin_mut!(socket_fut, msg_fut);

        select! {
            closed = socket_fut => {
                match closed {
                    Ok(true) => {
                        log::info!("Connection closed");
                        break;
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::ConnectionAborted
                            || e.kind() == std::io::ErrorKind::ConnectionReset => {
                        log::info!("Connection closed");
                        break;
                    }
                    Err(e) => {
                        log::error!("Error receiving commands: {}", e);
                        break;
                    }
                    Ok(false) => {
                        for command in received_commands_reader.pop_iter() {
                            if send_to_all(&response_senders, SocketThreadResponse::Command(command)).await {
                                log::error!("Failed to send command to response channel");
                                break;
                            }
                        }
                    }
                }
            },

            msg = msg_fut => {
                match msg {
                    Ok(SocketThreadMessage::KillServer) => {
                        log::info!("Received kill command from channel. Closing connection.");
                        socket_send(&mut stream_writer, "kill\n").await;
                        let _ = stream_writer.flush().await;
                        break;
                    },
                    Ok(SocketThreadMessage::Send(message)) => {
                        if socket_send(&mut stream_writer, &message).await {
                            break;
                        }
                    },
                    Ok(SocketThreadMessage::ParameterUpdate(pedalboard_index, pedal_index, parameter_name, value)) => {
                        let message = format!(
                            "setparameter {} {} {} {}\n",
                            pedalboard_index,
                            pedal_index,
                            parameter_name,
                            serde_json::to_string(&value).expect("Failed to serialize parameter value")
                        );

                        if send_to_all(&response_senders, SocketThreadResponse::ParameterUpdate(pedalboard_index, pedal_index, parameter_name.clone(), value.clone())).await {
                            break;
                        }
                        if socket_send(&mut stream_writer, &message).await {
                            break;
                        }
                    },
                    Ok(SocketThreadMessage::SubscribeToResponses(sender)) => {
                        response_senders.push(sender);
                    },
                    Ok(SocketThreadMessage::ThreadAliveTest) => { },
                    Err(_) => {
                        log::info!("Channel closed. Exiting event loop.");
                        break;
                    }
                }
            }
        }
    }
}

/// Returns true if closed
async fn socket_send(mut stream: impl AsyncWrite + Unpin, message: &str) -> bool {
    match stream.write_all(message.as_bytes()).await {
        Ok(()) => {
            if message.len() < 40 || cfg!(feature="log_full_commands") {
                log::info!("Sent: {:?}", message);
            } else {
                log::info!("Sent: {:?}...", &message[..40]);
            }
            false
        }
        Err(e) => {
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
            true
        }
    }
}
