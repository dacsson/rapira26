//! Generic optimization pass trait, all optimizations mutate AST

use crate::module::Module;

pub struct OptimizationPassOpts {
    pub dump: bool,
}

pub trait OptimizationPass {
    /// Apply this optimization pass, mutating the ast
    fn transform(
        &self,
        module: &mut Module,
        opts: &OptimizationPassOpts,
    ) -> Result<(), Box<dyn std::error::Error>>;
}

/// Run all optimization passes on the given program
pub fn run_optimizations(
    module: &mut Module,
    passes: &[&dyn OptimizationPass],
    opts: &OptimizationPassOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    for pass in passes {
        pass.transform(module, opts)?;
    }
    Ok(())
}
