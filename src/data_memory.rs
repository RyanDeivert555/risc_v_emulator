use std::ops::Range;

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

fn range(
    addr: u64,
    count: usize,
    data_size: usize,
    data_align: usize,
) -> Result<Range<usize>, AddrError> {
    let start = usize::try_from(addr).map_err(|_| AddrError::OutOfRange)?;

    if !start.is_multiple_of(data_align) {
        return Err(AddrError::Misaligned);
    }

    let len = count.checked_mul(data_size).ok_or(AddrError::OutOfRange)?;
    let end = start.checked_add(len).ok_or(AddrError::OutOfRange)?;

    Ok(start..end)
}

#[derive(Debug)]
pub struct DataMemory {
    // We need the memory to have a higher alignment
    ram: Box<[u64]>,
}

impl DataMemory {
    pub fn new(ram_size: u64) -> Self {
        let count = (ram_size as usize).div_ceil(std::mem::size_of::<u64>());

        Self {
            ram: vec![0; count].into_boxed_slice(),
        }
    }

    fn as_bytes(&self) -> &[u8] {
        let ptr = self.ram.as_ptr().cast();
        let len = self.ram.len() * std::mem::size_of::<u64>();

        unsafe { std::slice::from_raw_parts(ptr, len) }
    }

    fn as_bytes_mut(&mut self) -> &mut [u8] {
        let ptr = self.ram.as_mut_ptr().cast();
        let len = self.ram.len() * std::mem::size_of::<u64>();

        unsafe { std::slice::from_raw_parts_mut(ptr, len) }
    }

    fn range_null_terminated(&self, addr: u64) -> Result<Range<usize>, AddrError> {
        let start = usize::try_from(addr).map_err(|_| AddrError::OutOfRange)?;
        let tail = self.as_bytes().get(start..).ok_or(AddrError::OutOfRange)?;
        let pos = tail
            .iter()
            .position(|&c| c == 0)
            .ok_or(AddrError::OutOfRange)?;

        Ok(start..start + pos + 1)
    }

    pub fn read(&self, addr: u64, size: DataSize, extend: SignExtend) -> Result<u64, AddrError> {
        let range = range(addr, 1, size as _, size as _)?;

        let src = self.as_bytes().get(range).ok_or(AddrError::OutOfRange)?;
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
        let range = range(addr, 1, size as _, size as _)?;

        let dest = self
            .as_bytes_mut()
            .get_mut(range)
            .ok_or(AddrError::OutOfRange)?;
        let src = &data.to_le_bytes()[0..size as usize];
        dest.copy_from_slice(src);

        Ok(())
    }

    pub fn bytes(&self, addr: u64, size: usize) -> Result<&[u8], AddrError> {
        let range = range(addr, size, DataSize::Byte as _, 1)?;

        Ok(&self.as_bytes()[range])
    }

    pub fn bytes_mut(&mut self, addr: u64, size: usize) -> Result<&mut [u8], AddrError> {
        let range = range(addr, size, DataSize::Byte as _, 1)?;

        Ok(&mut self.as_bytes_mut()[range])
    }

    pub fn bytes_null_terminated(&self, addr: u64) -> Result<&[u8], AddrError> {
        let range = self.range_null_terminated(addr)?;

        Ok(&self.as_bytes()[range])
    }

    pub fn raw_storage<T>(&self, addr: u64) -> Result<&[u8], AddrError> {
        let data_size = std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>();
        let range = range(addr, 1, data_size, align)?;

        Ok(&self.as_bytes()[range])
    }

    pub fn raw_storage_mut<T>(&mut self, addr: u64) -> Result<&mut [u8], AddrError> {
        let data_size = std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>();
        let range = range(addr, 1, data_size, align)?;

        Ok(&mut self.as_bytes_mut()[range])
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
            let mem = dm.bytes_mut(0, 10).unwrap();
            mem.fill(1);
        }

        let mem = dm.bytes(0, 10).unwrap();
        assert_eq!(mem, &[1, 1, 1, 1, 1, 1, 1, 1, 1, 1]);
    }

    #[test]
    fn raw_storage() {
        #[repr(C)]
        #[derive(Copy, Clone, Debug)]
        struct Thing {
            x: i32,
            y: i32,
            z: i32,
        }

        let thing = Thing { x: 1, y: 2, z: 3 };
        let mut dm = DataMemory::new(1 << 12);
        {
            let mem = dm.raw_storage_mut::<Thing>(0).unwrap();
            let bytes = {
                let ptr = &raw const thing;
                let ptr = ptr.cast();

                unsafe { std::slice::from_raw_parts(ptr, std::mem::size_of::<Thing>()) }
            };

            mem.copy_from_slice(bytes);
        }

        let raw = dm.raw_storage::<Thing>(0).unwrap();
        let ptr = raw.as_ptr().cast::<Thing>();
        let thing = unsafe { *ptr };

        assert_eq!(thing.x, 1);
        assert_eq!(thing.y, 2);
        assert_eq!(thing.z, 3);
    }
}
