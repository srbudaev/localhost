use crate::common::error::{Result, ServerError};
use std::os::unix::io::{AsRawFd, RawFd};

pub struct FileDescriptor {
    fd: RawFd,
    owned: bool,
}

impl FileDescriptor {
    pub fn new(fd: RawFd) -> Self {
        Self { fd, owned: false }
    }

    pub fn from_raw(fd: RawFd) -> Self {
        Self { fd, owned: true }
    }

    pub fn as_raw_fd(&self) -> RawFd {
        self.fd
    }

    pub fn set_non_blocking(&self) -> Result<()> {
        unsafe {
            let flags = libc::fcntl(self.fd, libc::F_GETFL);
            if flags < 0 {
                return Err(ServerError::NetworkError(
                    "Failed to get socket flags".to_string(),
                ));
            }

            if libc::fcntl(self.fd, libc::F_SETFL, flags | libc::O_NONBLOCK) < 0 {
                return Err(ServerError::NetworkError(
                    "Failed to set non-blocking mode".to_string(),
                ));
            }
        }
        Ok(())
    }
}

impl AsRawFd for FileDescriptor {
    fn as_raw_fd(&self) -> RawFd {
        self.fd
    }
}

impl Drop for FileDescriptor {
    fn drop(&mut self) {
        if self.owned && self.fd >= 0 {
            unsafe {
                libc::close(self.fd);
            }
        }
    }
}

