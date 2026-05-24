use std::collections::{HashMap, VecDeque};
use std::fmt::{Debug, Display, Formatter};

use bitvec::array::BitArray;
use bitvec::vec::BitVec;
use bitvec::{BitArr, prelude as bv};
use vm_core::bytecode::{Builtin, Instruction, PattKind, ValueRel};
use vm_core::bytefile::Bytefile;
use vm_core::decoder::{Decoder, DecoderError};

pub const MAX_SEXP_TAGLEN: usize = 10;
pub const MAX_CAPTURES: usize = 0x7fffffff;
#[cfg(test)]
pub const MAX_OPERAND_STACK_SIZE: usize = 0xffff;
#[cfg(not(test))]
pub const MAX_OPERAND_STACK_SIZE: usize = 0xffff;
pub const MAX_ARG_LEN: usize = 50;
pub const MAX_SEXP_MEMBERS: usize = 0xffff;
pub const MAX_ARRAY_MEMBERS: usize = 0xffff;
pub const MAX_PARAMS: usize = 0xffff;

const UNSEEN: u32 = u32::MAX;
const TAG_MASK: u32 = 1;
const TAG_VISITED: u32 = 0;
const TAG_PENDING: u32 = 1;

#[derive(Debug)]
pub enum VerifierError {
    DecoderError(DecoderError),
    InvalidJumpOffset(usize, i32, usize),
    StringIndexOutOfBounds,
    InvalidStoreIndex(ValueRel, i32, i64),
    InvalidLoadIndex(ValueRel, i32, i64),
    TooMuchMembers(usize, usize),
    TooManyCaptures(usize),
    SexpTagTooLong(usize),
    InvalidBeginArgs(i32),
    StackUnderflow(isize),
    StackOverflow,
    ExceededCodeSize(usize),
    InvalidControlFlow,
    NegativeArity,
    ExpectedBegin(String),
    ExpectedInstruction,
    ExpectedFunction,
    StackUnbalanced(u32, isize),
}

impl std::fmt::Display for VerifierError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VerifierError::DecoderError(e) => {
                write!(f, "{}", e)
            }
            VerifierError::InvalidJumpOffset(ip, offset, code_len) => {
                write!(
                    f,
                    "Invalid jump offset: current ip at {}, offset is {}, but code length is {}",
                    ip, offset, code_len
                )
            }
            VerifierError::StringIndexOutOfBounds => {
                write!(f, "String index out of bounds")
            }
            VerifierError::InvalidStoreIndex(rel, index, n) => {
                write!(f, "Invalid store index {}/{} for {}", index, n, rel)
            }
            VerifierError::InvalidLoadIndex(rel, index, n) => {
                write!(f, "Invalid load index {}/{} for {}", index, n, rel)
            }
            VerifierError::TooMuchMembers(n, max) => {
                write!(f, "Too many members: {} > {}", n, max)
            }
            VerifierError::TooManyCaptures(n) => {
                write!(f, "Too many captures: {} > {}", n, MAX_CAPTURES)
            }
            VerifierError::SexpTagTooLong(n) => {
                write!(f, "Sexp tag too long: {} > {}", n, MAX_SEXP_MEMBERS)
            }
            VerifierError::InvalidBeginArgs(n) => {
                write!(f, "Too much begin arguments: {} > {}", n, MAX_PARAMS)
            }
            VerifierError::StackUnderflow(expected) => {
                write!(f, "Stack underflow, expected at least {}", expected)
            }
            VerifierError::StackOverflow => {
                write!(f, "Stack overflow, max is {}", MAX_OPERAND_STACK_SIZE)
            }
            VerifierError::ExceededCodeSize(n) => {
                write!(f, "Exceeded code size: {}", n)
            }
            VerifierError::InvalidControlFlow => {
                write!(f, "Invalid control flow")
            }
            VerifierError::NegativeArity => {
                write!(f, "Negative arity")
            }
            VerifierError::ExpectedBegin(msg) => {
                write!(f, "Expected BEGIN instruction, found {}", msg)
            }
            VerifierError::ExpectedInstruction => {
                write!(f, "Expected to find instruction")
            }
            VerifierError::ExpectedFunction => {
                write!(f, "Expected to find function")
            }
            VerifierError::StackUnbalanced(expected, actual) => {
                write!(
                    f,
                    "Stack unbalance found at merge point, expected stack depth {}, but got {}",
                    expected, actual
                )
            }
        }
    }
}

impl std::error::Error for VerifierError {}

pub struct Verifier {
    pub decoder: Decoder,
    // TODO: maybe we can make it as a single stack?
    instr_queue: [i64; MAX_OPERAND_STACK_SIZE / 2], // each element is (offset << 32 | func_count) packed in i64
    func_queue: [i64; MAX_OPERAND_STACK_SIZE / 2], // each element is (offset << 32 | stack_depth) packed in i64
    instr_len: usize,
    func_len: usize,
}

impl Verifier {
    pub fn new(decoder: Decoder) -> Self {
        Verifier {
            decoder,
            instr_queue: [0; MAX_OPERAND_STACK_SIZE / 2],
            func_queue: [0; MAX_OPERAND_STACK_SIZE / 2],
            instr_len: 0,
            func_len: 0,
        }
    }

    fn is_terminal(instr: &Instruction) -> bool {
        match instr {
            Instruction::RET | Instruction::END | Instruction::FAIL { .. } => true,
            _ => false,
        }
    }

    fn decode_at(&mut self, addr: u32) -> Instruction {
        self.decoder.ip = addr as usize;

        let enc = self.decoder.next::<u8>().unwrap();
        self.decoder.decode(enc).unwrap()
    }

    fn get_opcode(&mut self, offset: u32) -> u8 {
        self.decoder.ip = offset as usize;
        self.decoder.next::<u8>().unwrap()
    }

    fn instruction_length(&mut self, start: u32) -> usize {
        let _ = self.decode_at(start);
        self.decoder.ip - start as usize
    }

    /// Pushes (offset, func_begin_offset) of the current instruction onto the queue
    fn queue_instruction(&mut self, offset: u32, func_begin_offset: usize) {
        self.instr_queue[self.instr_len] = (offset as i64) << 32 | func_begin_offset as i64;
        self.instr_len += 1;
    }

    /// Returns (offset, func_begin_offset) of the current instruction from the queue
    fn instruction_queue_pop(&mut self) -> (u32, usize) {
        self.instr_len -= 1;
        let val = self.instr_queue[self.instr_len];
        ((val >> 32) as u32, (val & 0xffffffff) as usize)
    }

    /// Pushes (offset, stack_depth) of the current func
    fn queue_function(&mut self, offset: u32, curr_stack_depth: isize) {
        self.func_queue[self.func_len] = (offset as i64) << 32 | curr_stack_depth as i64;
        self.func_len += 1;
    }

    /// Returns (offset, stack_depth) of the current function in queue
    fn function_queue_pop(&mut self) -> (u32, usize) {
        self.func_len -= 1;
        let val = self.func_queue[self.func_len];
        ((val >> 32) as u32, (val & 0xffffffff) as usize)
    }

    fn instruction_queue_is_empty(&self) -> bool {
        self.instr_len == 0
    }

    fn function_queue_is_empty(&self) -> bool {
        self.func_len == 0
    }

    // Change stack depth of current func
    fn set_function_stack_depth(&mut self, stack_depth: isize) {
        let prev_val = self.func_queue[self.func_len - 1];
        let (offset, curr_max) = ((prev_val >> 32) as u32, (prev_val & 0xffffffff) as usize);
        let new_max = std::cmp::max(curr_max as isize, std::cmp::max(0, stack_depth));
        self.func_queue[self.func_len - 1] = (offset as i64) << 32 | new_max as i64;
    }

    /// Returns (offset, func_count)
    fn peek_instruction(&self, index: usize) -> Option<(u32, usize)> {
        if self.instr_len == 0 || index >= self.instr_len {
            None
        } else {
            let val = self.instr_queue[self.instr_len - index - 1];
            Some(((val >> 32) as u32, (val & 0xffffffff) as usize))
        }
    }

    /// Writes the stack depth at big word of begin instruction byte
    fn write_function_stack_depth_at(&mut self, offset: usize, stack_depth: isize) {
        let begin_instr_bytes = &self.decoder.bf.code_section[offset + 1..offset + 1 + 4];
        let mut payload = u32::from_le_bytes(begin_instr_bytes.try_into().unwrap());
        payload |= (stack_depth.to_le() as u32) << 16;
        self.decoder.bf.code_section[offset + 1..offset + 1 + 4]
            .copy_from_slice(&payload.to_le_bytes());
    }

    fn update_max_stack_depth(&mut self, func_offset: usize, current_depth: isize) {
        for i in 0..self.func_len {
            let val = self.func_queue[i];
            let offset = (val >> 32) as usize;
            if offset == func_offset {
                let curr_max = (val & 0xffffffff) as isize;
                let new_max = std::cmp::max(curr_max, std::cmp::max(0, current_depth));
                self.func_queue[i] = (func_offset as i64) << 32 | new_max as i64;
                return;
            }
        }
        // add to lookup aftr
        self.func_queue[self.func_len] =
            (func_offset as i64) << 32 | std::cmp::max(0, current_depth) as i64;
        self.func_len += 1;
    }

    pub fn verify(&mut self) -> Result<(), VerifierError> {
        let mut stack_depth = [UNSEEN; MAX_OPERAND_STACK_SIZE];
        let mut curr_stack_depth = 2;

        // Main is the only starting point
        self.queue_instruction(
            self.decoder.bf.main_offset,
            self.decoder.bf.main_offset as usize,
        );

        while !self.instruction_queue_is_empty() {
            let (offset_at, func_offset) = self.instruction_queue_pop();
            let instr = self.decode_at(offset_at);
            let next_offset = offset_at + self.instruction_length(offset_at) as u32;
            let length = self.instruction_length(offset_at as u32);
            let offset = offset_at as usize;

            // println!(
            //     "[LOG] offset_at={} func_offset={} instr={:?}",
            //     offset_at, func_offset, instr
            // );

            if offset + length > self.decoder.code_section_len {
                return Err(VerifierError::ExceededCodeSize(offset));
            }

            if Verifier::is_pending(stack_depth[offset]) {
                // Jump target
                curr_stack_depth = Verifier::decode_depth(stack_depth[offset]) as isize;
            }

            // Skip visited
            if Verifier::is_visited(stack_depth[offset]) {
                if self.instruction_queue_is_empty() {
                    continue;
                }

                let (next_offset, _) = self
                    .peek_instruction(0)
                    .ok_or(VerifierError::ExpectedInstruction)?;

                curr_stack_depth =
                    Verifier::decode_depth(stack_depth[next_offset as usize]) as isize;

                if Verifier::is_unseen(stack_depth[next_offset as usize]) {
                    return Err(VerifierError::InvalidControlFlow);
                }

                if Verifier::is_visited(stack_depth[next_offset as usize]) {
                    // Force the next isntr to have the correct stack depth
                    stack_depth[next_offset as usize] =
                        Verifier::encode_pending(curr_stack_depth as u32);
                }
                continue;
            }

            stack_depth[offset] = Verifier::encode_visited(curr_stack_depth as u32);
            let push_effect = instr.stack_push_effect();
            let pop_effect = instr.stack_pop_effect();

            if curr_stack_depth - (pop_effect as isize) < 0 {
                return Err(VerifierError::StackUnderflow(curr_stack_depth));
            }

            curr_stack_depth += push_effect as isize - pop_effect as isize;

            if curr_stack_depth < 0 {
                return Err(VerifierError::StackUnderflow(curr_stack_depth));
            }
            if curr_stack_depth > MAX_OPERAND_STACK_SIZE as isize {
                return Err(VerifierError::StackOverflow);
            }

            // Dynamically updates max stack depth on-the-fly (like a boss)
            self.update_max_stack_depth(func_offset, curr_stack_depth);

            match instr {
                Instruction::CJMP {
                    offset: target_at, ..
                }
                | Instruction::JMP { offset: target_at } => {
                    let target = target_at as usize;

                    if target + self.instruction_length(target_at as u32)
                        > self.decoder.code_section_len
                    {
                        return Err(VerifierError::InvalidJumpOffset(
                            offset,
                            target_at,
                            self.decoder.code_section_len,
                        ));
                    }

                    if !Verifier::is_unseen(stack_depth[target]) {
                        // We are at merge point of control flow
                        if Verifier::decode_depth(stack_depth[target]) != curr_stack_depth as u32 {
                            return Err(VerifierError::StackUnbalanced(
                                Verifier::decode_depth(stack_depth[target]),
                                curr_stack_depth,
                            ));
                        }
                    } else {
                        stack_depth[target] = Verifier::encode_pending(curr_stack_depth as u32);
                        // IMPORTANT: He inherits the current func_offset
                        self.queue_instruction(target_at as u32, func_offset);
                    }

                    // Skip direct (no branchings)
                    if let Instruction::JMP { .. } = instr {
                        if self.instruction_queue_is_empty() {
                            continue;
                        }
                        let (next_offset, _) = self.peek_instruction(0).unwrap();
                        let depth =
                            Verifier::decode_depth(stack_depth[next_offset as usize]) as isize;
                        curr_stack_depth = depth;
                        continue;
                    }
                }
                Instruction::CALL {
                    offset: target_at,
                    n,
                } => {
                    let target = target_at as usize;
                    if target + self.instruction_length(target_at as u32)
                        > self.decoder.code_section_len
                    {
                        return Err(VerifierError::InvalidJumpOffset(
                            offset,
                            target_at,
                            self.decoder.code_section_len,
                        ));
                    }

                    if !matches!(
                        self.decode_at(target_at as u32),
                        Instruction::BEGIN { .. } | Instruction::CBEGIN { .. }
                    ) {
                        return Err(VerifierError::ExpectedBegin(format!("{:?}", instr)));
                    }

                    if Verifier::is_unseen(stack_depth[target]) {
                        // 2 is CALL stack push effect, BEGINs pop effect is 2 (closure_obj and ret_ip)
                        stack_depth[target] = Verifier::encode_pending(2);

                        // New func/context we create
                        self.queue_instruction(target_at as u32, target);
                    }
                }
                _ => {}
            }

            // NOTE: use offset insted of id
            self.verify_instruction(func_offset, &instr)?;

            if Verifier::is_terminal(&instr) {
                // (Function dropping logic is completely deleted from here)
                if self.instruction_queue_is_empty() {
                    continue;
                }

                let (next_offset, _) = self
                    .peek_instruction(0)
                    .ok_or(VerifierError::ExpectedInstruction)?;

                curr_stack_depth =
                    Verifier::decode_depth(stack_depth[next_offset as usize]) as isize;

                if Verifier::is_visited(stack_depth[next_offset as usize]) {
                    stack_depth[next_offset as usize] =
                        Verifier::encode_pending(curr_stack_depth as u32);
                }
                continue;
            }

            // He again inherits the cur func_offset, sequentially
            self.queue_instruction(next_offset, func_offset);
        }

        // Here we patch all bytecode instructions at the very end
        // TODO: verifyer should actually move its decoder to interpreter to avoid cloning a whole bytefile
        for i in 0..self.func_len {
            let val = self.func_queue[i];
            let target_func_offset = (val >> 32) as usize;
            let max_depth = (val & 0xffffffff) as isize;

            self.write_function_stack_depth_at(target_func_offset, max_depth);
        }

        Ok(())
    }

    #[inline]
    pub fn encode_visited(depth: u32) -> u32 {
        // low bit 0
        (depth << 1) | TAG_VISITED
    }

    #[inline]
    pub fn encode_pending(depth: u32) -> u32 {
        // low bit 1
        (depth << 1) | TAG_PENDING
    }

    #[inline]
    pub fn is_unseen(v: u32) -> bool {
        v == UNSEEN
    }

    #[inline]
    pub fn is_pending(v: u32) -> bool {
        v != UNSEEN && (v & TAG_MASK) == TAG_PENDING
    }

    #[inline]
    pub fn is_visited(v: u32) -> bool {
        v != UNSEEN && (v & TAG_MASK) == TAG_VISITED
    }

    /// Returns the stored stack depth for either VISITED or PENDING values.
    /// (Callers should typically check `is_unseen` first.)
    #[inline]
    pub fn decode_depth(v: u32) -> u32 {
        v >> 1
    }

    /// Convenience: mark an offset as a required jump target depth if it is currently UNSEEN.
    /// Returns true if it was set, false if it was already non-UNSEEN.
    #[inline]
    pub fn mark_pending_if_unseen(stack_depth: &mut [u32], idx: usize, required: u32) -> bool {
        if stack_depth[idx] == UNSEEN {
            stack_depth[idx] = Verifier::encode_pending(required);
            true
        } else {
            false
        }
    }

    fn verify_instruction(
        &mut self,
        func_offset: usize,
        instruction: &Instruction,
    ) -> Result<(), VerifierError> {
        match instruction {
            Instruction::BEGIN {
                args: payload,
                locals,
            }
            | Instruction::CBEGIN {
                args: payload,
                locals,
            } => {
                // TODO: main check for 2 arguments

                let stack_size_for_function = payload >> 16;
                let args = (payload & 0xFFFF) as usize;

                if args > MAX_PARAMS {
                    return Err(VerifierError::InvalidBeginArgs(args as i32));
                }
            }
            Instruction::END => {}
            Instruction::JMP { offset } | Instruction::CJMP { offset, .. } => {
                let offset_at = *offset as usize;

                if (*offset) < 0 || offset_at >= self.decoder.code_section_len {
                    return Err(VerifierError::InvalidJumpOffset(
                        self.decoder.ip,
                        *offset,
                        self.decoder.code_section_len,
                    ));
                }
            }
            Instruction::CALL { offset, n } => {
                let offset_at = *offset as usize;

                if (*offset) < 0 || offset_at >= self.decoder.code_section_len {
                    return Err(VerifierError::InvalidJumpOffset(
                        self.decoder.ip,
                        *offset,
                        self.decoder.code_section_len,
                    ));
                }

                if *n < 0 {
                    return Err(VerifierError::NegativeArity);
                }
            }
            Instruction::STRING { index } => {
                let string_index = *index as usize;
                if string_index >= self.decoder.bf.stringtab_size as usize {
                    return Err(VerifierError::StringIndexOutOfBounds);
                }
            }
            Instruction::SEXP { s_index, n_members } => {
                let string_index = *s_index as usize;
                if string_index >= self.decoder.bf.stringtab_size as usize {
                    return Err(VerifierError::StringIndexOutOfBounds);
                }

                if *n_members < 0 {
                    return Err(VerifierError::NegativeArity);
                }
                let mems = *n_members as usize;
                if mems >= MAX_SEXP_MEMBERS {
                    return Err(VerifierError::TooMuchMembers(mems, MAX_SEXP_MEMBERS));
                }

                let tag = self
                    .decoder
                    .bf
                    .get_string_at_offset(string_index)
                    .map_err(|_| VerifierError::StringIndexOutOfBounds)?;

                if tag.len() > MAX_SEXP_TAGLEN {
                    return Err(VerifierError::SexpTagTooLong(tag.len()));
                }
            }
            Instruction::TAG { index, n } => {
                if *n < 0 {
                    return Err(VerifierError::NegativeArity);
                }

                if *index < 0 || *index >= self.decoder.bf.stringtab_size as i32 {
                    return Err(VerifierError::StringIndexOutOfBounds);
                }
            }
            Instruction::ARRAY { n } => {
                if *n < 0 {
                    return Err(VerifierError::NegativeArity);
                }

                let array_members = *n as usize;
                if array_members >= MAX_ARRAY_MEMBERS {
                    return Err(VerifierError::TooMuchMembers(
                        array_members,
                        MAX_ARRAY_MEMBERS,
                    ));
                }
            }
            Instruction::STORE { rel, index } | Instruction::LOAD { rel, index } => match rel {
                ValueRel::Global => {
                    let el_index = *index as usize;

                    if el_index >= self.decoder.bf.global_area_size as usize {
                        if let Instruction::STORE { .. } = instruction {
                            return Err(VerifierError::InvalidStoreIndex(
                                ValueRel::Global,
                                *index,
                                self.decoder.bf.global_area_size as i64,
                            ));
                        } else {
                            return Err(VerifierError::InvalidLoadIndex(
                                ValueRel::Global,
                                *index,
                                self.decoder.bf.global_area_size as i64,
                            ));
                        }
                    }
                }
                ValueRel::Local => {
                    let instr = self.decode_at(func_offset as u32);
                    let Instruction::BEGIN { args: _, locals } = instr else {
                        return Err(VerifierError::ExpectedBegin(format!("{:?}", instr)));
                    };

                    if *index >= locals {
                        return Err(VerifierError::InvalidLoadIndex(
                            ValueRel::Local,
                            *index,
                            locals as i64,
                        ));
                    }
                }
                ValueRel::Arg => {
                    let instr = self.decode_at(func_offset as u32);
                    let Instruction::BEGIN { args: payload, .. } = instr else {
                        return Err(VerifierError::ExpectedBegin(format!("{:?}", instr)));
                    };

                    let stack_size_for_function = payload >> 16;
                    let args = (payload & 0xFFFF) as usize;

                    if *index >= args as i32 {
                        return Err(VerifierError::InvalidLoadIndex(
                            ValueRel::Arg,
                            *index,
                            args as i64,
                        ));
                    }
                }
                _ => {}
            },
            Instruction::CLOSURE { offset, arity } => {
                let offset_at = *offset as usize;

                if (*offset) < 0 || offset_at >= self.decoder.code_section_len {
                    return Err(VerifierError::InvalidJumpOffset(
                        self.decoder.ip,
                        *offset,
                        self.decoder.code_section_len,
                    ));
                }

                let arity = *arity as usize;

                if arity >= MAX_CAPTURES {
                    return Err(VerifierError::TooManyCaptures(arity));
                }
            }
            _ => {}
        };

        Ok(())
    }
}

trait StackEffect {
    /// Returns the stack effect of the instruction.
    fn stack_size_effect(&self) -> isize;
    fn stack_push_effect(&self) -> usize;
    fn stack_pop_effect(&self) -> usize;
}

impl StackEffect for Instruction {
    fn stack_size_effect(&self) -> isize {
        match self {
            Instruction::NOP => 0,
            Instruction::BINOP { .. } => -1, // pop two, push one
            Instruction::CONST { .. } => 1,
            Instruction::STRING { .. } => 1,
            Instruction::SEXP { n_members, .. } => {
                // pop n_members arguments, push the new S‑exp
                1 - (*n_members as isize)
            }
            Instruction::JMP { .. } => 0,
            Instruction::STA => -2, // pop 3, push aggregate back
            Instruction::STI => 0,
            Instruction::CBEGIN { args, locals } | Instruction::BEGIN { args, locals } => {
                // /*frame_ptr*/
                // 1 + /* closure */ 1 + /*arg count */1 + /*local count*/1
                // + /*ret frame ptr*/ 1 + /*ret ip*/ 1 + /*args*/ *args as isize + /*locals*/ *locals as isize
                (3 + locals) as isize // args do not get moved
            }
            // depends on the current frame metadata
            Instruction::END | Instruction::RET => 0,
            Instruction::STORE { .. } => 0,
            Instruction::LOAD { .. } => 1,
            Instruction::DROP => -1,
            Instruction::DUP => 1,  // pop 1, push 2 copies
            Instruction::SWAP => 0, // pop 2, push 2 (swapped)
            Instruction::LINE { .. } => 0,
            Instruction::CALL { offset, n } => -(*n as isize) + 1, // we dont count temporary pushes (ret_ip)
            Instruction::CALLBUILTIN { name, n } => match name {
                Builtin::Barray => 1 - *n as isize,
                Builtin::Llength => 0,
                Builtin::Lread => 1,
                Builtin::Lwrite => 0,
                Builtin::Lstring => 0,
                _ => 0,
            },
            Instruction::CJMP { .. } => -1,
            Instruction::ELEM => -1, // pop index n container, push result
            Instruction::ARRAY { .. } => 0,
            Instruction::TAG { .. } => 0,
            Instruction::FAIL { .. } => -1,
            Instruction::CLOSURE { .. } => 1,
            Instruction::CALLC { arity } => -(*arity as isize),
            Instruction::PATT { kind } => match kind {
                PattKind::BothAreStr => -1,
                _ => 0,
            },
            _ => 0,
        }
    }

    fn stack_pop_effect(&self) -> usize {
        match self {
            Instruction::NOP => 0,
            Instruction::BINOP { .. } => 2,
            Instruction::CONST { .. } => 0,
            Instruction::STRING { .. } => 0,
            Instruction::SEXP { n_members, .. } => *n_members as usize,
            Instruction::JMP { .. } => 0,
            Instruction::STA => 3,
            Instruction::STI => 0,
            Instruction::CBEGIN { .. } | Instruction::BEGIN { .. } => 2,
            Instruction::END | Instruction::RET => 0,
            Instruction::STORE { .. } => 1,
            Instruction::LOAD { .. } => 0,
            Instruction::DROP => 1,
            Instruction::DUP => 1,
            Instruction::SWAP => 2,
            Instruction::LINE { .. } => 0,
            Instruction::CALL { offset, n } => *n as usize,
            Instruction::CALLBUILTIN { name, n } => match name {
                Builtin::Barray => *n as usize,
                Builtin::Llength => 1,
                Builtin::Lread => 0,
                Builtin::Lwrite => 1,
                Builtin::Lstring => 1,
                _ => 0,
            },
            Instruction::CJMP { .. } => 1,
            Instruction::ELEM => 2,
            Instruction::ARRAY { .. } => 1,
            Instruction::TAG { .. } => 1,
            Instruction::FAIL { .. } => 1,
            Instruction::CLOSURE { arity, .. } => *arity as usize + 1,
            Instruction::CALLC { arity, .. } => *arity as usize,
            Instruction::PATT { kind } => match kind {
                PattKind::BothAreStr => 2,
                _ => 1,
            },
            Instruction::LOADREF { .. } | Instruction::HALT => 0,
        }
    }

    fn stack_push_effect(&self) -> usize {
        match self {
            Instruction::NOP => 0,
            Instruction::BINOP { .. } => 1,
            Instruction::CONST { .. } => 1,
            Instruction::STRING { .. } => 1,
            Instruction::SEXP { .. } => 1,
            Instruction::JMP { .. } => 0,
            Instruction::STA => 1,
            Instruction::STI => 0,
            Instruction::CBEGIN { locals, .. } | Instruction::BEGIN { locals, .. } => {
                5 + *locals as usize
            }
            Instruction::END | Instruction::RET => 0,
            Instruction::STORE { .. } => 1,
            Instruction::LOAD { .. } => 1,
            Instruction::DROP => 0,
            Instruction::DUP => 2,
            Instruction::SWAP => 2,
            Instruction::LINE { .. } => 0,
            Instruction::CALL { .. } => 1,
            Instruction::CALLBUILTIN { .. } => 1,
            Instruction::CJMP { .. } => 0,
            Instruction::ELEM => 1,
            Instruction::ARRAY { .. } => 1,
            Instruction::TAG { .. } => 1,
            Instruction::FAIL { .. } => 0,
            Instruction::CLOSURE { arity, .. } => *arity as usize + 2,
            Instruction::CALLC { .. } => 0,
            Instruction::PATT { .. } => 1,
            Instruction::LOADREF { .. } | Instruction::HALT => 0,
        }
    }
}
