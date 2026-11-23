use crate::{
    data_memory::{AddrError, DataMemory},
    instruction_memory::{
        AluOp, AluWidth, Condition, FetchError, Instruction, InstructionLoadError,
        InstructionMemory,
    },
    register_file::RegisterFile,
};

pub struct Cpu {
    instruction_memory: InstructionMemory,
    register_file: RegisterFile,
    data_memory: DataMemory,
    pc: u64,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CpuStepError {
    InstructionMemory(FetchError),
    DataMemory(AddrError),
    UnsupportedSyscall(u64),
}

impl std::fmt::Display for CpuStepError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InstructionMemory(FetchError::OutOfRange) => {
                write!(f, "instruction out of range")
            }
            Self::InstructionMemory(FetchError::Misaligned) => {
                write!(f, "misaliged instruction access")
            }
            Self::DataMemory(AddrError::OutOfRange) => write!(f, "memory out of range"),
            Self::DataMemory(AddrError::Misaligned) => write!(f, "misaligned memory access"),
            Self::UnsupportedSyscall(sys) => write!(f, "unsupported or invalid syscall: {}", sys),
        }
    }
}

impl std::error::Error for CpuStepError {}

impl From<FetchError> for CpuStepError {
    fn from(value: FetchError) -> Self {
        Self::InstructionMemory(value)
    }
}

impl From<AddrError> for CpuStepError {
    fn from(value: AddrError) -> Self {
        Self::DataMemory(value)
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ExecutionResult {
    Continue,
    Breakpoint,
    Exit(i32),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum CpuCreationError {
    RamMisalign,
    InstructionLoad,
    InvalidElf,
    OutOfRam { current: u64, required: u64 },
}

impl std::fmt::Display for CpuCreationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::RamMisalign => write!(f, "ram size is not a multiple of 16"),
            Self::InstructionLoad => write!(f, "invalid instruction in binary"),
            Self::InvalidElf => write!(f, "invalid ELF file"),
            Self::OutOfRam { current, required } => write!(
                f,
                "not enough ram, need {} bytes, have {}",
                required, current
            ),
        }
    }
}

impl std::error::Error for CpuCreationError {}

impl From<InstructionLoadError> for CpuCreationError {
    fn from(_: InstructionLoadError) -> Self {
        Self::InstructionLoad
    }
}

fn alu(lhs: u64, rhs: u64, mode: AluOp, width: AluWidth) -> u64 {
    use AluOp as Op;

    match (mode, width) {
        (Op::Add, AluWidth::DoubleWord) => lhs.wrapping_add(rhs),
        (Op::Add, AluWidth::Word) => lhs.wrapping_add(rhs) as i32 as i64 as u64,
        (Op::Sub, AluWidth::DoubleWord) => lhs.wrapping_sub(rhs),
        (Op::Sub, AluWidth::Word) => lhs.wrapping_sub(rhs) as i32 as i64 as u64,
        (Op::Mul, AluWidth::DoubleWord) => lhs.wrapping_mul(rhs),
        (Op::MulHigh, _) => {
            ((lhs.cast_signed() as i128).wrapping_mul(rhs.cast_signed() as i128) >> 64) as u64
        }
        (Op::MulHighSignedU, _) => {
            ((lhs.cast_signed() as i128).wrapping_mul(rhs as u128 as i128) >> 64) as u64
        }
        (Op::MulHighU, _) => ((lhs as u128).wrapping_mul(rhs as u128) >> 64) as u64,
        (Op::Div, AluWidth::DoubleWord) => {
            if rhs == 0 {
                u64::MAX
            } else {
                lhs.cast_signed()
                    .wrapping_div(rhs.cast_signed())
                    .cast_unsigned()
            }
        }
        (Op::DivU, AluWidth::DoubleWord) => {
            if rhs == 0 {
                u64::MAX
            } else {
                lhs / rhs
            }
        }
        (Op::Rem, AluWidth::DoubleWord) => {
            if rhs == 0 {
                lhs
            } else {
                lhs.cast_signed()
                    .wrapping_rem(rhs.cast_signed())
                    .cast_unsigned()
            }
        }
        (Op::RemU, AluWidth::DoubleWord) => {
            if rhs == 0 {
                lhs
            } else {
                lhs % rhs
            }
        }
        (Op::Mul, AluWidth::Word) => (lhs as u32).wrapping_mul(rhs as _) as i32 as i64 as u64,
        (Op::Div, AluWidth::Word) => {
            let rhs = rhs as i32;

            if rhs == 0 {
                u64::MAX
            } else {
                (lhs as i32).wrapping_div(rhs) as i64 as u64
            }
        }
        (Op::DivU, AluWidth::Word) => {
            let rhs = rhs as u32;

            if rhs == 0 {
                u64::MAX
            } else {
                (lhs as u32).wrapping_div(rhs) as i32 as i64 as u64
            }
        }
        (Op::Rem, AluWidth::Word) => {
            let lhs = lhs as i32;
            let rhs = rhs as i32;

            if rhs == 0 {
                lhs as i64 as u64
            } else {
                lhs.wrapping_rem(rhs) as i64 as u64
            }
        }
        (Op::RemU, AluWidth::Word) => {
            let lhs = lhs as u32;
            let rhs = rhs as u32;

            if rhs == 0 {
                lhs as i32 as i64 as u64
            } else {
                lhs.wrapping_rem(rhs) as i32 as i64 as u64
            }
        }
        (Op::Xor, _) => lhs ^ rhs,
        (Op::Or, _) => lhs | rhs,
        (Op::And, _) => lhs & rhs,
        (Op::Sll, AluWidth::DoubleWord) => lhs.wrapping_shl(rhs as _),
        (Op::Sll, AluWidth::Word) => (lhs as u32).wrapping_shl(rhs as _) as i32 as i64 as u64,
        (Op::Srl, AluWidth::DoubleWord) => lhs.wrapping_shr(rhs as _),
        (Op::Srl, AluWidth::Word) => (lhs as u32).wrapping_shr(rhs as _) as i32 as i64 as u64,
        (Op::Sra, AluWidth::DoubleWord) => lhs.cast_signed().wrapping_shr(rhs as _).cast_unsigned(),
        (Op::Sra, AluWidth::Word) => (lhs as i32).wrapping_shr(rhs as _) as i64 as u64,
        (Op::Slt, _) => u64::from(lhs.cast_signed() < rhs.cast_signed()),
        (Op::Sltu, _) => u64::from(lhs < rhs),
    }
}

impl Cpu {
    fn build(
        code: InstructionMemory,
        pc: u64,
        data_memory: DataMemory,
        ram_size: u64,
    ) -> Result<Self, CpuCreationError> {
        if !ram_size.is_multiple_of(16) {
            return Err(CpuCreationError::RamMisalign);
        }

        let mut register_file = RegisterFile::new();
        register_file.write(RegisterFile::STACK_PTR, ram_size);

        Ok(Self {
            instruction_memory: code,
            register_file,
            data_memory,
            pc,
        })
    }

    #[allow(unused)]
    pub fn new(
        instructions: &[u32],
        pc: u64,
        virtual_address: u64,
        ram_size: u64,
    ) -> Result<Self, CpuCreationError> {
        let bytes = {
            let ptr = instructions.as_ptr().cast();
            let len = std::mem::size_of_val(instructions);

            // SAFETY: pointer is properly aligned with a correct length
            unsafe { std::slice::from_raw_parts(ptr, len) }
        };

        Self::from_bytes(bytes, pc, virtual_address, ram_size)
    }

    pub fn from_bytes(
        bytes: &[u8],
        pc: u64,
        virtual_address: u64,
        ram_size: u64,
    ) -> Result<Self, CpuCreationError> {
        let instruction_memory = InstructionMemory::from_bytes(bytes, virtual_address)?;
        let data_memory = DataMemory::new(ram_size);

        Self::build(instruction_memory, pc, data_memory, ram_size)
    }

    #[allow(unused)]
    pub fn from_flat_binary(bytes: &[u8], ram_size: u64) -> Result<Self, CpuCreationError> {
        let instruction_memory = InstructionMemory::from_bytes(bytes, 0)?;
        let data_memory = DataMemory::new(ram_size);

        Self::build(instruction_memory, 0, data_memory, ram_size)
    }

    pub fn from_elf(elf: &[u8], ram_size: u64) -> Result<Self, CpuCreationError> {
        fn validate_bytes(
            bytes: &[u8],
            range: std::ops::Range<usize>,
            target: &[u8],
        ) -> Result<(), CpuCreationError> {
            let section = bytes.get(range).ok_or(CpuCreationError::InvalidElf)?;

            if section == target {
                Ok(())
            } else {
                Err(CpuCreationError::InvalidElf)
            }
        }

        fn validate_byte(bytes: &[u8], index: usize, target: u8) -> Result<(), CpuCreationError> {
            let section = bytes.get(index).ok_or(CpuCreationError::InvalidElf)?;

            if *section == target {
                Ok(())
            } else {
                Err(CpuCreationError::InvalidElf)
            }
        }

        // https://en.wikipedia.org/wiki/Executable_and_Linkable_Format
        if elf.len() < 64 {
            Err(CpuCreationError::InvalidElf)
        } else {
            // ELF magic header
            validate_bytes(elf, 0..4, &[0x7f, 0x45, 0x4c, 0x46])?;
            // 64-bit elf
            validate_byte(elf, 4, 2)?;
            // little endian
            validate_byte(elf, 5, 1)?;
            // executable
            validate_bytes(elf, 0x10..0x12, &0x02u16.to_le_bytes())?;
            // RISC-V arch
            validate_bytes(elf, 0x12..0x14, &0xf3u16.to_le_bytes())?;
            // we know it is a valid 64-bit ELF executable now, treat it as such
            let pc = u64::from_le_bytes(elf[0x18..0x20].try_into().unwrap());
            let program_header_offset =
                u64::from_le_bytes(elf[0x20..0x28].try_into().unwrap()) as usize;
            let program_header_size =
                u16::from_le_bytes(elf[0x36..0x38].try_into().unwrap()) as usize;
            let program_header_count =
                u16::from_le_bytes(elf[0x38..0x3a].try_into().unwrap()) as usize;

            let mut data_memory = DataMemory::new(ram_size);
            let mut code: Vec<u8> = Vec::new();
            let mut code_virtual_address = 0;

            if program_header_size < 56 {
                return Err(CpuCreationError::InvalidElf);
            }

            for i in 0..program_header_count {
                let offset = program_header_offset
                    .checked_add(i * program_header_size)
                    .ok_or(CpuCreationError::InvalidElf)?;
                let program_header = elf
                    .get(offset..offset + 56)
                    .ok_or(CpuCreationError::InvalidElf)?;

                let ty = u32::from_le_bytes(program_header[0..4].try_into().unwrap());
                // Skip non-loadable segments
                if ty != 1 {
                    continue;
                }

                let flags = u32::from_le_bytes(program_header[0x04..0x08].try_into().unwrap());
                let program_offset =
                    u64::from_le_bytes(program_header[0x08..0x10].try_into().unwrap()) as usize;
                let virutal_address =
                    u64::from_le_bytes(program_header[0x10..0x18].try_into().unwrap());
                let segment_size =
                    u64::from_le_bytes(program_header[0x20..0x28].try_into().unwrap()) as usize;
                let memory_size =
                    u64::from_le_bytes(program_header[0x28..0x30].try_into().unwrap()) as usize;

                if segment_size > memory_size {
                    return Err(CpuCreationError::InvalidElf);
                }

                let end_offset = program_offset
                    .checked_add(segment_size)
                    .ok_or(CpuCreationError::InvalidElf)?;
                let segment_data = elf
                    .get(program_offset..end_offset)
                    .ok_or(CpuCreationError::InvalidElf)?;

                if flags & 0x1 != 0 {
                    code_virtual_address = virutal_address;
                    code = segment_data.to_vec();
                    // Pad trailing unaligned .rodata bytes so InstructionMemory::from_bytes succeeds
                    let rem = code.len() % 4;
                    if rem != 0 {
                        code.resize(code.len() + (4 - rem), 0);
                    }
                }

                if memory_size > 0 {
                    let dest = data_memory
                        .bytes_mut(virutal_address, memory_size)
                        .map_err(|_| CpuCreationError::OutOfRam {
                            current: ram_size,
                            required: virutal_address + memory_size as u64,
                        })?;
                    dest[..segment_size].copy_from_slice(segment_data);
                    dest[segment_size..].fill(0);
                }
            }

            if code.is_empty() {
                return Err(CpuCreationError::InvalidElf);
            }

            let instruction_memory = InstructionMemory::from_bytes(&code, code_virtual_address)?;

            Self::build(instruction_memory, pc, data_memory, ram_size)
        }
    }

    fn syscall(&mut self) -> Result<ExecutionResult, CpuStepError> {
        let sys = self.register_file.read(RegisterFile::A7);

        // https://jborza.com/post/2021-05-11-riscv-linux-syscalls
        match sys {
            0 => {
                let n = self.register_file.read(RegisterFile::A0) as libc::c_uint;
                let ctx = {
                    let addr = self.register_file.read(RegisterFile::A1);
                    let raw = self.data_memory.raw_storage_mut::<libc::c_ulong>(addr)?;

                    raw.as_mut_ptr().cast::<libc::c_ulong>()
                };

                let result = unsafe { libc::syscall(libc::SYS_io_setup, n, ctx) };
                self.register_file.write(RegisterFile::A0, result as _);

                Ok(ExecutionResult::Continue)
            }
            1 => {
                let ctx = self.register_file.read(RegisterFile::A0) as libc::c_ulong;

                let result = unsafe { libc::syscall(libc::SYS_io_destroy, ctx) };
                self.register_file.write(RegisterFile::A0, result as _);

                Ok(ExecutionResult::Continue)
            }
            56 => {
                let dir_fd = self.register_file.read(RegisterFile::A0) as libc::c_int;
                let filename = {
                    let addr = self.register_file.read(RegisterFile::A1);
                    let slice = self.data_memory.bytes_null_terminated(addr)?;

                    // SAFETY: slice is null at the end
                    unsafe { std::ffi::CStr::from_bytes_with_nul_unchecked(slice) }
                };
                let flags = self.register_file.read(RegisterFile::A2) as libc::c_int;
                let mode = self.register_file.read(RegisterFile::A3) as libc::c_int;

                let fd = unsafe { libc::openat(dir_fd, filename.as_ptr(), flags, mode) };
                self.register_file.write(RegisterFile::A0, fd as _);

                Ok(ExecutionResult::Continue)
            }
            57 => {
                let fd = self.register_file.read(RegisterFile::A0) as libc::c_int;

                let result = unsafe { libc::close(fd) };
                self.register_file.write(RegisterFile::A0, result as _);

                Ok(ExecutionResult::Continue)
            }
            59 => {
                let pipes = {
                    let addr = self.register_file.read(RegisterFile::A0);
                    let raw = self.data_memory.raw_storage_mut::<[libc::c_int; 2]>(addr)?;

                    raw.as_mut_ptr().cast()
                };
                let flags = self.register_file.read(RegisterFile::A1) as libc::c_int;

                let result = unsafe { libc::pipe2(pipes, flags) };
                self.register_file.write(RegisterFile::A0, result as _);

                Ok(ExecutionResult::Continue)
            }
            62 => {
                let fd = self.register_file.read(RegisterFile::A0) as libc::c_int;
                let offset = self.register_file.read(RegisterFile::A1) as libc::off_t;
                let whence = self.register_file.read(RegisterFile::A2) as libc::c_int;

                let result = unsafe { libc::lseek(fd, offset, whence) };
                self.register_file.write(RegisterFile::A0, result as _);

                Ok(ExecutionResult::Continue)
            }
            63 => {
                let fd = self.register_file.read(RegisterFile::A0) as libc::c_int;
                let (buffer, len) = {
                    let addr = self.register_file.read(RegisterFile::A1);
                    let count = self.register_file.read(RegisterFile::A2) as usize;
                    let memory = self.data_memory.bytes_mut(addr, count)?;

                    (memory.as_mut_ptr().cast(), memory.len())
                };

                let result = unsafe { libc::read(fd, buffer, len) };
                self.register_file.write(RegisterFile::A0, result as _);

                Ok(ExecutionResult::Continue)
            }
            64 => {
                let fd = self.register_file.read(RegisterFile::A0) as libc::c_int;
                let (buffer, len) = {
                    let addr = self.register_file.read(RegisterFile::A1);
                    let count = self.register_file.read(RegisterFile::A2) as usize;
                    let memory = self.data_memory.bytes(addr, count)?;

                    (memory.as_ptr().cast(), memory.len())
                };

                let result = unsafe { libc::write(fd, buffer, len) };
                self.register_file.write(RegisterFile::A0, result as _);

                Ok(ExecutionResult::Continue)
            }
            93 | 94 => {
                let status_code = self.register_file.read(RegisterFile::A0) as i32;

                Ok(ExecutionResult::Exit(status_code))
            }
            172 => {
                let result = unsafe { libc::getpid() };
                self.register_file.write(RegisterFile::A0, result as _);

                Ok(ExecutionResult::Continue)
            }
            198 => {
                let domain = self.register_file.read(RegisterFile::A0) as libc::c_int;
                let ty = self.register_file.read(RegisterFile::A1) as libc::c_int;
                let protocol = self.register_file.read(RegisterFile::A2) as libc::c_int;

                let result = unsafe { libc::socket(domain, ty, protocol) };
                self.register_file.write(RegisterFile::A0, result as _);

                Ok(ExecutionResult::Continue)
            }
            200 => {
                let sockfd = self.register_file.read(RegisterFile::A0) as libc::c_int;
                let sockaddr = {
                    let addr = self.register_file.read(RegisterFile::A1);
                    let raw = self.data_memory.raw_storage::<libc::sockaddr>(addr)?;

                    raw.as_ptr().cast()
                };
                let addrlen = self.register_file.read(RegisterFile::A2) as libc::socklen_t;

                let result = unsafe { libc::bind(sockfd, sockaddr, addrlen) };
                self.register_file.write(RegisterFile::A0, result as _);

                Ok(ExecutionResult::Continue)
            }
            201 => {
                let sockfd = self.register_file.read(RegisterFile::A0) as libc::c_int;
                let backlog = self.register_file.read(RegisterFile::A1) as libc::c_int;

                let result = unsafe { libc::listen(sockfd, backlog) };
                self.register_file.write(RegisterFile::A0, result as _);

                Ok(ExecutionResult::Continue)
            }
            202 => {
                let sockfd = self.register_file.read(RegisterFile::A0) as libc::c_int;
                let addr_ptr = self.register_file.read(RegisterFile::A1);
                let len_ptr = self.register_file.read(RegisterFile::A2);

                let result = if addr_ptr == 0 {
                    unsafe { libc::accept(sockfd, std::ptr::null_mut(), std::ptr::null_mut()) }
                } else {
                    let len_bytes = self
                        .data_memory
                        .bytes(len_ptr, std::mem::size_of::<libc::socklen_t>())?;
                    let mut len = libc::socklen_t::from_le_bytes(len_bytes.try_into().unwrap());

                    let mut addr: libc::sockaddr = unsafe { std::mem::zeroed() };

                    let result = unsafe { libc::accept(sockfd, &mut addr, &mut len) };

                    if result >= 0 {
                        let bytes = {
                            let ptr = &raw const addr;
                            let ptr = ptr.cast();

                            unsafe { std::slice::from_raw_parts(ptr, len as _) }
                        };

                        self.data_memory
                            .bytes_mut(addr_ptr, len as _)?
                            .copy_from_slice(bytes);
                        self.data_memory
                            .bytes_mut(len_ptr, std::mem::size_of::<libc::socklen_t>())?
                            .copy_from_slice(&len.to_le_bytes());
                    }

                    result
                };

                self.register_file.write(RegisterFile::A0, result as _);

                Ok(ExecutionResult::Continue)
            }
            203 => {
                let sockfd = self.register_file.read(RegisterFile::A0) as libc::c_int;
                let sockaddr = {
                    let addr = self.register_file.read(RegisterFile::A1);
                    let raw = self.data_memory.raw_storage::<libc::sockaddr>(addr)?;

                    raw.as_ptr().cast()
                };
                let addrlen = self.register_file.read(RegisterFile::A2) as libc::socklen_t;

                let result = unsafe { libc::connect(sockfd, sockaddr, addrlen) };
                self.register_file.write(RegisterFile::A0, result as _);

                Ok(ExecutionResult::Continue)
            }
            sys => Err(CpuStepError::UnsupportedSyscall(sys)),
        }
    }

    fn step(&mut self) -> Result<ExecutionResult, CpuStepError> {
        let instruction = self.instruction_memory.fetch(self.pc)?;
        let mut next_pc = self.pc.wrapping_add(4);

        match instruction {
            Instruction::AluRType {
                rd,
                rs1,
                rs2,
                alu_op,
                alu_width,
            } => {
                let lhs = self.register_file.read(rs1);
                let rhs = self.register_file.read(rs2);
                let result = alu(lhs, rhs, alu_op, alu_width);

                self.register_file.write(rd, result);
            }
            Instruction::AluIType {
                rd,
                rs1,
                imm,
                alu_op,
                alu_width,
            } => {
                let lhs = self.register_file.read(rs1);
                let result = alu(lhs, imm.cast_unsigned(), alu_op, alu_width);

                self.register_file.write(rd, result);
            }
            Instruction::Load {
                rd,
                rs1,
                imm,
                size,
                extend,
            } => {
                let base = self.register_file.read(rs1);
                let addr = base.wrapping_add(imm.cast_unsigned());
                let data = self.data_memory.read(addr, size, extend)?;

                self.register_file.write(rd, data);
            }
            Instruction::Store {
                rs1,
                rs2,
                imm,
                size,
            } => {
                let base = self.register_file.read(rs1);
                let value = self.register_file.read(rs2);
                let addr = base.wrapping_add(imm.cast_unsigned());

                self.data_memory.write(addr, value, size)?;
            }
            Instruction::Branch {
                rs1,
                rs2,
                imm,
                cond,
            } => {
                let lhs = self.register_file.read(rs1);
                let rhs = self.register_file.read(rs2);

                let branch = match cond {
                    Condition::Eq => lhs == rhs,
                    Condition::Ne => lhs != rhs,
                    Condition::Lt => lhs.cast_signed() < rhs.cast_signed(),
                    Condition::Ge => lhs.cast_signed() >= rhs.cast_signed(),
                    Condition::Ltu => lhs < rhs,
                    Condition::Geu => lhs >= rhs,
                };

                if branch {
                    next_pc = self.pc.wrapping_add(imm.cast_unsigned());
                }
            }
            Instruction::Jal { rd, imm } => {
                self.register_file.write(rd, self.pc.wrapping_add(4));

                next_pc = self.pc.wrapping_add(imm.cast_unsigned());
            }
            Instruction::Jalr { rd, rs1, imm } => {
                next_pc = self
                    .register_file
                    .read(rs1)
                    .wrapping_add(imm.cast_unsigned())
                    & !1;

                self.register_file.write(rd, self.pc.wrapping_add(4));
            }
            Instruction::Lui { rd, imm } => {
                self.register_file.write(rd, imm.cast_unsigned());
            }
            Instruction::Auipc { rd, imm } => {
                self.register_file
                    .write(rd, self.pc.wrapping_add(imm.cast_unsigned()));
            }
            Instruction::Fence => (),
            Instruction::ECall => {
                let result = self.syscall();
                self.pc = next_pc;

                return result;
            }
            Instruction::EBreak => return Ok(ExecutionResult::Breakpoint),
        }

        self.pc = next_pc;

        Ok(ExecutionResult::Continue)
    }

    pub fn run(&mut self) -> Result<ExecutionResult, CpuStepError> {
        loop {
            match self.step()? {
                ExecutionResult::Continue => (),
                stopping_point => return Ok(stopping_point),
            }
        }
    }

    #[cfg(test)]
    pub fn run_test(&mut self, count: usize) -> Result<ExecutionResult, CpuStepError> {
        for _ in 0..count {
            match self.step()? {
                ExecutionResult::Continue => (),
                stopping_point => return Ok(stopping_point),
            }
        }

        Ok(ExecutionResult::Exit(0))
    }
}

#[cfg(test)]
mod test {
    use std::os::fd::AsRawFd;

    use super::*;
    use crate::data_memory::*;
    // https://luplab.gitlab.io/rvcodecjs/#q=ld&abi=false&isa=AUTO

    #[test]
    #[should_panic]
    fn invalid_program() {
        let program = [0xdeadbeef, 0xeefeff, 0xf00];
        let mut cpu = Cpu::new(&program, 0, 0, 1 << 12).unwrap();
        cpu.run().unwrap();
    }

    #[test]
    fn arithmetic() {
        let program = [
            0x00500093, // addi x1, x0, 5
            0x00a00113, // addi x2, x0, 10
            0x002080b3, // add x1, x1, x2
        ];
        let mut cpu = Cpu::new(&program, 0, 0, 1 << 12).unwrap();
        // For testing
        cpu.register_file.write(RegisterFile::STACK_PTR, 0);
        cpu.run_test(program.len()).unwrap();

        let value = cpu.register_file.read(1);
        assert_eq!(value, 15);

        let program = [
            0x00500093, // addi x1, x0, 5
            0x00a00113, // addi x2, x0, 10
            0x402080b3, // sub x1, x1, x2
        ];
        let mut cpu = Cpu::new(&program, 0, 0, 1 << 12).unwrap();
        // For testing
        cpu.register_file.write(RegisterFile::STACK_PTR, 0);
        cpu.run_test(program.len()).unwrap();

        let value = cpu.register_file.read(1);
        assert_eq!(value, (5i64 - 10).cast_unsigned());

        let program = [
            0x00a00093, // addi x1, x0, 10
            0x00a00113, // addi x2, x0, 10
            0x0020f0b3, // and x1, x1, x2
        ];
        let mut cpu = Cpu::new(&program, 0, 0, 1 << 12).unwrap();
        // For testing
        cpu.register_file.write(RegisterFile::STACK_PTR, 0);
        cpu.run_test(program.len()).unwrap();

        let value = cpu.register_file.read(1);
        assert_eq!(value, 10);
    }

    #[test]
    fn load_and_store() {
        // misaligned load
        let program = [
            0x00a00093, // addi x1, x0, 10
            0x0000a103, // lw x2, 0(x1)
        ];
        let mut cpu = Cpu::new(&program, 0, 0, 1 << 12).unwrap();
        // For testing
        cpu.register_file.write(RegisterFile::STACK_PTR, 0);
        assert!(
            cpu.run_test(program.len()).unwrap_err()
                == CpuStepError::DataMemory(AddrError::Misaligned)
        );

        let program = [
            0x00c00093, // addi x1, x0, 12
            0x06400113, // addi x2, x0, 100
            0x0020a023, // sw x2, 0(x1)
        ];
        let mut cpu = Cpu::new(&program, 0, 0, 1 << 12).unwrap();
        // For testing
        cpu.register_file.write(RegisterFile::STACK_PTR, 0);
        cpu.run_test(program.len()).unwrap();

        // Mimic lw
        let value = cpu
            .data_memory
            .read(12, DataSize::Word, SignExtend::Msb)
            .unwrap();
        assert_eq!(value, 100);

        // out of bounds load
        let program = [
            0x01400093, // addi x1, x0, 20
            0x0000a103, // lw x2, 0(x1)
        ];
        let mut cpu = Cpu::new(&program, 0, 0, 16).unwrap();
        // For testing
        cpu.register_file.write(RegisterFile::STACK_PTR, 0);
        assert!(
            cpu.run_test(program.len()).unwrap_err()
                == CpuStepError::DataMemory(AddrError::OutOfRange)
        );
    }

    #[test]
    fn exit() {
        let program = [
            0x05d00893, // addi a7, x0, 93
            0x00900513, // addi a0, x0, 9
            0x00000073, // ecall
        ];
        let mut cpu = Cpu::new(&program, 0, 0, 1 << 12).unwrap();
        let result = cpu.run_test(program.len()).unwrap();

        assert_eq!(result, ExecutionResult::Exit(9));
    }

    #[test]
    fn file_write() {
        use std::io::Write;

        let program = [
            0x03f00893, // addi a7, x0, 63
            0x00000593, // addi, a1, x0, 0
            0x00500613, // addi a2, x0, 5
            0x00000073, // ecall
        ];
        let s = b"hello";

        let (reader, mut writer) = std::io::pipe().unwrap();
        writer.write_all(s).unwrap();

        let mut cpu = Cpu::new(&program, 0, 0, 1 << 12).unwrap();
        // addi a0, x0, <fd>
        cpu.register_file
            .write(RegisterFile::A0, reader.as_raw_fd() as _);
        cpu.run_test(program.len()).unwrap();

        let read_count = cpu.register_file.read(RegisterFile::A0);
        assert_eq!(read_count, s.len() as _);

        let mem = cpu.data_memory.bytes(0, s.len()).unwrap();
        assert_eq!(mem, b"hello");
    }

    #[test]
    fn cpu_creation_and_stack_init() {
        // ram_size must be a multiple of 16
        assert!(matches!(
            Cpu::from_flat_binary(&[], 18),
            Err(CpuCreationError::RamMisalign)
        ));

        let ram_size = 1 << 12;
        let cpu = Cpu::from_flat_binary(&[], ram_size).unwrap();
        assert_eq!(cpu.register_file.read(RegisterFile::STACK_PTR), ram_size);
        assert_eq!(cpu.pc, 0);
    }

    #[test]
    fn rv64_word_and_shift_operations() {
        let program = [
            0xfff00093, // addi x1, x0, -1
            0x0010811b, // addiw x2, x1, 1 -> 0
            0x800001b7, // lui x3, 0x80000 -> 0xffffffff_80000000
            0xfff1821b, // addiw x4, x3, -1 -> 0x00000000_7fffffff (32-bit wrap + sign-ext)
            0xff000293, // addi x5, x0, -16
            0x00200313, // addi x6, x0, 2
            0x0062d3bb, // srlw x7, x5, x6 -> 0x3ffffffc
            0x4062d43b, // sraw x8, x5, x6 -> -4 (0xffffffff_fffffffc)
            0x0062a4b3, // slt x9, x5, x6 -> 1 (signed: -16 < 2)
            0x0062b533, // sltu x10, x5, x6 -> 0 (unsigned: 0xfff..f0 > 2)
        ];

        let mut cpu = Cpu::new(&program, 0, 0, 1 << 12).unwrap();
        cpu.run_test(program.len()).unwrap();

        assert_eq!(cpu.register_file.read(2), 0);
        assert_eq!(cpu.register_file.read(4), 0x7fffffff);
        assert_eq!(cpu.register_file.read(7), 0x3ffffffc);
        assert_eq!(cpu.register_file.read(8), (-4i64).cast_unsigned());
        assert_eq!(cpu.register_file.read(9), 1);
        assert_eq!(cpu.register_file.read(10), 0);
    }

    #[test]
    fn upper_immediates() {
        let program = [
            0x123450b7, // lui x1, 0x12345 -> 0x12345000
            0x80000137, // lui x2, 0x80000 -> 0xffffffff_80000000
            0x00001197, // auipc x3, 0x1 -> pc (8) + 0x1000 = 0x1008
        ];

        let mut cpu = Cpu::new(&program, 0, 0, 1 << 12).unwrap();
        cpu.run_test(program.len()).unwrap();

        assert_eq!(cpu.register_file.read(1), 0x12345000);
        assert_eq!(cpu.register_file.read(2), 0xffffffff_80000000);
        assert_eq!(cpu.register_file.read(3), 0x1008);
    }

    #[test]
    fn branches() {
        let program = [
            0xfff00093, // 0x00: addi x1, x0, -1
            0x00100113, // 0x04: addi x2, x0, 1
            0x0020e663, // 0x08: bltu x1, x2, +12 (not taken: -1u > 1u)
            0x0020c463, // 0x0c: blt x1, x2, +8 (taken: -1 < 1 -> jumps to 0x14)
            0x06300193, // 0x10: addi x3, x0, 99 (skipped)
            0x02a00213, // 0x14: addi x4, x0, 42 (executed)
        ];

        let mut cpu = Cpu::new(&program, 0, 0, 1 << 12).unwrap();
        cpu.run_test(5).unwrap();

        assert_eq!(cpu.register_file.read(3), 0);
        assert_eq!(cpu.register_file.read(4), 42);
    }

    #[test]
    fn jumps_jal_and_jalr() {
        let program = [
            0x008000ef, // 0x00: jal x1, +8 (x1 = 0x04, jumps to 0x08)
            0x06300113, // 0x04: addi x2, x0, 99 (skipped)
            0x01400193, // 0x08: addi x3, x0, 20 (x3 = 20)
            0x001181e7, // 0x0c: jalr x3, 1(x3) (target = (20+1) & !1 = 0x14, x3 = 0x10)
            0x06300213, // 0x10: addi x4, x0, 99 (skipped)
            0x00700293, // 0x14: addi x5, x0, 7 (executed)
        ];

        let mut cpu = Cpu::new(&program, 0, 0, 1 << 12).unwrap();
        cpu.run_test(4).unwrap();

        assert_eq!(cpu.register_file.read(1), 0x04);
        assert_eq!(cpu.register_file.read(3), 0x10);
        assert_eq!(cpu.register_file.read(4), 0);
        assert_eq!(cpu.register_file.read(5), 7);
    }

    #[test]
    fn subword_loads_and_stores() {
        let program = [
            0x08000093, // addi x1, x0, 0x80
            0x00100023, // sb x1, 0(x0)
            0x00000103, // lb x2, 0(x0) -> sign-extended (-128)
            0x00004183, // lbu x3, 0(x0) -> zero-extended (128)
        ];

        let mut cpu = Cpu::new(&program, 0, 0, 1 << 12).unwrap();
        cpu.run_test(program.len()).unwrap();

        assert_eq!(cpu.register_file.read(2), (-128i64).cast_unsigned());
        assert_eq!(cpu.register_file.read(3), 0x80);
    }

    #[test]
    fn ebreak_and_unsupported_syscall() {
        let bp_program = [0x00100073]; // ebreak
        let mut cpu = Cpu::new(&bp_program, 0, 0, 1 << 12).unwrap();
        assert_eq!(cpu.run().unwrap(), ExecutionResult::Breakpoint);

        let bad_sys_program = [
            0x3e700893, // addi a7, x0, 999
            0x00000073, // ecall
        ];
        let mut cpu = Cpu::new(&bad_sys_program, 0, 0, 1 << 12).unwrap();
        assert_eq!(
            cpu.run().unwrap_err(),
            CpuStepError::UnsupportedSyscall(999)
        );
    }

    #[test]
    fn syscalls_write_and_getpid() {
        use std::io::Read;

        let write_program = [
            0x04000893, // addi a7, x0, 64 (write)
            0x00000593, // addi a1, x0, 0 (buf = 0)
            0x00500613, // addi a2, x0, 5 (count = 5)
            0x00000073, // ecall
        ];

        let (mut reader, writer) = std::io::pipe().unwrap();
        let mut cpu = Cpu::new(&write_program, 0, 0, 1 << 12).unwrap();
        cpu.data_memory
            .bytes_mut(0, 5)
            .unwrap()
            .copy_from_slice(b"world");
        cpu.register_file
            .write(RegisterFile::A0, writer.as_raw_fd() as _);

        cpu.run_test(write_program.len()).unwrap();
        drop(writer);

        assert_eq!(cpu.register_file.read(RegisterFile::A0), 5);
        let mut out = [0u8; 5];
        reader.read_exact(&mut out).unwrap();
        assert_eq!(&out, b"world");

        let pid_program = [
            0x0ac00893, // addi a7, x0, 172 (getpid)
            0x00000073, // ecall
        ];
        let mut cpu = Cpu::new(&pid_program, 0, 0, 1 << 12).unwrap();
        cpu.run_test(pid_program.len()).unwrap();
        assert_eq!(
            cpu.register_file.read(RegisterFile::A0),
            std::process::id() as u64
        );
    }

    #[test]
    fn non_zero_virtual_address_and_elf() {
        // Non-zero base virtual address
        let program = [
            0x00000097, // 0x1000: auipc x1, 0 -> 0x1000
            0x05d00893, // 0x1004: addi a7, x0, 93
            0x00700513, // 0x1008: addi a0, x0, 7
            0x00000073, // 0x100c: ecall
        ];
        let mut cpu = Cpu::new(&program, 0x1000, 0x1000, 1 << 12).unwrap();
        assert_eq!(cpu.run().unwrap(), ExecutionResult::Exit(7));
        assert_eq!(cpu.register_file.read(1), 0x1000);

        // Reject invalid ELF header
        assert!(matches!(
            Cpu::from_elf(b"not an elf", 1 << 12),
            Err(CpuCreationError::InvalidElf)
        ));

        // Minimal valid 64-bit RISC-V ELF executable loaded at 0x1000
        let code: Vec<u8> = [
            0x02a00513u32, // addi a0, x0, 42
            0x05d00893,    // addi a7, x0, 93
            0x00000073,    // ecall
        ]
        .iter()
        .flat_map(|w| w.to_le_bytes())
        .collect();

        // Header + program header + code
        let mut elf = vec![0u8; 64 + 56 + code.len()];
        elf[0..4].copy_from_slice(b"\x7fELF");
        elf[4] = 2; // ELFCLASS64
        elf[5] = 1; // ELFDATA2LSB
        elf[0x10..0x12].copy_from_slice(&2u16.to_le_bytes()); // ET_EXEC
        elf[0x12..0x14].copy_from_slice(&0xf3u16.to_le_bytes()); // EM_RISCV
        elf[0x18..0x20].copy_from_slice(&0x1000u64.to_le_bytes()); // e_entry
        elf[0x20..0x28].copy_from_slice(&64u64.to_le_bytes()); // e_phoff
        elf[0x36..0x38].copy_from_slice(&56u16.to_le_bytes()); // e_phentsize
        elf[0x38..0x3a].copy_from_slice(&1u16.to_le_bytes()); // e_phnum

        // Program header (PT_LOAD, PF_R | PF_X)
        let ph = &mut elf[64..120];
        ph[0x00..0x04].copy_from_slice(&1u32.to_le_bytes()); // PT_LOAD
        ph[0x04..0x08].copy_from_slice(&5u32.to_le_bytes()); // PF_R | PF_X
        ph[0x08..0x10].copy_from_slice(&120u64.to_le_bytes()); // p_offset
        ph[0x10..0x18].copy_from_slice(&0x1000u64.to_le_bytes()); // p_vaddr
        ph[0x20..0x28].copy_from_slice(&(code.len() as u64).to_le_bytes()); // p_filesz
        ph[0x28..0x30].copy_from_slice(&(code.len() as u64).to_le_bytes()); // p_memsz

        elf[120..].copy_from_slice(&code);

        let mut cpu = Cpu::from_elf(&elf, 1 << 16).unwrap();
        assert_eq!(cpu.run().unwrap(), ExecutionResult::Exit(42));
    }
}
