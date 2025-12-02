use std::collections::VecDeque;

pub struct Buffer {
    data: VecDeque<u8>,
}

impl Buffer {
    pub fn new() -> Self {
        Self {
            data: VecDeque::new(),
        }
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: VecDeque::with_capacity(capacity),
        }
    }

    pub fn push(&mut self, byte: u8) {
        self.data.push_back(byte);
    }

    pub fn extend(&mut self, bytes: &[u8]) {
        self.data.extend(bytes.iter().copied());
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    pub fn clear(&mut self) {
        self.data.clear();
    }

    pub fn drain(&mut self, n: usize) -> Vec<u8> {
        self.data.drain(..n.min(self.data.len())).collect()
    }

    pub fn as_slice(&self) -> Vec<u8> {
        self.data.iter().copied().collect()
    }

    pub fn find(&self, pattern: &[u8]) -> Option<usize> {
        if pattern.is_empty() || pattern.len() > self.data.len() {
            return None;
        }

        for i in 0..=self.data.len().saturating_sub(pattern.len()) {
            if self.data.range(i..i + pattern.len()).eq(pattern.iter()) {
                return Some(i);
            }
        }
        None
    }
}

impl Default for Buffer {
    fn default() -> Self {
        Self::new()
    }
}

