#[derive(Debug, Clone, Copy)]
pub enum EventType {
    Read,
    Write,
    ReadWrite,
}

#[derive(Debug)]
pub struct Event {
    pub fd: i32,
    pub event_type: EventType,
    pub user_data: usize,
}

impl Event {
    pub fn new(fd: i32, event_type: EventType, user_data: usize) -> Self {
        Self {
            fd,
            event_type,
            user_data,
        }
    }
}

