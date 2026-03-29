//! Generic optimization pass trait, all optimizations mutate AST

use crate::ast::Program;

pub struct OptimizationPassOpts {
    pub dump: bool,
}

pub trait OptimizationPass {
    /// Apply this optimization pass, mutating the ast
    fn transform(
        &self,
        program: &mut Program,
        opts: &OptimizationPassOpts,
    ) -> Result<(), Box<dyn std::error::Error>>;
}

/// Run all optimization passes on the given program
pub fn run_optimizations(
    program: &mut Program,
    passes: &[&dyn OptimizationPass],
    opts: &OptimizationPassOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    for pass in passes {
        pass.transform(program, opts)?;
    }
    Ok(())
}
