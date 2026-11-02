use crate::data_memory::{DataSize, SignExtend};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Condition {
    Eq,
    Ne,
    Lt,
    Ge,
    Ltu,
    Geu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AluOp {
    Add,
    Sub,
    Xor,
    Or,
    And,
    Sll,
    Srl,
    Sra,
    Slt,
    Sltu,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AluWidth {
    Word,
    DoubleWord,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Instruction {
    AluRType {
        rd: u8,
        rs1: u8,
        rs2: u8,
        alu_op: AluOp,
        alu_width: AluWidth,
    },
    AluIType {
        rd: u8,
        rs1: u8,
        imm: i64,
        alu_op: AluOp,
        alu_width: AluWidth,
    },
    Load {
        rd: u8,
        rs1: u8,
        imm: i64,
        size: DataSize,
        extend: SignExtend,
    },
    Store {
        rs1: u8,
        rs2: u8,
        imm: i64,
        size: DataSize,
    },
    Branch {
        rs1: u8,
        rs2: u8,
        imm: i64,
        cond: Condition,
    },
    Jal {
        rd: u8,
        imm: i64,
    },
    Jalr {
        rd: u8,
        rs1: u8,
        imm: i64,
    },
    Lui {
        rd: u8,
        imm: i64,
    },
    Auipc {
        rd: u8,
        imm: i64,
    },
    Fence,
    ECall,
    EBreak,
}

fn bits(value: u32, start: u32, end: u32) -> u32 {
    let mask = ((1 << (end - start)) - 1) as u32;

    (value >> start) & mask
}

fn sign_extend(value: u32, width: u32) -> i64 {
    let shift = u32::BITS - width;

    ((value << shift).cast_signed() >> shift) as _
}

const F7_START: u32 = 25;
const F7_END: u32 = 32;

const RS2_START: u32 = 20;
const RS2_END: u32 = 25;

const RS1_START: u32 = 15;
const RS1_END: u32 = 20;

const F3_START: u32 = 12;
const F3_END: u32 = 15;

const RD_START: u32 = 7;
const RD_END: u32 = 12;

const OP_START: u32 = 0;
const OP_END: u32 = 7;

fn f7(value: u32) -> u8 {
    bits(value, F7_START, F7_END) as u8
}

fn rs2(value: u32) -> u8 {
    bits(value, RS2_START, RS2_END) as u8
}

fn rs1(value: u32) -> u8 {
    bits(value, RS1_START, RS1_END) as u8
}

fn f3(value: u32) -> u8 {
    bits(value, F3_START, F3_END) as u8
}

fn rd(value: u32) -> u8 {
    bits(value, RD_START, RD_END) as u8
}

fn op(value: u32) -> u8 {
    bits(value, OP_START, OP_END) as u8
}

fn i_type_imm(value: u32) -> i64 {
    let imm11_to_0 = bits(value, 20, 32);

    sign_extend(imm11_to_0, 12)
}

fn s_type_imm(value: u32) -> i64 {
    let imm11_to_5 = bits(value, 25, 32);
    let imm4_to_0 = bits(value, 7, 12);

    sign_extend((imm11_to_5 << 5) | imm4_to_0, 12)
}

fn b_type_imm(value: u32) -> i64 {
    let imm12 = bits(value, 31, 32);
    let imm10_to_5 = bits(value, 25, 31);
    let imm4_to_1 = bits(value, 8, 12);
    let imm11 = bits(value, 7, 8);

    let imm = (imm12 << 12) | (imm11 << 11) | (imm10_to_5 << 5) | (imm4_to_1 << 1);

    sign_extend(imm, 13)
}

fn u_type_imm(value: u32) -> i64 {
    // Sign extend with casting
    (value & 0xFFFF_F000) as i32 as i64
}

fn j_type_imm(value: u32) -> i64 {
    let imm20 = bits(value, 31, 32);
    let imm10_to_1 = bits(value, 21, 31);
    let imm11 = bits(value, 20, 21);
    let imm19_to_12 = bits(value, 12, 20);

    let imm = (imm20 << 20) | (imm19_to_12 << 12) | (imm11 << 11) | (imm10_to_1 << 1);

    sign_extend(imm, 21)
}

impl Instruction {
    pub fn new(instr: u32) -> Self {
        let f7 = f7(instr);
        let f3 = f3(instr);
        let op = op(instr);

        // https://book.rvemu.app/instruction-set/01-rv64i.html
        match (f7, f3, op) {
            // R-Type
            (0x00, 0x0, 0b0110011) => Instruction::r_type(instr, AluOp::Add, AluWidth::DoubleWord),
            (0x20, 0x0, 0b0110011) => Instruction::r_type(instr, AluOp::Sub, AluWidth::DoubleWord),
            (0x00, 0x4, 0b0110011) => Instruction::r_type(instr, AluOp::Xor, AluWidth::DoubleWord),
            (0x00, 0x6, 0b0110011) => Instruction::r_type(instr, AluOp::Or, AluWidth::DoubleWord),
            (0x00, 0x7, 0b0110011) => Instruction::r_type(instr, AluOp::And, AluWidth::DoubleWord),
            (0x00, 0x1, 0b0110011) => Instruction::r_type(instr, AluOp::Sll, AluWidth::DoubleWord),
            (0x00, 0x5, 0b0110011) => Instruction::r_type(instr, AluOp::Srl, AluWidth::DoubleWord),
            (0x20, 0x5, 0b0110011) => Instruction::r_type(instr, AluOp::Sra, AluWidth::DoubleWord),
            (0x00, 0x2, 0b0110011) => Instruction::r_type(instr, AluOp::Slt, AluWidth::DoubleWord),
            (0x00, 0x3, 0b0110011) => Instruction::r_type(instr, AluOp::Sltu, AluWidth::DoubleWord),
            // Word
            (0x00, 0x0, 0b0111011) => Instruction::r_type(instr, AluOp::Add, AluWidth::Word),
            (0x20, 0x0, 0b0111011) => Instruction::r_type(instr, AluOp::Sub, AluWidth::Word),
            (0x00, 0x1, 0b0111011) => Instruction::r_type(instr, AluOp::Sll, AluWidth::Word),
            (0x00, 0x5, 0b0111011) => Instruction::r_type(instr, AluOp::Srl, AluWidth::Word),
            (0x20, 0x5, 0b0111011) => Instruction::r_type(instr, AluOp::Sra, AluWidth::Word),
            // I-Type
            (_, 0x0, 0b0010011) => Instruction::i_type(instr, AluOp::Add, AluWidth::DoubleWord),
            (_, 0x4, 0b0010011) => Instruction::i_type(instr, AluOp::Xor, AluWidth::DoubleWord),
            (_, 0x6, 0b0010011) => Instruction::i_type(instr, AluOp::Or, AluWidth::DoubleWord),
            (_, 0x7, 0b0010011) => Instruction::i_type(instr, AluOp::And, AluWidth::DoubleWord),
            (f7, 0x1, 0b0010011) if f7 >> 1 == 0x00 => {
                Instruction::i_type(instr, AluOp::Sll, AluWidth::DoubleWord)
            }
            (f7, 0x5, 0b0010011) if f7 >> 1 == 0x00 => {
                Instruction::i_type(instr, AluOp::Srl, AluWidth::DoubleWord)
            }
            (f7, 0x5, 0b0010011) if f7 >> 1 == 0x10 => {
                Instruction::i_type(instr, AluOp::Sra, AluWidth::DoubleWord)
            }
            (_, 0x2, 0b0010011) => Instruction::i_type(instr, AluOp::Slt, AluWidth::DoubleWord),
            (_, 0x3, 0b0010011) => Instruction::i_type(instr, AluOp::Sltu, AluWidth::DoubleWord),
            // Word
            (_, 0x0, 0b0011011) => Instruction::i_type(instr, AluOp::Add, AluWidth::Word),
            (0x00, 0x1, 0b0011011) => Instruction::i_type(instr, AluOp::Sll, AluWidth::Word),
            (0x00, 0x5, 0b0011011) => Instruction::i_type(instr, AluOp::Srl, AluWidth::Word),
            (0x20, 0x5, 0b0011011) => Instruction::i_type(instr, AluOp::Sra, AluWidth::Word),
            // Load
            (_, 0x0, 0b0000011) => Instruction::load(instr, DataSize::Byte, SignExtend::Msb),
            (_, 0x1, 0b0000011) => Instruction::load(instr, DataSize::HalfWord, SignExtend::Msb),
            (_, 0x2, 0b0000011) => Instruction::load(instr, DataSize::Word, SignExtend::Msb),
            (_, 0x3, 0b0000011) => Instruction::load(instr, DataSize::DoubleWord, SignExtend::Zero),
            (_, 0x4, 0b0000011) => Instruction::load(instr, DataSize::Byte, SignExtend::Zero),
            (_, 0x5, 0b0000011) => Instruction::load(instr, DataSize::HalfWord, SignExtend::Zero),
            (_, 0x6, 0b0000011) => Instruction::load(instr, DataSize::Word, SignExtend::Zero),
            // Store
            (_, 0x0, 0b0100011) => Instruction::store(instr, DataSize::Byte),
            (_, 0x1, 0b0100011) => Instruction::store(instr, DataSize::HalfWord),
            (_, 0x2, 0b0100011) => Instruction::store(instr, DataSize::Word),
            (_, 0x3, 0b0100011) => Instruction::store(instr, DataSize::DoubleWord),
            // Branch
            (_, 0x0, 0b1100011) => Instruction::branch(instr, Condition::Eq),
            (_, 0x1, 0b1100011) => Instruction::branch(instr, Condition::Ne),
            (_, 0x4, 0b1100011) => Instruction::branch(instr, Condition::Lt),
            (_, 0x5, 0b1100011) => Instruction::branch(instr, Condition::Ge),
            (_, 0x6, 0b1100011) => Instruction::branch(instr, Condition::Ltu),
            (_, 0x7, 0b1100011) => Instruction::branch(instr, Condition::Geu),
            // jal
            (_, _, 0b1101111) => Instruction::jal(instr),
            // jalr
            (_, 0x0, 0b1100111) => Instruction::jalr(instr),
            // lui
            (_, _, 0b0110111) => Instruction::lui(instr),
            (_, _, 0b0010111) => Instruction::auipc(instr),
            // fence
            (_, 0x0, 0b0001111) => Instruction::fence(),
            // ecall
            _ if instr == 0x00000073 => Instruction::ecall(),
            // ebreak
            _ if instr == 0x00100073 => Instruction::ebreak(),

            (f7, f3, op) => unimplemented!("0x{:x} 0x{:x} 0b{:b}", f7, f3, op),
        }
    }

    #[allow(unused)]
    pub fn from_bytes(bytes: [u8; 4]) -> Self {
        Self::new(u32::from_le_bytes(bytes))
    }

    fn r_type(instruction: u32, alu_op: AluOp, alu_width: AluWidth) -> Self {
        Self::AluRType {
            rd: rd(instruction),
            rs1: rs1(instruction),
            rs2: rs2(instruction),
            alu_op,
            alu_width,
        }
    }

    fn i_type(instruction: u32, alu_op: AluOp, alu_width: AluWidth) -> Self {
        Self::AluIType {
            rd: rd(instruction),
            rs1: rs1(instruction),
            imm: i_type_imm(instruction),
            alu_op,
            alu_width,
        }
    }

    fn load(instruction: u32, size: DataSize, extend: SignExtend) -> Self {
        Self::Load {
            rd: rd(instruction),
            rs1: rs1(instruction),
            imm: i_type_imm(instruction),
            size,
            extend,
        }
    }

    fn store(instruction: u32, size: DataSize) -> Self {
        Self::Store {
            rs1: rs1(instruction),
            rs2: rs2(instruction),
            imm: s_type_imm(instruction),
            size,
        }
    }

    fn branch(instruction: u32, cond: Condition) -> Self {
        Self::Branch {
            rs1: rs1(instruction),
            rs2: rs2(instruction),
            imm: b_type_imm(instruction),
            cond,
        }
    }

    fn jal(instruction: u32) -> Self {
        Self::Jal {
            rd: rd(instruction),
            imm: j_type_imm(instruction),
        }
    }

    fn jalr(instruction: u32) -> Self {
        Self::Jalr {
            rd: rd(instruction),
            rs1: rs1(instruction),
            imm: i_type_imm(instruction),
        }
    }

    fn lui(instruction: u32) -> Self {
        Self::Lui {
            rd: rd(instruction),
            imm: u_type_imm(instruction),
        }
    }

    fn auipc(instruction: u32) -> Self {
        Self::Auipc {
            rd: rd(instruction),
            imm: u_type_imm(instruction),
        }
    }

    fn fence() -> Self {
        Self::Fence
    }

    fn ecall() -> Self {
        Self::ECall
    }

    fn ebreak() -> Self {
        Self::EBreak
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum FetchError {
    Misaligned,
    OutOfRange,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct InstructionLoadError;

pub struct InstructionMemory {
    instructions: Box<[u32]>,
}

impl InstructionMemory {
    #[allow(unused)]
    pub fn new(instructions: &[u32]) -> Self {
        Self {
            instructions: Box::from(instructions),
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, InstructionLoadError> {
        let (instructions, leftover) = bytes.as_chunks::<4>();
        if leftover.is_empty() {
            let instructions = instructions
                .iter()
                .map(|b| u32::from_le_bytes(*b))
                .collect();

            Ok(Self { instructions })
        } else {
            Err(InstructionLoadError)
        }
    }

    pub fn fetch(&self, pc: u64) -> Result<Instruction, FetchError> {
        if !pc.is_multiple_of(4) {
            Err(FetchError::Misaligned)
        } else {
            self.instructions
                .get((pc / 4) as usize)
                .map(|instr| Instruction::new(*instr))
                .ok_or(FetchError::OutOfRange)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn creation() {
        let instr = Instruction::new(0b0100000_00011_00010_000_00001_0110011); // sub x1, x2, x3
        assert_eq!(
            instr,
            Instruction::AluRType {
                rd: 1,
                rs1: 2,
                rs2: 3,
                alu_op: AluOp::Sub,
                alu_width: AluWidth::DoubleWord,
            }
        );

        let instr = Instruction::new(0b0000100_00010_00100_010_00000_0100011); // sw x2, 128(x4)
        assert_eq!(
            instr,
            Instruction::Store {
                rs1: 4,
                rs2: 2,
                imm: 128,
                size: DataSize::Word,
            }
        );
    }
}
