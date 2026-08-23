use crate::bytecode::*;
use crate::bytefile::Bytefile;
use crate::numeric::LeBytes;

#[derive(Debug, PartialEq)]
pub enum DecoderError {
    ReadingMoreThenCodeSection,
    InvalidOpcode(u8),
    InvalidInstruction(Instruction),
    UnknownLabel(String),
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
            DecoderError::InvalidInstruction(instruction) => {
                write!(f, "Invalid instruction: {:?}", instruction)
            }
            DecoderError::UnknownLabel(name) => {
                write!(f, "Unknown label: {}", name)
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
            (0x00, _) if (0x1..=0xf).contains(&subopcode) => Ok(Instruction::BINOP {
                op: Op::try_from(subopcode).map_err(|_| DecoderError::from(byte))?,
            }),
            (0x00, _) => Err(DecoderError::from(byte)),
            (0x10, 0x0) => Ok(Instruction::CONST {
                value: self.next::<i32>()?,
            }),
            (0x10, 0x1) => Ok(Instruction::STRING {
                index: self.next::<i32>()?,
            }),
            (0x10, 0x2) => Ok(Instruction::TUPLE {
                n: self.next::<i32>()?,
            }),
            (0x10, 0x3) => Ok(Instruction::STS),
            (0x10, 0x4) => Ok(Instruction::STA),
            (0x10, 0x5) => Ok(Instruction::JMP {
                dest: Label {
                    // TODO: get rid of empty str
                    name: String::new(),
                    offset: Some(self.next::<i32>()?),
                },
            }),
            (0x10, 0x6) => Ok(Instruction::END),
            (0x10, 0x7) => Ok(Instruction::SLICE {
                bounds: self.next::<u8>()?,
            }),
            (0x10, 0x8) => Ok(Instruction::DROP),
            (0x10, 0x9) => Ok(Instruction::DUP),
            (0x10, 0xa) => Ok(Instruction::SWAP),
            (0x10, 0xb) => Ok(Instruction::ELEM),
            (0x10, 0xc) => Ok(Instruction::CONSTF {
                value: self.next::<f64>()?,
            }),
            (0x20, _) if subopcode <= 0x3 => Ok(Instruction::LOAD {
                rel: ValueRel::try_from(subopcode).map_err(|_| DecoderError::from(byte))?,
                index: self.next::<i32>()?,
            }),
            (0x30, _) => Ok(Instruction::UNARY {
                op: UnaryOp::try_from(subopcode).map_err(|_| DecoderError::from(byte))?,
            }),
            (0x40, _) if subopcode <= 0x3 => Ok(Instruction::STORE {
                rel: ValueRel::try_from(subopcode).map_err(|_| DecoderError::from(byte))?,
                index: self.next::<i32>()?,
            }),
            (0x50, 0x0) => Ok(Instruction::CJMP {
                dest: Label {
                    // TODO: get rid of empty str
                    name: String::new(),
                    offset: Some(self.next::<i32>()?),
                },
                kind: CompareJumpKind::ISZERO,
            }),
            (0x50, 0x1) => Ok(Instruction::CJMP {
                dest: Label {
                    // TODO: get rid of empty str
                    name: String::new(),
                    offset: Some(self.next::<i32>()?),
                },
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
                dest: Label {
                    // TODO: get rid of empty str
                    name: String::new(),
                    offset: Some(self.next::<i32>()?),
                },
                n: self.next::<i32>()?,
            }),
            (0x50, 0x8) => Ok(Instruction::ARRAY {
                n: self.next::<i32>()?,
            }),
            (0x50, 0x9) => Ok(Instruction::VARIANT {
                schema: self.next::<i32>()?,
            }),
            (0x50, 0xa) => Ok(Instruction::LINE {
                n: self.next::<i32>()?,
            }),
            (0x50, 0xb) => Ok(Instruction::VARIANTTAG),
            (0x50, 0xc) => Ok(Instruction::FIELD {
                index: self.next::<i32>()?,
            }),
            (0x50, 0xd) => Ok(Instruction::SETFIELD {
                index: self.next::<i32>()?,
            }),
            (0x50, 0xe) => Ok(Instruction::ISVARIANT {
                schema: self.next::<i32>()?,
            }),
            (0x70, 0x0) => Ok(Instruction::CALLBUILTIN {
                // `n` packs the [un]typed flag and arg count
                n: self.next::<i32>()?,
                name: Builtin::Lread,
            }),
            (0x70, 0x1) => Ok(Instruction::CALLBUILTIN {
                // `n` packs the newline flag and arg count
                n: self.next::<i32>()?,
                name: Builtin::Lwrite,
            }),
            (0x70, _)
                if [0x3, 0x4, 0x7, 0x8, 0x9, 0xa, 0xb, 0xc, 0xd, 0xe, 0xf].contains(&subopcode) =>
            {
                Ok(Instruction::CALLBUILTIN {
                    n: 0,
                    name: Builtin::try_from(subopcode).map_err(|_| DecoderError::from(byte))?,
                })
            }
            (0x70, 0x5) => Ok(Instruction::BOOL {
                value: self.next::<u8>()? != 0,
            }),
            (0x70, 0x6) => Ok(Instruction::NULL),
            _ => Err(DecoderError::InvalidOpcode(byte)),
        }
    }

    /// Encode an instruction into a bytecode buffer, which can be
    /// directly written into a bytefile
    pub fn encode(instruction: &Instruction) -> Result<Vec<u8>, DecoderError> {
        let mut buf = Vec::new();
        match instruction {
            Instruction::NOP => buf.push(0x00),
            Instruction::BINOP { op } => {
                let op_: &i32 = op.into();
                buf.push(0x00 | (*op_ as u8) + 1);
            }
            Instruction::CONST { value } => {
                buf.push(0x10);
                buf.extend(&value.to_le_bytes());
            }
            Instruction::STRING { index } => {
                buf.push(0x11);
                buf.extend(&index.to_le_bytes());
            }
            Instruction::TUPLE { n } => {
                buf.push(0x12);
                buf.extend(&n.to_le_bytes());
            }
            Instruction::STS => buf.push(0x13),
            Instruction::STA => buf.push(0x14),
            Instruction::JMP {
                dest: Label { name, offset },
            } => {
                buf.push(0x15);

                let Some(offset) = offset else {
                    return Err(DecoderError::UnknownLabel(name.clone()));
                };

                buf.extend(&offset.to_le_bytes());
            }
            Instruction::END => buf.push(0x16),
            Instruction::SLICE { bounds } => {
                buf.push(0x17);

                // Slice uses n as a bounds bitmask - 0 means
                // `[:]`, so its zero value must still be encoded explicitly.
                buf.extend(&bounds.to_le_bytes());
            }
            Instruction::DROP => buf.push(0x18),
            Instruction::DUP => buf.push(0x19),
            Instruction::SWAP => buf.push(0x1a),
            Instruction::ELEM => buf.push(0x1b),
            Instruction::CONSTF { value } => {
                buf.push(0x1c);
                buf.extend(&value.to_le_bytes());
            }
            Instruction::LOAD { rel, index } => {
                let rel_: &i32 = rel.into();
                buf.push(0x20 | *rel_ as u8);
                buf.extend(&index.to_le_bytes());
            }
            Instruction::UNARY { op } => {
                let _op: &i32 = op.into();
                buf.push(0x30 | *_op as u8);
            }
            Instruction::STORE { rel, index } => {
                let rel_: &i32 = rel.into();
                buf.push(0x40 | *rel_ as u8);
                buf.extend(&index.to_le_bytes());
            }
            Instruction::CJMP {
                dest: Label { name, offset },
                kind: CompareJumpKind::ISZERO,
            } => {
                buf.push(0x50);

                let Some(offset) = offset else {
                    return Err(DecoderError::UnknownLabel(name.clone()));
                };

                buf.extend(&offset.to_le_bytes());
            }
            Instruction::CJMP {
                dest: Label { name, offset },
                kind: CompareJumpKind::ISNONZERO,
            } => {
                buf.push(0x51);

                let Some(offset) = offset else {
                    return Err(DecoderError::UnknownLabel(name.clone()));
                };

                buf.extend(&offset.to_le_bytes());
            }
            Instruction::BEGIN { args, locals } => {
                buf.push(0x52);
                buf.extend(&args.to_le_bytes());
                buf.extend(&locals.to_le_bytes());
            }
            Instruction::CBEGIN { args, locals } => {
                buf.push(0x53);
                buf.extend(&args.to_le_bytes());
                buf.extend(&locals.to_le_bytes());
            }
            Instruction::CLOSURE { offset, arity } => {
                buf.push(0x54);
                buf.extend(&offset.to_le_bytes());
                buf.extend(&arity.to_le_bytes());
            }
            Instruction::CALLC { arity } => {
                buf.push(0x55);
                buf.extend(&arity.to_le_bytes());
            }
            Instruction::CALL {
                dest: Label { name, offset },
                n,
            } => {
                buf.push(0x56);

                let Some(offset) = offset else {
                    return Err(DecoderError::UnknownLabel(name.clone()));
                };
                buf.extend(&offset.to_le_bytes());

                buf.extend(&n.to_le_bytes());
            }
            Instruction::ARRAY { n } => {
                buf.push(0x58);
                buf.extend(&n.to_le_bytes());
            }
            Instruction::VARIANT { schema } => {
                buf.push(0x59);
                buf.extend(&schema.to_le_bytes());
            }
            Instruction::LINE { n } => {
                buf.push(0x5a);
                buf.extend(&n.to_le_bytes());
            }
            Instruction::VARIANTTAG => buf.push(0x5b),
            Instruction::FIELD { index } => {
                buf.push(0x5c);
                buf.extend(&index.to_le_bytes());
            }
            Instruction::SETFIELD { index } => {
                buf.push(0x5d);
                buf.extend(&index.to_le_bytes());
            }
            Instruction::ISVARIANT { schema } => {
                buf.push(0x5e);
                buf.extend(&schema.to_le_bytes());
            }
            Instruction::CALLBUILTIN { n, name } => {
                let name_: &i32 = name.into();
                buf.push(0x70 | (*name_ as u8));

                // println!("{:?} to u8: {:x}", name, *name_ as u8);

                // NOTE: not all builtins need to push n,
                // however some use this as arg count as well as
                // pack their flags into the lower bits of the opcode
                // like `Builtin::Lread` or `Builtin::Lwrite`
                if *n != 0 {
                    buf.extend(&n.to_le_bytes());
                }
            }
            Instruction::BOOL { value } => {
                buf.push(0x75);
                buf.extend((*value as u8).to_le_bytes());
            }
            Instruction::NULL => {
                buf.push(0x76);
            }
            Instruction::LABEL { .. } => {}
            _ => return Err(DecoderError::InvalidInstruction(instruction.clone())),
        }

        Ok(buf)
    }
}
