use std::ops::Range;

#[derive(Debug)]
pub struct DataMemory {
    ram: Box<[u8]>,
}

#[repr(usize)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DataSize {
    Byte = 1,
    HalfWord = 2,
    Word = 4,
    DoubleWord = 8,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SignExtend {
    Zero,
    Msb,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum AddrError {
    Misaligned,
    OutOfRange,
}

fn range(addr: u64, count: usize, data_size: DataSize) -> Result<Range<usize>, AddrError> {
    let data_size = data_size as usize;
    let start = usize::try_from(addr).map_err(|_| AddrError::OutOfRange)?;

    if !start.is_multiple_of(data_size) {
        return Err(AddrError::Misaligned);
    }

    let len = count.checked_mul(data_size).ok_or(AddrError::OutOfRange)?;
    let end = start.checked_add(len).ok_or(AddrError::OutOfRange)?;

    Ok(start..end)
}

impl DataMemory {
    pub fn new(ram_size: u64) -> Self {
        Self {
            ram: vec![0; ram_size as usize].into(),
        }
    }

    fn range_null_terminated(&self, addr: u64) -> Result<Range<usize>, AddrError> {
        let start = usize::try_from(addr).map_err(|_| AddrError::OutOfRange)?;

        let end = self
            .ram
            .iter()
            .skip(start)
            .position(|c| *c == 0) // TODO: position with overflow checking?
            .ok_or(AddrError::OutOfRange)?;

        // include null terminator
        Ok(start..end + 1)
    }

    pub fn read(&self, addr: u64, size: DataSize, extend: SignExtend) -> Result<u64, AddrError> {
        let range = range(addr, 1, size)?;

        let src = self.ram.get(range).ok_or(AddrError::OutOfRange)?;
        let mut buffer = [0u8; 8];
        buffer[0..src.len()].copy_from_slice(src);
        let raw = u64::from_le_bytes(buffer);

        Ok(match extend {
            SignExtend::Zero => raw,
            SignExtend::Msb => {
                let shift = u64::BITS - (size as u32 * 8);

                ((raw << shift).cast_signed() >> shift).cast_unsigned()
            }
        })
    }

    pub fn write(&mut self, addr: u64, data: u64, size: DataSize) -> Result<(), AddrError> {
        let range = range(addr, 1, size)?;

        let dest = self.ram.get_mut(range).ok_or(AddrError::OutOfRange)?;
        let src = &data.to_le_bytes()[0..size as usize];
        dest.copy_from_slice(src);

        Ok(())
    }

    pub fn slice(&self, addr: u64, size: usize) -> Result<&[u8], AddrError> {
        let range = range(addr, size, DataSize::Byte)?;

        Ok(&self.ram[range])
    }

    pub fn slice_mut(&mut self, addr: u64, size: usize) -> Result<&mut [u8], AddrError> {
        let range = range(addr, size, DataSize::Byte)?;

        Ok(&mut self.ram[range])
    }

    pub fn slice_null_terminated(&self, addr: u64) -> Result<&[u8], AddrError> {
        let range = self.range_null_terminated(addr)?;

        Ok(&self.ram[range])
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn read_and_write() -> Result<(), AddrError> {
        let mut dm = DataMemory::new(1 << 12);

        dm.write(0, 0xffff_ffff_ffff_ffff, DataSize::Byte)?;
        let value = dm.read(0, DataSize::Byte, SignExtend::Zero)?;
        assert_eq!(value, 0xff);

        dm.write(0, 0xffff_ffff_ffff_ffff, DataSize::HalfWord)?;
        let value = dm.read(0, DataSize::HalfWord, SignExtend::Zero)?;
        assert_eq!(value, 0xffff);

        dm.write(0, 0xffff_ffff_ffff_ffff, DataSize::Word)?;
        let value = dm.read(0, DataSize::Word, SignExtend::Zero)?;
        assert_eq!(value, 0xffff_ffff);

        dm.write(0, 0xffff_ffff_ffff_ffff, DataSize::DoubleWord)?;
        let value = dm.read(0, DataSize::DoubleWord, SignExtend::Zero)?;
        assert_eq!(value, 0xffff_ffff_ffff_ffff);

        dm.write(0, 0xffff_ffff_ffff_ffff, DataSize::DoubleWord)?;
        let value = dm.read(0, DataSize::Byte, SignExtend::Zero)?;
        assert_eq!(value, 0xff);

        dm.write(0, 0x80, DataSize::Byte)?;
        let value = dm.read(0, DataSize::Byte, SignExtend::Msb)?;
        assert_eq!(value, 0xffff_ffff_ffff_ff80);

        dm.write(0, 1, DataSize::Byte)?;
        dm.write(1, 1, DataSize::Byte)?;
        dm.write(2, 1, DataSize::Byte)?;
        dm.write(3, 1, DataSize::Byte)?;
        let value = dm.read(0, DataSize::Word, SignExtend::Zero)?;
        assert_eq!(value, 0x01_01_01_01);

        dm.write(0, 0x01_01_01_01, DataSize::Word)?;
        let value = dm.read(0, DataSize::Byte, SignExtend::Zero)?;
        assert_eq!(value, 1);
        let value = dm.read(1, DataSize::Byte, SignExtend::Zero)?;
        assert_eq!(value, 1);
        let value = dm.read(2, DataSize::Byte, SignExtend::Zero)?;
        assert_eq!(value, 1);
        let value = dm.read(3, DataSize::Byte, SignExtend::Zero)?;
        assert_eq!(value, 1);

        Ok(())
    }

    #[test]
    fn slicing() {
        let mut dm = DataMemory::new(1 << 12);
        {
            let mem = dm.slice_mut(0, 10).unwrap();
            mem.fill(1);
        }

        let mem = dm.slice(0, 10).unwrap();
        assert_eq!(mem, &[1, 1, 1, 1, 1, 1, 1, 1, 1, 1]);
    }
}
