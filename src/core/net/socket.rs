use crate::common::error::{Result, ServerError};
use crate::core::net::fd::FileDescriptor;
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::io::AsRawFd;

pub struct ListeningSocket {
    listener: TcpListener,
    fd: FileDescriptor,
}

impl ListeningSocket {
    pub fn bind(addr: SocketAddr) -> Result<Self> {
        let listener = TcpListener::bind(addr)
            .map_err(|e| ServerError::NetworkError(format!("Failed to bind to {}: {}", addr, e)))?;

        let fd = FileDescriptor::new(listener.as_raw_fd());
        fd.set_non_blocking()?;

        Ok(Self { listener, fd })
    }

    pub fn accept(&self) -> Result<Option<ClientSocket>> {
        match self.listener.accept() {
            Ok((stream, addr)) => {
                let socket = ClientSocket::from_stream(stream, addr)?;
                Ok(Some(socket))
            }
            Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(None),
            Err(e) => Err(ServerError::NetworkError(format!(
                "Failed to accept connection: {}",
                e
            ))),
        }
    }

    pub fn as_raw_fd(&self) -> i32 {
        self.fd.as_raw_fd()
    }
}

pub struct ClientSocket {
    stream: TcpStream,
    addr: SocketAddr,
    fd: FileDescriptor,
}

impl ClientSocket {
    pub fn from_stream(stream: TcpStream, addr: SocketAddr) -> Result<Self> {
        let fd = FileDescriptor::new(stream.as_raw_fd());
        fd.set_non_blocking()?;

        Ok(Self {
            stream,
            addr,
            fd,
        })
    }

    pub fn peer_addr(&self) -> SocketAddr {
        self.addr
    }

    pub fn as_raw_fd(&self) -> i32 {
        self.fd.as_raw_fd()
    }

    pub fn as_stream(&self) -> &TcpStream {
        &self.stream
    }

    pub fn as_stream_mut(&mut self) -> &mut TcpStream {
        &mut self.stream
    }
}

