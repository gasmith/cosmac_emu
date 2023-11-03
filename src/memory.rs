use rand::distributions::Standard;
use rand::{thread_rng, Rng};

#[derive(Default)]
pub struct Memory {
    size: u16,
    data: Vec<u8>,
}

impl Memory {
    pub fn new(size: u16) -> Self {
        let rng = thread_rng();
        Self {
            size,
            data: rng.sample_iter(Standard).take(32 * 1024).collect(),
        }
    }

    pub fn write_image(&mut self, image: Vec<u8>) {
        self.data
            .splice(0..image.len(), image.into_iter())
            .for_each(drop);
    }

    pub fn store(&mut self, addr: u16, byte: u8) {
        if addr < self.size {
            self.data[addr as usize] = byte;
        }
    }

    pub fn load(&self, addr: u16) -> u8 {
        if addr < self.size {
            self.data[addr as usize]
        } else {
            0xff
        }
    }

    pub fn as_slice(&self, addr: u16, len: u16) -> &[u8] {
        let start = std::cmp::min(self.size, addr) as usize;
        let end = std::cmp::min(self.size, addr + len) as usize;
        &self.data[start..end]
    }

    pub fn maybe_load(&self, addr: u16) -> Option<u8> {
        if addr < self.size {
            Some(self.data[addr as usize])
        } else {
            None
        }
    }
}
