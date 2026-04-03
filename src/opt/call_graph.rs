//! A simple call graph construction with declared variables for functions and procedures.
//!
//! Graph nodes also store local, foreign variables as well as variables that should be saved
//! in function/procedure frames.

use crate::ast::*;
use petgraph::dot::{Config, Dot};
use petgraph::graph::DiGraph;
use std::collections::HashMap;

/// A call graph of [`crate::ast::Program`]
pub struct CallGraph {
    graph: DiGraph<CallNode, ()>,
}

impl CallGraph {
    /// Constructs a new [`CallGraph`] from a [`crate::ast::Program`].
    pub fn new(program: &Program) -> Self {
        let mut graph = DiGraph::new();
        let mut top_level_node = CallNode {
            name: "top_level".to_string(),
            local_args: Vec::new(),
            foreign_args: Vec::new(),
            sent_as_inout_params: Vec::new(),
        };
        let mut top_level_call_exprs = Vec::new();

        // func_name -> (func_call_name1, func_call_name2, ...)
        let mut func_to_call: HashMap<String, Vec<String>> = HashMap::new();

        for unit in &program.units {
            match unit {
                ProgramUnit::FunctionDefinition(Spannable {
                    node:
                        FunctionDefinition {
                            name,
                            name_declarations,
                            body,
                            ..
                        },
                    ..
                })
                | ProgramUnit::ProcedureDefinition(Spannable {
                    node:
                        ProcedureDefinition {
                            name,
                            name_declarations,
                            body,
                            ..
                        },
                    ..
                }) => {
                    let Some(func_name) = name.clone() else {
                        continue;
                    };
                    let mut local_args = name_declarations.own_names.clone();
                    let foreign_args = name_declarations.foreign_names.clone();
                    let mut sent_as_inout_params = Vec::new();

                    // Also add local vars which are not set via `свои`
                    local_args.extend(body.iter().filter_map(|stmt| {
                        if let Statement::Assignment { target, .. } = &stmt.node {
                            if let LValue::Name(name) = &target.node {
                                Some(name.clone())
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    }));

                    sent_as_inout_params.extend(
                        body.iter()
                            .map(|stmt| match &stmt.node {
                                Statement::ProcedureCall { arguments, .. } => arguments
                                    .iter()
                                    .map(|arg| match arg {
                                        CallArgument::InOut(Spannable {
                                            node: LValue::Name(name),
                                            ..
                                        }) => Some(name.clone()),
                                        _ => None,
                                    })
                                    .collect::<Vec<_>>(),
                                _ => vec![],
                            })
                            .flatten()
                            .filter(|s| s.is_some())
                            .map(|s| s.unwrap()),
                    );

                    func_to_call.insert(
                        func_name.clone(),
                        body.iter()
                            .filter_map(|stmt| match &stmt.node {
                                Statement::ProcedureCall { procedure, .. } => {
                                    match &procedure.node {
                                        Expr::Name(name) => Some(name.clone()),
                                        _ => None,
                                    }
                                }
                                _ => None,
                            })
                            .collect::<Vec<_>>(),
                    );

                    // Also add func calls which are epxressions
                    let all_exprs: Vec<&Box<Spannable<Expr>>> = body
                        .iter()
                        .flat_map(|stmt| CallGraph::expressions_in_statement(&stmt.node))
                        .collect();
                    let all_func_calls_strings = all_exprs
                        .iter()
                        .flat_map(|expr| CallGraph::find_function_call_expr(*expr))
                        .collect::<Vec<_>>();

                    func_to_call
                        .get_mut(&func_name)
                        .unwrap()
                        .extend(all_func_calls_strings);

                    graph.add_node(CallNode {
                        name: func_name,
                        local_args,
                        foreign_args,
                        sent_as_inout_params,
                    });
                }
                ProgramUnit::Statement(stmt) => {
                    let all_exprs = CallGraph::expressions_in_statement(&stmt.node);
                    let all_func_calls_strings = all_exprs
                        .iter()
                        .flat_map(|expr| CallGraph::find_function_call_expr(expr))
                        .collect::<Vec<_>>();
                    top_level_call_exprs.extend(all_func_calls_strings);

                    match &stmt.node {
                        Statement::Assignment { target, .. } => {
                            if let LValue::Name(name) = &target.node {
                                top_level_node.local_args.push(name.clone());
                            }
                        }
                        Statement::ProcedureCall {
                            procedure,
                            arguments,
                        } => {
                            top_level_node.sent_as_inout_params.extend(
                                arguments.iter().filter_map(|arg| {
                                    if let CallArgument::InOut(lval) = arg {
                                        if let LValue::Name(name) = &lval.node {
                                            Some(name.clone())
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                }),
                            );

                            if let Expr::Name(name) = &procedure.node {
                                top_level_call_exprs.push(name.clone());
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Append top level node
        // TODO: for now, `чужие` usage from top-level is forbidden
        //       but we might change it later, when we do this code
        //       needs to be patched
        func_to_call.insert("top_level".to_string(), top_level_call_exprs);

        graph.add_node(top_level_node);

        for (func_name, call_exprs) in func_to_call {
            // Find node in graph with func_name
            if let Some(node) = graph
                .node_indices()
                .find(|&i| graph.node_weight(i).unwrap().name == func_name)
            {
                for call_expr in call_exprs {
                    if let Some(called_node) = graph
                        .node_indices()
                        .find(|&i| graph.node_weight(i).unwrap().name == call_expr)
                    {
                        // Connect nodes
                        graph.add_edge(node, called_node, ());
                    }
                }
            }
        }

        Self { graph }
    }

    /// Returns the name of the function being called by the given expression, searches recursively into expression tree
    fn find_function_call_expr(expr: &Box<Spannable<Expr>>) -> Vec<String> {
        match &expr.node {
            Expr::TupleConstruct(elems) => elems
                .iter()
                .map(|e| CallGraph::find_function_call_expr(e))
                .flatten()
                .collect(),
            Expr::Slice {
                collection,
                from,
                to,
            } => CallGraph::find_function_call_expr(collection)
                .into_iter()
                .chain(if from.is_some() {
                    CallGraph::find_function_call_expr(from.as_ref().unwrap())
                } else {
                    Vec::new()
                })
                .chain(if to.is_some() {
                    CallGraph::find_function_call_expr(to.as_ref().unwrap())
                } else {
                    Vec::new()
                })
                .collect(),
            Expr::BinaryOp { left, right, .. } => CallGraph::find_function_call_expr(left)
                .into_iter()
                .chain(CallGraph::find_function_call_expr(right))
                .collect(),
            Expr::FunctionCall {
                function,
                arguments,
            } => {
                let mut res = vec![];

                if let Expr::Name(fname) = &function.node {
                    res.push(fname.clone());
                }

                arguments.iter().for_each(|arg| {
                    res.extend(CallGraph::find_function_call_expr(arg));
                });

                res
            }
            Expr::Literal(_) => vec![],
            Expr::Subscript { collection, index } => CallGraph::find_function_call_expr(collection)
                .into_iter()
                .chain(CallGraph::find_function_call_expr(index))
                .collect(),
            Expr::Name(_) => vec![],
            Expr::UnaryOp { operand, .. } => CallGraph::find_function_call_expr(operand),
        }
    }

    pub fn expressions_in_statement(statement: &Statement) -> Vec<&Box<Spannable<Expr>>> {
        match statement {
            Statement::Empty
            | Statement::ReturnFromProcedure
            | Statement::ReturnFromFunction(_) => Vec::new(),
            Statement::Assignment { value, .. } => vec![value],
            Statement::ProcedureCall { procedure, .. } => vec![procedure],
            Statement::Conditional { condition, .. } => vec![condition],
            Statement::Selection(selection) => match selection {
                SelectionStatement::ValueMatch { expression, .. } => vec![expression],
                SelectionStatement::ConditionList { cases, .. } => {
                    cases.iter().map(|c| &c.node.condition).collect()
                }
            },
            Statement::Loop(loop_) => {
                let mut expressions = Vec::new();

                match loop_.header {
                    LoopHeader::Infinite => {}
                    LoopHeader::Repeat(ref expr) => {
                        expressions.push(expr);
                    }
                    LoopHeader::For {
                        ref from,
                        ref to,
                        ref step,
                        ..
                    } => {
                        if let Some(from) = from {
                            expressions.push(from);
                        }
                        if let Some(to) = to {
                            expressions.push(to);
                        }
                        if let Some(step) = step {
                            expressions.push(step);
                        }
                    }
                }

                if let Some(condition) = &loop_.while_condition {
                    expressions.push(condition);
                }
                if let Some(post_condition) = &loop_.post_condition {
                    expressions.push(post_condition);
                }

                expressions.extend(
                    loop_
                        .body
                        .iter()
                        .map(|s| &s.node)
                        .flat_map(CallGraph::expressions_in_statement),
                );

                expressions
            }
            Statement::Output { values, .. } => values.iter().collect(),
            Statement::Input { .. } => Vec::new(),
            Statement::ExitLoop => Vec::new(),
        }
    }

    pub fn graph(&self) -> &DiGraph<CallNode, ()> {
        &self.graph
    }

    /// Dump the call graph as a DOT graph
    pub fn dump(&self) {
        println!(
            "{:?}",
            Dot::with_config(&self.graph, &[Config::EdgeNoLabel])
        );
    }
}

/// A node in the call graph, representing a function or procedure call with its local and foreign arguments
#[derive(Debug)]
pub struct CallNode {
    pub name: String,
    pub local_args: Vec<String>,
    pub foreign_args: Vec<String>,
    pub sent_as_inout_params: Vec<String>,
}

// #[cfg(test)]
// mod tests {
//     use std::collections::HashSet;

//     use super::*;

//     #[test]
//     fn test_find_function_call_expr() {
//         let expr = Expr::FunctionCall {
//             function: Box::new(Spannable::new(Expr::Name("foo".to_string()), (0, 0))),
//             arguments: vec![
//                 Box::new(Spannable::new(Expr::Literal(Literal::Integer(42)), (0, 0))),
//                 Box::new(Spannable::new(
//                     Expr::Subscript {
//                         collection: Box::new(Spannable::new(Expr::Name("bar".to_string()), (0, 0))),
//                         index: Box::new(Spannable::new(Expr::Literal(Literal::Integer(0)), (0, 0))),
//                     },
//                     (0, 0),
//                 )),
//             ],
//         };

//         let expr2 = Expr::BinaryOp {
//             operator: BinaryOperator::Add,
//             left: Box::new(Expr::FunctionCall {
//                 function: Box::new(Expr::Name("foo".to_string())),
//                 arguments: vec![Box::new(Expr::FunctionCall {
//                     function: Box::new(Expr::Name("bar".to_string())),
//                     arguments: vec![],
//                 })],
//             }),
//             right: Box::new(Expr::Literal(Literal::Integer(42))),
//         };

//         assert_eq!(
//             CallGraph::find_function_call_expr(&Box::new(expr)),
//             vec!["foo"],
//         );

//         assert_eq!(
//             CallGraph::find_function_call_expr(&Box::new(expr2)),
//             vec!["foo", "bar"],
//         );
//     }

//     #[test]
//     fn test_dump() {
//         let program = Program {
//             units: vec![
//                 ProgramUnit::FunctionDefinition(FunctionDefinition {
//                     name: Some("foo".to_string()),
//                     parameters: vec![],
//                     body: vec![Statement::Assignment {
//                         target: LValue::Name("local".to_string()),
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
//                         foreign_names: vec![],
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
//                         foreign_names: vec![],
//                         own_names: vec![],
//                     },
//                 }),
//             ],
//         };

//         let graph = CallGraph::new(&program);
//         graph.dump();
//     }
// }
