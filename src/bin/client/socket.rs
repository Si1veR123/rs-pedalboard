use std::io::Write;
use std::net::TcpStream;
use std::net::Ipv4Addr;

pub struct ClientSocket {
    port: u16,
    stream: Option<TcpStream>,
}

impl ClientSocket {
    pub fn new(port: u16) -> Self {
        ClientSocket {
            port,
            stream: None
        }
    }

    pub fn connect(&mut self) -> std::io::Result<()> {
        self.stream = Some(TcpStream::connect((Ipv4Addr::LOCALHOST, self.port))?);
        log::info!("Connected to server on port {}", self.port);
        Ok(())
    }

    pub fn send(&mut self, message: &str) -> std::io::Result<()> {
        if let Some(stream) = &mut self.stream {
            stream.write_all(message.as_bytes())?;
            log::info!("Sent: {:?}", message);
            Ok(())
        } else {
            log::error!("Not connected to server");
            Err(std::io::Error::new(std::io::ErrorKind::NotConnected, "Not connected to server"))
        }
    }
}
