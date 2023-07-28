#[derive(Default)]
pub struct Memory {
    size: u16,
    data: Vec<u8>,
}

impl Memory {
    pub fn new(size: u16) -> Self {
        Self {
            size,
            data: vec![0; size as usize],
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
}

