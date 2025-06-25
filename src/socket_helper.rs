use std::io::{Read, Result};
use std::net::TcpStream;

pub struct CommandReceiver {
    partial_buffer: Vec<u8>,
}

impl CommandReceiver {
    pub fn new() -> Self {
        Self {
            partial_buffer: Vec::new(),
        }
    }

    /// Reads from the nonblocking stream, collects complete newline-terminated commands into `into`.
    /// Returns Ok(true) if connection closed, Ok(false) otherwise.
    pub fn receive_commands(
        &mut self,
        stream: &mut TcpStream,
        into: &mut Vec<String>,
    ) -> Result<bool> {
        let mut buf = [0u8; 1024];

        loop {
            match stream.read(&mut buf) {
                Ok(0) => {
                    // Connection closed
                    return Ok(true);
                }
                Ok(n) => {
                    self.partial_buffer.extend_from_slice(&buf[..n]);

                    // Extract commands separated by newline
                    while let Some(pos) = self.partial_buffer.iter().position(|&b| b == b'\n') {
                        // Split out one complete line (including newline)
                        let line_bytes = self.partial_buffer.drain(..=pos).collect::<Vec<u8>>();

                        // Convert to String (strip trailing newline)
                        let line = match String::from_utf8(line_bytes) {
                            Ok(mut s) => {
                                if s.ends_with('\n') {
                                    s.pop();
                                    if s.ends_with('\r') {
                                        s.pop();
                                    }
                                }
                                s
                            }
                            Err(_) => {
                                // Invalid UTF-8, skip this line
                                continue;
                            }
                        };
                        if !line.is_empty() {
                            if line.len() < 40 {
                                log::info!("Received command: {:?}", line);
                            } else {
                                log::info!("Received command: {:?}...", &line[..40]);
                            }
                            
                            into.push(line);
                        }
                    }
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No more data available now
                    break;
                }
                Err(e) => {
                    return Err(e);
                }
            }
        }

        Ok(false)
    }

    pub fn reset(&mut self) {
        self.partial_buffer.clear();
    }
}