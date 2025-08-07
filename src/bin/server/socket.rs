use ringbuf::traits::{Consumer, Split};
use smol::{io::AsyncWriteExt, net::{TcpListener, TcpStream, Ipv4Addr}, stream::StreamExt};
use futures::{FutureExt, select, pin_mut};
use smol::channel::{Sender, Receiver};
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
        smol::block_on(async {
            log::info!("Starting server on port {}", self.port);
            let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, self.port)).await?;
            log::info!("Server listening on port {}", self.port);

            while let Some(stream) = listener.incoming().next().await {
                match stream {
                    Ok(stream) => {
                        log::info!("New connection: {}", stream.peer_addr()?);
                        // Don't make a new thread as currently only one client is supported
                        self.handle_client(stream).await;
                        log::info!("Finished handling client.");
                    }
                    Err(e) => log::error!("Connection failed: {}", e),
                }
            }

            Ok(())
        })
    }

    async fn handle_client(&mut self, stream: TcpStream) {
        let (mut received_commands_writer, mut received_commands_reader) = ringbuf::HeapRb::new(128).split();
        let (mut stream_reader, mut stream_writer) = smol::io::split(stream);

        loop {
            let socket_fut = self.command_receive_helper.receive_commands_async(&mut stream_reader, &mut received_commands_writer).fuse();
            let channel_fut = self.command_receiver.recv().fuse();

            pin_mut!(socket_fut, channel_fut);

            select! {
                result = socket_fut => {
                    match result {
                        Ok(closed) => {
                            for command in received_commands_reader.pop_iter() {
                                if self.command_sender.send(command.into()).await.is_err() {
                                    log::error!("Failed to send command to audio thread");
                                    break;
                                }
                            }

                            if closed {
                                log::info!("Client closed connection");
                                break;
                            }
                        },
                        Err(e) if e.kind() == std::io::ErrorKind::ConnectionAborted || e.kind() == std::io::ErrorKind::ConnectionReset => {
                            log::info!("Client closed connection");
                            break;
                        },
                        Err(e) => {
                            log::error!("Error receiving commands: {}", e);
                            break;
                        }
                    }
                }
                result = channel_fut => {
                    match result {
                        Ok(command) => {
                            match stream_writer.write_all(command.as_bytes()).await {
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
                            if command.len() <= 20 || cfg!(feature="log_full_commands") {
                                log::info!("Sent command: {:?}", command);
                            } else {
                                log::info!("Sent command: {:?}...", &command[..20]);
                            }
                        },
                        Err(_) => {
                            log::error!("Audio thread channel has disconnected");
                            break;
                        }
                    }
                }
            }
        }
        self.command_sender.send("disconnect".into()).await.expect("Failed to send disconnect command");
    }
}
