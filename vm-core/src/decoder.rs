use crate::bytecode::*;
use crate::bytefile::Bytefile;
use crate::numeric::LeBytes;

#[derive(Debug, Eq, PartialEq)]
pub enum DecoderError {
    ReadingMoreThenCodeSection,
    InvalidOpcode(u8),
}

impl std::fmt::Display for DecoderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DecoderError::ReadingMoreThenCodeSection => {
                write!(f, "Reading more than code section")
            }
            DecoderError::InvalidOpcode(opcode) => {
                write!(f, "Invalid opcode: {:#x}", opcode)
            }
        }
    }
}

/// Convert a byte, that couldnt be incoded into an interpreter error.
impl From<u8> for DecoderError {
    fn from(opcode: u8) -> Self {
        DecoderError::InvalidOpcode(opcode)
    }
}

impl std::error::Error for DecoderError {}

pub struct Decoder {
    pub bf: Bytefile,
    pub ip: usize,
    pub code_section_len: usize,
}

impl Decoder {
    pub fn new(bf: Bytefile) -> Self {
        let code_section_len = bf.code_section.len();
        let main_offset = bf.main_offset as usize;

        Decoder {
            bf,
            ip: main_offset,
            code_section_len,
        }
    }

    /// Reads the next n bytes from the code section,
    /// where n is the size of type `T`.
    /// Returns the value read as type `T`, where `T` is an integer type.
    pub fn next<T: LeBytes>(&mut self) -> Result<T, DecoderError> {
        if self.ip + std::mem::size_of::<T>() > self.code_section_len {
            return Err(DecoderError::ReadingMoreThenCodeSection);
        }

        let bit_size = std::mem::size_of::<T>();
        let bytes = &self.bf.code_section[self.ip..self.ip + bit_size];

        self.ip += bit_size;

        Ok(T::from_le_bytes(bytes.try_into().unwrap()))
    }

    /// Decode a byte into an instruction
    pub fn decode(&mut self, byte: u8) -> Result<Instruction, DecoderError> {
        let (opcode, subopcode) = (byte & 0xF0, byte & 0x0F);

        match (opcode, subopcode) {
            (0x00, 0x0) => Ok(Instruction::NOP),
            (0x00, _) if (0x1..=0xd).contains(&subopcode) => Ok(Instruction::BINOP {
                op: Op::try_from(subopcode).map_err(|_| DecoderError::from(byte))?,
            }),
            (0x00, _) => Err(DecoderError::from(byte)),
            (0x10, 0x0) => Ok(Instruction::CONST {
                value: self.next::<i32>()?,
            }),
            (0x10, 0x1) => Ok(Instruction::STRING {
                index: self.next::<i32>()?,
            }),
            (0x10, 0x3) => Ok(Instruction::STI),
            (0x10, 0x4) => Ok(Instruction::STA),
            (0x10, 0x5) => Ok(Instruction::JMP {
                offset: self.next::<i32>()?,
            }),
            (0x10, 0x6) => Ok(Instruction::END),
            (0x10, 0x7) => Ok(Instruction::RET),
            (0x10, 0x8) => Ok(Instruction::DROP),
            (0x10, 0x9) => Ok(Instruction::DUP),
            (0x10, 0xa) => Ok(Instruction::SWAP),
            (0x10, 0xb) => Ok(Instruction::ELEM),
            (0x20, _) if subopcode <= 0x3 => Ok(Instruction::LOAD {
                rel: ValueRel::try_from(subopcode).map_err(|_| DecoderError::from(byte))?,
                index: self.next::<i32>()?,
            }),
            (0x40, _) if subopcode <= 0x3 => Ok(Instruction::STORE {
                rel: ValueRel::try_from(subopcode).map_err(|_| DecoderError::from(byte))?,
                index: self.next::<i32>()?,
            }),
            (0x50, 0x0) => Ok(Instruction::CJMP {
                offset: self.next::<i32>()?,
                kind: CompareJumpKind::ISZERO,
            }),
            (0x50, 0x1) => Ok(Instruction::CJMP {
                offset: self.next::<i32>()?,
                kind: CompareJumpKind::ISNONZERO,
            }),
            (0x50, 0x2) => Ok(Instruction::BEGIN {
                args: self.next::<i32>()?,
                locals: self.next::<i32>()?,
            }),
            (0x50, 0x3) => Ok(Instruction::CBEGIN {
                args: self.next::<i32>()?,
                locals: self.next::<i32>()?,
            }),
            (0x50, 0x4) => {
                let offset = self.next::<i32>()?;
                let arity = self.next::<i32>()?;

                Ok(Instruction::CLOSURE { offset, arity })
            }
            (0x50, 0x5) => Ok(Instruction::CALLC {
                arity: self.next::<i32>()?,
            }),
            (0x50, 0x6) => Ok(Instruction::CALL {
                offset: self.next::<i32>()?,
                n: self.next::<i32>()?,
            }),
            (0x50, 0x8) => Ok(Instruction::ARRAY {
                n: self.next::<i32>()?,
            }),
            (0x50, 0xa) => Ok(Instruction::LINE {
                n: self.next::<i32>()?,
            }),
            (0x70, _) if subopcode <= 0x3 => Ok(Instruction::CALLBUILTIN {
                n: 0,
                name: Builtin::try_from(subopcode).map_err(|_| DecoderError::from(byte))?,
            }),
            (0x70, 0x4) => Ok(Instruction::CALLBUILTIN {
                n: self.next::<i32>()?,
                name: Builtin::Barray,
            }),
            _ => Err(DecoderError::InvalidOpcode(byte)),
        }
    }
}
