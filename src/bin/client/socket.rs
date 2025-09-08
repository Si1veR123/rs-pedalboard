use std::time::Duration;

use futures::{pin_mut, select, FutureExt};
use ringbuf::traits::{Consumer, Split};
use smol::channel::{Receiver, Sender, TryRecvError};
use smol::io::{AsyncWriteExt, AsyncWrite};
use smol::net::{TcpStream, Ipv4Addr};

use rs_pedalboard::pedals::PedalParameterValue;
use rs_pedalboard::socket_helper::CommandReceiver;

use crate::settings::VolumeNormalizationMode;

pub const RESPONSE_TIMEOUT: Duration = Duration::from_secs(5);

/// Can uniquely identify a parameter.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ParameterPath {
    pub pedalboard_id: u32,
    pub pedal_id: u32,
    pub parameter_name: String,
}

/// Manages a handle to a client socket thread, when connected.
pub struct ClientSocket {
    port: u16,
    socket_thread_responses: Vec<String>,
    pub received_server_commands: Vec<String>,
    pub handle: Option<ClientSocketThreadHandle>
}

impl ClientSocket {
    pub fn new(port: u16) -> Self {
        ClientSocket {
            port,
            handle: None,
            received_server_commands: Vec::new(),
            socket_thread_responses: Vec::new()
        }
    }

    pub fn connect(&mut self) -> std::io::Result<()> {
        if self.is_connected() {
            log::info!("Already connected to server on port {}", self.port);
            return Ok(());
        }

        match new_client_socket_thread(self.port, true) {
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

    pub fn send(&mut self, command: Command) {
        if let Some(handle) = &self.handle {
            if handle.send_command(command) {
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
                    self.received_server_commands.push(response);
                }
            }
        }
    }

    pub fn kill(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.kill();
            self.received_server_commands.clear();
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
pub enum Command {
    ParameterUpdate(ParameterPath, PedalParameterValue),
    // pedalboard id, pedal id, new pedal index
    MovePedal(u32, u32, usize),
    // pedalboard id, pedal id
    DeletePedal(u32, u32),
    // from index, to index
    MovePedalboard(usize, usize),
    // pedalboard index
    DeletePedalboard(usize),
    AddPedalboard(String),
    // pedalboard id, serialized pedal
    AddPedal(u32, String),
    ThreadAliveTest,
    KillServer,
    SubscribeToResponses(Sender<String>),
    MasterIn(f32),
    MasterOut(f32),
    VolumeNormalization(VolumeNormalizationMode, Option<f32>),
    VolumeNormalizationReset,
    SetRecording(bool),
    ToggleRecording,
    RecordClean(bool),
    ToggleClean,
    VolumeMonitor(bool),
    ToggleVolumeMonitor,
    Metronome(bool, u32, f32),
    ToggleMetronome,
    Tuner(bool),
    ToggleTuner,
    // Serialized PedalboardSet
    LoadSet(String),
    Play(usize),
    NextPedalboard,
    PrevPedalboard,
    RequestSampleRate,
    SetMute(bool),
    ToggleMute,
}

pub struct ClientSocketThreadHandle {
    message_sender: Sender<Command>,
    response_receiver: Receiver<String>
}

impl ClientSocketThreadHandle {
    pub fn new(message_sender: Sender<Command>, response_receiver: Receiver<String>) -> Self {
        ClientSocketThreadHandle {
            message_sender,
            response_receiver
        }
    }

    pub fn send_command(&self, command: Command) -> bool {
        match smol::block_on(
            self.message_sender.send(command)
        ) {
            Ok(_) => false,
            Err(_) => {
                log::error!("Failed to send command to socket thread");
                true
            }
        }
    }

    pub fn kill(&self) {
        if smol::block_on(self.message_sender.send(Command::KillServer)).is_err() {
            log::error!("Failed to send kill command");
        }
    }

    /// Returns true if closed
    pub fn all_socket_responses(&self, into: &mut Vec<String>) -> bool {
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
        match smol::block_on(self.message_sender.send(Command::ThreadAliveTest)) {
            Ok(_) => true,
            Err(_) => false
        }
    }

    pub fn clone_with_responses(&self) -> Self {
        let (new_sender, new_receiver) = smol::channel::unbounded();
        let _ = smol::block_on(self.message_sender.send(Command::SubscribeToResponses(new_sender)));
        ClientSocketThreadHandle {
            message_sender: self.message_sender.clone(),
            response_receiver: new_receiver
        }
    }

    pub fn clone_without_responses(&self) -> Self {
        let (_new_sender, new_receiver) = smol::channel::bounded(1);
        ClientSocketThreadHandle {
            message_sender: self.message_sender.clone(),
            response_receiver: new_receiver
        }
    }
}

impl Clone for ClientSocketThreadHandle {
    fn clone(&self) -> Self {
        self.clone_without_responses()
    }
}

pub fn new_client_socket_thread(port: u16, subscribe_to_responses: bool) -> std::io::Result<ClientSocketThreadHandle> {
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
            let response_senders = if subscribe_to_responses {
                vec![response_sender.clone()]
            } else {
                Vec::new()
            };
            match connected_status_oneshot_sender.send(Ok(())) {
                Ok(_) => client_socket_event_loop(stream, message_receiver, response_senders).await,
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
    message_receiver: Receiver<Command>,
    mut response_senders: Vec<Sender<String>>
) {
    let mut command_receiver = CommandReceiver::new();
    // 128 is large but it is only storing String, which is small
    let (mut received_commands_writer, mut received_commands_reader) = ringbuf::HeapRb::new(128).split();
    
    let (mut stream_reader, mut stream_writer) = smol::io::split(stream);

    loop {
        let socket_fut = command_receiver.receive_commands_async(&mut stream_reader, &mut received_commands_writer).fuse();
        let command_fut = message_receiver.recv().fuse();

        pin_mut!(socket_fut, command_fut);

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
                            if send_to_all(&response_senders, command).await {
                                log::error!("Failed to send command to response channel");
                                break;
                            }
                        }
                    }
                }
            },

            command = command_fut => {
                if command.is_err() {
                    log::info!("Channel closed. Exiting event loop.");
                    break;
                }

                match command.unwrap() {
                    Command::KillServer => {
                        log::info!("Received kill command from channel. Closing connection.");
                        socket_send(&mut stream_writer, "kill\n").await;
                        let _ = stream_writer.flush().await;
                        break;
                    },
                    Command::ParameterUpdate(parameter_path, value) => {
                        let message = format!(
                            "setparameter|{}|{}|{}|{}\n",
                            parameter_path.pedalboard_id,
                            parameter_path.pedal_id,
                            &parameter_path.parameter_name,
                            serde_json::to_string(&value).expect("Failed to serialize parameter value")
                        );

                        if socket_send(&mut stream_writer, &message).await {
                            break;
                        }
                    },
                    Command::SubscribeToResponses(sender) => {
                        response_senders.push(sender);
                    },
                    Command::MovePedal(pedalboard_id, pedal_id, to_index) => {
                        let message = format!(
                            "movepedal|{}|{}|{}\n",
                            pedalboard_id,
                            pedal_id,
                            to_index
                        );
                        if socket_send(&mut stream_writer, &message).await {
                            break;
                        }
                    },
                    Command::DeletePedal(pedalboard_id, pedal_id) => {
                        let message = format!(
                            "deletepedal|{}|{}\n",
                            pedalboard_id,
                            pedal_id
                        );
                        if socket_send(&mut stream_writer, &message).await {
                            break;
                        }
                    },
                    Command::MovePedalboard(from_index, to_index) => {
                        let message = format!(
                            "movepedalboard|{}|{}\n",
                            from_index,
                            to_index
                        );
                        if socket_send(&mut stream_writer, &message).await {
                            break;
                        }
                    },
                    Command::DeletePedalboard(pedalboard_index) => {
                        let message = format!(
                            "deletepedalboard|{}\n",
                            pedalboard_index
                        );
                        if socket_send(&mut stream_writer, &message).await {
                            break;
                        }
                    },
                    Command::AddPedal(pedalboard_id, serialized_pedal) => {
                        let message = format!(
                            "addpedal|{}|{}\n",
                            pedalboard_id,
                            serialized_pedal
                        );
                        if socket_send(&mut stream_writer, &message).await {
                            break;
                        }
                    },
                    Command::AddPedalboard(serialized_pedalboard) => {
                        let message = format!(
                            "addpedalboard|{}\n",
                            serialized_pedalboard
                        );
                        if socket_send(&mut stream_writer, &message).await {
                            break;
                        }
                    },
                    Command::LoadSet(pedalboard_set) => {
                        let message = format!(
                            "loadset|{}\n",
                            pedalboard_set
                        );
                        if socket_send(&mut stream_writer, &message).await {
                            break;
                        }
                    },
                    Command::MasterIn(value) => {
                        let message = format!("masterin|{}\n", value);
                        if socket_send(&mut stream_writer, &message).await {
                            break;
                        }
                    },
                    Command::MasterOut(value) => {
                        let message = format!("masterout|{}\n", value);
                        if socket_send(&mut stream_writer, &message).await {
                            break;
                        }
                    },
                    Command::VolumeNormalization(mode, auto_decay) => {
                        let message: String;
                        
                        if let Some(decay) = auto_decay {
                            message = format!("volumenormalization|{}|{}\n", match mode {
                                VolumeNormalizationMode::None => "none",
                                VolumeNormalizationMode::Manual => "manual",
                                VolumeNormalizationMode::Automatic => "automatic"
                            }, decay);
                        } else {
                            message = format!("volumenormalization|{}|none\n", match mode {
                                VolumeNormalizationMode::None => "none",
                                VolumeNormalizationMode::Manual => "manual",
                                VolumeNormalizationMode::Automatic => "automatic"
                            });
                        }

                        if socket_send(&mut stream_writer, &message).await {
                            break;
                        }
                    },
                    Command::VolumeNormalizationReset => {
                        if socket_send(&mut stream_writer, "volumenormalization|reset\n").await {
                            break;
                        }
                    }
                    Command::SetRecording(active) => {
                        let message = format!("recording|{}\n", if active { "on" } else { "off" });
                        if socket_send(&mut stream_writer, &message).await {
                            break;
                        }
                    },
                    Command::ToggleRecording => {
                        let message = "recording|toggle\n";
                        if socket_send(&mut stream_writer, message).await {
                            break;
                        }
                    },
                    Command::RecordClean(clean) => {
                        let message = format!("recordclean|{}\n", if clean { "on" } else { "off" });
                        if socket_send(&mut stream_writer, &message).await {
                            break;
                        }
                    },
                    Command::VolumeMonitor(enable) => {
                        let message = format!("volumemonitor|{}\n", if enable { "on" } else { "off" });
                        if socket_send(&mut stream_writer, &message).await {
                            break;
                        }
                    },
                    Command::ToggleClean => {
                        let message = "recordclean|toggle\n";
                        if socket_send(&mut stream_writer, message).await {
                            break;
                        }
                    },
                    Command::Metronome(enable, bpm, volume) => {
                        let message = format!("metronome|{}|{}|{}\n", if enable { "on" } else { "off" }, bpm, volume);
                        if socket_send(&mut stream_writer, &message).await {
                            break;
                        }
                    },
                    Command::ToggleMetronome => {
                        let message = "metronome|toggle\n";
                        if socket_send(&mut stream_writer, message).await {
                            break;
                        }
                    },
                    Command::ToggleVolumeMonitor => {
                        let message = "volumemonitor|toggle\n";
                        if socket_send(&mut stream_writer, message).await {
                            break;
                        }
                    },
                    Command::Tuner(enable) => {
                        let message = format!("tuner|{}\n", if enable { "on" } else { "off" });
                        if socket_send(&mut stream_writer, &message).await {
                            break;
                        }
                    },
                    Command::ToggleTuner => {
                        let message = "tuner|toggle\n";
                        if socket_send(&mut stream_writer, message).await {
                            break;
                        }
                    },
                    Command::Play(index) => {
                        let message = format!("play|{}\n", index);
                        if socket_send(&mut stream_writer, &message).await {
                            break;
                        }
                    },
                    Command::NextPedalboard => {
                        if socket_send(&mut stream_writer, "nextpedalboard\n").await {
                            break;
                        }
                    },
                    Command::PrevPedalboard => {
                        if socket_send(&mut stream_writer, "prevpedalboard\n").await {
                            break;
                        }
                    },
                    Command::RequestSampleRate => {
                        if socket_send(&mut stream_writer, "requestsr\n").await {
                            break;
                        }
                    },
                    Command::SetMute(mute) => {
                        let message = format!("mute|{}\n", if mute { "on" } else { "off" });
                        if socket_send(&mut stream_writer, &message).await {
                            break;
                        }
                    },
                    Command::ToggleMute => {
                        let message = "mute|toggle\n";
                        if socket_send(&mut stream_writer, message).await {
                            break;
                        }
                    },
                    Command::ThreadAliveTest => { },
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
