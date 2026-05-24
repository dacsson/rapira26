//! This module defines frame metadata for the interpreter.
//! Because we have only one stack, we keep index
//! of the frame pointer.

use crate::object::Object;

/// In operand stack out frame data looks like this:
/// ```txt
/// ARG1
/// ARG2
/// ...
/// ARGN
/// CLOSURE_OBJ <- frame points to this index
/// ARGS_COUNT
/// LOCALS_COUNT
/// OLD_FRAME_POINTER
/// OLD_IP
/// LOCAL1
/// LOCAL2
/// ...
/// LOCALN
/// ```
#[derive(Debug, PartialEq, Eq, Clone)]
pub struct FrameMetadata {
    pub closure_obj: i64,
    pub n_locals: i64,
    pub n_args: i64,
    pub ret_frame_pointer: usize,
    pub ret_ip: usize,
}

impl<'a> FrameMetadata {
    /// Given an operand stack, construct a new frame metadata.
    /// Accompanies the `BEGIN` instruction.
    #[inline(always)]
    pub fn get_from_stack(stack: &[Object], frame_pointer: usize) -> Option<FrameMetadata> {
        let closure_obj = stack.get(frame_pointer)?.raw();
        let n_args = stack.get(frame_pointer + 1)?.raw();
        let n_locals = stack.get(frame_pointer + 2)?.raw();
        let ret_frame_pointer = stack.get(frame_pointer + 3)?.raw() as usize;
        let ret_ip = stack.get(frame_pointer + 4)?.raw() as usize;

        Some(FrameMetadata {
            closure_obj,
            n_locals,
            n_args,
            ret_frame_pointer,
            ret_ip,
        })
    }

    #[inline(always)]
    pub fn get_arg_at(
        &'a self,
        stack: &'a [Object],
        frame_pointer: usize,
        index: usize,
    ) -> Option<&'a Object> {
        let arg_index = frame_pointer - self.n_args as usize + index;
        stack.get(arg_index)
    }

    #[inline(always)]
    pub fn set_arg_at(
        &'a mut self,
        stack: &'a mut [Object],
        frame_pointer: usize,
        index: usize,
        value: Object,
    ) -> Option<()> {
        let arg_index = frame_pointer - self.n_args as usize + index;

        #[cfg(feature = "runtime_checks")]
        if arg_index >= stack.len() || index > self.n_args as usize {
            return None;
        }

        stack[arg_index] = value;
        Some(())
    }

    #[inline(always)]
    pub fn get_local_at(
        &'a self,
        stack: &'a [Object],
        frame_pointer: usize,
        index: usize,
    ) -> Option<&'a Object> {
        let local_index = frame_pointer + 5 + index;
        stack.get(local_index)
    }

    #[inline(always)]
    pub fn set_local_at(
        &'a mut self,
        stack: &'a mut [Object],
        frame_pointer: usize,
        index: usize,
        value: Object,
    ) -> Option<()> {
        let local_index = frame_pointer + 5 + index;

        #[cfg(feature = "runtime_checks")]
        if local_index >= stack.len() || index > self.n_locals as usize {
            return None;
        }

        stack[local_index] = value;
        Some(())
    }

    pub fn save_closure(
        &'a mut self,
        stack: &'a mut [Object],
        frame_pointer: usize,
        closure_obj: Object,
    ) -> Option<()> {
        let closure_index = frame_pointer;

        #[cfg(feature = "runtime_checks")]
        if closure_index >= stack.len() {
            return None;
        }

        stack[closure_index] = closure_obj;
        Some(())
    }

    #[inline(always)]
    pub fn get_closure(
        &'a mut self,
        stack: &'a mut [Object],
        frame_pointer: usize,
    ) -> Option<&'a Object> {
        let closure_index = frame_pointer;

        #[cfg(feature = "runtime_checks")]
        if closure_index >= stack.len() {
            return None;
        }

        Some(&stack[closure_index])
    }

    #[cfg(test)]
    pub fn new(
        n_locals: i64,
        n_args: i64,
        ret_frame_pointer: usize,
        ret_ip: usize,
    ) -> FrameMetadata {
        FrameMetadata {
            closure_obj: 0,
            n_locals,
            n_args,
            ret_frame_pointer,
            ret_ip,
        }
    }
}
