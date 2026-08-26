// pub mod cgen;
pub mod bcgen;

use std::collections::HashMap;

use crate::{
    ast::{FunctionDefinition, Spannable, Statement, TypeDefinition},
    module::{AbsolutModulePath, Module},
};
use clap::ValueEnum;
use vm_core::bytecode::Instruction;

pub enum CodegenWarning {
    UndeclaredVariable(usize, String, usize),
}

/// Available backends
#[derive(ValueEnum, Clone, Debug)]
pub enum CodegenTargetName {
    C,
    RBC, // Rapira Bytecode
}

/// Generated code for a module
pub type ModuleCode = Vec<u8>;

/// A map of module paths to their generated code
pub type ModuleMap = HashMap<AbsolutModulePath, ModuleCode>;

pub trait CodegenTarget {
    /// Generate a map of module paths to their generated code
    fn generate(&mut self, modules: Vec<Module>) -> ModuleMap;

    // Base constructs that a backend should implement, you
    // can emit more than that if you want to
    fn emit_function_def(
        &mut self,
        func_def_span: &Spannable<FunctionDefinition>,
    ) -> Vec<Instruction>;
    fn emit_type_def(&mut self, type_def_span: &Spannable<TypeDefinition>) -> Vec<Instruction>;
    fn emit_top_level_def(&mut self, top_level: &Vec<Spannable<Statement>>) -> Vec<Instruction>;
}

/// Generate modules using the given target without writing temporary output.
pub fn run_codegen(target: &mut dyn CodegenTarget, modules: Vec<Module>) -> ModuleMap {
    target.generate(modules)
}
