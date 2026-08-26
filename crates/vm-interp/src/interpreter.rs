//! VM Interpreter

use crate::object::{Object, ObjectError};
use crate::{
    RAP_IS_BOOL, RAP_IS_CALLABLE, RAP_IS_FLOAT, RAP_IS_NULL, RAP_IS_SLICE, RAP_IS_SMI, RAP_IS_TEXT,
    RAP_IS_TUPLE, RAP_IS_VARIANT, RAP_abs, RAP_add, RAP_and, RAP_create_callable_obj,
    RAP_create_custom_typed_obj, RAP_create_slice, RAP_dec_ref, RAP_divide, RAP_equal, RAP_floor,
    RAP_floor_divide, RAP_get_callable_arity, RAP_get_callable_offset_or_ptr, RAP_get_tuple_item,
    RAP_get_variant_field_at, RAP_get_variant_tag, RAP_greater_or_equal, RAP_greater_than,
    RAP_inc_ref, RAP_input_text, RAP_input_value, RAP_length, RAP_less_or_equal, RAP_less_than,
    RAP_modulo, RAP_multiply, RAP_negate, RAP_not, RAP_not_equal, RAP_or, RAP_power, RAP_round,
    RAP_set_tuple_item, RAP_set_variant_field_at, RAP_slice_assign, RAP_sqrt, RAP_stringify_object,
    RAP_subtract, isPtr, isSMI,
};
use core::ffi::{CStr, c_char};
use std::ffi::CString;
use vm_core::bytecode::{
    Builtin, CompareJumpKind, Instruction, LWRITE_NEWLINE_FLAG, LWRITE_NEWLINE_MASK, UnaryOp,
};
use vm_core::decoder::{Decoder, DecoderError};

#[cfg(test)]
const MAX_OPERAND_STACK_SIZE: usize = 64;

#[cfg(not(test))]
const MAX_OPERAND_STACK_SIZE: usize = 1024 * 64; // 0x7fffffff;

const INVALID_HANDLER: u8 = u8::MAX;

// TODO: look into `extern "rust-preserve-none"`
const DISPATCH_TABLE: [fn(&mut Interpreter) -> Result<(), InterpreterError>; 41] = [
    Interpreter::eval_nop,
    Interpreter::eval_end,
    Interpreter::eval_binop,
    Interpreter::eval_const,
    Interpreter::eval_string,
    Interpreter::eval_begin,
    Interpreter::eval_cbegin,
    Interpreter::eval_closure,
    // Despite LOAD/STOREs being a single opcode we split
    // on different functions to take advantage of branch prediction
    Interpreter::eval_store_global,
    Interpreter::eval_load_global,
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
    Interpreter::eval_load_local,
    Interpreter::eval_load_arg,
    Interpreter::eval_load_capture,
    Interpreter::eval_store_local,
    Interpreter::eval_store_arg,
    Interpreter::eval_store_capture,
    Interpreter::eval_variant,
    Interpreter::eval_variant_tag,
    Interpreter::eval_field,
    Interpreter::eval_set_field,
    Interpreter::eval_is_variant,
];

const fn build_dispatch_indices() -> [u8; 256] {
    let mut table = [INVALID_HANDLER; 256];

    table[0x00] = 0;
    let mut opcode = 0x01;
    while opcode <= 0x0f {
        table[opcode] = 2;
        opcode += 1;
    }

    table[0x10] = 3;
    table[0x11] = 4;
    table[0x12] = 26;
    table[0x13] = 20;
    table[0x14] = 21;
    table[0x15] = 17;
    table[0x16] = 1;
    table[0x17] = 29;
    table[0x18] = 14;
    table[0x19] = 15;
    table[0x1a] = 16;
    table[0x1b] = 19;
    table[0x1c] = 25;

    table[0x21] = 9;
    table[0x22] = 30;
    table[0x23] = 31;
    table[0x24] = 32;
    table[0x30] = 24;
    table[0x31] = 24;
    table[0x41] = 8;
    table[0x42] = 33;
    table[0x43] = 34;
    table[0x44] = 35;

    table[0x50] = 18;
    table[0x51] = 18;
    table[0x52] = 5;
    table[0x53] = 6;
    table[0x54] = 7;
    table[0x55] = 12;
    table[0x56] = 10;
    table[0x58] = 22;
    table[0x59] = 36;
    table[0x5a] = 13;
    table[0x5b] = 37;
    table[0x5c] = 38;
    table[0x5d] = 39;
    table[0x5e] = 40;

    table[0x70] = 11;
    table[0x71] = 11;
    table[0x72] = 11;
    table[0x73] = 11;
    table[0x74] = 11;
    table[0x75] = 23;
    table[0x76] = 27;
    opcode = 0x77;
    while opcode <= 0x7f {
        table[opcode] = 11;
        opcode += 1;
    }

    table
}

const DISPATCH_INDICES: [u8; 256] = build_dispatch_indices();

pub struct Interpreter {
    operand_stack: Vec<Object>,
    frame_pointer: usize,
    frame_arg_count: usize,
    frame_local_count: usize,
    /// Bytefile decoder
    decoder: Decoder,
    current_opcode: u8,
    /// Globals length
    global_areas_size: usize,
    variant_schemas: Vec<RuntimeVariantSchema>,
}

struct RuntimeVariantSchema {
    name: *const c_char,
    tag: u16,
    field_names: Vec<*const c_char>,
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

        let variant_schemas = decoder
            .bf
            .variant_schemas
            .iter()
            .map(|schema| RuntimeVariantSchema {
                name: decoder.bf.string_table[schema.name_offset as usize..]
                    .as_ptr()
                    .cast(),
                tag: schema.tag,
                field_names: schema
                    .field_offsets
                    .iter()
                    .map(|offset| decoder.bf.string_table[*offset as usize..].as_ptr().cast())
                    .collect(),
            })
            .collect();

        Interpreter {
            operand_stack,
            frame_pointer: global_areas_size,
            frame_arg_count: 2,
            frame_local_count: 0,
            decoder,
            current_opcode: 0,
            global_areas_size,
            variant_schemas,
        }
    }

    /// Main interpreter loop
    pub fn run(&mut self) -> Result<(), RunError> {
        self.decoder.ip = self.decoder.bf.main_offset as usize;

        self.dispatch().map_err(|e| -> RunError {
            let global_offset = core::mem::size_of::<i32>()
                + core::mem::size_of::<i32>()
                + core::mem::size_of::<i32>()
                + (core::mem::size_of::<i32>()
                    * 2
                    * self.decoder.bf.public_symbols_number as usize)
                + self.decoder.bf.stringtab_size as usize
                + self.decoder.ip;

            RunError::ErrorAtOffset(global_offset, e, Instruction::END)
        })?;

        // Release every remaining owned reference so the
        // run ends with a clean heap
        for obj in self.operand_stack.drain(..) {
            Self::dec_ref_if_ptr(obj);
        }

        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn run_with_result(&mut self) -> Result<usize, RunError> {
        self.decoder.ip = self.decoder.bf.main_offset as usize;

        self.dispatch().map_err(|e| -> RunError {
            let global_offset = core::mem::size_of::<i32>()
                + core::mem::size_of::<i32>()
                + core::mem::size_of::<i32>()
                + (core::mem::size_of::<i32>()
                    * 2
                    * self.decoder.bf.public_symbols_number as usize)
                + self.decoder.bf.stringtab_size as usize
                + self.decoder.ip;

            RunError::ErrorAtOffset(global_offset, e, Instruction::END)
        })?;

        let result = self
            .operand_stack
            .last()
            .copied()
            .expect("completed program must leave a return value on the stack")
            .raw();

        for obj in self.operand_stack.drain(..) {
            Self::dec_ref_if_ptr(obj);
        }

        Ok(result)
    }

    #[inline(always)]
    fn dispatch(&mut self) -> Result<(), InterpreterError> {
        let opcode = self.decoder.next::<u8>()?;
        let index = DISPATCH_INDICES[opcode as usize];
        if index == INVALID_HANDLER {
            return Err(InterpreterError::InvalidOpcode(opcode));
        }
        self.current_opcode = opcode;
        become DISPATCH_TABLE[index as usize](self)
    }

    fn eval_bool(&mut self) -> Result<(), InterpreterError> {
        let index = self.decoder.next::<u8>()? != 0;

        self.push(Object::new_bool(index))?;

        become self.dispatch()
    }

    fn eval_line(&mut self) -> Result<(), InterpreterError> {
        let _ = self.decoder.next::<i32>()?;
        become self.dispatch()
    }

    fn eval_nop(&mut self) -> Result<(), InterpreterError> {
        become self.dispatch()
    }

    fn eval_closure(&mut self) -> Result<(), InterpreterError> {
        let offset = self.decoder.next::<i32>()?;
        let arity = self.decoder.next::<i32>()?;

        if offset < 0 || arity < 0 {
            return Err(InterpreterError::InvalidObjectPointer);
        }

        let callable =
            unsafe { RAP_create_callable_obj(offset as usize, arity as u32, std::ptr::null(), 0) };
        self.push(Object::new(callable))?;

        become self.dispatch()
    }

    fn eval_callc(&mut self) -> Result<(), InterpreterError> {
        let argument_count = self.decoder.next::<i32>()?;
        let argument_count = usize::try_from(argument_count)
            .map_err(|_| InterpreterError::NotEnoughArguments("CALLC"))?;
        let callable_index = self
            .operand_stack
            .len()
            .checked_sub(argument_count + 1)
            .ok_or(InterpreterError::NotEnoughArguments("CALLC"))?;
        let callable = self.operand_stack[callable_index];

        if !unsafe { RAP_IS_CALLABLE(callable.raw()) } {
            return Err(InterpreterError::InvalidType("expected callable"));
        }

        let expected_arity = unsafe { RAP_get_callable_arity(callable.raw()) } as usize;
        if expected_arity != argument_count {
            return Err(InterpreterError::CallableArityMismatch {
                expected: expected_arity,
                actual: argument_count,
            });
        }
        let entry = unsafe { RAP_get_callable_offset_or_ptr(callable.raw()) };

        // The callable sits directly below its args so we remove only that
        // owning reference the arguments stay in their original order for
        // BEGIN to frame them
        let callable = self.operand_stack.remove(callable_index);
        Self::dec_ref_if_ptr(callable);
        self.push(Object::new_boxed(self.decoder.ip as i64))?;
        self.decoder.ip = entry;

        become self.dispatch()
    }

    fn eval_load_ref(&mut self) -> Result<(), InterpreterError> {
        panic!("You shouldn't be here")
    }

    fn eval_array(&mut self) -> Result<(), InterpreterError> {
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

        unreachable!()
    }

    fn get_variant_schema(&self, id: i32) -> Result<&RuntimeVariantSchema, InterpreterError> {
        usize::try_from(id)
            .ok()
            .and_then(|id| self.variant_schemas.get(id))
            .ok_or(InterpreterError::InvalidVariantSchema(id))
    }

    fn eval_variant(&mut self) -> Result<(), InterpreterError> {
        let id = self.decoder.next::<i32>()?;
        let count = self.get_variant_schema(id)?.field_names.len();
        if self.operand_stack.len() < count {
            return Err(InterpreterError::StackUnderflow);
        }

        let start = self.operand_stack.len() - count;
        let values: Vec<Object> = self.operand_stack[start..].to_vec();
        let mut payload = vec![0u8; 2 + count * core::mem::size_of::<usize>()];
        let tag = self.get_variant_schema(id)?.tag;
        payload[..2].copy_from_slice(&tag.to_le_bytes());
        for (index, value) in values.iter().enumerate() {
            unsafe {
                payload
                    .as_mut_ptr()
                    .add(2 + index * core::mem::size_of::<usize>())
                    .cast::<usize>()
                    .write_unaligned(value.raw());
            }
        }

        // Removing the arguments transfers their owning stack references to
        // the newly created variant so dont decrement them here
        for _ in 0..count {
            self.pop()?;
        }
        let schema = self.get_variant_schema(id)?;
        let value = unsafe {
            RAP_create_custom_typed_obj(
                schema.name,
                schema.field_names.as_ptr().cast_mut(),
                count,
                payload.as_ptr().cast_mut().cast(),
            )
        };
        self.push(Object::new(value))?;
        become self.dispatch()
    }

    fn eval_variant_tag(&mut self) -> Result<(), InterpreterError> {
        let value = self.pop()?;
        if !unsafe { RAP_IS_VARIANT(value.raw()) } {
            return Err(InterpreterError::InvalidType("expected variant"));
        }
        let tag = unsafe { RAP_get_variant_tag(value.raw()) };
        Self::dec_ref_if_ptr(value);
        self.push(Object::new_boxed(tag as i64))?;
        become self.dispatch()
    }

    fn eval_is_variant(&mut self) -> Result<(), InterpreterError> {
        let id = self.decoder.next::<i32>()?;
        let tag = self.get_variant_schema(id)?.tag;
        let value = self.pop()?;
        let matches = unsafe { RAP_IS_VARIANT(value.raw()) }
            && unsafe { RAP_get_variant_tag(value.raw()) } == tag;
        Self::dec_ref_if_ptr(value);
        self.push(Object::new_bool(matches))?;
        become self.dispatch()
    }

    fn eval_field(&mut self) -> Result<(), InterpreterError> {
        let index = self.decoder.next::<i32>()?;
        let index = usize::try_from(index).map_err(|_| InterpreterError::InvalidFieldIndex)?;
        let value = self.pop()?;
        if !unsafe { RAP_IS_VARIANT(value.raw()) } {
            return Err(InterpreterError::InvalidType("expected variant"));
        }
        let field = unsafe { RAP_get_variant_field_at(value.raw(), index) };
        Self::inc_ref_if_ptr(Object::new(field));
        Self::dec_ref_if_ptr(value);
        self.push(Object::new(field))?;
        become self.dispatch()
    }

    fn eval_set_field(&mut self) -> Result<(), InterpreterError> {
        let index = self.decoder.next::<i32>()?;
        let index = usize::try_from(index).map_err(|_| InterpreterError::InvalidFieldIndex)?;
        // Code generation evaluates: replacement then target
        // object so the variant is at the top
        let variant_value = self.pop()?;
        let value = self.pop()?;
        if !unsafe { RAP_IS_VARIANT(variant_value.raw()) } {
            return Err(InterpreterError::InvalidType("expected variant"));
        }
        unsafe { RAP_set_variant_field_at(variant_value.raw(), index, value.raw()) };
        Self::dec_ref_if_ptr(value);
        Self::dec_ref_if_ptr(variant_value);
        become self.dispatch()
    }

    fn eval_elem(&mut self) -> Result<(), InterpreterError> {
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

        become self.dispatch()
    }

    fn eval_sts(&mut self) -> Result<(), InterpreterError> {
        // Stack: [value, slice] (bottom→top), so pop slice first (top),
        // then value (next down).
        let slice = self.pop()?;
        let value = self.pop()?;

        unsafe { RAP_slice_assign(slice.raw(), value.raw()) };

        // TODO: is this needed?
        Self::dec_ref_if_ptr(slice);
        Self::dec_ref_if_ptr(value);

        become self.dispatch()
    }

    fn eval_cjmp(&mut self) -> Result<(), InterpreterError> {
        let offset_at = self.decoder.next::<i32>()?;
        let kind = match self.current_opcode & 0x0f {
            0 => CompareJumpKind::ISZERO,
            1 => CompareJumpKind::ISNONZERO,
            _ => return Err(InterpreterError::InvalidOpcode(self.current_opcode)),
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

        become self.dispatch()
    }

    fn eval_call_builtin(&mut self) -> Result<(), InterpreterError> {
        // println!("in eval_call_builtin: {:?}", instr);

        let subopcode = self.current_opcode & 0x0f;
        let name = Builtin::try_from(subopcode)
            .map_err(|_| InterpreterError::InvalidOpcode(self.current_opcode))?;

        let n = if [0x0, 0x1].contains(&subopcode) {
            self.decoder.next::<i32>()?
        } else {
            0
        };

        match name {
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
                // `n` packs the format flag (bit 30) with arg count
                // signaling whether we want typed or unptyed (text) input
                let typed_bit = n & LWRITE_NEWLINE_FLAG;
                let is_typed = typed_bit != 0;
                let n = n & LWRITE_NEWLINE_MASK;
                let arg_count = n as usize;

                let RAP_input_func = if is_typed {
                    RAP_input_value
                } else {
                    RAP_input_text
                };

                // Parses a line of stdin into a typed RAP_Value (int/float/text).
                for _ in 0..arg_count {
                    self.push(Object::new(RAP_input_func()))?;
                }
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
            Builtin::Abs | Builtin::Sqrt | Builtin::Floor | Builtin::Round => {
                // These are always unary by construction
                let value = self.pop()?;
                let result = unsafe {
                    match name {
                        Builtin::Abs => RAP_abs(value.raw()),
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
            Builtin::Tint => {
                let value = self.pop()?;
                let result = unsafe { RAP_IS_SMI(value.raw()) };
                Self::dec_ref_if_ptr(value);
                self.push(Object::new_bool(result))?;
            }
            Builtin::Tbool => {
                let value = self.pop()?;
                let result = unsafe { RAP_IS_BOOL(value.raw()) };
                Self::dec_ref_if_ptr(value);
                self.push(Object::new_bool(result))?;
            }
            Builtin::Tfloat => {
                let value = self.pop()?;
                let result = unsafe { RAP_IS_FLOAT(value.raw()) };
                Self::dec_ref_if_ptr(value);
                self.push(Object::new_bool(result))?;
            }
            Builtin::Ttext => {
                let value = self.pop()?;
                let result = unsafe { RAP_IS_TEXT(value.raw()) };
                Self::dec_ref_if_ptr(value);
                self.push(Object::new_bool(result))?;
            }
            Builtin::Ttuple => {
                let value = self.pop()?;
                let result = unsafe { RAP_IS_TUPLE(value.raw()) };
                Self::dec_ref_if_ptr(value);
                self.push(Object::new_bool(result))?;
            }
            Builtin::Tslice => {
                let value = self.pop()?;
                let result = unsafe { RAP_IS_SLICE(value.raw()) };
                Self::dec_ref_if_ptr(value);
                self.push(Object::new_bool(result))?;
            }
            Builtin::Tnull => {
                let value = self.pop()?;
                let result = unsafe { RAP_IS_NULL(value.raw()) };
                Self::dec_ref_if_ptr(value);
                self.push(Object::new_bool(result))?;
            }
        }

        become self.dispatch()
    }

    fn eval_call(&mut self) -> Result<(), InterpreterError> {
        let offset = self.decoder.next::<i32>()?;
        let _n = self.decoder.next::<i32>()?;

        // Push old instruction pointer
        // `BEGIN` instruction will collect it
        self.push(Object::new_boxed(self.decoder.ip as i64))?;

        // // Push empty closure object
        // self.push(Object::new_empty())?;

        self.decoder.ip = offset as usize;

        become self.dispatch()
    }

    fn eval_swap(&mut self) -> Result<(), InterpreterError> {
        let value1 = self.pop()?;
        let value2 = self.pop()?;
        self.push(value1)?;
        self.push(value2)?;

        become self.dispatch()
    }

    fn eval_dup(&mut self) -> Result<(), InterpreterError> {
        let value = self.pop()?;
        self.push(value)?;
        self.push(value)?;

        Self::inc_ref_if_ptr(value);

        become self.dispatch()
    }

    fn eval_drop(&mut self) -> Result<(), InterpreterError> {
        let obj = self.pop()?;
        Self::dec_ref_if_ptr(obj);

        become self.dispatch()
    }

    fn eval_load_global(&mut self) -> Result<(), InterpreterError> {
        let index = self.decoder.next::<i32>()? as usize;
        let value = self.globals()[index];

        Self::inc_ref_if_ptr(value);
        self.push(value)?;

        become self.dispatch()
    }

    fn eval_load_local(&mut self) -> Result<(), InterpreterError> {
        let index = self.decoder.next::<i32>()? as usize;
        let value = self.load_local(index)?;

        Self::inc_ref_if_ptr(value);
        self.push(value)?;

        become self.dispatch()
    }

    fn eval_load_arg(&mut self) -> Result<(), InterpreterError> {
        let index = self.decoder.next::<i32>()? as usize;
        let value = self.load_arg(index)?;

        Self::inc_ref_if_ptr(value);
        self.push(value)?;

        become self.dispatch()
    }

    fn eval_load_capture(&mut self) -> Result<(), InterpreterError> {
        todo!("captured-variable loads are not implemented")
    }

    fn eval_store_global(&mut self) -> Result<(), InterpreterError> {
        let index = self.decoder.next::<i32>()? as usize;
        let value = self.pop()?;
        let old = self.globals()[index];
        self.globals_mut()[index] = value;

        if old.raw() != value.raw() {
            Self::dec_ref_if_ptr(old);
        }

        become self.dispatch()
    }

    fn eval_store_local(&mut self) -> Result<(), InterpreterError> {
        let index = self.decoder.next::<i32>()? as usize;
        let value = self.pop()?;
        let old = self.store_local(index, value)?;

        if old.raw() != value.raw() {
            Self::dec_ref_if_ptr(old);
        }

        become self.dispatch()
    }

    fn eval_store_arg(&mut self) -> Result<(), InterpreterError> {
        let index = self.decoder.next::<i32>()? as usize;
        let value = self.pop()?;
        let old = self.store_arg(index, value)?;

        if old.raw() != value.raw() {
            Self::dec_ref_if_ptr(old);
        }

        become self.dispatch()
    }

    fn eval_store_capture(&mut self) -> Result<(), InterpreterError> {
        todo!("captured-variable stores are not implemented")
    }

    fn eval_end(&mut self) -> Result<(), InterpreterError> {
        // Get procedures return value
        let return_value = self
            .pop()
            .map_err(|_| InterpreterError::NotEnoughArguments("END"))?;

        let n_locals = self.frame_local_count;
        let n_args = self.frame_arg_count;
        let ret_frame_pointer = self.frame_return_pointer()?;
        let ret_ip = self.frame_return_ip()?;

        // `return_value` is removed from the operand stack above, but may
        // alias an argument/local slot that is about to be released -> so we retain
        // it before tearing down the frame (caller becomes owner)
        if isPtr(return_value.raw()) {
            let args_start = self.frame_pointer - n_args;
            let locals_start = self.frame_pointer + 5;
            if self.operand_stack[args_start..self.frame_pointer]
                .iter()
                .chain(self.operand_stack[locals_start..locals_start + n_locals].iter())
                .any(|value| value.raw() == return_value.raw())
            {
                Self::inc_ref_if_ptr(return_value);
            }
        }

        // A value may be aliased by more than one frame slot, e.g. stores currently
        // transfer stack ownership, so releasing every slot would free the
        // same native object repeatedly.
        //
        // The bytecode generator avoids materializing a selector local for a
        // name expression, which prevents value-match statements from adding a
        // second frame slot for their subject.
        let mut released = Vec::new();
        for _ in 0..n_locals {
            let obj = self.pop()?;
            if !released.contains(&obj.raw()) {
                Self::dec_ref_if_ptr(obj);
                released.push(obj.raw());
            }
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
            if !released.contains(&obj.raw()) {
                Self::dec_ref_if_ptr(obj);
                released.push(obj.raw());
            }
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

        // The caller's frame is still present on the operand stack. Restore
        // its cached metadata after removing the callee frame.
        self.frame_arg_count = self
            .operand_stack
            .get(ret_frame_pointer + 1)
            .copied()
            .ok_or(InterpreterError::NotEnoughArguments(
                "caller frame arg count",
            ))?
            .unbox() as usize;
        self.frame_local_count = self
            .operand_stack
            .get(ret_frame_pointer + 2)
            .copied()
            .ok_or(InterpreterError::NotEnoughArguments(
                "caller frame local count",
            ))?
            .unbox() as usize;

        become self.dispatch()
    }

    fn eval_begin(&mut self) -> Result<(), InterpreterError> {
        // println!("eval_begin: {:?}", instr);

        let payload = self.decoder.next::<i32>()?;
        let locals = self.decoder.next::<i32>()?;

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
        self.frame_arg_count = args;
        self.frame_local_count = locals as usize;

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

        become self.dispatch()
    }

    fn eval_cbegin(&mut self) -> Result<(), InterpreterError> {
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

    fn eval_sta(&mut self) -> Result<(), InterpreterError> {
        let aggregate = self.pop()?;
        let index = self.pop()?;
        let value = self.pop()?;

        let obj = unsafe { RAP_set_tuple_item(aggregate.raw(), index.unbox() as u32, value.raw()) };

        self.push(Object::new(obj))?;

        Self::dec_ref_if_ptr(aggregate);
        Self::dec_ref_if_ptr(index);
        Self::dec_ref_if_ptr(value);

        become self.dispatch()
    }

    fn eval_jmp(&mut self) -> Result<(), InterpreterError> {
        let offset_at = self.decoder.next::<i32>()?;

        // NOTE: Frame shifting is delegated to `BEGIN` instruction

        // println!(
        //     "jmp called to {} | {:x}",
        //     offset_at, self.decoder.bf.code_section[offset_at as usize]
        // );

        self.decoder.ip = offset_at as usize;

        become self.dispatch()
    }

    fn eval_string(&mut self) -> Result<(), InterpreterError> {
        let index = self.decoder.next::<i32>()?;

        let string_bytes = self
            .decoder
            .bf
            .get_string_at_offset(index as usize)
            .map_err(|_| InterpreterError::StringIndexOutOfBounds)?;
        let string = CStr::from_bytes_with_nul(string_bytes)
            .map_err(|_| InterpreterError::InvalidCString)?;

        self.push(Object::new_string(string))?;

        become self.dispatch()
    }

    fn eval_const(&mut self) -> Result<(), InterpreterError> {
        let index = self.decoder.next::<i32>()?;

        self.push(Object::new_boxed(index as i64))?;

        become self.dispatch()
    }

    fn eval_binop(&mut self) -> Result<(), InterpreterError> {
        // println!("binop called at {}", self.decoder.ip);

        let subopcode = self.current_opcode & 0x0f;

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

        if matches!(subopcode, 0x4 | 0x5 | 0xe) && right.unbox() == 0 {
            return Err(InterpreterError::DivisionByZero);
        }

        let result = if let Some(result) = Self::eval_immediate_binop(subopcode, left, right) {
            result
        } else {
            let result = unsafe {
                match subopcode {
                    0x1 => RAP_add(left.raw(), right.raw()),
                    0x2 => RAP_subtract(left.raw(), right.raw()),
                    0x3 => RAP_multiply(left.raw(), right.raw()),
                    0x4 => RAP_divide(left.raw(), right.raw()),
                    0x5 => RAP_modulo(left.raw(), right.raw()),
                    0x6 => RAP_less_than(left.raw(), right.raw()),
                    0x7 => RAP_less_or_equal(left.raw(), right.raw()),
                    0x8 => RAP_greater_than(left.raw(), right.raw()),
                    0x9 => RAP_greater_or_equal(left.raw(), right.raw()),
                    0xa => RAP_equal(left.raw(), right.raw()),
                    0xb => RAP_not_equal(left.raw(), right.raw()),
                    0xc => RAP_and(left.raw(), right.raw()),
                    0xd => RAP_or(left.raw(), right.raw()),
                    0xe => RAP_floor_divide(left.raw(), right.raw()),
                    0xf => RAP_power(left.raw(), right.raw()),
                    _ => return Err(InterpreterError::InvalidOpcode(self.current_opcode)),
                }
            };

            Self::dec_ref_if_ptr(right);
            Self::dec_ref_if_ptr(left);
            Object::new(result)
        };

        // unsafe {
        //     println!(
        //         "result: {:#?} ",
        //         CStr::from_ptr(RAP_stringify_object(result))
        //     );
        // }

        self.push(result)?;

        become self.dispatch()
    }

    /// Evaluate operations whose operands are represented entirely inside a
    /// `RAP_Value`. This avoids crossing the Rust/C boundary for the common
    /// integer and boolean cases while retaining the generic runtime fallback
    /// for floats and aggregate values.
    #[inline(always)]
    fn eval_immediate_binop(subopcode: u8, left: Object, right: Object) -> Option<Object> {
        if isSMI(left.raw()) && isSMI(right.raw()) {
            let left = left.unbox();
            let right = right.unbox();

            let result = match subopcode {
                0x1 => Object::new_boxed(left + right),
                0x2 => Object::new_boxed(left - right),
                0x3 => Object::new_boxed(left * right),
                0x4 if left % right == 0 => Object::new_boxed(left / right),
                0x5 => {
                    let mut remainder = left % right;
                    if remainder != 0 && ((remainder < 0) != (right < 0)) {
                        remainder += right;
                    }
                    Object::new_boxed(remainder)
                }
                0x6 => Object::new_bool(left < right),
                0x7 => Object::new_bool(left <= right),
                0x8 => Object::new_bool(left > right),
                0x9 => Object::new_bool(left >= right),
                0xa => Object::new_bool(left == right),
                0xb => Object::new_bool(left != right),
                0xe => {
                    let mut quotient = left / right;
                    let remainder = left % right;
                    if remainder != 0 && ((remainder < 0) != (right < 0)) {
                        quotient -= 1;
                    }
                    Object::new_boxed(quotient)
                }
                _ => return None,
            };
            return Some(result);
        }

        let (Some(left), Some(right)) = (left.get_bool(), right.get_bool()) else {
            return None;
        };
        match subopcode {
            0xa => Some(Object::new_bool(left == right)),
            0xb => Some(Object::new_bool(left != right)),
            0xc => Some(Object::new_bool(left && right)),
            0xd => Some(Object::new_bool(left || right)),
            _ => None,
        }
    }

    fn eval_unary(&mut self) -> Result<(), InterpreterError> {
        let op = UnaryOp::try_from(self.current_opcode & 0x0f)
            .map_err(|_| InterpreterError::InvalidOpcode(self.current_opcode))?;

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

        become self.dispatch()
    }

    fn eval_constf(&mut self) -> Result<(), InterpreterError> {
        let value = self.decoder.next::<f64>()?;

        self.push(Object::new_float(value))?;

        become self.dispatch()
    }

    fn eval_tuple(&mut self) -> Result<(), InterpreterError> {
        let n = self.decoder.next::<i32>()?;

        if n == 0 {
            self.push(Object::new_tuple(0, &mut []))?;

            become self.dispatch()
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

        become self.dispatch()
    }

    /// Push null value on the operand stack
    fn eval_null(&mut self) -> Result<(), InterpreterError> {
        self.push(Object::new_null())?;

        become self.dispatch()
    }

    /// Resolves a label to an offset
    ///
    /// FIXME: Can it appear here?
    fn eval_label(&mut self) -> Result<(), InterpreterError> {
        // let Instruction::LABEL { name } = instr else {
        //     return Err(InterpreterError::InvalidOpcode(instr.discriminant()));
        // };

        become self.dispatch()
    }

    fn eval_slice(&mut self) -> Result<(), InterpreterError> {
        let bounds = self.decoder.next::<u8>()?;

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

        become self.dispatch()
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
    fn frame_stack_value(
        &self,
        offset: usize,
        opname: &'static str,
    ) -> Result<Object, InterpreterError> {
        self.operand_stack
            .get(self.frame_pointer + offset)
            .copied()
            .ok_or(InterpreterError::NotEnoughArguments(opname))
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
        let n_args = self.frame_arg_count;
        if index >= n_args {
            return Err(InterpreterError::NotEnoughArguments("LOAD/STORE arg"));
        }

        Ok(self.frame_pointer - n_args + index)
    }

    #[inline(always)]
    fn local_slot_index(&self, index: usize) -> Result<usize, InterpreterError> {
        let n_locals = self.frame_local_count;
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
    DecoderError(DecoderError),
    StackOverflow,
    UnknownLabel(String),
    InvalidVariantSchema(i32),
    InvalidFieldIndex,
    CallableArityMismatch { expected: usize, actual: usize },
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
            InterpreterError::DecoderError(err) => {
                write!(f, "Decoder error: {}", err)
            }
            InterpreterError::StackOverflow => {
                write!(f, "Stack overflow")
            }
            InterpreterError::UnknownLabel(name) => {
                write!(f, "Unknown label: {}", name)
            }
            InterpreterError::InvalidVariantSchema(id) => {
                write!(f, "Invalid variant schema: {}", id)
            }
            InterpreterError::InvalidFieldIndex => write!(f, "Invalid variant field index"),
            InterpreterError::CallableArityMismatch { expected, actual } => write!(
                f,
                "Callable expects {} argument(s), received {}",
                expected, actual
            ),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dispatch_table_specializes_variable_access() {
        assert_eq!(DISPATCH_INDICES[0x21], 9);
        assert_eq!(DISPATCH_INDICES[0x22], 30);
        assert_eq!(DISPATCH_INDICES[0x23], 31);
        assert_eq!(DISPATCH_INDICES[0x24], 32);
        assert_eq!(DISPATCH_INDICES[0x41], 8);
        assert_eq!(DISPATCH_INDICES[0x42], 33);
        assert_eq!(DISPATCH_INDICES[0x43], 34);
        assert_eq!(DISPATCH_INDICES[0x44], 35);
        assert_eq!(DISPATCH_INDICES[0x20], INVALID_HANDLER);
        assert_eq!(DISPATCH_INDICES[0x45], INVALID_HANDLER);
    }

    fn immediate_integer(left: i64, subopcode: u8, right: i64) -> i64 {
        Interpreter::eval_immediate_binop(
            subopcode,
            Object::new_boxed(left),
            Object::new_boxed(right),
        )
        .expect("integer operation should use the immediate fast path")
        .unbox()
    }

    #[test]
    fn immediate_modulo_uses_floor_division_semantics() {
        for (left, right, expected) in [(7, 2, 1), (-7, 2, 1), (7, -2, -1), (-7, -2, -1)] {
            assert_eq!(immediate_integer(left, 0x5, right), expected);

            let runtime_result = unsafe {
                RAP_modulo(
                    Object::new_boxed(left).raw(),
                    Object::new_boxed(right).raw(),
                )
            };
            assert_eq!(Object::new(runtime_result).unbox(), expected);
        }
    }

    #[test]
    fn immediate_floor_division_rounds_toward_negative_infinity() {
        for (left, right, expected) in [(7, 2, 3), (-7, 2, -4), (7, -2, -4), (-7, -2, 3)] {
            assert_eq!(immediate_integer(left, 0xe, right), expected);
        }
    }

    #[test]
    fn immediate_boolean_operations_stay_unboxed() {
        for (subopcode, expected) in [(0xc, false), (0xd, true), (0xb, true)] {
            let result = Interpreter::eval_immediate_binop(
                subopcode,
                Object::new_bool(true),
                Object::new_bool(false),
            )
            .expect("boolean operation should use the immediate fast path");
            assert_eq!(result.get_bool(), Some(expected));
        }
    }
}
