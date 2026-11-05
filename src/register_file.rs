#[derive(Debug, Default)]
pub struct RegisterFile {
    registers: [u64; 32],
}

impl RegisterFile {
    pub const ZERO: u8 = 0;
    pub const STACK_PTR: u8 = 2;
    pub const A0: u8 = 10;
    pub const A1: u8 = 11;
    pub const A2: u8 = 12;
    pub const A3: u8 = 13;
    #[allow(unused)]
    pub const A4: u8 = 14;
    #[allow(unused)]
    pub const A5: u8 = 15;
    #[allow(unused)]
    pub const A6: u8 = 16;
    #[allow(unused)]
    pub const A7: u8 = 17;

    pub fn new() -> Self {
        Self::default()
    }

    pub fn read(&self, reg: u8) -> u64 {
        self.registers[reg as usize]
    }

    pub fn write(&mut self, rd: u8, data: u64) {
        if rd != Self::ZERO {
            self.registers[rd as usize] = data;
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn write_and_read() {
        let mut reg = RegisterFile::new();
        // Write at x0 is discarded
        reg.write(0, 10);
        assert_eq!(reg.read(0), 0);

        reg.write(10, 10);
        assert_eq!(reg.read(10), 10);
    }
}
