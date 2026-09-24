use std::io::{self, Read};

use smol::io::{AsyncRead, AsyncReadExt};

/// Returns the start of `s`, cut to at most `max_bytes` bytes, for log messages that would
/// otherwise be too long.
///
/// Commands carry user entered text such as pedal names, so a cut has to land on a character
/// boundary rather than a byte one to avoid panicking on multi byte characters.
pub fn truncated_for_log(s: &str, max_bytes: usize) -> &str {
    if s.len() <= max_bytes {
        return s;
    }

    let mut end = max_bytes;
    while !s.is_char_boundary(end) {
        end -= 1;
    }

    &s[..end]
}

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
                    tracing::debug!("Received command: {:?}", line);
                } else {
                    tracing::debug!("Received command: {:?}...", truncated_for_log(&line, 40));
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
        P: ringbuf::producer::Producer<Item = String>,
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
                tracing::warn!(
                    "Failed to push command into ringbuf producer, it is full. Command: {:?}",
                    command
                );
                break;
            }
        }

        Ok(false)
    }

    pub fn reset(&mut self) {
        self.partial_buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::truncated_for_log;

    #[test]
    fn a_message_is_cut_to_the_limit_without_splitting_a_character() {
        // A message short enough to be logged whole is left alone, and a longer one is cut to the
        // limit it is given
        assert_eq!(truncated_for_log("setoutputeq|none", 40), "setoutputeq|none");

        let message = "setoutputeq|".to_string() + &"a".repeat(200);
        assert_eq!(truncated_for_log(&message, 40).len(), 40);

        // A cut that lands in the middle of a multi byte character, which a name in a command can
        // hold, is taken back to the character before it, so that every cut leaves a message that
        // can be logged
        let message = "setoutputeq|{\"name\":\"аааааааааааааааааааааа\"}";

        for max_bytes in 0..message.len() {
            let truncated = truncated_for_log(message, max_bytes);

            assert!(message.starts_with(truncated), "cut to {max_bytes} bytes");
            assert!(
                truncated.len() <= max_bytes,
                "cut to {max_bytes} bytes left {}",
                truncated.len()
            );
        }
    }
}
