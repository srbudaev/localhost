use crate::common::error::{Result, ServerError};
use libc::c_int;
use std::os::unix::io::RawFd;

// Platform-specific imports and types
#[cfg(target_os = "macos")]
use libc::{kevent, kqueue, kevent as KeventStruct, EV_ADD, EV_DELETE, EV_ENABLE, EVFILT_READ, EVFILT_WRITE};

#[cfg(target_os = "linux")]
use libc::{epoll_create1, epoll_ctl, epoll_wait, epoll_event, EPOLL_CTL_ADD, EPOLL_CTL_DEL, EPOLL_CTL_MOD, EPOLLIN, EPOLLOUT, EPOLL_CLOEXEC};

// Unified event structure
#[derive(Clone, Copy)]
pub struct Event {
    pub fd: RawFd,
    pub user_data: usize,
    pub is_read: bool,
    pub is_write: bool,
}

#[cfg(target_os = "macos")]
pub type Kevent = KeventStruct;

#[cfg(target_os = "linux")]
#[derive(Clone, Copy)]
pub struct Kevent {
    pub fd: RawFd,
    pub user_data: usize,
    pub is_read: bool,
    pub is_write: bool,
}

// Helper trait to get fd from event (works for both macOS and Linux)
#[cfg(target_os = "macos")]
impl Kevent {
    pub fn get_fd(&self) -> RawFd {
        self.ident as RawFd
    }
}

#[cfg(target_os = "linux")]
impl Kevent {
    pub fn get_fd(&self) -> RawFd {
        self.fd
    }
}

pub struct Poller {
    #[cfg(target_os = "macos")]
    kq: RawFd,
    
    #[cfg(target_os = "linux")]
    epfd: RawFd,
}

impl Poller {
    pub fn new() -> Result<Self> {
        unsafe {
            #[cfg(target_os = "macos")]
            {
                let kq = kqueue();
                if kq < 0 {
                    return Err(ServerError::NetworkError(
                        "Failed to create kqueue".to_string(),
                    ));
                }
                Ok(Self { kq })
            }
            
            #[cfg(target_os = "linux")]
            {
                let epfd = epoll_create1(EPOLL_CLOEXEC);
                if epfd < 0 {
                    return Err(ServerError::NetworkError(
                        "Failed to create epoll instance".to_string(),
                    ));
                }
                Ok(Self { epfd })
            }
            
            #[cfg(not(any(target_os = "macos", target_os = "linux")))]
            {
                Err(ServerError::NetworkError(
                    "Unsupported platform".to_string(),
                ))
            }
        }
    }

    pub fn register_read(&self, fd: RawFd, user_data: usize) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            self.register_event(fd, EVFILT_READ as c_int, user_data)
        }
        
        #[cfg(target_os = "linux")]
        {
            self.register_epoll_event(fd, EPOLLIN as u32, user_data)
        }
    }

    pub fn register_write(&self, fd: RawFd, user_data: usize) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            self.register_event(fd, EVFILT_WRITE as c_int, user_data)
        }
        
        #[cfg(target_os = "linux")]
        {
            self.register_epoll_event(fd, EPOLLOUT as u32, user_data)
        }
    }

    pub fn unregister_read(&self, fd: RawFd) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            self.unregister_event(fd, EVFILT_READ as c_int)
        }
        
        #[cfg(target_os = "linux")]
        {
            self.unregister_epoll_event(fd)
        }
    }

    pub fn unregister_write(&self, fd: RawFd) -> Result<()> {
        #[cfg(target_os = "macos")]
        {
            self.unregister_event(fd, EVFILT_WRITE as c_int)
        }
        
        #[cfg(target_os = "linux")]
        {
            self.unregister_epoll_event(fd)
        }
    }

    #[cfg(target_os = "macos")]
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

    #[cfg(target_os = "macos")]
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

            let _ = kevent(
                self.kq,
                &kev as *const KeventStruct,
                1,
                std::ptr::null_mut(),
                0,
                std::ptr::null(),
            );
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn register_epoll_event(&self, fd: RawFd, events: u32, user_data: usize) -> Result<()> {
        unsafe {
            // epoll_event uses a union
            // We store fd in the u64 field so we can retrieve it later
            // Combine fd and user_data: upper 32 bits = fd, lower 32 bits = user_data
            let mut ev: epoll_event = std::mem::zeroed();
            ev.events = events;
            // Store fd in upper 32 bits, user_data in lower 32 bits of u64
            ev.u64 = ((fd as u64) << 32) | (user_data as u64);

            if epoll_ctl(self.epfd, EPOLL_CTL_ADD, fd, &mut ev) < 0 {
                // If fd already exists, try to modify it
                ev.u64 = ((fd as u64) << 32) | (user_data as u64);
                if epoll_ctl(self.epfd, EPOLL_CTL_MOD, fd, &mut ev) < 0 {
                    return Err(ServerError::NetworkError(format!(
                        "Failed to register epoll event for fd {}",
                        fd
                    )));
                }
            }
        }
        Ok(())
    }

    #[cfg(target_os = "linux")]
    fn unregister_epoll_event(&self, fd: RawFd) -> Result<()> {
        unsafe {
            let _ = epoll_ctl(self.epfd, EPOLL_CTL_DEL, fd, std::ptr::null_mut());
        }
        Ok(())
    }

    pub fn wait(&self, events: &mut [Kevent], timeout_ms: i32) -> Result<usize> {
        #[cfg(target_os = "macos")]
        {
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
                    events.as_mut_ptr() as *mut KeventStruct,
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
        
        #[cfg(target_os = "linux")]
        {
            unsafe {
                let mut epoll_events = vec![
                    epoll_event { events: 0, u64: 0 };
                    events.len()
                ];

                let timeout = if timeout_ms >= 0 { timeout_ms } else { -1 };
                let n = epoll_wait(
                    self.epfd,
                    epoll_events.as_mut_ptr(),
                    events.len() as c_int,
                    timeout,
                );

                if n < 0 {
                    return Err(ServerError::NetworkError(
                        "Failed to wait for epoll events".to_string(),
                    ));
                }

                // Convert epoll events to Kevent format
                // Extract fd and user_data from u64 (fd in upper 32 bits, user_data in lower 32 bits)
                for i in 0..(n as usize) {
                    let ep_ev = &epoll_events[i];
                    let fd = (ep_ev.u64 >> 32) as RawFd;
                    let user_data = (ep_ev.u64 & 0xFFFFFFFF) as usize;
                    events[i] = Kevent {
                        fd,
                        user_data,
                        is_read: (ep_ev.events & (EPOLLIN as u32)) != 0,
                        is_write: (ep_ev.events & (EPOLLOUT as u32)) != 0,
                    };
                }

                Ok(n as usize)
            }
        }
    }

    pub fn as_raw_fd(&self) -> RawFd {
        #[cfg(target_os = "macos")]
        {
            self.kq
        }
        
        #[cfg(target_os = "linux")]
        {
            self.epfd
        }
    }
}

impl Drop for Poller {
    fn drop(&mut self) {
        unsafe {
            #[cfg(target_os = "macos")]
            {
                libc::close(self.kq);
            }
            
            #[cfg(target_os = "linux")]
            {
                libc::close(self.epfd);
            }
        }
    }
}
