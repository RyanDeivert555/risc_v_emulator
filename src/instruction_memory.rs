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
    bits(value, F7_START, F7_END) as _
}

fn rs2(value: u32) -> u8 {
    bits(value, RS2_START, RS2_END) as _
}

fn rs1(value: u32) -> u8 {
    bits(value, RS1_START, RS1_END) as _
}

fn f3(value: u32) -> u8 {
    bits(value, F3_START, F3_END) as _
}

fn rd(value: u32) -> u8 {
    bits(value, RD_START, RD_END) as _
}

fn op(value: u32) -> u8 {
    bits(value, OP_START, OP_END) as _
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
    fn decode_r_type_all() {
        let cases = [
            (0x007302b3, AluOp::Add, AluWidth::DoubleWord), // add x5, x6, x7
            (0x407302b3, AluOp::Sub, AluWidth::DoubleWord), // sub x5, x6, x7
            (0x007342b3, AluOp::Xor, AluWidth::DoubleWord), // xor x5, x6, x7
            (0x007362b3, AluOp::Or, AluWidth::DoubleWord),  // or x5, x6, x7
            (0x007372b3, AluOp::And, AluWidth::DoubleWord), // and x5, x6, x7
            (0x007312b3, AluOp::Sll, AluWidth::DoubleWord), // sll x5, x6, x7
            (0x007352b3, AluOp::Srl, AluWidth::DoubleWord), // srl x5, x6, x7
            (0x407352b3, AluOp::Sra, AluWidth::DoubleWord), // sra x5, x6, x7
            (0x007322b3, AluOp::Slt, AluWidth::DoubleWord), // slt x5, x6, x7
            (0x007332b3, AluOp::Sltu, AluWidth::DoubleWord), // sltu x5, x6, x7
            (0x007302bb, AluOp::Add, AluWidth::Word),       // addw x5, x6, x7
            (0x407302bb, AluOp::Sub, AluWidth::Word),       // subw x5, x6, x7
            (0x007312bb, AluOp::Sll, AluWidth::Word),       // sllw x5, x6, x7
            (0x007352bb, AluOp::Srl, AluWidth::Word),       // srlw x5, x6, x7
            (0x407352bb, AluOp::Sra, AluWidth::Word),       // sraw x5, x6, x7
        ];

        for (raw, alu_op, alu_width) in cases {
            assert_eq!(
                Instruction::new(raw),
                Instruction::AluRType {
                    rd: 5,
                    rs1: 6,
                    rs2: 7,
                    alu_op,
                    alu_width,
                }
            );
        }
    }

    #[test]
    fn decode_i_type_and_shifts() {
        // Negative 12-bit immediate (-2048)
        assert_eq!(
            Instruction::new(0x80010093), // addi x1, x2, -2048
            Instruction::AluIType {
                rd: 1,
                rs1: 2,
                imm: -2048,
                alu_op: AluOp::Add,
                alu_width: AluWidth::DoubleWord,
            }
        );

        // RV64 6-bit shamt (shamt = 63 sets bit 25, making f7 = 0x01 / 0x21)
        assert_eq!(
            Instruction::new(0x03f11093), // slli x1, x2, 63
            Instruction::AluIType {
                rd: 1,
                rs1: 2,
                imm: 63,
                alu_op: AluOp::Sll,
                alu_width: AluWidth::DoubleWord,
            }
        );
        assert_eq!(
            Instruction::new(0x03f15093), // srli x1, x2, 63
            Instruction::AluIType {
                rd: 1,
                rs1: 2,
                imm: 63,
                alu_op: AluOp::Srl,
                alu_width: AluWidth::DoubleWord,
            }
        );
        assert_eq!(
            Instruction::new(0x43f15093), // srai x1, x2, 63
            Instruction::AluIType {
                rd: 1,
                rs1: 2,
                imm: 0x43f, // upper bits encode SRA; ALU masks shamt
                alu_op: AluOp::Sra,
                alu_width: AluWidth::DoubleWord,
            }
        );

        // RV64I 32-bit word shifts (5-bit shamt = 31)
        assert_eq!(
            Instruction::new(0x01f1109b), // slliw x1, x2, 31
            Instruction::AluIType {
                rd: 1,
                rs1: 2,
                imm: 31,
                alu_op: AluOp::Sll,
                alu_width: AluWidth::Word,
            }
        );
        assert_eq!(
            Instruction::new(0x01f1509b), // srliw x1, x2, 31
            Instruction::AluIType {
                rd: 1,
                rs1: 2,
                imm: 31,
                alu_op: AluOp::Srl,
                alu_width: AluWidth::Word,
            }
        );
        assert_eq!(
            Instruction::new(0x41f1509b), // sraiw x1, x2, 31
            Instruction::AluIType {
                rd: 1,
                rs1: 2,
                imm: 0x41f,
                alu_op: AluOp::Sra,
                alu_width: AluWidth::Word,
            }
        );
    }

    #[test]
    fn decode_loads_and_stores() {
        let loads = [
            (0xffc10083, DataSize::Byte, SignExtend::Msb), // lb x1, -4(x2)
            (0xffc11083, DataSize::HalfWord, SignExtend::Msb), // lh x1, -4(x2)
            (0xffc12083, DataSize::Word, SignExtend::Msb), // lw x1, -4(x2)
            (0xffc13083, DataSize::DoubleWord, SignExtend::Zero), // ld x1, -4(x2)
            (0xffc14083, DataSize::Byte, SignExtend::Zero), // lbu x1, -4(x2)
            (0xffc15083, DataSize::HalfWord, SignExtend::Zero), // lhu x1, -4(x2)
            (0xffc16083, DataSize::Word, SignExtend::Zero), // lwu x1, -4(x2)
        ];

        for (raw, size, extend) in loads {
            assert_eq!(
                Instruction::new(raw),
                Instruction::Load {
                    rd: 1,
                    rs1: 2,
                    imm: -4,
                    size,
                    extend,
                }
            );
        }

        let stores = [
            (0xfe220823, DataSize::Byte),       // sb x2, -16(x4)
            (0xfe221823, DataSize::HalfWord),   // sh x2, -16(x4)
            (0xfe223823, DataSize::DoubleWord), // sd x2, -16(x4)
        ];

        for (raw, size) in stores {
            assert_eq!(
                Instruction::new(raw),
                Instruction::Store {
                    rs1: 4,
                    rs2: 2,
                    imm: -16,
                    size,
                }
            );
        }
    }

    #[test]
    fn decode_branches_and_jumps() {
        let branches = [
            (0xfe208ce3, -8, Condition::Eq),    // beq x1, x2, -8
            (0x00209863, 16, Condition::Ne),    // bne x1, x2, 16
            (0x8020c063, -4096, Condition::Lt), // blt x1, x2, -4096 (min B-imm)
            (0x7e20dfe3, 4094, Condition::Ge),  // bge x1, x2, 4094  (max B-imm)
            (0xfe20ece3, -8, Condition::Ltu),   // bltu x1, x2, -8
            (0x0020f463, 8, Condition::Geu),    // bgeu x1, x2, 8
        ];

        for (raw, imm, cond) in branches {
            assert_eq!(
                Instruction::new(raw),
                Instruction::Branch {
                    rs1: 1,
                    rs2: 2,
                    imm,
                    cond,
                }
            );
        }

        // JAL min (-1 MiB) and max (+1 MiB - 2) offsets
        assert_eq!(
            Instruction::new(0x800000ef), // jal x1, -1048576
            Instruction::Jal {
                rd: 1,
                imm: -1_048_576,
            }
        );
        assert_eq!(
            Instruction::new(0x7ffff0ef), // jal x1, 1048574
            Instruction::Jal {
                rd: 1,
                imm: 1_048_574,
            }
        );

        assert_eq!(
            Instruction::new(0xff4100e7), // jalr x1, -12(x2)
            Instruction::Jalr {
                rd: 1,
                rs1: 2,
                imm: -12,
            }
        );
    }

    #[test]
    fn decode_upper_and_system() {
        // Sign-extension of bit 31 in U-type immediates
        assert_eq!(
            Instruction::new(0x800000b7), // lui x1, 0x80000
            Instruction::Lui {
                rd: 1,
                imm: -0x8000_0000,
            }
        );
        assert_eq!(
            Instruction::new(0x12345097), // auipc x1, 0x12345
            Instruction::Auipc {
                rd: 1,
                imm: 0x1234_5000,
            }
        );

        assert_eq!(Instruction::new(0x0000000f), Instruction::Fence);
        assert_eq!(Instruction::new(0x00000073), Instruction::ECall);
        assert_eq!(Instruction::new(0x00100073), Instruction::EBreak);
        assert_eq!(
            Instruction::from_bytes(0x00100073u32.to_le_bytes()),
            Instruction::EBreak
        );
    }

    #[test]
    fn instruction_memory_load_and_fetch() {
        // Rejects byte slices whose length is not a multiple of 4
        assert!(matches!(
            InstructionMemory::from_bytes(&[0x13, 0x00, 0x00]),
            Err(InstructionLoadError)
        ));

        let words = [0x0000000f_u32, 0x00000073, 0x00100073];
        let bytes: Vec<u8> = words.iter().flat_map(|w| w.to_le_bytes()).collect();
        let imem = InstructionMemory::from_bytes(&bytes).unwrap();

        assert_eq!(imem.fetch(0), Ok(Instruction::Fence));
        assert_eq!(imem.fetch(4), Ok(Instruction::ECall));
        assert_eq!(imem.fetch(8), Ok(Instruction::EBreak));

        // Unaligned PC
        assert_eq!(imem.fetch(2), Err(FetchError::Misaligned));
        assert_eq!(imem.fetch(5), Err(FetchError::Misaligned));

        // Out-of-range PC
        assert_eq!(imem.fetch(12), Err(FetchError::OutOfRange));
        assert_eq!(imem.fetch(u64::MAX - 3), Err(FetchError::OutOfRange));
    }
}
