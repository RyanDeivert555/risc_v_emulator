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

impl From<FetchError> for CpuStepError {
    fn from(value: FetchError) -> Self {
        CpuStepError::InstructionMemory(value)
    }
}

impl From<AddrError> for CpuStepError {
    fn from(value: AddrError) -> Self {
        CpuStepError::DataMemory(value)
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
    InstuctionLoad,
}

impl From<InstructionLoadError> for CpuCreationError {
    fn from(_: InstructionLoadError) -> Self {
        Self::InstuctionLoad
    }
}

fn alu(lhs: u64, rhs: u64, mode: AluOp, width: AluWidth) -> u64 {
    use AluOp as Op;

    match (mode, width) {
        (Op::Add, AluWidth::DoubleWord) => lhs.wrapping_add(rhs),
        (Op::Add, AluWidth::Word) => lhs.wrapping_add(rhs) as i32 as i64 as u64,
        (Op::Sub, AluWidth::DoubleWord) => lhs.wrapping_sub(rhs),
        (Op::Sub, AluWidth::Word) => lhs.wrapping_sub(rhs) as i32 as i64 as u64,
        (Op::Xor, _) => lhs ^ rhs,
        (Op::Or, _) => lhs | rhs,
        (Op::And, _) => lhs & rhs,
        (Op::Sll, AluWidth::DoubleWord) => lhs.wrapping_shl(rhs as _),
        (Op::Sll, AluWidth::Word) => (lhs as u32).wrapping_shl(rhs as _) as i32 as i64 as u64,
        (Op::Srl, AluWidth::DoubleWord) => lhs.wrapping_shr(rhs as _),
        (Op::Srl, AluWidth::Word) => (lhs as u32).wrapping_shr(rhs as _) as i32 as i64 as u64,
        (Op::Sra, AluWidth::DoubleWord) => lhs.cast_signed().wrapping_shr(rhs as _).cast_unsigned(),
        (Op::Sra, AluWidth::Word) => (lhs as u32).wrapping_shr(rhs as _) as i32 as i64 as u64,
        (Op::Slt, _) => u64::from(lhs.cast_signed() < rhs.cast_signed()),
        (Op::Sltu, _) => u64::from(lhs < rhs),
    }
}

impl Cpu {
    pub fn new(instructions: &[u32], ram_size: u64) -> Result<Self, CpuCreationError> {
        let bytes = {
            let ptr = instructions.as_ptr().cast();
            let len = std::mem::size_of_val(instructions);

            // SAFETY: pointer is properly aligned with a correct length
            unsafe { std::slice::from_raw_parts(ptr, len) }
        };

        Self::from_bytes(bytes, ram_size)
    }

    pub fn from_bytes(bytes: &[u8], ram_size: u64) -> Result<Self, CpuCreationError> {
        if !ram_size.is_multiple_of(16) {
            Err(CpuCreationError::RamMisalign)
        } else {
            let mut register_file = RegisterFile::new();
            register_file.write(RegisterFile::STACK_PTR, ram_size);

            Ok(Self {
                instruction_memory: InstructionMemory::from_bytes(bytes)?,
                register_file,
                data_memory: DataMemory::new(ram_size),
                pc: 0,
            })
        }
    }

    fn syscall(&mut self) -> Result<ExecutionResult, CpuStepError> {
        let sys = self.register_file.read(RegisterFile::A7);

        // https://jborza.com/post/2021-05-11-riscv-linux-syscalls
        match sys {
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
                let buffer = {
                    let addr = self.register_file.read(RegisterFile::A1);
                    let count = self.register_file.read(RegisterFile::A2) as usize;

                    self.data_memory.bytes_mut(addr, count)?
                };

                let result = unsafe { libc::read(fd, buffer.as_mut_ptr().cast(), buffer.len()) };
                self.register_file.write(RegisterFile::A0, result as _);

                Ok(ExecutionResult::Continue)
            }
            64 => {
                let fd = self.register_file.read(RegisterFile::A0) as libc::c_int;
                let buffer = {
                    let addr = self.register_file.read(RegisterFile::A1);
                    let count = self.register_file.read(RegisterFile::A2) as usize;

                    self.data_memory.bytes(addr, count)?
                };

                let result = unsafe { libc::write(fd, buffer.as_ptr().cast(), buffer.len()) };
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
                    let addr = self.register_file.read(RegisterFile::A0);
                    let raw = self.data_memory.raw_storage::<libc::sockaddr>(addr)?;

                    raw.as_ptr().cast()
                };
                let addrlen = self.register_file.read(RegisterFile::A1) as libc::socklen_t;

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
        let mut cpu = Cpu::new(&program, 1 << 12).unwrap();
        cpu.run().unwrap();
    }

    #[test]
    fn arithmetic() {
        let program = [
            0x00500093, // addi x1, x0, 5
            0x00a00113, // addi x2, x0, 10
            0x002080b3, // add x1, x1, x2
        ];
        let mut cpu = Cpu::new(&program, 1 << 12).unwrap();
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
        let mut cpu = Cpu::new(&program, 1 << 12).unwrap();
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
        let mut cpu = Cpu::new(&program, 1 << 12).unwrap();
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
        let mut cpu = Cpu::new(&program, 1 << 12).unwrap();
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
        let mut cpu = Cpu::new(&program, 1 << 12).unwrap();
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
        let mut cpu = Cpu::new(&program, 16).unwrap();
        // For testing
        cpu.register_file.write(RegisterFile::STACK_PTR, 0);
        assert!(
            cpu.run_test(program.len()).unwrap_err()
                == CpuStepError::DataMemory(AddrError::OutOfRange)
        );
    }

    #[test]
    fn syscall() {
        use std::io::Write;

        let program = [
            0x05d00893, // addi a7, x0, 93
            0x00900513, // addi a0, x0, 9
            0x00000073, // ecall
        ];
        let mut cpu = Cpu::new(&program, 1 << 12).unwrap();
        let result = cpu.run_test(program.len()).unwrap();

        assert_eq!(result, ExecutionResult::Exit(9));

        let program = [
            0x03f00893, // addi a7, x0, 63
            0x00000593, // addi, a1, x0, 0
            0x00500613, // addi a2, x0, 5
            0x00000073, // ecall
        ];
        let s = b"hello";

        let (reader, mut writer) = std::io::pipe().unwrap();
        writer.write_all(s).unwrap();

        let mut cpu = Cpu::new(&program, 1 << 12).unwrap();
        // addi a0, x0, <fd>
        cpu.register_file
            .write(RegisterFile::A0, reader.as_raw_fd() as _);
        cpu.run_test(program.len()).unwrap();

        let read_count = cpu.register_file.read(RegisterFile::A0);
        assert_eq!(read_count, s.len() as _);

        let mem = cpu.data_memory.bytes(0, s.len()).unwrap();
        assert_eq!(mem, b"hello");
    }
}
