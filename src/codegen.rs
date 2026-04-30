pub mod cgen;
use std::{collections::HashMap, path::PathBuf};

use crate::module::Module;
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
}

pub type ModulePath = String;
pub type ModuleCode = String;
/// A map of module paths to their generated code
pub type ModuleMap = HashMap<ModulePath, ModuleCode>;

pub trait CodegenTarget {
    fn generate(&mut self, modules: Vec<Module>) -> ModuleMap;
    fn compile(
        &mut self,
        modules_codes: ModuleMap,
        current_dir: &PathBuf,
        flags: &[String],
        run: bool,
    ) -> Result<(), RunError>;
}

/// Generate and compile modules using the given target
pub fn run_codegen(
    target: &mut dyn CodegenTarget,
    modules: Vec<Module>,
    current_dir: &PathBuf,
    flags: &[String],
    run: bool,
    dump: bool,
) -> Result<(), RunError> {
    let code_map = target.generate(modules);
    if dump {
        println!("{:#?}", code_map);
    } else {
        target.compile(code_map, current_dir, flags, run)?;
    }

    Ok(())
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
