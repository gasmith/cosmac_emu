use std::{fs::File, io::Read, ops::RangeInclusive};

use color_eyre::eyre;
use rand::prelude::*;

use crate::{
    cli::ImageArg,
    instr::{Instr, InstrSchema as _},
};

use super::Cdp1802Pins;

#[derive(Debug)]
pub enum MemoryAccessMode {
    Read,
    Write,
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct MemoryAccess {
    mode: MemoryAccessMode,
    addr: u16,
    data: u8,
}
impl MemoryAccess {
    fn new(mode: MemoryAccessMode, addr: u16, data: u8) -> Self {
        Self { mode, addr, data }
    }
    fn read(addr: u16, data: u8) -> Self {
        Self::new(MemoryAccessMode::Read, addr, data)
    }
    fn write(addr: u16, data: u8) -> Self {
        Self::new(MemoryAccessMode::Write, addr, data)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MemoryAccessError {
    #[error("write protection fault at {0:04x}")]
    WriteProtectionFault(u16),
}

#[derive(Debug, thiserror::Error)]
pub enum MemoryBuilderError {
    #[error("image at 0x{0:x} out of range")]
    ImageOutOfRange(u16),
}

#[derive(Debug, Default, Clone, Copy)]
pub struct MemoryRange {
    pub start: Option<u16>,
    pub end: Option<u16>,
}
impl MemoryRange {
    fn into_range_inclusive(self, max: u16) -> RangeInclusive<u16> {
        self.start.unwrap_or(0)..=self.end.unwrap_or(max)
    }
}

#[must_use]
pub struct Builder {
    capacity: usize,
    images: Vec<(u16, Vec<u8>)>,
    write_protect: Vec<MemoryRange>,
    random: bool,
}
impl Default for Builder {
    fn default() -> Self {
        Self {
            capacity: 64 * 1024,
            images: vec![],
            write_protect: vec![],
            random: false,
        }
    }
}
impl Builder {
    pub fn with_address_width(mut self, bits: u8) -> color_eyre::Result<Self> {
        if bits > 16 {
            eyre::bail!("maximum address width is 16 bits");
        }
        self.capacity = 2_usize.pow(bits.into());
        Ok(self)
    }

    pub fn with_capacity(self, size: usize) -> color_eyre::Result<Self> {
        if size > 64 * 1024 {
            eyre::bail!("memory size cannot exceed 64KiB");
        }
        if size & (size - 1) != 0 {
            eyre::bail!("memory size must be a power of two");
        }
        let bits = size.trailing_zeros() as u8;
        self.with_address_width(bits)
    }

    pub fn with_image(mut self, addr: u16, data: impl Into<Vec<u8>>) -> Self {
        self.images.push((addr, data.into()));
        self
    }

    pub fn with_image_args(mut self, images: &[ImageArg]) -> std::io::Result<Self> {
        for image in images {
            let mut file = File::open(&image.path)?;
            let mut buf = vec![];
            file.read_to_end(&mut buf)?;
            if image.write_protect && !buf.is_empty() {
                let start = Some(image.base_addr);
                let end = Some(image.base_addr.saturating_add((buf.len() as u16) - 1));
                let range = MemoryRange { start, end };
                self.write_protect.push(range);
            }
            self.images.push((image.base_addr, buf));
        }
        Ok(self)
    }

    pub fn with_random(self) -> Self {
        Self {
            random: true,
            ..self
        }
    }

    pub fn with_write_protect_range(mut self, range: MemoryRange) -> Self {
        self.write_protect.push(range);
        self
    }

    pub fn with_write_protect_ranges(mut self, ranges: &[MemoryRange]) -> Self {
        for range in ranges {
            self = self.with_write_protect_range(*range);
        }
        self
    }

    pub fn build(self) -> Result<Memory, MemoryBuilderError> {
        let mut data: Vec<u8> = if self.random {
            rand::rng().random_iter().take(self.capacity).collect()
        } else {
            vec![0; self.capacity]
        };
        for (addr, image) in self.images {
            let start = addr as usize;
            let end = start + image.len();
            if end > data.len() {
                return Err(MemoryBuilderError::ImageOutOfRange(addr));
            }
            data.splice(start..end, image);
        }
        let max_addr = (self.capacity - 1) as u16;
        let write_protect = self
            .write_protect
            .into_iter()
            .map(|mr| mr.into_range_inclusive(max_addr))
            .collect();
        Ok(Memory::new(data, write_protect))
    }
}

#[derive(Debug, Clone)]
pub struct Memory {
    data: Vec<u8>,
    write_protect: Vec<RangeInclusive<u16>>,
    addr_hi: u8,
}
impl Default for Memory {
    fn default() -> Self {
        Builder::default().build().unwrap()
    }
}
impl Memory {
    fn new(data: Vec<u8>, write_protect: Vec<RangeInclusive<u16>>) -> Self {
        Memory {
            data,
            write_protect,
            addr_hi: 0,
        }
    }

    pub fn builder() -> Builder {
        Builder::default()
    }

    pub fn tick(
        &mut self,
        pins: &mut Cdp1802Pins,
        write_enable: bool,
    ) -> Result<Option<MemoryAccess>, MemoryAccessError> {
        // TPA strobe to latch high address byte.
        if pins.get_tpa() {
            self.addr_hi = pins.get_ma();
        }

        if !pins.get_mrd() {
            // MRD low to read data from memory.
            let addr_lo = pins.get_ma();
            let addr = ((self.addr_hi as u16) << 8) | (addr_lo as u16);
            let data = self.data[addr as usize];
            pins.set_bus(data);
            Ok(Some(MemoryAccess::read(addr, data)))
        } else if write_enable && !pins.get_mwr() {
            // MWR low to write data to memory.
            let addr_lo = pins.get_ma();
            let addr = ((self.addr_hi as u16) << 8) | (addr_lo as u16);
            if !self.is_writable(addr) {
                return Err(MemoryAccessError::WriteProtectionFault(addr));
            }
            let data = pins.get_bus();
            self.data[addr as usize] = data;
            Ok(Some(MemoryAccess::write(addr, data)))
        } else {
            Ok(None)
        }
    }

    pub fn get_instr_at(&self, addr: u16) -> Option<Instr> {
        let start = addr as usize;
        let end = (start + 2).min(self.data.len() - 1);
        if start >= end {
            None
        } else {
            Instr::decode(&self.data[start..=end])
        }
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }

    pub fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub fn is_writable(&self, addr: u16) -> bool {
        !self.write_protect.iter().any(|r| r.contains(&addr))
    }
}
