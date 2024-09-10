use anyhow::{anyhow, Result};

use rand::distributions::Standard;
use rand::{thread_rng, Rng};

use crate::args::Args;

#[derive(Default)]
pub struct Memory {
    size: usize,
    data: Vec<u8>,
}
impl<'a> TryFrom<&'a Args> for Memory {
    type Error = anyhow::Error;

    fn try_from(args: &'a Args) -> Result<Self, Self::Error> {
        let mut mem = Memory::new(args.memory_size);
        for image in &args.image {
            let bin = std::fs::read(&image.path)?;
            mem.write_image(bin, image.base_addr.into())?;
        }
        Ok(mem)
    }
}

impl Memory {
    #[must_use]
    pub fn new(size: usize) -> Self {
        let rng = thread_rng();
        Self {
            size,
            data: rng.sample_iter(Standard).take(size).collect(),
        }
    }

    pub fn write_image(&mut self, image: Vec<u8>, base_addr: usize) -> Result<()> {
        let end_addr = base_addr + image.len();
        if end_addr > self.size {
            Err(anyhow!("image extends beyond available memory"))
        } else {
            self.data.splice(base_addr..end_addr, image).for_each(drop);
            Ok(())
        }
    }

    pub fn store(&mut self, addr: u16, byte: u8) {
        let addr = addr as usize;
        if addr < self.size {
            self.data[addr] = byte;
        }
    }

    #[must_use]
    pub fn load(&self, addr: u16) -> u8 {
        self.maybe_load(addr).unwrap_or(0xff)
    }

    pub fn as_slice(&self, addr: u16, len: u16) -> &[u8] {
        let addr = addr as usize;
        let len = len as usize;
        let start = std::cmp::min(self.size, addr);
        let end = std::cmp::min(self.size, addr + len);
        &self.data[start..end]
    }

    pub fn maybe_load(&self, addr: u16) -> Option<u8> {
        let addr = addr as usize;
        if addr < self.size {
            Some(self.data[addr])
        } else {
            None
        }
    }
}
