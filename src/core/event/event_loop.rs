// Main event loop orchestrator
use crate::common::error::Result;
use crate::core::event::poller::{Poller, Kevent};
use std::rc::Rc;

pub struct EventLoop {
    poller: Rc<Poller>,
    events: Vec<Kevent>,
}

impl EventLoop {
    pub fn new() -> Result<Self> {
        let poller = Rc::new(Poller::new()?);
        Ok(Self {
            poller,
            events: {
                #[cfg(target_os = "macos")]
                {
                    vec![unsafe { std::mem::zeroed() }; 1024]
                }
                #[cfg(target_os = "linux")]
                {
                    vec![Kevent { fd: 0, user_data: 0, is_read: false, is_write: false }; 1024]
                }
            },
        })
    }

    pub fn poller(&self) -> &Rc<Poller> {
        &self.poller
    }

    pub fn wait(&mut self, timeout_ms: i32) -> Result<&[Kevent]> {
        let n = self.poller.wait(&mut self.events, timeout_ms)?;
        Ok(&self.events[..n])
    }
}

