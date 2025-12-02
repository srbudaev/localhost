use crate::common::error::{Result, ServerError};
use crate::core::net::socket::ClientSocket;
use std::io::{Read, Write};

pub fn read_non_blocking(socket: &mut ClientSocket, buf: &mut [u8]) -> Result<usize> {
    match socket.as_stream_mut().read(buf) {
        Ok(n) => Ok(n),
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
        Err(e) => Err(ServerError::IoError(e)),
    }
}

pub fn write_non_blocking(socket: &mut ClientSocket, buf: &[u8]) -> Result<usize> {
    match socket.as_stream_mut().write(buf) {
        Ok(n) => Ok(n),
        Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(0),
        Err(e) => Err(ServerError::IoError(e)),
    }
}

