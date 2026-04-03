//! This module implements deframing optimization.
//!
//! In runtime we use frames to store variables names and values, so other functions and procedures
//! can look up variables by name, forming a dynamic lookup table/chain. But this is expensive.
//!
//! In this pass we determine which local variables do not need to be stored in a function frame,
//! to eliminate the need for frame variable lookups.
//! Only variables that are used in functions that request them as `чужие` will be stored.

use crate::ast::*;
use crate::opt::call_graph::CallGraph;
use crate::opt::opt_pass::{OptimizationPass, OptimizationPassOpts};
use petgraph::visit::Dfs;

pub struct DeframePass;

impl OptimizationPass for DeframePass {
    fn transform(
        &self,
        ast: &mut Program,
        opts: &OptimizationPassOpts,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // First build the call graph
        let call_graph = CallGraph::new(ast);

        // Walk the call graph to determine which variables are used in functions that request them as `чужие`
        for node in call_graph.graph().node_indices() {
            let call_node = call_graph.graph().node_weight(node).unwrap();

            // Check if other functions request this variable as `чужие`
            let mut dfs = Dfs::new(call_graph.graph(), node);
            while let Some(neighbor) = dfs.next(call_graph.graph()) {
                let neighbor_node = call_graph.graph().node_weight(neighbor).unwrap();

                // Variables that should be stored in the frame
                let same_vars: Vec<String> = call_node
                    .local_args
                    .iter()
                    .filter(|arg| {
                        neighbor_node.foreign_args.contains(arg)
                            || call_node.sent_as_inout_params.contains(arg)
                    })
                    .cloned()
                    .collect();

                let func_ast_node = ast
                    .units
                    .iter_mut()
                    .find(|unit| {
                        matches!(unit, ProgramUnit::FunctionDefinition(Spannable { node: FunctionDefinition { name, .. }, ..}) | ProgramUnit::ProcedureDefinition(Spannable { node: ProcedureDefinition { name, .. }, ..}) if name.as_deref() == Some(&call_node.name))
                    });

                if let Some(func_ast_node) = func_ast_node {
                    match func_ast_node {
                        ProgramUnit::FunctionDefinition(Spannable {
                            node:
                                FunctionDefinition {
                                    variables_need_saving,
                                    ..
                                },
                            ..
                        })
                        | ProgramUnit::ProcedureDefinition(Spannable {
                            node:
                                ProcedureDefinition {
                                    variables_need_saving,
                                    ..
                                },
                            ..
                        }) => {
                            variables_need_saving.extend(same_vars);
                        }
                        _ => {} // TODO: err
                    }
                }
            }
        }

        if opts.dump {
            println!("=== DOT GRAPH ===");
            call_graph.dump();
            println!("=== ПОСЛЕ DeframePass ===");
            println!("{:#?}", ast);
            println!("=========================");
        }

        Ok(())
    }
}

// #[cfg(test)]
// mod tests {
//     use std::collections::HashSet;

//     use super::*;

//     #[test]
//     fn test_deframe() {
//         let mut program = Program {
//             units: vec![
//                 ProgramUnit::FunctionDefinition(FunctionDefinition {
//                     name: Some("foo".to_string()),
//                     parameters: vec![],
//                     body: vec![Statement::Assignment {
//                         target: LValue::Name("local_foo".to_string()),
//                         value: Box::new(Expr::FunctionCall {
//                             function: Box::new(Expr::Name("bar".to_string())),
//                             arguments: vec![],
//                         }),
//                     }],
//                     variables_need_saving: HashSet::new(),
//                     name_declarations: NameDeclarations {
//                         foreign_names: vec![],
//                         own_names: vec![],
//                     },
//                 }),
//                 ProgramUnit::FunctionDefinition(FunctionDefinition {
//                     name: Some("bar".to_string()),
//                     parameters: vec![],
//                     body: vec![Statement::Assignment {
//                         target: LValue::Name("local".to_string()),
//                         value: Box::new(Expr::FunctionCall {
//                             function: Box::new(Expr::Name("baz".to_string())),
//                             arguments: vec![],
//                         }),
//                     }],
//                     variables_need_saving: HashSet::new(),
//                     name_declarations: NameDeclarations {
//                         foreign_names: vec!["local_foo".to_string()],
//                         own_names: vec![],
//                     },
//                 }),
//                 ProgramUnit::FunctionDefinition(FunctionDefinition {
//                     name: Some("baz".to_string()),
//                     parameters: vec![],
//                     body: vec![Statement::Assignment {
//                         target: LValue::Name("local".to_string()),
//                         value: Box::new(Expr::FunctionCall {
//                             function: Box::new(Expr::Name("foo".to_string())),
//                             arguments: vec![],
//                         }),
//                     }],
//                     variables_need_saving: HashSet::new(),
//                     name_declarations: NameDeclarations {
//                         foreign_names: vec!["local_foo".to_string()],
//                         own_names: vec![],
//                     },
//                 }),
//             ],
//         };

//         let deframe_pass = DeframePass;
//         assert!(
//             deframe_pass
//                 .transform(&mut program, &OptimizationPassOpts { dump: false })
//                 .is_ok()
//         );
//         println!("{:?}", program);
//     }
// }
