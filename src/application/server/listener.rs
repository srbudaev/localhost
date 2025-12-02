use crate::common::error::Result;
use crate::core::net::socket::{ClientSocket, ListeningSocket};
use std::net::SocketAddr;

/// Listener manages a listening socket for accepting connections
pub struct Listener {
    socket: ListeningSocket,
    addr: SocketAddr,
}

impl Listener {
    /// Create a new listener bound to the given address
    pub fn new(addr: SocketAddr) -> Result<Self> {
        let socket = ListeningSocket::bind(addr)?;
        Ok(Self { socket, addr })
    }

    /// Accept a new client connection (non-blocking)
    pub fn accept(&self) -> Result<Option<ClientSocket>> {
        self.socket.accept()
    }

    /// Get the socket address this listener is bound to
    pub fn addr(&self) -> SocketAddr {
        self.addr
    }

    /// Get the file descriptor for event polling
    pub fn as_raw_fd(&self) -> i32 {
        self.socket.as_raw_fd()
    }
}
