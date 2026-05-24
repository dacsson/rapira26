//! VM Interpreter

use crate::frame::FrameMetadata;
use crate::object::{Object, ObjectError};
use crate::{
    __gc_init, __gc_stack_bottom, __gc_stack_top, Barray, Barray_tag_patt, Bboxed_patt,
    Bclosure_tag_patt, Bsexp_tag_patt, Bstring_patt, Bstring_tag_patt, Bunboxed_patt,
    CONS_TAG_HASH, Llength, Lread, Lstring, LtagHash, Lwrite, NIL_TAG_HASH, createStringBuf, exit,
    failure, get_array_el, get_captured_variable, get_sexp_el, lama_type_ARRAY, lama_type_CLOSURE,
    lama_type_SEXP, lama_type_STRING, new_array, new_closure, new_sexp, new_string, printValue,
    rtBox, rtToData, rtToSexp, rtUnbox, set_array_el, set_captured_variable, set_sexp_el,
    stringBuf,
};
use core::array;
use core::convert::TryFrom;
use core::ffi::{CStr, c_char};
use vm_core::bytecode::{
    Builtin, CapturedVar, CompareJumpKind, Instruction, Op, PattKind, ValueRel,
};
use vm_core::decoder::{Decoder, DecoderError};

const MAX_SEXP_TAGLEN: usize = 10;
const MAX_CAPTURES: usize = 0xffff; // 0x7fffffff;

#[cfg(test)]
const MAX_OPERAND_STACK_SIZE: usize = 8 * 8 * 1024; // 0xffff;

#[cfg(not(test))]
const MAX_OPERAND_STACK_SIZE: usize = 8 * 8 * 1024 * 4; // 0x7fffffff;

const MAX_ARG_LEN: usize = 50;

#[repr(align(16))]
struct OperandStack([Object; MAX_OPERAND_STACK_SIZE]);

#[derive(Debug, Clone)]
pub struct InstructionTrace {
    pub instruction: Instruction,
    pub offset: usize,
}

const DISPATCH_TABLE: [fn(&mut Interpreter, instr: Instruction) -> Result<(), InterpreterError>;
    30] = [
    Interpreter::eval_nop,
    Interpreter::eval_end,
    Interpreter::eval_end, // ret
    Interpreter::eval_binop,
    Interpreter::eval_const,
    Interpreter::eval_string,
    Interpreter::eval_begin,
    Interpreter::eval_cbegin,
    Interpreter::eval_closure,
    Interpreter::eval_store,
    Interpreter::eval_load,
    Interpreter::eval_load_ref,
    Interpreter::eval_call,
    Interpreter::eval_call_builtin,
    Interpreter::eval_callc,
    Interpreter::eval_fail,
    Interpreter::eval_line,
    Interpreter::eval_drop,
    Interpreter::eval_dup,
    Interpreter::eval_swap,
    Interpreter::eval_jmp,
    Interpreter::eval_cjmp,
    Interpreter::eval_elem,
    Interpreter::eval_sti,
    Interpreter::eval_sta,
    Interpreter::eval_sexp,
    Interpreter::eval_tag,
    Interpreter::eval_patt,
    Interpreter::eval_array,
    Interpreter::eval_halt,
];

pub struct Interpreter {
    operand_stack: OperandStack,
    operand_stack_len: usize,
    frame_pointer: usize,
    /// Bytefile decoder
    decoder: Decoder,
    /// Code section length
    code_section_len: usize,
    /// Globals length
    global_areas_size: usize,
}

impl Interpreter {
    /// Create a new interpreter with operand stack filled with
    /// emulated call to main
    pub fn new(decoder: Decoder) -> Self {
        let mut operand_stack: OperandStack = OperandStack(array::repeat(Object::new_empty()));

        unsafe {
            __gc_init();
        }

        // Put globals at the start of operand stack
        let global_areas_size = decoder.bf.global_area_size as usize;
        for i in 0..global_areas_size {
            operand_stack.0[i] = Object::new_empty();
        }

        // Emulating call to main
        operand_stack.0[global_areas_size] = Object::new_empty(); // CLOSURE_OBJ
        operand_stack.0[global_areas_size + 1] = Object::new_unboxed(2); // ARGS_COUNT
        operand_stack.0[global_areas_size + 2] = Object::new_empty(); // LOCALS_COUNT
        operand_stack.0[global_areas_size + 3] = Object::new_empty(); // OLD_FRAME_POINTER
        operand_stack.0[global_areas_size + 4] = Object::new_empty(); // OLD_IP
        operand_stack.0[global_areas_size + 5] = Object::new_empty(); // ARGV
        operand_stack.0[global_areas_size + 6] = Object::new_empty(); // ARGC
        operand_stack.0[global_areas_size + 7] = Object::new_empty(); // CURR_IP

        unsafe {
            let ptr_top: *const Object = &operand_stack.0[global_areas_size + 8];
            __gc_stack_bottom = ptr_top as usize;

            let ptr_bottom: *const Object = &operand_stack.0[0];
            __gc_stack_top = ptr_bottom as usize;
        }

        let global_areas_size = decoder.bf.global_area_size as usize;
        let code_section_len = decoder.bf.code_section.len();

        Interpreter {
            operand_stack,
            operand_stack_len: global_areas_size + 8,
            frame_pointer: global_areas_size,
            decoder,
            code_section_len,
            global_areas_size,
        }
    }

    /// Main interpreter loop
    pub fn run(&mut self) -> Result<(), RunError> {
        if self.decoder.ip < self.code_section_len {
            let encoding = self.decoder.next::<u8>()?;
            let instr = self.decoder.decode(encoding)?;

            DISPATCH_TABLE[instr.discriminant() as usize](self, instr.clone()).map_err(
                |e| -> RunError {
                    let global_offset = core::mem::size_of::<i32>()
                        + core::mem::size_of::<i32>()
                        + core::mem::size_of::<i32>()
                        + (core::mem::size_of::<i32>()
                            * 2
                            * self.decoder.bf.public_symbols_number as usize)
                        + self.decoder.bf.stringtab_size as usize
                        + self.decoder.ip;

                    RunError::ErrorAtOffset(global_offset, e, instr.clone())
                },
            )?
        }

        Ok(())
    }

    fn eval_halt(&mut self, _: Instruction) -> Result<(), InterpreterError> {
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

    fn eval_fail(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::FAIL { line, column } = instr else {
            return Err(InterpreterError::InvalidObjectPointer);
        };

        let obj = self.pop()?;

        unsafe {
            let ptr = Lstring([obj.raw()].as_mut_ptr());
            let contents = (*rtToData(ptr)).contents.as_ptr();
            let c_str = CStr::from_ptr(contents);
            let string = c_str
                .to_str()
                .map_err(|_| InterpreterError::InvalidStringPointer)?;

            // IMPORTANT: This call ensures termination, therefore casting to mutable pointer is okay
            failure(
                "%d:%d: Failure: matching %s\n".as_ptr() as *mut i8,
                line as usize,
                column as usize,
                string.as_ptr(),
            );
        };

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_closure(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::CLOSURE { offset, arity } = instr else {
            return Err(InterpreterError::InvalidObjectPointer);
        };

        let offset_at = offset as usize;

        let length = arity as usize + 1; // + 1 for offset

        // Push offset - which is a first element to args of Bsexp
        self.push(Object::new_unboxed(offset as i64))?;

        // Read captured variables description from code section
        for i in 0..arity as usize {
            let desc = CapturedVar {
                rel: ValueRel::try_from(self.decoder.next::<u8>()?)
                    .map_err(|_| InterpreterError::InvalidValueRel)?,
                index: self.decoder.next::<i32>()?,
            };

            // Push captures
            match desc.rel {
                ValueRel::Arg => {
                    let frame =
                        FrameMetadata::get_from_stack(&self.operand_stack.0, self.frame_pointer)
                            .ok_or(InterpreterError::NotEnoughArguments(
                                "trying to create closure frame",
                            ))?;
                    let obj = frame
                        .get_arg_at(
                            &self.operand_stack.0,
                            self.frame_pointer,
                            desc.index as usize,
                        )
                        .ok_or(InterpreterError::NotEnoughArguments(
                            "trying to create closure frame, no function argument found",
                        ))?;
                    self.push(obj.clone())?;
                }
                ValueRel::Capture => unsafe {
                    let mut frame =
                        FrameMetadata::get_from_stack(&self.operand_stack.0, self.frame_pointer)
                            .ok_or(InterpreterError::NotEnoughArguments(
                                "trying to create closure frame",
                            ))?;

                    let closure = frame
                        .get_closure(&mut self.operand_stack.0, self.frame_pointer)
                        .unwrap();

                    let to_data = rtToData(
                        closure
                            .as_ptr_mut()
                            .ok_or(InterpreterError::InvalidObjectPointer)?,
                    );

                    let element = get_captured_variable(&*to_data, desc.index as usize);

                    self.push(Object::new_unboxed(element))?;
                },
                ValueRel::Global => {
                    let value = self.globals()[desc.index as usize].clone();
                    self.push(value.clone())?;
                }
                ValueRel::Local => {
                    let frame =
                        FrameMetadata::get_from_stack(&self.operand_stack.0, self.frame_pointer)
                            .ok_or(InterpreterError::NotEnoughArguments(
                                "trying to create closure frame",
                            ))?;

                    let obj = frame
                        .get_local_at(
                            &self.operand_stack.0,
                            self.frame_pointer,
                            desc.index as usize,
                        )
                        .ok_or(InterpreterError::NotEnoughArguments(
                            "trying to create closure frame",
                        ))?;
                    self.push(obj.clone())?;
                }
            }
        }

        // Create a new closure object
        let borrow_operand_stack_elements =
            &mut self.operand_stack.0[self.operand_stack_len - length + 1..=self.operand_stack_len];

        let closure = new_closure(borrow_operand_stack_elements);

        // Pop arguments from the stack
        for _ in 0..length {
            self.pop()?;
        }

        let mut closure_obj =
            Object::try_from(closure).map_err(|_| InterpreterError::InvalidObjectPointer)?;

        self.push(closure_obj)?;
        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_callc(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::CALLC { arity } = instr else {
            return Err(InterpreterError::InvalidObjectPointer);
        };

        let arity = arity as usize;

        let mut obj = self.take(arity)?;

        // check for closure
        #[cfg(feature = "runtime_checks")]
        let Some(lama_type) = obj.lama_type() else {
            return Err(InterpreterError::InvalidObjectPointer);
        };

        // check for closure type
        #[cfg(feature = "runtime_checks")]
        if lama_type != lama_type_CLOSURE {
            return Err(InterpreterError::InvalidType(
                "expected closure object at top of the stack to call a closure",
            ));
        }

        // Push old instruction pointer
        // `CBEGIN` instruction will collect it
        self.push(Object::new_unboxed(self.decoder.ip as i64))?;

        // Re-push closure object
        // `CBEGIN` instruction will collect it
        // self.push(obj.clone())?;

        unsafe {
            let to_data = rtToData(
                obj.as_ptr_mut()
                    .ok_or(InterpreterError::InvalidObjectPointer)?,
            );
            // First element in closure object is the offset
            self.decoder.ip = get_array_el(&*to_data, 0) as usize;
        }

        // Push closure object onto operand stack
        self.push(obj)?;

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_patt(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::PATT { kind } = instr else {
            return Err(InterpreterError::InvalidObjectPointer);
        };

        match kind {
            PattKind::BothAreStr => unsafe {
                let x = self.pop()?;
                let y = self.pop()?;

                let x_ptr = x
                    .as_ptr_mut()
                    .ok_or(InterpreterError::InvalidObjectPointer)?;
                let y_ptr = y
                    .as_ptr_mut()
                    .ok_or(InterpreterError::InvalidObjectPointer)?;

                let res = Bstring_patt(x_ptr, y_ptr);

                self.push(Object::new_unboxed(res))?;
            },
            PattKind::IsStr => unsafe {
                let obj = self.pop()?;
                let ptr = obj.as_ptr_mut_unchecked();

                let res = Bstring_tag_patt(ptr);

                self.push(Object::new_unboxed(res))?;
            },
            PattKind::IsArray => unsafe {
                let obj = self.pop()?;
                let ptr = obj.as_ptr_mut_unchecked();

                let res = Barray_tag_patt(ptr);

                self.push(Object::new_unboxed(res))?;
            },
            PattKind::IsSExp => unsafe {
                let obj = self.pop()?;
                let ptr = obj.as_ptr_mut_unchecked();

                let res = Bsexp_tag_patt(ptr);

                self.push(Object::new_unboxed(res))?;
            },
            PattKind::IsRef => unsafe {
                let obj = self.pop()?;
                let ptr = obj.as_ptr_mut_unchecked();

                let res = Bboxed_patt(ptr);

                self.push(Object::new_unboxed(res))?;
            },
            PattKind::IsVal => unsafe {
                let obj = self.pop()?;
                let ptr = obj.as_ptr_mut_unchecked();

                let res = Bunboxed_patt(ptr);

                self.push(Object::new_unboxed(res))?;
            },
            PattKind::IsLambda => unsafe {
                let obj = self.pop()?;
                let ptr = obj.as_ptr_mut_unchecked();

                let res = Bclosure_tag_patt(ptr);

                self.push(Object::new_unboxed(res))?;
            },
        }

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_load_ref(&mut self, _: Instruction) -> Result<(), InterpreterError> {
        panic!("You shouldn't be here")
    }

    fn eval_tag(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::TAG { index, n } = instr else {
            return Err(InterpreterError::InvalidObjectPointer);
        };

        let mut obj = self.pop()?;

        let Some(lama_type) = obj.lama_type() else {
            self.push(Object::new_boxed(0))?;

            let encoding = self.decoder.next::<u8>()?;
            let instr = self.decoder.decode(encoding)?;

            become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
        };

        // check aggregate type
        if lama_type != lama_type_SEXP {
            self.push(Object::new_boxed(0))?;
        } else {
            // get length of sexp
            let length = unsafe {
                Llength(
                    obj.as_ptr_mut()
                        .ok_or(InterpreterError::InvalidObjectPointer)?,
                )
            };

            // check length
            if rtUnbox(length) as i32 == n {
                let sexp = rtToSexp(
                    obj.as_ptr_mut()
                        .ok_or(InterpreterError::InvalidObjectPointer)?,
                );

                let tag_u8 = self
                    .decoder
                    .bf
                    .get_string_at_offset(index as usize)
                    .map_err(|_| InterpreterError::StringIndexOutOfBounds)?;

                let c_string = CStr::from_bytes_with_nul(tag_u8)
                    .map_err(|_| InterpreterError::InvalidCString)?;

                let hashed_string = if c_string.to_bytes() == "cons".as_bytes() {
                    CONS_TAG_HASH
                } else if c_string.to_bytes() == "nil".as_bytes() {
                    NIL_TAG_HASH
                } else {
                    unsafe { LtagHash(c_string.as_ptr() as *mut c_char) }
                };

                unsafe {
                    if rtBox((*sexp).tag as i64) == hashed_string {
                        self.push(Object::new_boxed(1))?;
                    } else {
                        self.push(Object::new_boxed(0))?;
                    }
                }
            } else {
                self.push(Object::new_boxed(0))?;
            }
        }

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_array(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::ARRAY { n } = instr else {
            return Err(InterpreterError::InvalidObjectPointer);
        };

        let mut obj = self.pop()?;

        let Some(lama_type) = obj.lama_type() else {
            self.push(Object::new_boxed(0))?;

            let encoding = self.decoder.next::<u8>()?;
            let instr = self.decoder.decode(encoding)?;

            become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
        };

        // check aggregate type
        if lama_type != lama_type_ARRAY {
            self.push(Object::new_boxed(0))?;
        } else {
            // get length of array
            let length = unsafe {
                Llength(
                    obj.as_ptr_mut()
                        .ok_or(InterpreterError::InvalidObjectPointer)?,
                )
            };

            // check length
            if rtUnbox(length) as i32 == n {
                self.push(Object::new_boxed(1))?;
            } else {
                self.push(Object::new_boxed(0))?;
            }
        }

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_elem(&mut self, _: Instruction) -> Result<(), InterpreterError> {
        let index_obj = self.pop()?;
        let mut obj = self.pop()?;

        let index = index_obj.unbox() as usize;

        // check for aggregate
        #[cfg(feature = "runtime_checks")]
        if obj.lama_type().is_none() {
            return Err(InterpreterError::InvalidType(
                "indexing into a type that is not an aggregate",
            ));
        }

        unsafe {
            let length = rtUnbox(Llength(obj.as_ptr_mut().unwrap())) as usize;

            // check for out of bounds access
            #[cfg(feature = "runtime_checks")]
            if (index_obj.unbox()) < 0 || index >= length {
                return Err(InterpreterError::OutOfBoundsAccess(index, length));
            }

            let as_ptr = obj
                .as_ptr_mut()
                .ok_or(InterpreterError::InvalidObjectPointer)?;

            let lama_type = obj.lama_type().unwrap();

            if lama_type == lama_type_SEXP {
                let sexp = rtToSexp(as_ptr);
                let element = get_sexp_el(&*sexp, index);

                self.push(Object::new_unboxed(element))?;
            } else if lama_type == lama_type_STRING {
                let contents = (*rtToData(as_ptr)).contents.as_ptr();

                let el = contents.add(index);

                self.push(Object::new_boxed(*el as i64))?;
            } else {
                let array = rtToData(as_ptr);
                let element = get_array_el(&*array, index);

                // push the boxed element onto the stack
                self.push(Object::new_unboxed(element))?;
            }
        }

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_cjmp(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::CJMP { offset, kind } = instr else {
            return Err(InterpreterError::InvalidObjectPointer);
        };

        let offset_at = offset as usize;

        match kind {
            CompareJumpKind::ISNONZERO => {
                let obj = self.pop()?;
                let value = obj.unbox();

                if value != 0 {
                    self.decoder.ip = offset_at;
                }
            }
            CompareJumpKind::ISZERO => {
                let obj = self.pop()?;
                let value = obj.unbox();

                if value == 0 {
                    self.decoder.ip = offset_at;
                }
            }
        }

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_call_builtin(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::CALLBUILTIN { name, n } = instr else {
            return Err(InterpreterError::InvalidObjectPointer);
        };

        match name {
            Builtin::Barray => unsafe {
                let length = n as usize;

                let borrow_operand_stack_elements = &mut self.operand_stack.0
                    [self.operand_stack_len - length + 1..=self.operand_stack_len];
                let array = new_array(borrow_operand_stack_elements);

                // remove args
                for _ in 0..length {
                    self.pop()?;
                }

                self.push(
                    Object::try_from(array).map_err(|_| InterpreterError::InvalidObjectPointer)?,
                )?;
            },
            Builtin::Llength => {
                let obj = self.pop()?;
                let as_ptr = obj
                    .as_ptr_mut()
                    .ok_or(InterpreterError::InvalidObjectPointer)?;

                unsafe {
                    // Llength returns a boxed length value
                    let length = Llength(as_ptr);
                    self.push(Object::new_boxed(rtUnbox(length)))?;
                }
            }
            Builtin::Lread => unsafe {
                let val = Lread();

                // Returns BOXED value
                self.push(Object::new_boxed(rtUnbox(val)))?;
            },
            Builtin::Lwrite => {
                let obj = self.pop()?;

                unsafe {
                    // Lwrite takes a boxed value
                    Lwrite(obj.raw());
                }

                self.push(obj)?;
            }
            Builtin::Lstring => {
                let obj = self.pop()?;

                let mut slice: [i64; 1] = [obj.raw()];

                unsafe {
                    let ptr = Lstring(slice.as_mut_ptr());
                    let contents = (*rtToData(ptr)).contents.as_ptr();

                    self.push(
                        Object::try_from(contents)
                            .map_err(|_| InterpreterError::InvalidStringPointer)?,
                    )?;
                }
            }
        }

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_call(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::CALL { offset, .. } = instr else {
            return Err(InterpreterError::InvalidObjectPointer);
        };

        // Push old instruction pointer
        // `BEGIN` instruction will collect it
        self.push(Object::new_unboxed(self.decoder.ip as i64))?;

        // Push empty closure object
        self.push(Object::new_empty())?;

        self.decoder.ip = offset as usize;

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
        self.push(value.clone())?;
        self.push(value)?;

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_drop(&mut self, _: Instruction) -> Result<(), InterpreterError> {
        self.pop()?;

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_load(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::LOAD { rel, index } = instr else {
            return Err(InterpreterError::InvalidObjectPointer);
        };

        let mut frame = FrameMetadata::get_from_stack(&self.operand_stack.0, self.frame_pointer)
            .ok_or(InterpreterError::NotEnoughArguments("STORE"))?;

        match rel {
            ValueRel::Arg => {
                let value = frame
                    .get_arg_at(&self.operand_stack.0, self.frame_pointer, index as usize)
                    .unwrap();

                self.push(value.clone())?;
            }
            ValueRel::Capture => unsafe {
                let closure = frame
                    .get_closure(&mut self.operand_stack.0, self.frame_pointer)
                    .unwrap();

                let to_data = rtToData(
                    closure
                        .as_ptr_mut()
                        .ok_or(InterpreterError::InvalidObjectPointer)?,
                );

                let element = get_captured_variable(&*to_data, index as usize);

                self.push(Object::new_unboxed(element))?;
            },
            ValueRel::Global => {
                let value = self.globals()[index as usize].clone();
                self.push(value)?;
            }
            ValueRel::Local => {
                let value = frame
                    .get_local_at(&self.operand_stack.0, self.frame_pointer, index as usize)
                    .unwrap();

                self.push(value.clone())?;
            }
        }

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_store(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::STORE { rel, index } = instr else {
            return Err(InterpreterError::InvalidObjectPointer);
        };

        let mut frame = FrameMetadata::get_from_stack(&self.operand_stack.0, self.frame_pointer)
            .ok_or(InterpreterError::NotEnoughArguments("STORE"))?;

        let value = self.pop()?;

        match rel {
            ValueRel::Arg => {
                frame
                    .set_arg_at(
                        &mut self.operand_stack.0,
                        self.frame_pointer,
                        index as usize,
                        value.clone(),
                    )
                    .unwrap();
            }
            ValueRel::Capture => unsafe {
                let closure = frame
                    .get_closure(&mut self.operand_stack.0, self.frame_pointer)
                    .unwrap();

                let to_data = rtToData(
                    closure
                        .as_ptr_mut()
                        .ok_or(InterpreterError::InvalidObjectPointer)?,
                );

                set_captured_variable(&mut *to_data, index as usize, value.raw());
            },
            ValueRel::Global => {
                self.globals_mut()[index as usize] = value.clone();
            }
            ValueRel::Local => frame
                .set_local_at(
                    &mut self.operand_stack.0,
                    self.frame_pointer,
                    index as usize,
                    value.clone(),
                )
                .unwrap(),
        }

        self.push(value)?;

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_end(&mut self, _: Instruction) -> Result<(), InterpreterError> {
        // Get procedures return value
        let return_value = self
            .pop()
            .map_err(|_| InterpreterError::NotEnoughArguments("END"))?;

        let FrameMetadata {
            closure_obj: _,
            n_locals,
            n_args,
            ret_frame_pointer,
            ret_ip,
        } = FrameMetadata::get_from_stack(&self.operand_stack.0, self.frame_pointer)
            .ok_or(InterpreterError::NotEnoughArguments("END"))?;

        for _ in 0..n_locals {
            self.pop()?;
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
            self.pop()?;
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

        if self.operand_stack_len + stack_size_for_function as usize > MAX_OPERAND_STACK_SIZE {
            return Err(InterpreterError::StackOverflow);
        }

        let closure_obj = self
            .pop()
            .map_err(|_| InterpreterError::NotEnoughArguments("BEGIN"))?;

        // Top object is either return_ip or a closure obj
        let ret_ip = self
            .pop()
            .map_err(|_| InterpreterError::NotEnoughArguments("BEGIN"))?; // must be a closure

        // Save previous frame pointer
        let ret_frame_pointer = self.frame_pointer;

        // Set new frame pointer as index into operand stack
        if self.operand_stack.0.is_empty() {
            return Err(InterpreterError::NotEnoughArguments("BEGIN"));
        }
        self.frame_pointer = self.operand_stack_len + 1;

        // Push closure object onto operand stack
        self.push(closure_obj)?;

        // Push arg and local count
        self.push(Object::new_unboxed(args as i64))?;
        self.push(Object::new_unboxed(locals as i64))?;

        // Push return frame pointer and ip
        // 1. Where to return in sack operand
        self.push(Object::new_unboxed(ret_frame_pointer as i64))?;
        // 2. Where to return in the bytecode after this call
        self.push(ret_ip)?;

        // Initialize local variables with 0
        // We create them as boxed objects
        for _ in 0..locals {
            self.push(Object::new_boxed(0))?;
        }

        let mut frame = FrameMetadata::get_from_stack(&self.operand_stack.0, self.frame_pointer)
            .ok_or(InterpreterError::NotEnoughArguments(
                "trying to call closure frame",
            ))?;
        frame.save_closure(&mut self.operand_stack.0, self.frame_pointer, closure_obj);

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_cbegin(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::CBEGIN {
            args: payload,
            locals,
            ..
        } = instr
        else {
            return Err(InterpreterError::InvalidObjectPointer);
        };

        let stack_size_for_function = payload >> 16;
        let args = (payload & 0xFFFF) as usize;

        // Top object is a closure obj
        let closure_obj = self
            .pop()
            .map_err(|_| InterpreterError::NotEnoughArguments("BEGIN"))?;

        let ret_ip = self
            .pop()
            .map_err(|_| InterpreterError::NotEnoughArguments("BEGIN"))?;

        let frame_closure_copy = closure_obj.clone();

        // Save previous frame pointer
        let ret_frame_pointer = self.frame_pointer;

        // Set new frame pointer as index into operand stack
        #[cfg(feature = "runtime_checks")]
        if self.operand_stack.0.is_empty() {
            return Err(InterpreterError::NotEnoughArguments("BEGIN"));
        }
        self.frame_pointer = self.operand_stack_len + 1;

        // Push closure object onto operand stack
        self.push(closure_obj)?;

        // Push arg and local count
        self.push(Object::new_unboxed(args as i64))?;
        self.push(Object::new_unboxed(locals as i64))?;

        // Push return frame pointer and ip
        // 1. Where to return in sack operand
        self.push(Object::new_unboxed(ret_frame_pointer as i64))?;
        // 2. Where to return in the bytecode after this call
        self.push(ret_ip)?;

        // Initialize local variables with 0
        // We create them as boxed objects
        for _ in 0..locals {
            self.push(Object::new_boxed(0))?;
        }

        let mut frame = FrameMetadata::get_from_stack(&self.operand_stack.0, self.frame_pointer)
            .ok_or(InterpreterError::NotEnoughArguments(
                "trying to call closure frame",
            ))?;
        frame.save_closure(&mut self.operand_stack.0, self.frame_pointer, closure_obj);

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_sti(&mut self, _: Instruction) -> Result<(), InterpreterError> {
        panic!(
            "Congratulations! Somehow, you emitted a STI instruction, while the compiler itself never should have"
        )
    }

    fn eval_sta(&mut self, _: Instruction) -> Result<(), InterpreterError> {
        let value_obj = self.pop()?;
        let index_obj = self.pop()?;
        let mut aggregate = self.pop()?;

        let index = index_obj.unbox() as usize;
        let value = value_obj.unbox();

        // check for aggregate
        #[cfg(feature = "runtime_checks")]
        if aggregate.lama_type().is_none() {
            return Err(InterpreterError::InvalidType(
                "Expected an aggregate type in STA instruction",
            ));
        }

        unsafe {
            let length = rtUnbox(Llength(aggregate.as_ptr_mut().unwrap())) as usize;

            // check for out of bounds access
            #[cfg(feature = "runtime_checks")]
            if (index_obj.unbox()) < 0 || index >= length {
                return Err(InterpreterError::OutOfBoundsAccess(index, length));
            }

            let as_ptr = aggregate
                .as_ptr_mut()
                .ok_or(InterpreterError::InvalidObjectPointer)?;

            let lama_type = aggregate.lama_type().unwrap();

            if lama_type == lama_type_SEXP {
                let sexp = rtToSexp(as_ptr);
                set_sexp_el(&mut *sexp, index, value);
            } else if lama_type == lama_type_STRING {
                let contents = (*rtToData(as_ptr)).contents.as_mut_ptr();

                contents.add(index).write(value as i8);
            } else {
                let array = rtToData(as_ptr);
                // NOTE: array stores raw values, no need to unwrap object (i.e. unbox)
                set_array_el(&mut *array, index, value_obj.raw());
            }
        }

        self.push(aggregate)?;

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_jmp(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::JMP { offset } = instr else {
            return Err(InterpreterError::NotEnoughArguments("JMP"));
        };

        // NOTE: Frame shifting is delegated to `BEGIN` instruction

        let offset_at = offset as usize;

        self.decoder.ip = offset_at;

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_sexp(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::SEXP { s_index, n_members } = instr else {
            return Err(InterpreterError::NotEnoughArguments("SEXP"));
        };

        let length = n_members as usize;

        let tag_u8 = self
            .decoder
            .bf
            .get_string_at_offset(s_index as usize)
            .map_err(|_| InterpreterError::StringIndexOutOfBounds)?;

        let c_string =
            CStr::from_bytes_with_nul(tag_u8).map_err(|_| InterpreterError::InvalidCString)?;

        let borrow_operand_stack_elements = &mut self.operand_stack.0
            [self.operand_stack_len - length + 1..=self.operand_stack_len + 1]; // + 1 for tag

        let sexp = new_sexp(c_string, borrow_operand_stack_elements);

        // Pop arguments from the stack
        for _ in 0..length {
            self.pop()?;
        }

        self.push(Object::try_from(sexp).map_err(|_| InterpreterError::InvalidObjectPointer)?)?;

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_string(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::STRING { index } = instr else {
            return Err(InterpreterError::NotEnoughArguments("STRING"));
        };

        let string = self
            .decoder
            .bf
            .get_string_at_offset(index as usize)
            .map_err(|_| InterpreterError::StringIndexOutOfBounds)?;

        let lama_string = new_string(string).map_err(|_| InterpreterError::InvalidStringPointer)?;

        self.push(
            Object::try_from(lama_string).map_err(|_| InterpreterError::InvalidStringPointer)?,
        )?;

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

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    fn eval_binop(&mut self, instr: Instruction) -> Result<(), InterpreterError> {
        let Instruction::BINOP { op } = instr else {
            return Err(InterpreterError::NotEnoughArguments("BINOP"));
        };

        let right = self.pop()?.unbox();
        let left = self.pop()?.unbox();
        let result = match op {
            Op::ADD => left + right,
            Op::SUB => left - right,
            Op::MUL => left * right,
            Op::DIV => {
                #[cfg(feature = "runtime_checks")]
                if right == 0 {
                    return Err(InterpreterError::DivisionByZero);
                }
                left / right
            }
            Op::MOD => {
                #[cfg(feature = "runtime_checks")]
                if right == 0 {
                    return Err(InterpreterError::DivisionByZero);
                }
                left % right
            }
            Op::EQ => {
                if left == right {
                    1
                } else {
                    0
                }
            }
            Op::NEQ => {
                if left != right {
                    1
                } else {
                    0
                }
            }
            Op::LT => {
                if left < right {
                    1
                } else {
                    0
                }
            }
            Op::LEQ => {
                if left <= right {
                    1
                } else {
                    0
                }
            }
            Op::GT => {
                if left > right {
                    1
                } else {
                    0
                }
            }
            Op::GEQ => {
                if left >= right {
                    1
                } else {
                    0
                }
            }
            Op::AND => {
                if left != 0 && right != 0 {
                    1
                } else {
                    0
                }
            }
            Op::OR => {
                if left != 0 || right != 0 {
                    1
                } else {
                    0
                }
            }
        };

        self.push(Object::new_boxed(result))?;

        let encoding = self.decoder.next::<u8>()?;
        let instr = self.decoder.decode(encoding)?;

        become DISPATCH_TABLE[instr.discriminant() as usize](self, instr)
    }

    /// Push to the operand stack
    #[inline(always)]
    fn push(&mut self, obj: Object) -> Result<(), InterpreterError> {
        if self.operand_stack_len >= MAX_OPERAND_STACK_SIZE {
            return Err(InterpreterError::StackOverflow);
        }

        #[cfg(feature = "runtime_checks")]
        if (self.operand_stack_len - 1) <= self.global_areas_size {
            return Err(InterpreterError::StackUnderflow);
        }

        unsafe {
            let ptr_bottom: *const Object = &self.operand_stack.0[0];
            __gc_stack_top = ptr_bottom as usize;

            self.operand_stack_len += 1;
            self.operand_stack.0[self.operand_stack_len] = obj;

            // Mutate empty object
            let ptr_to_top: *const Object = &self.operand_stack.0[self.operand_stack_len];

            __gc_stack_bottom = ptr_to_top as usize;
        }

        Ok(())
    }

    /// Pop from the operand stack
    #[inline(always)]
    fn pop(&mut self) -> Result<Object, InterpreterError> {
        #[cfg(feature = "runtime_checks")]
        if (self.operand_stack_len - 1) <= self.global_areas_size {
            return Err(InterpreterError::StackUnderflow);
        }

        unsafe {
            // Get top object
            let ptr_to_top = __gc_stack_bottom as *mut Object;

            // Move top pointer one object to the left
            __gc_stack_bottom = __gc_stack_bottom - core::mem::size_of::<Object>();

            self.operand_stack_len -= 1;

            Ok(ptr_to_top.read())
        }
    }

    /// Take from the operand stack at `index`, relative to the top of the stack
    /// removes the element and returns it
    fn take(&mut self, index: usize) -> Result<Object, InterpreterError> {
        #[cfg(feature = "runtime_checks")]
        if (self.operand_stack_len - index - 1) <= self.global_areas_size {
            return Err(InterpreterError::StackUnderflow);
        }

        unsafe {
            // Move top pointer one object to the left
            __gc_stack_bottom = __gc_stack_bottom - core::mem::size_of::<Object>();
        }
        let relative_index = self.operand_stack_len - index;

        let taken = self.operand_stack.0[relative_index].clone();

        // Remove taken element and shift remaining elements down
        if relative_index != self.operand_stack_len {
            self.operand_stack
                .0
                .copy_within(relative_index + 1..=self.operand_stack_len, relative_index);
        }

        self.operand_stack_len -= 1;

        Ok(taken)
    }

    /// Get global objects which occupy 0..global_size area in operand stack
    fn globals(&self) -> &[Object] {
        &self.operand_stack.0[0..self.global_areas_size]
    }

    fn globals_mut(&mut self) -> &mut [Object] {
        &mut self.operand_stack.0[0..self.global_areas_size]
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
        }
    }
}

impl core::error::Error for InterpreterError {}

pub enum RunError {
    ErrorAtOffset(usize, InterpreterError, Instruction),
    DecoderError(DecoderError),
}

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

// #[cfg(test)]
// mod tests;
