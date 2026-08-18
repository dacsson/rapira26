//! VM Interpreter

use crate::object::{Object, ObjectError};
use crate::{
    isPtr,
    RAP_IS_SMI, RAP_abs, RAP_add, RAP_and, RAP_create_slice, RAP_dec_ref, RAP_divide, RAP_equal,
    RAP_floor, RAP_floor_divide, RAP_get_tuple_item, RAP_greater_or_equal, RAP_greater_than,
    RAP_inc_ref, RAP_index_of, RAP_input_value, RAP_length, RAP_less_or_equal, RAP_less_than,
    RAP_modulo, RAP_multiply, RAP_negate, RAP_not, RAP_not_equal, RAP_or, RAP_power, RAP_round,
    RAP_set_tuple_item, RAP_sign, RAP_slice_assign, RAP_sqrt, RAP_stringify_object, RAP_subtract,
};
use core::array;
use core::ffi::CStr;
use std::collections::HashMap;
use std::ffi::CString;
use vm_core::bytecode::{
    Builtin, CompareJumpKind, Instruction, LWRITE_NEWLINE_FLAG, LWRITE_NEWLINE_MASK, Label, Op,
    UnaryOp, ValueRel,
};
use vm_core::decoder::{Decoder, DecoderError};

const MAX_SEXP_TAGLEN: usize = 10;

#[cfg(test)]
const MAX_OPERAND_STACK_SIZE: usize = 64;

#[cfg(not(test))]
const MAX_OPERAND_STACK_SIZE: usize = 1024 * 64; // 0x7fffffff;

const DISPATCH_TABLE: [fn(&mut Interpreter, instr: Instruction) -> Result<(), InterpreterError>;
    30] = [
    Interpreter::eval_nop,
    Interpreter::eval_end,
    Interpreter::eval_binop,
    Interpreter::eval_const,
    Interpreter::eval_string,
    Interpreter::eval_begin,
    Interpreter::eval_cbegin,
    Interpreter::eval_closure,
    Interpreter::eval_store,
    Interpreter::eval_load,
    Interpreter::eval_call,
    Interpreter::eval_call_builtin,
    Interpreter::eval_callc,
    Interpreter::eval_line,
    Interpreter::eval_drop,
    Interpreter::eval_dup,
    Interpreter::eval_swap,
    Interpreter::eval_jmp,
    Interpreter::eval_cjmp,
    Interpreter::eval_elem,
    Interpreter::eval_sts,
    Interpreter::eval_sta,
    Interpreter::eval_array,
    Interpreter::eval_bool,
    Interpreter::eval_unary,
    Interpreter::eval_constf,
    Interpreter::eval_tuple,
    Interpreter::eval_null,
    Interpreter::eval_label,
    Interpreter::eval_slice,
];

pub struct Interpreter {
    operand_stack: Vec<Object>,
    frame_pointer: usize,
    /// Bytefile decoder
    decoder: Decoder,
    /// Globals length
    global_areas_size: usize,
}

impl Interpreter {
    /// Create a new interpreter with operand stack filled with
    /// emulated call to main
    pub fn new(decoder: Decoder) -> Self {
        let mut operand_stack: Vec<Object> = Vec::new();

        // Put globals at the start of operand stack
        let global_areas_size = decoder.bf.global_area_size as usize;
        for _ in 0..global_areas_size {
            operand_stack.push(Object::new_empty());
        }

        // Emulating call to main
        // TODO: is this needed?
        operand_stack.push(Object::new_empty()); // CLOSURE_OBJ
        operand_stack.push(Object::new_boxed(2)); // ARGS_COUNT
        operand_stack.push(Object::new_empty()); // LOCALS_COUNT
        operand_stack.push(Object::new_empty()); // OLD_FRAME_POINTER
        operand_stack.push(Object::new_empty()); // OLD_IP
        operand_stack.push(Object::new_empty()); // ARGV
        operand_stack.push(Object::new_empty()); // ARGC
        operand_stack.push(Object::new_empty()); // CURR_IP

        let global_areas_size = decoder.bf.global_area_size as usize;

        Interpreter {
            operand_stack,
            frame_pointer: global_areas_size,
            decoder,
            global_areas_size,
        }
    }

    /// Main interpreter loop
    pub fn run(&mut self) -> Result<(), RunError> {
        self.decoder.ip = self.decoder.bf.main_offset as usize;

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;
        let err_instr = instr.clone();

        DISPATCH_TABLE[instr.discriminant() as usize](self, instr).map_err(
            |e| -> RunError {
                let global_offset = core::mem::size_of::<i32>()
                    + core::mem::size_of::<i32>()
                    + core::mem::size_of::<i32>()
                    + (core::mem::size_of::<i32>()
                        * 2
                        * self.decoder.bf.public_symbols_number as usize)
                    + self.decoder.bf.stringtab_size as usize
                    + self.decoder.ip;

                RunError::ErrorAtOffset(global_offset, e, err_instr)
            },
        )?;

        // Release every remaining owned reference so the
        // run ends with a clean heap
        for obj in self.operand_stack.drain(..) {
            Self::dec_ref_if_ptr(obj);
        }

        Ok(())
    }

    fn eval_bool(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::BOOL { value: index } = instr else {
            return Err(InterpreterError::NotEnoughArguments("BOOL"));
        };

        self.push(Object::new_bool(index))?;

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_line(&mut self, _: Instruction) -> Result<(), InterpreterError> {
        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_nop(&mut self, _: Instruction) -> Result<(), InterpreterError> {
        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_closure(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        // let Instruction::CLOSURE { offset, arity } = instr else {
        //     return Err(InterpreterError::InvalidObjectPointer);
        // };

        // let offset_at = offset as usize;

        // let length = arity as usize + 1; // + 1 for offset

        // // Push offset - which is a first element to args of Bsexp
        // self.push(Object::new_boxed(offset as i64))?;

        // // Read captured variables description from code section
        // for i in 0..arity as usize {
        //     let desc = CapturedVar {
        //         rel: ValueRel::try_from(self.decoder.next::<u8>()?)
        //             .map_err(|_| InterpreterError::InvalidValueRel)?,
        //         index: self.decoder.next::<i32>()?,
        //     };

        //     // Push captures
        //     match desc.rel {
        //         ValueRel::Arg => {
        //             let frame =
        //                 FrameMetadata::get_from_stack(&self.operand_stack.0, self.frame_pointer)
        //                     .ok_or(InterpreterError::NotEnoughArguments(
        //                         "trying to create closure frame",
        //                     ))?;
        //             let obj = frame
        //                 .get_arg_at(
        //                     &self.operand_stack.0,
        //                     self.frame_pointer,
        //                     desc.index as usize,
        //                 )
        //                 .ok_or(InterpreterError::NotEnoughArguments(
        //                     "trying to create closure frame, no function argument found",
        //                 ))?;
        //             self.push(obj.clone())?;
        //         }
        //         ValueRel::Capture => unsafe {
        //             let mut frame =
        //                 FrameMetadata::get_from_stack(&self.operand_stack.0, self.frame_pointer)
        //                     .ok_or(InterpreterError::NotEnoughArguments(
        //                         "trying to create closure frame",
        //                     ))?;

        //             let closure = frame
        //                 .get_closure(&mut self.operand_stack.0, self.frame_pointer)
        //                 .unwrap();

        //             let to_data = rtToData(
        //                 closure
        //                     .as_ptr_mut()
        //                     .ok_or(InterpreterError::InvalidObjectPointer)?,
        //             );

        //             let element = get_captured_variable(&*to_data, desc.index as usize);

        //             self.push(Object::new_boxed(element))?;
        //         },
        //         ValueRel::Global => {
        //             let value = self.globals()[desc.index as usize].clone();
        //             self.push(value.clone())?;
        //         }
        //         ValueRel::Local => {
        //             let frame =
        //                 FrameMetadata::get_from_stack(&self.operand_stack.0, self.frame_pointer)
        //                     .ok_or(InterpreterError::NotEnoughArguments(
        //                         "trying to create closure frame",
        //                     ))?;

        //             let obj = frame
        //                 .get_local_at(
        //                     &self.operand_stack.0,
        //                     self.frame_pointer,
        //                     desc.index as usize,
        //                 )
        //                 .ok_or(InterpreterError::NotEnoughArguments(
        //                     "trying to create closure frame",
        //                 ))?;
        //             self.push(obj.clone())?;
        //         }
        //     }
        // }

        // // Create a new closure object
        // let borrow_operand_stack_elements =
        //     &mut self.operand_stack.0[self.operand_stack_len - length + 1..=self.operand_stack_len];

        // let closure = new_closure(borrow_operand_stack_elements);

        // // Pop arguments from the stack
        // for _ in 0..length {
        //     self.pop()?;
        // }

        // let mut closure_obj =
        //     Object::try_from(closure).map_err(|_| InterpreterError::InvalidObjectPointer)?;

        // self.push(closure_obj)?;
        // let encoding = self.decoder.next::<u8>()?;
        // let instr = self.decoder.decode(encoding)?;

        todo!();

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_callc(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        // let Instruction::CALLC { arity } = instr else {
        //     return Err(InterpreterError::InvalidObjectPointer);
        // };

        // let arity = arity as usize;

        // let mut obj = self.take(arity)?;

        // // check for closure
        // #[cfg(feature = "runtime_checks")]
        // let Some(lama_type) = obj.lama_type() else {
        //     return Err(InterpreterError::InvalidObjectPointer);
        // };

        // // check for closure type
        // #[cfg(feature = "runtime_checks")]
        // if lama_type != lama_type_CLOSURE {
        //     return Err(InterpreterError::InvalidType(
        //         "expected closure object at top of the stack to call a closure",
        //     ));
        // }

        // // Push old instruction pointer
        // // `CBEGIN` instruction will collect it
        // self.push(Object::new_boxed(self.decoder.ip as i64))?;

        // // Re-push closure object
        // // `CBEGIN` instruction will collect it
        // // self.push(obj.clone())?;

        // unsafe {
        //     let to_data = rtToData(
        //         obj.as_ptr_mut()
        //             .ok_or(InterpreterError::InvalidObjectPointer)?,
        //     );
        //     // First element in closure object is the offset
        //     self.decoder.ip = get_array_el(&*to_data, 0) as usize;
        // }

        // // Push closure object onto operand stack
        // self.push(obj)?;

        // let encoding = self.decoder.next::<u8>()?;
        // let instr = self.decoder.decode(encoding)?;

        todo!();

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_load_ref(&mut self, _: Instruction) -> Result<(), InterpreterError> {
        panic!("You shouldn't be here")
    }

    fn eval_array(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        // let Instruction::ARRAY { n } = instr else {
        //     return Err(InterpreterError::InvalidObjectPointer);
        // };

        // let mut obj = self.pop()?;

        // let Some(lama_type) = obj.lama_type() else {
        //     self.push(Object::new_boxed(0))?;

        //     let encoding = self.decoder.next::<u8>()?;
        //     let instr = self.decoder.decode(encoding)?;

        //     become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
        // };

        // // check aggregate type
        // if lama_type != lama_type_ARRAY {
        //     self.push(Object::new_boxed(0))?;
        // } else {
        //     // get length of array
        //     let length = unsafe {
        //         Llength(
        //             obj.as_ptr_mut()
        //                 .ok_or(InterpreterError::InvalidObjectPointer)?,
        //         )
        //     };

        //     // check length
        //     if rtUnbox(length) as i32 == n {
        //         self.push(Object::new_boxed(1))?;
        //     } else {
        //         self.push(Object::new_boxed(0))?;
        //     }
        // }

        // let encoding = self.decoder.next::<u8>()?;
        // let instr = self.decoder.decode(encoding)?;

        todo!();

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_elem(&mut self, _: Instruction) -> Result<(), InterpreterError> {
        let index_obj = self.pop()?;
        let collection = self.pop()?;
        let index = index_obj.unbox();

        // Check bounds here because the runtime accepts an unsigned index and
        // text lookup otherwise reaches into its backing storage directly
        let length = Object::new(unsafe { RAP_length(collection.raw()) }).unbox();
        if index < 0 || index >= length {
            return Err(InterpreterError::OutOfBoundsAccess(
                index as usize,
                length as usize,
            ));
        }

        let element =
            unsafe { RAP_get_tuple_item(collection.raw(), u32::try_from(index).unwrap()) };
        self.push(Object::new(element))?;

        Self::dec_ref_if_ptr(collection);
        Self::dec_ref_if_ptr(index_obj);

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_sts(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::STS = instr else {
            return Err(InterpreterError::InvalidOpcode(instr.discriminant()));
        };

        // Stack: [value, slice] (bottom→top), so pop slice first (top),
        // then value (next down).
        let slice = self.pop()?;
        let value = self.pop()?;

        unsafe { RAP_slice_assign(slice.raw(), value.raw()) };

        // TODO: is this needed?
        Self::dec_ref_if_ptr(slice);
        Self::dec_ref_if_ptr(value);

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_cjmp(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::CJMP { dest, kind } = instr else {
            return Err(InterpreterError::InvalidObjectPointer);
        };

        let Some(offset_at) = dest.offset else {
            return Err(InterpreterError::UnknownLabel(dest.name));
        };

        let obj = self.pop()?;
        let Some(value) = obj.get_bool() else {
            return Err(InterpreterError::InvalidType("expected Bool"));
        };

        match kind {
            CompareJumpKind::ISNONZERO => {
                if value {
                    self.decoder.ip = offset_at as usize;
                }
            }
            CompareJumpKind::ISZERO => {
                if !value {
                    self.decoder.ip = offset_at as usize;
                }
            }
        }

        // println!(
        //     "cjmp {:?} called to {} | {:x} | {}",
        //     kind, offset_at, self.decoder.bf.code_section[offset_at as usize], value
        // );

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_call_builtin(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        // println!("in eval_call_builtin: {:?}", instr);

        let Instruction::CALLBUILTIN { name, n } = instr else {
            return Err(InterpreterError::InvalidObjectPointer);
        };

        match name {
            Builtin::Barray => unsafe {
                // let length = n as usize;

                // let borrow_operand_stack_elements = &mut self.operand_stack.0
                //     [self.operand_stack_len - length + 1..=self.operand_stack_len];
                // let array = new_array(borrow_operand_stack_elements);

                // // remove args
                // for _ in 0..length {
                //     self.pop()?;
                // }

                // self.push(
                //     Object::try_from(array).map_err(|_| InterpreterError::InvalidObjectPointer)?,
                // )?;

                todo!()
            },
            Builtin::Llength => {
                let obj = self.pop()?;

                // RAP_length returns RAP_Value (boxed SMI)
                let length = unsafe { RAP_length(obj.raw()) };
                // Consume the popped argument. If a result-style alias with the
                // same raw pointer is pushed below, its dec is guarded.
                Self::dec_ref_if_ptr(obj);

                self.push(Object::new(length))?;
            }
            Builtin::Lread => unsafe {
                // Parses a line of stdin into a typed RAP_Value (int/float/text).
                self.push(Object::new(RAP_input_value()))?;
            },
            Builtin::Lwrite => unsafe {
                // `n` packs the newline flag (bit 30) with the real argument count
                let newline_bit = n & LWRITE_NEWLINE_FLAG;
                let n = n & LWRITE_NEWLINE_MASK;
                let len = self.operand_stack.len();

                if n != 0 {
                    let borrow_operand_stack_elements =
                        &mut self.operand_stack[len - (n as usize)..len];
                    for obj in borrow_operand_stack_elements {
                        let cstr = CString::from_raw(RAP_stringify_object(obj.raw()));
                        libc::printf(cstr.as_ptr());
                    }
                    if newline_bit != 0 {
                        libc::printf(c"\n".as_ptr());
                    }
                }
                for _ in 0..n {
                    let obj = self.pop()?;
                    // `printf` borrows the string only for the duration of the
                    // call
                    // once printed, the argument's owning reference must be released.
                    Self::dec_ref_if_ptr(obj);
                }
            },
            Builtin::Lstring => {
                todo!();
                // let obj = self.pop()?;

                // let mut slice: [i64; 1] = [obj.raw()];

                // unsafe {
                //     let ptr = Lstring(slice.as_mut_ptr());
                //     let contents = (*rtToData(ptr)).contents.as_ptr();

                //     self.push(
                //         Object::try_from(contents)
                //             .map_err(|_| InterpreterError::InvalidStringPointer)?,
                //     )?;
                // }
            }
            Builtin::Abs | Builtin::Sign | Builtin::Sqrt | Builtin::Floor | Builtin::Round => {
                if n != 1 {
                    return Err(InterpreterError::NotEnoughArguments(
                        "unary arithmetic builtin",
                    ));
                }
                let value = self.pop()?;
                let result = unsafe {
                    match name {
                        Builtin::Abs => RAP_abs(value.raw()),
                        Builtin::Sign => RAP_sign(value.raw()),
                        Builtin::Sqrt => RAP_sqrt(value.raw()),
                        Builtin::Floor => RAP_floor(value.raw()),
                        Builtin::Round => RAP_round(value.raw()),
                        _ => unreachable!(),
                    }
                };
                // The operand was a reference onto the stack - release it once
                // the call has consumed it (unless the result aliases it?)
                Self::dec_ref_if_ptr(value);
                self.push(Object::new(result))?;
            }
            Builtin::Index => {
                if n != 2 {
                    return Err(InterpreterError::NotEnoughArguments("индекс"));
                }
                let haystack = self.pop()?;
                let needle = self.pop()?;
                let result = unsafe { RAP_index_of(needle.raw(), haystack.raw()) };
                Self::dec_ref_if_ptr(haystack);
                Self::dec_ref_if_ptr(needle);
                self.push(Object::new(result))?;
            }
            Builtin::Tint => {
                let value = self.pop()?;
                let result = unsafe { RAP_IS_SMI(value.raw()) };
                Self::dec_ref_if_ptr(value);
                self.push(Object::new_bool(result))?;
            }
        }

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_call(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::CALL {
            dest: Label { name, offset },
            ..
        } = instr
        else {
            return Err(InterpreterError::InvalidObjectPointer);
        };

        // Push old instruction pointer
        // `BEGIN` instruction will collect it
        self.push(Object::new_boxed(self.decoder.ip as i64))?;

        // // Push empty closure object
        // self.push(Object::new_empty())?;

        self.decoder.ip = offset.unwrap() as usize;

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_swap(&mut self, _: Instruction) -> Result<(), InterpreterError> {
        let value1 = self.pop()?;
        let value2 = self.pop()?;
        self.push(value1)?;
        self.push(value2)?;

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_dup(&mut self, _: Instruction) -> Result<(), InterpreterError> {
        let value = self.pop()?;
        self.push(value)?;
        self.push(value)?;

        Self::inc_ref_if_ptr(value);

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_drop(&mut self, _: Instruction) -> Result<(), InterpreterError> {
        let obj = self.pop()?;
        Self::dec_ref_if_ptr(obj);

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_load(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        // println!("LOAD called at {}", self.decoder.ip);

        let Instruction::LOAD { rel, index } = instr else {
            return Err(InterpreterError::InvalidObjectPointer);
        };

        // FIXME: not unwrap
        let value = match rel {
            ValueRel::Arg => self.load_arg(index as usize)?,
            ValueRel::Capture => unsafe {
                todo!()
                // let closure = frame
                //     .get_closure(&mut self.operand_stack.0, self.frame_pointer)
                //     .unwrap();

                // let to_data = rtToData(
                //     closure
                //         .as_ptr_mut()
                //         .ok_or(InterpreterError::InvalidObjectPointer)?,
                // );

                // let element = get_captured_variable(&*to_data, index as usize);

                // self.push(Object::new_boxed(element))?;
            },
            ValueRel::Global => self.globals()[index as usize],
            ValueRel::Local => self.load_local(index as usize)?,
        };

        // Because we create an alias, we must inc the refcount
        Self::inc_ref_if_ptr(value);
        self.push(value)?;

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_store(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        // println!("STORE called at {}", self.decoder.ip);

        let Instruction::STORE { rel, index } = instr else {
            return Err(InterpreterError::InvalidObjectPointer);
        };

        let value = self.pop()?;
        let old = match rel {
            ValueRel::Arg => self.store_arg(index as usize, value)?,
            ValueRel::Capture => unsafe {
                todo!()
                // let closure = frame
                //     .get_closure(&mut self.operand_stack, self.frame_pointer)
                //     .unwrap();

                // let to_data = rtToData(
                //     closure
                //         .as_ptr_mut()
                //         .ok_or(InterpreterError::InvalidObjectPointer)?,
                // );

                // set_captured_variable(&mut *to_data, index as usize, value.raw());
            },
            ValueRel::Global => {
                let old = self.globals()[index as usize];
                self.globals_mut()[index as usize] = value;
                old
            }
            ValueRel::Local => self.store_local(index as usize, value)?,
        };

        // STORE is a transfer: the values one reference moves from the temp
        // stack slot to the destination variable slot, so we only release the
        // previous occupant (own count stays unchanged)
        if old.raw() != value.raw() {
            Self::dec_ref_if_ptr(old);
        }

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_end(&mut self, _: Instruction) -> Result<(), InterpreterError> {
        // Get procedures return value
        let return_value = self
            .pop()
            .map_err(|_| InterpreterError::NotEnoughArguments("END"))?;

        let n_locals = self.frame_local_count()?;
        let n_args = self.frame_arg_count()?;
        let ret_frame_pointer = self.frame_return_pointer()?;
        let ret_ip = self.frame_return_ip()?;

        for _ in 0..n_locals {
            let obj = self.pop()?;
            Self::dec_ref_if_ptr(obj);
        }

        // Pop return ip
        self.pop()?;

        // Pop old frame pointer
        self.pop()?;

        // Pop local count
        self.pop()?;

        // Pop argument count
        self.pop()?;

        // Pop closure object
        self.pop()?;

        for _ in 0..n_args {
            let obj = self.pop()?;
            Self::dec_ref_if_ptr(obj);
        }

        // Return to callee's frame pointer
        self.frame_pointer = ret_frame_pointer;

        // Return to caller's instruction pointer
        // NOTE: returning from main is not possible in this implementation
        //       the program will exit after the main function returns
        self.decoder.ip = ret_ip;

        // After removing current frames metadata,
        // we can re-push the return value to send it back to the caller
        self.push(return_value)?;

        // if we encounter END instruction, while in frame 0
        // (a.k.a main function) we exit the interpreter
        if self.frame_pointer == self.global_areas_size {
            return Ok(());
        }

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_begin(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        // println!("eval_begin: {:?}", instr);

        let Instruction::BEGIN {
            args: payload,
            locals,
            ..
        } = instr
        else {
            return Err(InterpreterError::InvalidObjectPointer);
        };

        let stack_size_for_function = payload >> 16;
        let args = (payload & 0xFFFF) as usize;

        if self.operand_stack.len() + stack_size_for_function as usize > MAX_OPERAND_STACK_SIZE {
            return Err(InterpreterError::StackOverflow);
        }

        // let closure_obj = self
        //     .pop()
        //     .map_err(|_| InterpreterError::NotEnoughArguments("BEGIN"))?;

        // Top object is either return_ip or a closure obj
        let ret_ip = self
            .pop()
            .map_err(|_| InterpreterError::NotEnoughArguments("BEGIN"))?; // must be a closure

        // Save previous frame pointer
        let ret_frame_pointer = self.frame_pointer;

        // Set new frame pointer as index into operand stack
        if self.operand_stack.is_empty() {
            return Err(InterpreterError::NotEnoughArguments("BEGIN"));
        }
        self.frame_pointer = self.operand_stack.len();

        // the closure slot must still exist for the frame layout expected
        self.push(Object::new_empty())?;

        // Push arg and local count
        self.push(Object::new_boxed(args as i64))?;
        self.push(Object::new_boxed(locals as i64))?;

        // Push return frame pointer and ip
        // 1. Where to return in sack operand
        self.push(Object::new_boxed(ret_frame_pointer as i64))?;
        // 2. Where to return in the bytecode after this call
        self.push(ret_ip)?;

        // Initialize local variables with 0
        // We create them as boxed objects
        for _ in 0..locals {
            self.push(Object::new_boxed(0))?;
        }

        // TODO:
        // let mut frame = FrameMetadata::get_from_stack(&self.operand_stack.0, self.frame_pointer)
        //     .ok_or(InterpreterError::NotEnoughArguments(
        //         "trying to call closure frame",
        //     ))?;
        // frame.save_closure(&mut self.operand_stack.0, self.frame_pointer, closure_obj);

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        // println!("eval_begin: {:?} {}", instr, instr.discriminant());

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_cbegin(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        // let Instruction::CBEGIN {
        //     args: payload,
        //     locals,
        //     ..
        // } = instr
        // else {
        //     return Err(InterpreterError::InvalidObjectPointer);
        // };

        // let stack_size_for_function = payload >> 16;
        // let args = (payload & 0xFFFF) as usize;

        // // Top object is a closure obj
        // let closure_obj = self
        //     .pop()
        //     .map_err(|_| InterpreterError::NotEnoughArguments("BEGIN"))?;

        // let ret_ip = self
        //     .pop()
        //     .map_err(|_| InterpreterError::NotEnoughArguments("BEGIN"))?;

        // let frame_closure_copy = closure_obj.clone();

        // // Save previous frame pointer
        // let ret_frame_pointer = self.frame_pointer;

        // // Set new frame pointer as index into operand stack
        // #[cfg(feature = "runtime_checks")]
        // if self.operand_stack.0.is_empty() {
        //     return Err(InterpreterError::NotEnoughArguments("BEGIN"));
        // }
        // self.frame_pointer = self.operand_stack_len + 1;

        // // Push closure object onto operand stack
        // self.push(closure_obj)?;

        // // Push arg and local count
        // self.push(Object::new_boxed(args as i64))?;
        // self.push(Object::new_boxed(locals as i64))?;

        // // Push return frame pointer and ip
        // // 1. Where to return in sack operand
        // self.push(Object::new_boxed(ret_frame_pointer as i64))?;
        // // 2. Where to return in the bytecode after this call
        // self.push(ret_ip)?;

        // // Initialize local variables with 0
        // // We create them as boxed objects
        // for _ in 0..locals {
        //     self.push(Object::new_boxed(0))?;
        // }

        // let mut frame = FrameMetadata::get_from_stack(&self.operand_stack.0, self.frame_pointer)
        //     .ok_or(InterpreterError::NotEnoughArguments(
        //         "trying to call closure frame",
        //     ))?;
        // frame.save_closure(&mut self.operand_stack.0, self.frame_pointer, closure_obj);

        // let encoding = self.decoder.next::<u8>()?;
        // let instr = self.decoder.decode(encoding)?;

        // become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)

        todo!();
    }

    fn eval_sta(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::STA = instr else {
            return Err(InterpreterError::NotEnoughArguments("STI"));
        };

        let aggregate = self.pop()?;
        let index = self.pop()?;
        let value = self.pop()?;

        let obj = unsafe { RAP_set_tuple_item(aggregate.raw(), index.unbox() as u32, value.raw()) };

        self.push(Object::new(obj))?;

        Self::dec_ref_if_ptr(aggregate);
        Self::dec_ref_if_ptr(index);
        Self::dec_ref_if_ptr(value);

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_jmp(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::JMP { dest } = instr else {
            return Err(InterpreterError::NotEnoughArguments("JMP"));
        };

        // NOTE: Frame shifting is delegated to `BEGIN` instruction

        let Some(offset_at) = dest.offset else {
            return Err(InterpreterError::UnknownLabel(dest.name.clone()));
        };

        // println!(
        //     "jmp called to {} | {:x}",
        //     offset_at, self.decoder.bf.code_section[offset_at as usize]
        // );

        self.decoder.ip = offset_at as usize;

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_string(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::STRING { index } = instr else {
            return Err(InterpreterError::NotEnoughArguments("STRING"));
        };

        let string_bytes = self
            .decoder
            .bf
            .get_string_at_offset(index as usize)
            .map_err(|_| InterpreterError::StringIndexOutOfBounds)?;
        let string = CStr::from_bytes_with_nul(string_bytes)
            .map_err(|_| InterpreterError::InvalidCString)?;

        self.push(Object::new_string(string))?;

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_const(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::CONST { value: index } = instr else {
            return Err(InterpreterError::NotEnoughArguments("CONST"));
        };

        self.push(Object::new_boxed(index as i64))?;

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        // println!("eval_const next: {:?} {}", instr, instr.discriminant());

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_binop(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        // println!("binop called at {}", self.decoder.ip);

        let Instruction::BINOP { op } = instr else {
            return Err(InterpreterError::NotEnoughArguments("BINOP"));
        };

        let right = self.pop()?;
        let left = self.pop()?;

        // unsafe {
        //     print!(
        //         "left: {:#?} ",
        //         CStr::from_ptr(RAP_stringify_object(left.raw()))
        //     );
        //     print!(" {:#?} ", op);
        //     println!(
        //         "right: {:#?}",
        //         CStr::from_ptr(RAP_stringify_object(right.raw()))
        //     );
        // }

        if matches!(op, Op::DIV | Op::MOD | Op::IDIV) && right.unbox() == 0 {
            return Err(InterpreterError::DivisionByZero);
        }

        let result = unsafe {
            match op {
                Op::ADD => RAP_add(left.raw(), right.raw()),
                Op::SUB => RAP_subtract(left.raw(), right.raw()),
                Op::MUL => RAP_multiply(left.raw(), right.raw()),
                Op::DIV => RAP_divide(left.raw(), right.raw()),
                Op::MOD => RAP_modulo(left.raw(), right.raw()),
                Op::LT => RAP_less_than(left.raw(), right.raw()),
                Op::LEQ => RAP_less_or_equal(left.raw(), right.raw()),
                Op::GT => RAP_greater_than(left.raw(), right.raw()),
                Op::GEQ => RAP_greater_or_equal(left.raw(), right.raw()),
                Op::EQ => RAP_equal(left.raw(), right.raw()),
                Op::NEQ => RAP_not_equal(left.raw(), right.raw()),
                Op::AND => RAP_and(left.raw(), right.raw()),
                Op::OR => RAP_or(left.raw(), right.raw()),
                Op::IDIV => RAP_floor_divide(left.raw(), right.raw()),
                Op::POW => RAP_power(left.raw(), right.raw()),
            }
        };

        // unsafe {
        //     println!(
        //         "result: {:#?} ",
        //         CStr::from_ptr(RAP_stringify_object(result))
        //     );
        // }

        Self::dec_ref_if_ptr(right);
        Self::dec_ref_if_ptr(left);

        self.push(Object::new(result))?;

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_unary(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::UNARY { op } = instr else {
            return Err(InterpreterError::NotEnoughArguments("UNARY"));
        };

        let value = self.pop()?;

        let result = unsafe {
            match op {
                UnaryOp::Negate => RAP_negate(value.raw()),
                UnaryOp::Not => RAP_not(value.raw()),
            }
        };

        self.push(Object::new(result))?;

        // Decref old value
        Self::dec_ref_if_ptr(value);

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_constf(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::CONSTF { value } = instr else {
            return Err(InterpreterError::NotEnoughArguments("CONSTF"));
        };

        self.push(Object::new_float(value))?;

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_tuple(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::TUPLE { n } = instr else {
            return Err(InterpreterError::NotEnoughArguments("TUPLE"));
        };

        if n == 0 {
            self.push(Object::new_tuple(0, &mut []))?;

            let encoding = self.decoder.next::<u8>()?;
            let instr = self.decoder.decode(encoding)?;

            become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
        }

        // Take n elements from stack
        let len = self.operand_stack.len();
        let borrow_operand_stack_elements = &mut self.operand_stack[len - (n as usize)..len];

        let obj = Object::new_tuple(n as usize, borrow_operand_stack_elements);

        // Pop arguments from the stack
        for _ in 0..n {
            self.pop()?;
        }

        self.push(obj)?;

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    /// Push null value on the operand stack
    fn eval_null(&mut self, _: Instruction) -> Result<(), InterpreterError> {
        self.push(Object::new_null())?;

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    /// Resolves a label to an offset
    ///
    /// FIXME: Can it appear here?
    fn eval_label(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        // let Instruction::LABEL { name } = instr else {
        //     return Err(InterpreterError::InvalidOpcode(instr.discriminant()));
        // };

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_slice(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::SLICE { bounds } = instr else {
            return Err(InterpreterError::InvalidOpcode(instr.discriminant()));
        };

        // Unlike other builtins, n is not an argument count here:
        // bit 0 says `from` was pushed, and bit 1 says `to` was pushed.
        let to = if bounds & 2 != 0 {
            self.pop()?.unbox()
        } else {
            -1
        };
        let from = if bounds & 1 != 0 {
            self.pop()?.unbox()
        } else {
            0
        };
        let collection = self.pop()?;
        let to = if to < 0 {
            Object::new(unsafe { RAP_length(collection.raw()) }).unbox()
        } else {
            to
        };
        let result = unsafe { RAP_create_slice(collection.raw(), from, to) };
        // slice keeps the parent alive via its own reference
        // (`inc_ref` on the parent) and returns a fresh slice object,
        // the operand we popped is now consumed so release it
        Self::dec_ref_if_ptr(collection);

        self.push(Object::new(result))?;

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    /// Push to the operand stack
    #[inline(always)]
    fn push(&mut self, obj: Object) -> Result<(), InterpreterError> {
        if self.operand_stack.len() >= MAX_OPERAND_STACK_SIZE {
            return Err(InterpreterError::StackOverflow);
        }

        if (self.operand_stack.len() - 1) <= self.global_areas_size {
            return Err(InterpreterError::StackUnderflow);
        }

        self.operand_stack.push(obj);

        Ok(())
    }

    /// Pop from the operand stack
    #[inline(always)]
    fn pop(&mut self) -> Result<Object, InterpreterError> {
        if (self.operand_stack.len() - 1) <= self.global_areas_size {
            return Err(InterpreterError::StackUnderflow);
        }

        let obj = self.operand_stack.pop().unwrap();
        Ok(obj)
    }

    #[inline(always)]
    fn inc_ref_if_ptr(obj: Object) {
        if isPtr(obj.raw()) {
            unsafe { RAP_inc_ref(obj.raw()) };
        }
    }

    #[inline(always)]
    fn dec_ref_if_ptr(obj: Object) {
        if isPtr(obj.raw()) {
            unsafe { RAP_dec_ref(obj.raw()) };
        }
    }

    #[inline(always)]
    fn frame_stack_value(&self, offset: usize, opname: &'static str) -> Result<Object, InterpreterError> {
        self.operand_stack
            .get(self.frame_pointer + offset)
            .copied()
            .ok_or(InterpreterError::NotEnoughArguments(opname))
    }

    #[inline(always)]
    fn frame_arg_count(&self) -> Result<usize, InterpreterError> {
        Ok(self.frame_stack_value(1, "frame arg count")?.unbox() as usize)
    }

    #[inline(always)]
    fn frame_local_count(&self) -> Result<usize, InterpreterError> {
        Ok(self.frame_stack_value(2, "frame local count")?.unbox() as usize)
    }

    #[inline(always)]
    fn frame_return_pointer(&self) -> Result<usize, InterpreterError> {
        Ok(self.frame_stack_value(3, "frame return pointer")?.unbox() as usize)
    }

    #[inline(always)]
    fn frame_return_ip(&self) -> Result<usize, InterpreterError> {
        Ok(self.frame_stack_value(4, "frame return ip")?.unbox() as usize)
    }

    #[inline(always)]
    fn arg_slot_index(&self, index: usize) -> Result<usize, InterpreterError> {
        let n_args = self.frame_arg_count()?;
        if index >= n_args {
            return Err(InterpreterError::NotEnoughArguments("LOAD/STORE arg"));
        }

        Ok(self.frame_pointer - n_args + index)
    }

    #[inline(always)]
    fn local_slot_index(&self, index: usize) -> Result<usize, InterpreterError> {
        let n_locals = self.frame_local_count()?;
        if index >= n_locals {
            return Err(InterpreterError::NotEnoughArguments("LOAD/STORE local"));
        }

        Ok(self.frame_pointer + 5 + index)
    }

    #[inline(always)]
    fn load_arg(&self, index: usize) -> Result<Object, InterpreterError> {
        let slot = self.arg_slot_index(index)?;
        self.operand_stack
            .get(slot)
            .copied()
            .ok_or(InterpreterError::NotEnoughArguments("LOAD arg"))
    }

    #[inline(always)]
    fn load_local(&self, index: usize) -> Result<Object, InterpreterError> {
        let slot = self.local_slot_index(index)?;
        self.operand_stack
            .get(slot)
            .copied()
            .ok_or(InterpreterError::NotEnoughArguments("LOAD local"))
    }

    #[inline(always)]
    fn store_arg(&mut self, index: usize, value: Object) -> Result<Object, InterpreterError> {
        let slot = self.arg_slot_index(index)?;
        let old = self.operand_stack[slot];
        self.operand_stack[slot] = value;
        Ok(old)
    }

    #[inline(always)]
    fn store_local(&mut self, index: usize, value: Object) -> Result<Object, InterpreterError> {
        let slot = self.local_slot_index(index)?;
        let old = self.operand_stack[slot];
        self.operand_stack[slot] = value;
        Ok(old)
    }

    /// Returns the current top object on the operand stack, if any
    pub fn peek(&self) -> Option<Object> {
        self.operand_stack.last().cloned()
    }

    /// Take from the operand stack at `index`, relative to the top of the stack
    /// removes the element and returns it
    fn take(&mut self, index: usize) -> Result<Object, InterpreterError> {
        if (self.operand_stack.len() - index - 1) <= self.global_areas_size {
            return Err(InterpreterError::StackUnderflow);
        }

        let relative_index = self.operand_stack.len() - index;
        let taken = self.operand_stack.remove(relative_index);

        // unsafe {
        //     // Move top pointer one object to the left
        //     __gc_stack_bottom = __gc_stack_bottom - core::mem::size_of::<Object>();
        // }
        // let relative_index = self.operand_stack_len - index;

        // let taken = self.operand_stack.0[relative_index].clone();

        // // Remove taken element and shift remaining elements down
        // if relative_index != self.operand_stack_len {
        //     self.operand_stack
        //         .0
        //         .copy_within(relative_index + 1..=self.operand_stack_len, relative_index);
        // }

        // self.operand_stack_len -= 1;

        Ok(taken)
    }

    /// Get global objects which occupy 0..global_size area in operand stack
    fn globals(&self) -> &[Object] {
        &self.operand_stack[0..self.global_areas_size]
    }

    fn globals_mut(&mut self) -> &mut [Object] {
        &mut self.operand_stack[0..self.global_areas_size]
    }
}

#[derive(Debug, PartialEq)]
pub enum InterpreterError {
    StackUnderflow,
    EndOfCodeSection,
    ReadingMoreThenCodeSection,
    InvalidOpcode(u8),
    InvalidType(&'static str),
    OutOfBoundsAccess(usize, usize),
    InvalidByteSequence(usize),
    StringIndexOutOfBounds,
    InvalidStringPointer,
    InvalidUtf8String,
    InvalidCString,
    InvalidObjectPointer,
    NotEnoughArguments(&'static str),
    InvalidLengthForArray,
    ObjectError(ObjectError),
    InvalidValueRel,
    DivisionByZero,
    SexpTagTooLong(usize),
    DecoderError(DecoderError),
    StackOverflow,
    UnknownLabel(String),
}

/// Convert a byte, that couldnt be incoded into an interpreter error.
impl From<u8> for InterpreterError {
    fn from(opcode: u8) -> Self {
        InterpreterError::InvalidOpcode(opcode)
    }
}

impl From<ObjectError> for InterpreterError {
    fn from(err: ObjectError) -> Self {
        InterpreterError::ObjectError(err)
    }
}

impl From<DecoderError> for InterpreterError {
    fn from(err: DecoderError) -> Self {
        InterpreterError::DecoderError(err)
    }
}

impl core::fmt::Display for InterpreterError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            InterpreterError::StackUnderflow => write!(f, "Stack underflow"),
            InterpreterError::EndOfCodeSection => write!(f, "End of code section"),
            InterpreterError::ReadingMoreThenCodeSection => {
                write!(f, "Reading more bytes than code section currently has")
            }
            InterpreterError::InvalidOpcode(opcode) => write!(f, "Invalid opcode: {:#x}", opcode),
            InterpreterError::InvalidType(name) => write!(f, "Invalid type: {}", name),
            InterpreterError::OutOfBoundsAccess(index, length) => write!(
                f,
                "Out of bounds access at index {} with length {}",
                index, length
            ),
            InterpreterError::InvalidByteSequence(ip) => {
                write!(f, "Invalid byte sequence at index {}", ip)
            }
            InterpreterError::StringIndexOutOfBounds => {
                write!(f, "String index out of bounds")
            }
            InterpreterError::InvalidStringPointer => {
                write!(f, "Invalid string pointer")
            }
            InterpreterError::InvalidUtf8String => {
                write!(f, "Invalid UTF-8 string")
            }
            InterpreterError::InvalidCString => {
                write!(f, "Invalid C string")
            }
            InterpreterError::InvalidObjectPointer => {
                write!(f, "Invalid object pointer")
            }
            InterpreterError::NotEnoughArguments(instr) => {
                write!(f, "Not enough arguments for instruction `{}`", instr)
            }
            InterpreterError::InvalidLengthForArray => {
                write!(f, "Invalid length for array")
            }
            InterpreterError::ObjectError(err) => {
                write!(f, "Object creation error: {}", err)
            }
            InterpreterError::InvalidValueRel => {
                write!(
                    f,
                    "Invalid value relation, there is only: Global(0), Local(1), Argument(2) and Captured(3), encountered something else"
                )
            }
            InterpreterError::DivisionByZero => {
                write!(f, "Division by zero")
            }
            InterpreterError::SexpTagTooLong(len) => {
                write!(f, "Sexp tag too long: {}, max is {}", len, MAX_SEXP_TAGLEN)
            }
            InterpreterError::DecoderError(err) => {
                write!(f, "Decoder error: {}", err)
            }
            InterpreterError::StackOverflow => {
                write!(f, "Stack overflow")
            }
            InterpreterError::UnknownLabel(name) => {
                write!(f, "Unknown label: {}", name)
            }
        }
    }
}

impl core::error::Error for InterpreterError {}

#[derive(Debug)]
pub enum RunError {
    ErrorAtOffset(usize, InterpreterError, Instruction),
    DecoderError(DecoderError),
}

impl core::error::Error for RunError {}

impl core::fmt::Display for RunError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            RunError::ErrorAtOffset(offset, ie, instr) => write!(
                f,
                "Error at offset {}: {} \n  during evaluation of {:?}",
                offset, ie, instr
            ),
            RunError::DecoderError(err) => write!(f, "Decoder error: {}", err),
        }
    }
}

impl From<DecoderError> for RunError {
    fn from(err: DecoderError) -> Self {
        RunError::DecoderError(err)
    }
}
