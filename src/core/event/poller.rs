use crate::common::error::{Result, ServerError};
use libc::{c_int, c_void};
use std::os::unix::io::RawFd;

#[cfg(target_os = "macos")]
use libc::{kevent, kqueue, kevent as KeventStruct, EV_ADD, EV_DELETE, EV_ENABLE, EVFILT_READ, EVFILT_WRITE};

pub use libc::kevent as Kevent;

pub struct Poller {
    kq: RawFd,
}

impl Poller {
    pub fn new() -> Result<Self> {
        unsafe {
            let kq = kqueue();
            if kq < 0 {
                return Err(ServerError::NetworkError(
                    "Failed to create kqueue".to_string(),
                ));
            }

            Ok(Self { kq })
        }
    }

    pub fn register_read(&self, fd: RawFd, user_data: usize) -> Result<()> {
        self.register_event(fd, EVFILT_READ as c_int, user_data)
    }

    pub fn register_write(&self, fd: RawFd, user_data: usize) -> Result<()> {
        self.register_event(fd, EVFILT_WRITE as c_int, user_data)
    }

    pub fn unregister_read(&self, fd: RawFd) -> Result<()> {
        self.unregister_event(fd, EVFILT_READ as c_int)
    }

    pub fn unregister_write(&self, fd: RawFd) -> Result<()> {
        self.unregister_event(fd, EVFILT_WRITE as c_int)
    }

    fn register_event(&self, fd: RawFd, filter: c_int, user_data: usize) -> Result<()> {
        unsafe {
            let kev = KeventStruct {
                ident: fd as usize,
                filter: filter as i16,
                flags: (EV_ADD | EV_ENABLE) as u16,
                fflags: 0,
                data: 0,
                udata: user_data as *mut c_void,
            };

            if kevent(
                self.kq,
                &kev as *const KeventStruct,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            ) < 0
            {
                return Err(ServerError::NetworkError(format!(
                    "Failed to register event for fd {}",
                    fd
                )));
            }
        }
        Ok(())
    }

    fn unregister_event(&self, fd: RawFd, filter: c_int) -> Result<()> {
        unsafe {
            let kev = KeventStruct {
                ident: fd as usize,
                filter: filter as i16,
                flags: EV_DELETE as u16,
                fflags: 0,
                data: 0,
                udata: std::ptr::null_mut(),
            };

            if kevent(
                self.kq,
                &kev as *const KeventStruct,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            ) < 0
            {
                return Err(ServerError::NetworkError(format!(
                    "Failed to unregister event for fd {}",
                    fd
                )));
            }
        }
        Ok(())
    }

    pub fn wait(&self, events: &mut [KeventStruct], timeout_ms: i32) -> Result<usize> {
        unsafe {
            let timeout = if timeout_ms >= 0 {
                libc::timespec {
                    tv_sec: (timeout_ms / 1000) as i64,
                    tv_nsec: ((timeout_ms % 1000) * 1_000_000) as i64,
                }
            } else {
                libc::timespec {
                    tv_sec: 0,
                    tv_nsec: 0,
                }
            };

            let timeout_ptr = if timeout_ms >= 0 {
                &timeout as *const libc::timespec
            } else {
                std::ptr::null()
            };

            let n = kevent(
                self.kq,
                std::ptr::null(),
                0,
                events.as_mut_ptr(),
                events.len() as c_int,
                timeout_ptr,
            );

            if n < 0 {
                return Err(ServerError::NetworkError(
                    "Failed to wait for events".to_string(),
                ));
            }

            Ok(n as usize)
        }
    }

    pub fn as_raw_fd(&self) -> RawFd {
        self.kq
    }
}

impl Drop for Poller {
    fn drop(&mut self) {
        unsafe {
            libc::close(self.kq);
        }
    }
}
