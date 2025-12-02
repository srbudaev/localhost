// Event manager for registering/deregistering file descriptors with the Poller
use crate::common::error::Result;
use crate::core::event::poller::Poller;
use std::os::unix::io::RawFd;
use std::rc::Rc;

pub struct EventManager {
    poller: Rc<Poller>,
}

impl EventManager {
    pub fn new(poller: &Rc<Poller>) -> Self {
        Self {
            poller: Rc::clone(poller),
        }
    }

    pub fn register_read(&self, fd: RawFd, user_data: usize) -> Result<()> {
        self.poller.register_read(fd, user_data)
    }

    pub fn register_write(&self, fd: RawFd, user_data: usize) -> Result<()> {
        self.poller.register_write(fd, user_data)
    }

    pub fn unregister_read(&self, fd: RawFd) -> Result<()> {
        self.poller.unregister_read(fd)
    }

    pub fn unregister_write(&self, fd: RawFd) -> Result<()> {
        self.poller.unregister_write(fd)
    }
}

