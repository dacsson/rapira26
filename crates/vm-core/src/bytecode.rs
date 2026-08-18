//! Descriptor of Lama bytecode
use std::{convert::TryFrom, fmt};

pub const LWRITE_NEWLINE_FLAG: i32 = 1 << 30;
pub const LWRITE_NEWLINE_MASK: i32 = LWRITE_NEWLINE_FLAG - 1;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub enum Op {
    ADD,  // +
    SUB,  // -
    MUL,  // *
    DIV,  // /
    MOD,  // %
    LT,   // <
    LEQ,  // <=
    GT,   // >
    GEQ,  // >=
    EQ,   // ==
    NEQ,  // !=
    AND,  // &&, Tests if both integer operands are non-zero
    OR,   // !!, Tests if either of the operands is non-zero.
    IDIV, // //, integer floor division
    POW,  // **
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub enum UnaryOp {
    Negate,
    Not,
}

/// Scoping rule for a value
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum ValueRel {
    Global,
    Local,
    Arg,     // Function argument
    Capture, // Captured by closure
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub enum CompareJumpKind {
    ISZERO,    // jump if operand is zero
    ISNONZERO, // jump if operand is non-zero
}

/// Builtin functions
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub enum Builtin {
    Lread,
    Lwrite,
    Llength,
    Lstring, // Load string from string table
    Barray,
    Abs,
    Sign,
    Sqrt,
    Floor,
    Round,
    Index,
    Tint, // Type check for integer
}

/// A description of the captured variables of a closure.
#[derive(Debug, Clone, PartialEq)]
pub struct CapturedVar {
    pub rel: ValueRel,
    pub index: i32,
}

/// Represents a jump point in a bytecode
///
/// During compilation its a named point, but later
/// it should be resolved into a code section offset
/// in a bytefile
#[derive(Debug, PartialEq, Clone, PartialOrd)]
pub struct Label {
    pub name: String,
    // TODO: if interpreter resolves at runtime,
    //       maybe offset is not needed?
    pub offset: Option<i32>,
}

#[repr(u8)]
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum Instruction {
    NOP,
    /// Marks the end of the procedure definition. When executed
    /// returns the top value to the caller of this procedure.
    END,
    /// See [`Op`]
    ///
    /// Example: BINOP ("*")
    BINOP {
        op: Op,
    },
    /// Pushes value in the operand stack.
    CONST {
        value: i32,
    },
    /// Pushes the 𝑠th string from the string table.
    STRING {
        index: i32,
    },
    /// Marks the start of a procedure definition with
    /// 𝑎 arguments and 𝑛 locals.
    /// When executed, initializes locals to an empty
    /// value. Unlike CBEGIN, the defined procedure
    /// cannot use captured variables.
    ///
    /// Example: BEGIN ("main", 2, 0, [], [], [])
    BEGIN {
        args: i32,
        locals: i32,
    },
    /// Marks the start of a closure definition with 𝑎 arguments
    /// and 𝑛 locals. When executed, initializes locals to an empty value.
    ///
    /// Unlike BEGIN, the defined closure may use captured variables.
    CBEGIN {
        args: i32,
        locals: i32,
    },
    /// Pushes a new closure with 𝑛 captured variables onto the
    /// stack. The bytecode for the closure begins at 𝑙 (given as an offset
    /// from the start of the bytecode).
    ///
    /// The instruction has a variable-length encoding; the description of
    /// each captured variable is specified as a 5-byte immediate.
    CLOSURE {
        offset: i32,
        arity: i32,
    },
    /// Store a value somewhere, depending on ValueRel
    ///
    /// Example: ST (Global ("z"))
    STORE {
        rel: ValueRel,
        index: i32,
    },
    /// Load a value from somewhere, depending on ValueRel
    ///
    /// Example: LD (Global ("z"))
    LOAD {
        rel: ValueRel,
        index: i32,
    },
    /// Calls a function with 𝑛 arguments. The bytecode for the
    /// function begins at 𝑙 (given as an offset from the start of the byte
    /// code). Pushes the returned value onto the stack.
    CALL {
        dest: Label,
        n: i32,
    },
    /// calls a builtin function.
    CALLBUILTIN {
        name: Builtin,
        n: i32,
    },
    /// Calls a closure with 𝑛 arguments. The first
    /// operand must be the closure, followed by the arguments. Pushes
    /// the returned value onto the stack.
    CALLC {
        arity: i32,
    },
    /// Marks the following bytecode as corresponding to line n
    /// in the source text. Only used for diagnostics.
    LINE {
        n: i32,
    },
    /// Removes the top value from the stack.
    DROP,
    /// Duplicates the top value of the stack.
    DUP,
    /// Swaps the two top values on the stack.
    SWAP,
    /// Jumps to the given offset
    JMP {
        dest: Label,
    },
    /// Set instruction pointer to offset if operand is zero/non-zero
    CJMP {
        dest: Label,
        kind: CompareJumpKind,
    },
    /// Look up an element of some aggregate
    /// NOTE: takes an operand and index from top of stack
    ELEM,
    /// Store To Slice
    ///
    /// Pop slice, pop operand (result) that we assign to
    STS,
    /// Store To Aggregate
    ///
    /// Pop agregate, pop index, pop operand (result) that we assign to
    STA,
    /// Tests whether the operand is an array of 𝑛 elements.
    ARRAY {
        /// Number of elements in the array
        n: i32,
    },
    /// Pushes a boolean value onto the stack
    BOOL {
        value: bool,
    },
    /// Unary operations
    UNARY {
        op: UnaryOp,
    },
    /// Push floating point value on operand stack
    CONSTF {
        value: f64,
    },
    /// Push a tuple on the operand stack
    ///
    /// Takes `n` elements from the top of the stack
    TUPLE {
        n: i32,
    },
    /// Push null value on the operand stack
    NULL,
    /// Zero-width symbolic marker resolved before bytecode encoding.
    LABEL {
        name: String,
    },
    /// Push a slice (view of the aggregate) on the operand stack
    ///
    /// Here - `bounds` is a bounds bitmask:
    /// bit 0 means `from` is on the stack, and bit 1 means `to` is on the stack.
    /// Thus `bounds` ranges from 0 (`[:]`) to 3 (`[from:to]`)
    /// TODO: bounds can be encoded as enum rather then numbers
    SLICE {
        bounds: u8,
    },
}

/// Usefull feature to convert subopcode of
/// binary operation encoding into a variant of Op
impl TryFrom<u8> for Op {
    type Error = ();

    fn try_from(subopcode: u8) -> Result<Self, Self::Error> {
        match subopcode {
            0x1 => Ok(Op::ADD),
            0x2 => Ok(Op::SUB),
            0x3 => Ok(Op::MUL),
            0x4 => Ok(Op::DIV),
            0x5 => Ok(Op::MOD),
            0x6 => Ok(Op::LT),
            0x7 => Ok(Op::LEQ),
            0x8 => Ok(Op::GT),
            0x9 => Ok(Op::GEQ),
            0xa => Ok(Op::EQ),
            0xb => Ok(Op::NEQ),
            0xc => Ok(Op::AND),
            0xd => Ok(Op::OR),
            0xe => Ok(Op::IDIV),
            0xf => Ok(Op::POW),
            _ => Err(()),
        }
    }
}

/// Usefull feature to convert subopcode of
/// load/store/... [`ValueRel`] into a variant of ValueRel
impl TryFrom<u8> for ValueRel {
    type Error = ();

    fn try_from(subopcode: u8) -> Result<Self, Self::Error> {
        match subopcode {
            0x1 => Ok(ValueRel::Global),
            0x2 => Ok(ValueRel::Local),
            0x3 => Ok(ValueRel::Arg),
            0x4 => Ok(ValueRel::Capture),
            _ => Err(()),
        }
    }
}

impl TryFrom<u8> for Builtin {
    type Error = ();

    fn try_from(subopcode: u8) -> Result<Self, Self::Error> {
        match subopcode {
            0x0 => Ok(Builtin::Lread),
            0x1 => Ok(Builtin::Lwrite),
            0x2 => Ok(Builtin::Llength),
            0x3 => Ok(Builtin::Lstring),
            0x4 => Ok(Builtin::Barray),
            0x7 => Ok(Builtin::Abs),
            0x8 => Ok(Builtin::Sign),
            0x9 => Ok(Builtin::Sqrt),
            0xa => Ok(Builtin::Floor),
            0xb => Ok(Builtin::Round),
            0xc => Ok(Builtin::Index),
            0xd => Ok(Builtin::Tint),
            _ => Err(()),
        }
    }
}

impl TryFrom<u8> for CompareJumpKind {
    type Error = ();

    fn try_from(subopcode: u8) -> Result<Self, Self::Error> {
        match subopcode {
            0x0 => Ok(CompareJumpKind::ISZERO),
            0x1 => Ok(CompareJumpKind::ISNONZERO),
            _ => Err(()),
        }
    }
}

impl TryFrom<u8> for UnaryOp {
    type Error = ();

    fn try_from(subopcode: u8) -> Result<Self, Self::Error> {
        match subopcode {
            0x0 => Ok(UnaryOp::Negate),
            0x1 => Ok(UnaryOp::Not),
            _ => Err(()),
        }
    }
}

impl From<&Op> for &i32 {
    fn from(op: &Op) -> Self {
        match op {
            Op::ADD => &0,
            Op::SUB => &1,
            Op::MUL => &2,
            Op::DIV => &3,
            Op::MOD => &4,
            Op::LT => &5,
            Op::LEQ => &6,
            Op::GT => &7,
            Op::GEQ => &8,
            Op::EQ => &9,
            Op::NEQ => &10,
            Op::AND => &11,
            Op::OR => &12,
            Op::IDIV => &13,
            Op::POW => &14,
        }
    }
}

impl From<&UnaryOp> for &i32 {
    fn from(op: &UnaryOp) -> Self {
        match op {
            UnaryOp::Negate => &0,
            UnaryOp::Not => &1,
        }
    }
}

impl From<&ValueRel> for &i32 {
    fn from(rel: &ValueRel) -> Self {
        match rel {
            ValueRel::Global => &1,
            ValueRel::Local => &2,
            ValueRel::Arg => &3,
            ValueRel::Capture => &4,
        }
    }
}

impl From<&Builtin> for &i32 {
    fn from(builtin: &Builtin) -> Self {
        match builtin {
            Builtin::Lread => &1,
            Builtin::Lwrite => &2,
            Builtin::Llength => &3,
            Builtin::Lstring => &4,
            Builtin::Barray => &5,
            Builtin::Abs => &8,
            Builtin::Sign => &9,
            Builtin::Sqrt => &10,
            Builtin::Floor => &11,
            Builtin::Round => &12,
            Builtin::Index => &13,
            Builtin::Tint => &14,
        }
    }
}

impl From<&CompareJumpKind> for &i32 {
    fn from(jump_kind: &CompareJumpKind) -> Self {
        match jump_kind {
            CompareJumpKind::ISZERO => &1,
            CompareJumpKind::ISNONZERO => &2,
        }
    }
}

impl std::fmt::Display for ValueRel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ValueRel::Global => write!(f, "global variable"),
            ValueRel::Local => write!(f, "local variable"),
            ValueRel::Arg => write!(f, "function argument"),
            ValueRel::Capture => write!(f, "captured variable"),
        }
    }
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Instruction::NOP => write!(f, "NOP"),
            Instruction::END => write!(f, "END"),
            Instruction::BINOP { op } => write!(f, "BINOP {:#?}", op),
            Instruction::CONST { value } => write!(f, "CONST {}", value),
            Instruction::STRING { index } => write!(f, "STRING {}", index),
            Instruction::BEGIN { args, locals } => write!(f, "BEGIN {} {}", args, locals),
            Instruction::CBEGIN { args, locals } => write!(f, "CBEGIN {} {}", args, locals),
            Instruction::CLOSURE { offset, arity } => write!(f, "CLOSURE {} {}", offset, arity),
            Instruction::STORE { rel, index } => write!(f, "STORE {} {}", rel, index),
            Instruction::LOAD { rel, index } => write!(f, "LOAD {} {}", rel, index),
            Instruction::CALL { dest: offset, n } => write!(f, "CALL {:?} {}", offset, n),
            Instruction::CALLBUILTIN { name, n } => {
                if let Builtin::Barray = name {
                    write!(f, "CALLB {:#?} {}", name, n)
                } else {
                    write!(f, "CALL {:#?}", name)
                }
            }
            Instruction::CALLC { arity } => write!(f, "CALLC {}", arity),
            Instruction::LINE { n } => write!(f, "LINE {}", n),
            Instruction::DROP => write!(f, "DROP"),
            Instruction::DUP => write!(f, "DUP"),
            Instruction::SWAP => write!(f, "SWAP"),
            Instruction::JMP { dest: offset } => write!(f, "JMP {:#?}", offset),
            Instruction::CJMP { dest: offset, kind } => {
                write!(f, "CJMP {:#?} {:#?}", offset, kind)
            }
            Instruction::ELEM => write!(f, "ELEM"),
            Instruction::STS => write!(f, "STS"),
            Instruction::STA => write!(f, "STA"),
            Instruction::ARRAY { n } => write!(f, "ARRAY {}", n),
            Instruction::BOOL { value } => write!(f, "BOOL {}", value),
            Instruction::UNARY { op } => write!(f, "UNARY {:#?}", op),
            Instruction::CONSTF { value } => write!(f, "CONSTF {}", value),
            Instruction::TUPLE { n } => write!(f, "TUPLE {}", n),
            Instruction::NULL => write!(f, "NULL"),
            Instruction::LABEL { name } => write!(f, "LABEL {}", name),
            Instruction::SLICE { bounds } => write!(f, "SLICE {}", bounds),
        }
    }
}

impl Instruction {
    pub fn discriminant(&self) -> u8 {
        // SAFETY: Because `Self` is marked `repr(u8)`, its layout is a `repr(C)` `union`
        // between `repr(C)` structs, each of which has the `u8` discriminant as its first
        // field, so we can read the discriminant without offsetting the pointer.
        unsafe { *<*const _>::from(self).cast::<u8>() }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn discriminant() {
        assert_eq!(Instruction::NOP.discriminant(), 0);
        assert_eq!(Instruction::BINOP { op: Op::ADD }.discriminant(), 2);
        assert_eq!(Instruction::BINOP { op: Op::SUB }.discriminant(), 2);
    }
}
