use crate::common::buffer::Buffer;
use crate::common::time::Timeout;
use crate::core::net::socket::ClientSocket;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Reading,
    Writing,
    Closed,
}

pub struct Connection {
    socket: ClientSocket,
    read_buffer: Buffer,
    write_buffer: Buffer,
    state: ConnectionState,
    timeout: Timeout,
    keep_alive: bool,
    /// Server port this connection came in on (for virtual host routing)
    server_port: Option<u16>,
}

impl Connection {
    pub fn new(socket: ClientSocket, timeout_secs: u64) -> Self {
        Self {
            socket,
            read_buffer: Buffer::new(),
            write_buffer: Buffer::new(),
            state: ConnectionState::Reading,
            timeout: Timeout::new(timeout_secs),
            keep_alive: false,
            server_port: None,
        }
    }

    pub fn with_port(socket: ClientSocket, timeout_secs: u64, server_port: u16) -> Self {
        Self {
            socket,
            read_buffer: Buffer::new(),
            write_buffer: Buffer::new(),
            state: ConnectionState::Reading,
            timeout: Timeout::new(timeout_secs),
            keep_alive: false,
            server_port: Some(server_port),
        }
    }

    pub fn server_port(&self) -> Option<u16> {
        self.server_port
    }

    pub fn set_server_port(&mut self, port: u16) {
        self.server_port = Some(port);
    }

    pub fn socket(&self) -> &ClientSocket {
        &self.socket
    }

    pub fn socket_mut(&mut self) -> &mut ClientSocket {
        &mut self.socket
    }

    pub fn read_buffer(&self) -> &Buffer {
        &self.read_buffer
    }

    pub fn read_buffer_mut(&mut self) -> &mut Buffer {
        &mut self.read_buffer
    }

    pub fn write_buffer(&self) -> &Buffer {
        &self.write_buffer
    }

    pub fn write_buffer_mut(&mut self) -> &mut Buffer {
        &mut self.write_buffer
    }

    pub fn state(&self) -> &ConnectionState {
        &self.state
    }

    pub fn set_state(&mut self, state: ConnectionState) {
        self.state = state;
    }

    pub fn is_timeout(&self) -> bool {
        self.timeout.is_expired()
    }

    pub fn set_keep_alive(&mut self, keep_alive: bool) {
        self.keep_alive = keep_alive;
    }

    pub fn should_keep_alive(&self) -> bool {
        self.keep_alive && !self.write_buffer.is_empty()
    }

    pub fn as_raw_fd(&self) -> i32 {
        self.socket.as_raw_fd()
    }
}

