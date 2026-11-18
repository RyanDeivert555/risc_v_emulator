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

        self.as_bytes().get(range).ok_or(AddrError::OutOfRange)
    }

    pub fn bytes_mut(&mut self, addr: u64, size: usize) -> Result<&mut [u8], AddrError> {
        let range = range(addr, size, DataSize::Byte as _, 1)?;

        self.as_bytes_mut()
            .get_mut(range)
            .ok_or(AddrError::OutOfRange)
    }

    pub fn bytes_null_terminated(&self, addr: u64) -> Result<&[u8], AddrError> {
        let range = self.range_null_terminated(addr)?;

        self.as_bytes().get(range).ok_or(AddrError::OutOfRange)
    }

    pub fn raw_storage<T>(&self, addr: u64) -> Result<&[u8], AddrError> {
        let data_size = std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>();
        let range = range(addr, 1, data_size, align)?;

        self.as_bytes().get(range).ok_or(AddrError::OutOfRange)
    }

    pub fn raw_storage_mut<T>(&mut self, addr: u64) -> Result<&mut [u8], AddrError> {
        let data_size = std::mem::size_of::<T>();
        let align = std::mem::align_of::<T>();
        let range = range(addr, 1, data_size, align)?;

        self.as_bytes_mut()
            .get_mut(range)
            .ok_or(AddrError::OutOfRange)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn zero_extend() -> Result<(), AddrError> {
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

        Ok(())
    }

    #[test]
    fn sign_extend() -> Result<(), AddrError> {
        let mut dm = DataMemory::new(1 << 12);

        dm.write(0, 0x80, DataSize::Byte)?;
        let value = dm.read(0, DataSize::Byte, SignExtend::Msb)?;
        assert_eq!(value, 0xffff_ffff_ffff_ff80);

        dm.write(0, 0xffff_ffff_ffff_ffff, DataSize::Byte)?;
        let value = dm.read(0, DataSize::Byte, SignExtend::Msb)?;
        assert_eq!(value, 0xffff_ffff_ffff_ffff);

        dm.write(0, 0xffff_ffff_ffff_ffff, DataSize::HalfWord)?;
        let value = dm.read(0, DataSize::HalfWord, SignExtend::Msb)?;
        assert_eq!(value, 0xffff_ffff_ffff_ffff);

        dm.write(0, 0xffff_ffff_ffff_ffff, DataSize::Word)?;
        let value = dm.read(0, DataSize::Word, SignExtend::Msb)?;
        assert_eq!(value, 0xffff_ffff_ffff_ffff);

        dm.write(0, 0xffff_ffff_ffff_ffff, DataSize::DoubleWord)?;
        let value = dm.read(0, DataSize::DoubleWord, SignExtend::Msb)?;
        assert_eq!(value, 0xffff_ffff_ffff_ffff);

        Ok(())
    }

    #[test]
    fn reinterpret() -> Result<(), AddrError> {
        let mut dm = DataMemory::new(1 << 12);

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
        // SAFETY: ptr is properly aligned, points to valid object
        let thing = unsafe { *ptr };

        assert_eq!(thing.x, 1);
        assert_eq!(thing.y, 2);
        assert_eq!(thing.z, 3);
    }

    #[test]
    fn alignment_errors() {
        let mut dm = DataMemory::new(1 << 12);

        // HalfWord requires 2-byte alignment
        assert_eq!(
            dm.write(1, 0, DataSize::HalfWord),
            Err(AddrError::Misaligned)
        );
        assert_eq!(
            dm.read(1, DataSize::HalfWord, SignExtend::Zero),
            Err(AddrError::Misaligned)
        );
        assert!(dm.write(2, 0x1234, DataSize::HalfWord).is_ok());

        // Word requires 4-byte alignment
        for offset in [1, 2, 3] {
            assert_eq!(
                dm.write(offset, 0, DataSize::Word),
                Err(AddrError::Misaligned)
            );
            assert_eq!(
                dm.read(offset, DataSize::Word, SignExtend::Zero),
                Err(AddrError::Misaligned)
            );
        }
        assert!(dm.write(4, 0x1234_5678, DataSize::Word).is_ok());

        // DoubleWord requires 8-byte alignment
        assert_eq!(
            dm.write(4, 0, DataSize::DoubleWord),
            Err(AddrError::Misaligned)
        );
        assert_eq!(
            dm.read(4, DataSize::DoubleWord, SignExtend::Zero),
            Err(AddrError::Misaligned)
        );
        assert!(
            dm.write(8, 0x1234_5678_9abc_def0, DataSize::DoubleWord)
                .is_ok()
        );

        // raw_storage enforces T's alignment
        assert_eq!(dm.raw_storage::<u32>(2).unwrap_err(), AddrError::Misaligned);
        assert_eq!(
            dm.raw_storage_mut::<u64>(4).unwrap_err(),
            AddrError::Misaligned
        );
    }

    #[test]
    fn bounds_and_overflow() {
        let mut dm = DataMemory::new(16);

        // Last valid address for each width
        assert!(dm.write(15, 0xaa, DataSize::Byte).is_ok());
        assert!(dm.write(14, 0xbbbb, DataSize::HalfWord).is_ok());
        assert!(dm.write(12, 0xcccc_cccc, DataSize::Word).is_ok());
        assert!(
            dm.write(8, 0xdddd_dddd_dddd_dddd, DataSize::DoubleWord)
                .is_ok()
        );

        // One step past the end of RAM
        assert_eq!(dm.write(16, 0, DataSize::Byte), Err(AddrError::OutOfRange));
        assert_eq!(
            dm.read(16, DataSize::Byte, SignExtend::Zero),
            Err(AddrError::OutOfRange)
        );
        assert_eq!(
            dm.write(16, 0, DataSize::HalfWord),
            Err(AddrError::OutOfRange)
        );
        assert_eq!(dm.write(16, 0, DataSize::Word), Err(AddrError::OutOfRange));
        assert_eq!(
            dm.write(16, 0, DataSize::DoubleWord),
            Err(AddrError::OutOfRange)
        );

        // Address arithmetic overflow (0xffff_ffff_ffff_fffc is 4-byte aligned)
        assert_eq!(
            dm.read(u64::MAX - 3, DataSize::Word, SignExtend::Zero),
            Err(AddrError::OutOfRange)
        );

        // Slice and raw_storage out-of-range checks
        assert_eq!(dm.bytes(12, 5).unwrap_err(), AddrError::OutOfRange);
        assert_eq!(dm.bytes_mut(16, 1).unwrap_err(), AddrError::OutOfRange);
        assert_eq!(
            dm.raw_storage::<u64>(16).unwrap_err(),
            AddrError::OutOfRange
        );
        assert_eq!(
            dm.raw_storage_mut::<u64>(16).unwrap_err(),
            AddrError::OutOfRange
        );
    }

    #[test]
    fn sign_extend_boundaries() -> Result<(), AddrError> {
        let mut dm = DataMemory::new(1 << 12);

        // Positive MSB=0 values must not be sign-extended
        dm.write(0, 0x7f, DataSize::Byte)?;
        assert_eq!(dm.read(0, DataSize::Byte, SignExtend::Msb)?, 0x7f);

        dm.write(0, 0x7fff, DataSize::HalfWord)?;
        assert_eq!(dm.read(0, DataSize::HalfWord, SignExtend::Msb)?, 0x7fff);

        dm.write(0, 0x7fff_ffff, DataSize::Word)?;
        assert_eq!(dm.read(0, DataSize::Word, SignExtend::Msb)?, 0x7fff_ffff);

        // Negative MSB=1 values for HalfWord and Word
        dm.write(0, 0x8000, DataSize::HalfWord)?;
        assert_eq!(
            dm.read(0, DataSize::HalfWord, SignExtend::Msb)?,
            0xffff_ffff_ffff_8000
        );

        dm.write(0, 0x8000_0000, DataSize::Word)?;
        assert_eq!(
            dm.read(0, DataSize::Word, SignExtend::Msb)?,
            0xffff_ffff_8000_0000
        );

        Ok(())
    }

    #[test]
    fn null_terminated_strings() -> Result<(), AddrError> {
        let mut dm = DataMemory::new(16);

        // Empty C-string at offset 0
        assert_eq!(dm.bytes_null_terminated(0)?, b"\0");

        // String embedded in memory (includes trailing null byte)
        dm.bytes_mut(4, 8)?.copy_from_slice(b"risc-v\0!");
        assert_eq!(dm.bytes_null_terminated(4)?, b"risc-v\0");

        // Start address out of bounds
        assert_eq!(
            dm.bytes_null_terminated(16).unwrap_err(),
            AddrError::OutOfRange
        );

        // No null terminator present before end of RAM
        dm.bytes_mut(0, 16)?.fill(b'a');
        assert_eq!(
            dm.bytes_null_terminated(0).unwrap_err(),
            AddrError::OutOfRange
        );

        Ok(())
    }
}
