// Main event loop orchestrator
use crate::common::error::Result;
use crate::core::event::poller::Poller;
use libc::kevent;

pub struct EventLoop {
    poller: Poller,
    events: Vec<kevent>,
}

impl EventLoop {
    pub fn new() -> Result<Self> {
        let poller = Poller::new()?;
        Ok(Self {
            poller,
            events: vec![unsafe { std::mem::zeroed() }; 1024],
        })
    }

    pub fn poller(&self) -> &Poller {
        &self.poller
    }

    pub fn wait(&mut self, timeout_ms: i32) -> Result<&[kevent]> {
        let n = self.poller.wait(&mut self.events, timeout_ms)?;
        Ok(&self.events[..n])
    }
}

