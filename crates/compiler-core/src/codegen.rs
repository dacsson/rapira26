// pub mod cgen;
pub mod bcgen;

use std::{collections::HashMap, path::PathBuf};

use crate::{
    ast::{FunctionDefinition, ProcedureDefinition, Spannable, Statement, TypeDefinition},
    module::{AbsolutModulePath, Module},
};
use clap::ValueEnum;

pub enum CodegenWarning {
    UndeclaredVariable(usize, String, usize),
}

pub enum RunError {
    NoSuchFile,
    FileNotFound(String),
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

/// Canonical path to a modules compiled code
#[derive(Debug, PartialEq, Eq, Hash, Clone)]
pub struct AbsolutGeneratedCodePath(pub PathBuf);

pub trait CodegenTarget {
    /// Generate a map of module paths to their generated code
    fn generate(&mut self, modules: Vec<Module>) -> ModuleMap;

    /// Compile and possibly run code after generation
    fn compile(
        &mut self,
        modules_codes: ModuleMap,
        current_dir: &PathBuf,
        flags: &[String],
        dump: bool,
    ) -> Result<Vec<AbsolutGeneratedCodePath>, RunError>;

    // Base constructs that a backend should implement, you
    // can emit more than that if you want to
    fn emit_procedure_def(&mut self, proc_def_span: &Spannable<ProcedureDefinition>);
    fn emit_function_def(&mut self, func_def_span: &Spannable<FunctionDefinition>);
    fn emit_type_def(&mut self, type_def_span: &Spannable<TypeDefinition>);
    fn emit_top_level_def(&mut self, top_level: &Vec<Spannable<Statement>>);
}

/// Generate and compile modules using the given target
///
/// Returns a list of paths to generated files
pub fn run_codegen(
    target: &mut dyn CodegenTarget,
    modules: Vec<Module>,
    current_dir: &PathBuf,
    flags: &[String],
    dump: bool,
) -> Result<Vec<AbsolutGeneratedCodePath>, RunError> {
    let code_map = target.generate(modules);
    let generated_paths = target.compile(code_map, current_dir, flags, dump)?;
    Ok(generated_paths)
}

/// Find the runtime/ directory containing librapruntime.a and headers.
/// Checks:
/// 1. $RAPIRA_RUNTIME env var
/// 2. Next to the compiler binary
/// 3. ./runtime/ in cwd (development)
fn find_runtime_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("RAPIRA_RUNTIME") {
        let candidate = PathBuf::from(dir);
        if candidate.join("lib/librapruntime.a").exists() {
            return candidate;
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        let candidate = exe.parent().unwrap().join("runtime");
        if candidate.join("lib/librapruntime.a").exists() {
            return candidate;
        }
    }

    let candidate = PathBuf::from("runtime");
    if candidate.join("lib/librapruntime.a").exists() {
        return candidate;
    }

    eprintln!(
        "error: cannot find librapruntime.a (tried $RAPIRA_RUNTIME, next to binary, and ./runtime/)\n\
         hint: run 'make' in the runtime/ directory first"
    );
    std::process::exit(1);
}

impl std::fmt::Display for RunError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RunError::NoSuchFile => write!(f, "нет такого файла"),
            RunError::FileNotFound(path) => write!(f, "не нашёл файл: {}", path),
        }
    }
}
