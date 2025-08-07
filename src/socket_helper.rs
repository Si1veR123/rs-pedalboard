use std::io::{self, Read};

use smol::io::{AsyncRead, AsyncReadExt};

pub struct CommandReceiver {
    partial_buffer: Vec<u8>,
    temp_command_buffer: Vec<String>,
}

impl CommandReceiver {
    pub fn new() -> Self {
        Self {
            partial_buffer: Vec::new(),
            temp_command_buffer: Vec::new(),
        }
    }

    pub fn process_buffer_chunk(&mut self, chunk: &[u8]) {
        self.partial_buffer.extend_from_slice(chunk);

        while let Some(pos) = self.partial_buffer.iter().position(|&b| b == b'\n') {
            // Allocation is ok since it is converted to a String and moved into buffer
            let line_bytes = self.partial_buffer.drain(..=pos).collect::<Vec<u8>>();

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
                Err(_) => continue,
            };

            if !line.is_empty() {
                if line.len() < 40 || cfg!(feature = "log_full_commands") {
                    log::info!("Received command: {:?}", line);
                } else {
                    log::info!("Received command: {:?}...", &line[..40]);
                }

                self.temp_command_buffer.push(line);
            }
        }
    }

    /// Reads from the (non blocking) stream, collects complete newline-terminated commands into `into`.
    /// Returns Ok(true) if connection closed, Ok(false) otherwise.
    pub fn receive_commands(
        &mut self,
        stream: &mut impl Read,
        into: &mut Vec<String>,
    ) -> io::Result<bool> {
        let mut buf = [0u8; 1024];

        loop {
            match stream.read(&mut buf) {
                Ok(0) => return Ok(true), // Connection closed
                Ok(n) => self.process_buffer_chunk(&buf[..n]),
                Err(ref e) if e.kind() == io::ErrorKind::WouldBlock => break,
                Err(e) => return Err(e),
            }
        }

        into.extend(self.temp_command_buffer.drain(..));

        Ok(false)
    }

    /// Async version of `receive_commands`. Into is a ringbuf producer.
    pub async fn receive_commands_async<R, P>(
        &mut self,
        reader: &mut R,
        into: &mut P,
    ) -> io::Result<bool>
    where
        R: AsyncRead + Unpin,
        P: ringbuf::producer::Producer<Item = String>
    {
        let mut buf = [0u8; 1024];
        let n = match reader.read(&mut buf).await {
            Ok(0) => return Ok(true), // Connection closed
            Ok(n) => n,
            Err(e) => return Err(e),
        };
    
        self.process_buffer_chunk(&buf[..n]);
        
        for command in self.temp_command_buffer.drain(..) {
            if let Err(command) = into.try_push(command) {
                log::warn!("Failed to push command into ringbuf producer, it is full. Command: {:?}", command);
                break;
            }
        }

        Ok(false)
    }

    pub fn reset(&mut self) {
        self.partial_buffer.clear();
    }
}