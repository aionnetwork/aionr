use std::ops::Deref;

/// Return data buffer. Holds memory from a previous call and a slice into that memory.
#[derive(Debug, PartialEq, Clone)]
pub struct ReturnData {
    mem: Vec<u8>,
    offset: usize,
    size: usize,
}

impl Deref for ReturnData {
    type Target = [u8];
    fn deref(&self) -> &[u8] { &self.mem[self.offset..self.offset + self.size] }
}

impl ReturnData {
    /// Create empty `ReturnData`.
    pub fn empty() -> Self {
        ReturnData {
            mem: Vec::new(),
            offset: 0,
            size: 0,
        }
    }
    /// Create `ReturnData` from give buffer and slice.
    pub fn new(mem: Vec<u8>, offset: usize, size: usize) -> Self {
        ReturnData {
            mem,
            offset,
            size,
        }
    }
}
