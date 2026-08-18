//! Bytecode generation for rapira virtual machine
//!
//! The bytecode itself is an adopted/reworked version of LaMa VM bytecode.
//! To explore the bytecode format, see the `vm-core` crate.

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use vm_core::{
    bytecode::{
        Builtin, CompareJumpKind, Instruction, LWRITE_NEWLINE_FLAG, Label, Op, UnaryOp, ValueRel,
    },
    bytefile::Bytefile,
};

use crate::{
    ast::{
        BinaryOperator, CallArgument, Expr, FunctionDefinition, LValue, Literal, LoopHeader,
        LoopStatement, NameDeclarations, SelectionStatement, Spannable, Statement, TypeDefinition,
        UnaryOperator,
    },
    codegen::{AbsolutGeneratedCodePath, CodegenTarget, ModuleMap, RunError},
    module::Module,
};

const BUILTIN_FUNCS: [(&str, Builtin); 10] = [
    ("abs", Builtin::Abs),
    ("абс", Builtin::Abs),
    ("sign", Builtin::Sign),
    ("знак", Builtin::Sign),
    ("корень", Builtin::Sqrt),
    ("sqrt", Builtin::Sqrt),
    ("целч", Builtin::Floor),
    ("окрч", Builtin::Round),
    ("индекс", Builtin::Index),
    ("тип_цел", Builtin::Tint),
];

#[derive(Debug, Clone, PartialEq, Eq)]
enum Context {
    Global,
    Function,
}

struct Env {
    local_counter: i32,
    arg_counter: i32,
    label_counter: i32,
    global_counter: i32,
    /// Mapping from local variable names to their bytecode index
    locals: HashMap<String, i32>,
    args: HashMap<String, i32>,
    /// Mapping from global variable names to their bytecode index
    globals: HashMap<String, i32>,
    /// Currently active loop `end` labels, innermost on top.
    ///
    /// An `exit` statement must jump out of the nearest enclosing loop, so
    /// `ExitLoop` reads the this stack top
    loop_end_labels: Vec<Label>,
    /// Current context (global or function)
    context: Context,
}

impl Env {
    /// Allocate a new variable in the current context, returning its index and
    /// whether it is local or global
    fn allocate_variable(&mut self, name: String) -> (i32, ValueRel) {
        if let Context::Function = self.context {
            let index = self.local_counter;
            self.locals.insert(name, index);
            self.local_counter = index + 1;
            (index, ValueRel::Local)
        } else {
            let index = self.global_counter;
            self.globals.insert(name, index);
            self.global_counter = index + 1;
            (index, ValueRel::Global)
        }
    }

    fn allocate_argument(&mut self, name: String) -> (i32, ValueRel) {
        let index = self.arg_counter;
        self.args.insert(name, index);
        self.arg_counter = index + 1;
        (index, ValueRel::Arg)
    }

    fn deallocate_variable(&mut self, name: &str) -> Option<i32> {
        if let Context::Function = self.context {
            self.locals.remove(name)
        } else {
            self.globals.remove(name)
        }
    }

    fn find_variable(&self, name: &str) -> Option<(i32, ValueRel)> {
        let arg = self.args.get(name).map(|id| (*id, ValueRel::Arg));
        let local = self.locals.get(name).map(|id| (*id, ValueRel::Local));
        let global = self.globals.get(name).map(|id| (*id, ValueRel::Global));
        local.or(arg).or(global)
    }

    fn fresh_label(&mut self, prefix: &str) -> Label {
        let name = format!("{prefix}_{}", self.label_counter);
        self.label_counter += 1;

        Label { name, offset: None }
    }

    /// Register a loop `end` label so that `exit` inside the loop's body
    /// jumps to it
    ///
    /// Must be paired with [`Self::pop_loop_end`] when the loop
    /// is finished emitting
    fn push_loop_end(&mut self, label: Label) {
        self.loop_end_labels.push(label);
    }

    /// Forget the most recently pushed loop `end` label
    fn pop_loop_end(&mut self) {
        if self.loop_end_labels.pop().is_none() {
            panic!("Unbalanced push/pop of loop end labels");
        }
    }

    /// The end label of the innermost currently emitting loop, if any
    fn current_loop_end(&self) -> Option<&Label> {
        self.loop_end_labels.last()
    }
}

/// Bytecode generator for the rapira virtual machine
pub struct BcGen {
    bytefile: Bytefile,
    env: Env,
}

impl BcGen {
    pub fn new() -> Self {
        Self {
            bytefile: Bytefile::new(),
            env: Env {
                // TODO: maybe we can use `count_locals` to
                //       pre-allocate enough indecies?
                local_counter: 0,
                arg_counter: 0,
                label_counter: 0,
                global_counter: 0,

                locals: HashMap::new(),
                args: HashMap::new(),
                globals: HashMap::new(),
                loop_end_labels: Vec::new(),
                context: Context::Global,
            },
        }
    }

    fn emit_expr(&mut self, expr_span: &Spannable<Expr>) -> Vec<Instruction> {
        let expr = &expr_span.node;
        let mut instrs = Vec::new();

        match expr {
            Expr::Literal(lit) => match lit {
                Literal::Integer(value) => {
                    // TODO: i32 vs i64
                    instrs.push(Instruction::CONST {
                        value: *value as i32,
                    });
                }
                Literal::Boolean(value) => {
                    instrs.push(Instruction::BOOL { value: *value });
                }
                Literal::Real(value) => {
                    instrs.push(Instruction::CONSTF { value: *value });
                }
                Literal::Text(value) => {
                    // We should push all new strings to
                    // a string table of bytefile first
                    if let Some(index) = self.bytefile.find_string_offset(value) {
                        instrs.push(Instruction::STRING {
                            index: index as i32,
                        })
                    } else {
                        // First occurence of a string
                        instrs.push(Instruction::STRING {
                            index: self.bytefile.add_string(value.clone()) as i32,
                        });
                    }
                }
                Literal::Null => {
                    instrs.push(Instruction::NULL);
                }
            },
            Expr::BinaryOp {
                operator,
                left,
                right,
            } => {
                instrs.extend(self.emit_expr(left));
                instrs.extend(self.emit_expr(right));

                instrs.push(Instruction::BINOP {
                    op: self.ast_binop_to_vm_op(operator),
                });
            }
            Expr::UnaryOp { operator, operand } => {
                // TODO: maybe add unary opcodes to bytecode
                match operator {
                    UnaryOperator::Negate => {
                        instrs.extend(self.emit_expr(operand));
                        instrs.push(Instruction::UNARY {
                            op: UnaryOp::Negate,
                        });
                    }
                    UnaryOperator::Plus => {
                        instrs.extend(self.emit_expr(operand));
                        // Nothing to do
                    }
                    UnaryOperator::Not => {
                        instrs.extend(self.emit_expr(operand));
                        instrs.push(Instruction::UNARY { op: UnaryOp::Not });
                    }
                    UnaryOperator::Length => {
                        instrs.extend(self.emit_expr(operand));
                        instrs.push(Instruction::CALLBUILTIN {
                            name: Builtin::Llength,
                            n: 0,
                        });
                    }
                }
            }
            Expr::TupleConstruct(elements) => {
                elements
                    .iter()
                    .for_each(|el| instrs.extend(self.emit_expr(el)));

                instrs.push(Instruction::TUPLE {
                    n: elements.len() as i32,
                });
            }
            Expr::FunctionCall {
                function,
                arguments,
            } => {
                let Expr::Name(name) = &function.node else {
                    panic!("Anonymous functions not implemented");
                };

                for argument in arguments {
                    instrs.extend(self.emit_expr(argument));
                }

                if let Some(&(_, builtin)) = BUILTIN_FUNCS.iter().find(|(n, _)| name == n) {
                    // A builtin call
                    instrs.push(Instruction::CALLBUILTIN {
                        name: builtin, // why do need to clone this?
                        n: arguments.len() as i32,
                    });
                } else {
                    instrs.push(Instruction::CALL {
                        dest: Label {
                            name: name.clone(),
                            offset: None,
                        },
                        n: arguments.len() as i32,
                    });
                }
            }
            Expr::Name(name) => {
                let Some((index, rel)) = self.env.find_variable(name) else {
                    panic!("Unknown local variable: {}", name);
                };

                instrs.push(Instruction::LOAD { rel, index });
            }
            Expr::Subscript { collection, index } => {
                instrs.extend(self.emit_expr(collection));
                instrs.extend(self.emit_expr(index));
                instrs.push(Instruction::ELEM);
            }
            Expr::Slice {
                collection,
                from,
                to,
            } => {
                instrs.extend(self.emit_expr(collection));

                // For Slice, `bound` is a bound-presence bitmask rather
                // than an argument count: bit 0 = from, bit 1 = to.
                let mut bounds = 0;
                if let Some(from) = from {
                    instrs.extend(self.emit_expr(from));
                    bounds |= 1;
                }
                if let Some(to) = to {
                    instrs.extend(self.emit_expr(to));
                    bounds |= 2;
                }

                instrs.push(Instruction::SLICE { bounds });
            }
        }

        instrs
    }

    fn emit_statement(&mut self, stmt_span: &Spannable<Statement>) -> Vec<Instruction> {
        let stmt = &stmt_span.node;

        match stmt {
            Statement::Declaration { target, value } => {
                let mut instrs = self.emit_expr(value);

                let LValue::Name(name) = &target.node else {
                    todo!("Not implemented: {:?}", target.node);
                };

                let (index, rel) = self.env.allocate_variable(name.clone());
                instrs.push(Instruction::STORE { rel, index });
                instrs
            }
            Statement::Assignment { target, value } => {
                let mut instrs = self.emit_expr(value);

                match &target.node {
                    LValue::Name(name) => {
                        let Some((index, rel)) = self.env.find_variable(name) else {
                            todo!("unknown variable")
                        };

                        instrs.push(Instruction::STORE { rel, index });
                    }
                    LValue::Subscript { collection, index } => {
                        let Expr::Name(name) = &collection.node else {
                            todo!("Not implemented: {:?}", target.node);
                        };

                        let Some((vindex, rel)) = self.env.find_variable(name) else {
                            todo!("unknown variable")
                        };

                        // Stack: [value, index, collection]
                        // STA pops: collection, index, value
                        instrs.extend(self.emit_expr(index));
                        instrs.push(Instruction::LOAD { rel, index: vindex });
                        instrs.push(Instruction::STA);
                    }
                    LValue::Slice {
                        collection,
                        from,
                        to,
                    } => {
                        let Expr::Name(name) = &collection.node else {
                            todo!("Not implemented: {:?}", target.node);
                        };

                        let Some((vindex, rel)) = self.env.find_variable(name) else {
                            todo!("unknown variable")
                        };

                        // collection -> then from/to bounds -> SLICE -> STS
                        // SLICE: pops (collection, from, to) and pushes a slice
                        // STS: pops (value, slice) for the assignment
                        instrs.push(Instruction::LOAD { rel, index: vindex });

                        let mut bounds = 0;
                        if let Some(from) = from {
                            instrs.extend(self.emit_expr(from));
                            bounds |= 1;
                        }
                        if let Some(to) = to {
                            instrs.extend(self.emit_expr(to));
                            bounds |= 2;
                        }

                        instrs.push(Instruction::SLICE { bounds });
                        instrs.push(Instruction::STS);
                    }
                    LValue::Field { .. } => todo!(),
                };

                instrs
            }
            Statement::Output { no_newline, values } => {
                let mut instrs = Vec::new();

                // Emit bytecode instructions for each value
                for value in values {
                    instrs.extend(self.emit_expr(value));
                }

                // Newline is a single bit stores in n
                let n = if *no_newline {
                    // clear the newline bit, keep the argument count
                    values.len() as i32 & !LWRITE_NEWLINE_FLAG
                } else {
                    // 1 means add "\n" at the end
                    values.len() as i32 | LWRITE_NEWLINE_FLAG
                };

                // Finally emit call to write builtin
                let print_instr = Instruction::CALLBUILTIN {
                    name: Builtin::Lwrite,
                    n,
                };
                instrs.push(print_instr);

                instrs
            }
            Statement::Loop(LoopStatement { header, body }) => {
                let mut instrs = Vec::new();

                match header {
                    LoopHeader::Repeat(count) => {
                        let (index, rel) = self.env.allocate_variable(format!(
                            "repeat_{}_{}",
                            count.position_start, count.position_end
                        ));

                        instrs.extend(self.emit_expr(count));
                        instrs.push(Instruction::STORE { rel, index });

                        let loop_label = self.env.fresh_label("repeat_loop");
                        let end_label = self.env.fresh_label("repeat_end");
                        self.env.push_loop_end(end_label.clone());

                        instrs.push(Instruction::LABEL {
                            name: loop_label.name.clone(),
                        });

                        // While the remaining count is positive, run the body
                        instrs.push(Instruction::LOAD { rel, index });
                        instrs.push(Instruction::CONST { value: 0 });
                        instrs.push(Instruction::BINOP { op: Op::GT });
                        instrs.push(Instruction::CJMP {
                            dest: end_label.clone(),
                            kind: CompareJumpKind::ISZERO,
                        });

                        for stmt in body {
                            instrs.extend(self.emit_statement(stmt));
                        }

                        // Decrement the remaining count, then re-test the guard
                        instrs.push(Instruction::LOAD { rel, index });
                        instrs.push(Instruction::CONST { value: 1 });
                        instrs.push(Instruction::BINOP { op: Op::SUB });
                        instrs.push(Instruction::STORE { rel, index });
                        instrs.push(Instruction::JMP {
                            dest: loop_label.clone(),
                        });
                        instrs.push(Instruction::LABEL {
                            name: end_label.name.clone(),
                        });

                        // Destroy the counter
                        let counter_name =
                            format!("repeat_{}_{}", count.position_start, count.position_end);
                        self.env.deallocate_variable(&counter_name);

                        self.env.pop_loop_end();

                        return instrs;
                    }
                    LoopHeader::For {
                        variable,
                        from,
                        to,
                        step,
                    } => {
                        let from_expr = from.as_ref().map(|expr| self.emit_expr(expr));
                        let to_expr = to.as_ref().map(|expr| self.emit_expr(expr));
                        let step_expr = step
                            .as_ref()
                            .map(|expr| self.emit_expr(expr))
                            .unwrap_or(vec![Instruction::CONST { value: 1 }]);

                        // TODO: we can make a helper for workin in scopes
                        //       maybe something like: `do_in_scope(||)`

                        let (index, rel) = self.env.allocate_variable(variable.clone());

                        // Declare loop variable
                        if let Some(from) = from_expr {
                            instrs.extend(from);
                        } else {
                            // TODO: should we default to 0?
                            instrs.extend(vec![Instruction::CONST { value: 0 }]);
                        }
                        instrs.push(Instruction::STORE { rel, index });

                        let loop_label = self.env.fresh_label("for_loop");
                        let end_label = self.env.fresh_label("for_end");
                        self.env.push_loop_end(end_label.clone());
                        instrs.push(Instruction::LABEL {
                            name: loop_label.name.clone(),
                        });

                        // An omitted upper bound makes the `for` loop unbounded.
                        if let Some(to) = to_expr {
                            // step > 0 && variable <= to
                            instrs.extend(step_expr.clone());
                            instrs.push(Instruction::CONST { value: 0 });
                            instrs.push(Instruction::BINOP { op: Op::GT });
                            instrs.push(Instruction::LOAD { rel, index });
                            instrs.extend(to.clone());
                            instrs.push(Instruction::BINOP { op: Op::LEQ });
                            instrs.push(Instruction::BINOP { op: Op::AND });

                            // step <= 0 && variable >= to
                            instrs.extend(step_expr.clone());
                            instrs.push(Instruction::CONST { value: 0 });
                            instrs.push(Instruction::BINOP { op: Op::LEQ });
                            instrs.push(Instruction::LOAD { rel, index });
                            instrs.extend(to);
                            instrs.push(Instruction::BINOP { op: Op::GEQ });
                            instrs.push(Instruction::BINOP { op: Op::AND });

                            instrs.push(Instruction::BINOP { op: Op::OR });

                            instrs.push(Instruction::CJMP {
                                dest: end_label.clone(),
                                kind: CompareJumpKind::ISZERO,
                            });
                        }

                        for stmt in body {
                            instrs.extend(self.emit_statement(stmt));
                        }

                        // Advance the loop variable before jumping back to the guard.
                        instrs.push(Instruction::LOAD { rel, index });
                        instrs.extend(step_expr);
                        instrs.push(Instruction::BINOP { op: Op::ADD });
                        instrs.push(Instruction::STORE { rel, index });
                        instrs.push(Instruction::JMP {
                            dest: loop_label.clone(),
                        });
                        instrs.push(Instruction::LABEL {
                            name: end_label.name.clone(),
                        });

                        // Destroy loop variable
                        self.env.deallocate_variable(&variable);

                        self.env.pop_loop_end();
                    }
                    LoopHeader::While(condition) => {
                        let loop_label = self.env.fresh_label("while_loop");
                        let end_label = self.env.fresh_label("while_end");

                        instrs.push(Instruction::LABEL {
                            name: loop_label.name.clone(),
                        });
                        instrs.extend(self.emit_expr(condition));

                        self.env.push_loop_end(end_label.clone());
                        instrs.push(Instruction::CJMP {
                            dest: end_label.clone(),
                            kind: CompareJumpKind::ISZERO,
                        });
                        for stmt in body {
                            instrs.extend(self.emit_statement(stmt));
                        }
                        instrs.push(Instruction::JMP {
                            dest: loop_label.clone(),
                        });
                        instrs.push(Instruction::LABEL {
                            name: end_label.name.clone(),
                        });

                        self.env.pop_loop_end();
                    }
                }

                instrs
            }
            Statement::ReturnFromFunction(expr) => {
                let mut instrs = self.emit_expr(expr);

                instrs.push(Instruction::END);

                instrs
            }
            Statement::Conditional {
                condition,
                then_body,
                else_body,
            } => {
                let mut instrs = self.emit_expr(condition);

                let begin_label = self.env.fresh_label("if_begin");
                let end_label = self.env.fresh_label("if_skip");
                let mut else_label = None;

                if let Some(..) = else_body {
                    else_label = Some(self.env.fresh_label("else"));
                }

                // Push begin label
                instrs.push(Instruction::LABEL {
                    name: begin_label.name.clone(),
                });

                // guard
                if let Some(else_label) = &else_label {
                    // goto else
                    instrs.push(Instruction::CJMP {
                        dest: else_label.clone(),
                        kind: CompareJumpKind::ISZERO,
                    });
                } else {
                    // goto next block
                    instrs.push(Instruction::CJMP {
                        dest: end_label.clone(),
                        kind: CompareJumpKind::ISZERO,
                    });
                }

                for stmt in then_body {
                    instrs.extend(self.emit_statement(stmt));
                }

                if let Some(else_body) = else_body {
                    // success then branch must skip the else branch
                    instrs.push(Instruction::JMP {
                        dest: end_label.clone(),
                    });

                    instrs.push(Instruction::LABEL {
                        name: else_label.clone().unwrap().name.clone(),
                    });

                    for stmt in else_body {
                        instrs.extend(self.emit_statement(stmt));
                    }
                }

                instrs.push(Instruction::LABEL {
                    name: end_label.name.clone(),
                });

                instrs
            }
            Statement::ProcedureCall {
                procedure,
                arguments,
            } => {
                let mut instrs = vec![];

                let Expr::Name(name) = &procedure.node else {
                    panic!("Anonymous functions not implemented");
                };

                for argument in arguments {
                    let CallArgument::Input(arg) = &argument else {
                        todo!("inout arguments are not supported");
                    };
                    instrs.extend(self.emit_expr(arg));
                }

                if let Some(&(_, builtin)) = BUILTIN_FUNCS.iter().find(|(n, _)| name == n) {
                    // A builtin call
                    instrs.push(Instruction::CALLBUILTIN {
                        name: builtin, // why do need to clone this?
                        n: arguments.len() as i32,
                    });
                } else {
                    instrs.push(Instruction::CALL {
                        dest: Label {
                            name: name.clone(),
                            offset: None,
                        },
                        n: arguments.len() as i32,
                    });
                }

                // Because this is a statement, we need to
                // discard the return value
                instrs.push(Instruction::DROP);

                instrs
            }
            Statement::Input {
                text_mode,
                variables,
            } => {
                if *text_mode {
                    todo!("text_mode in input");
                }

                let mut instrs = vec![];

                for var in variables {
                    instrs.push(Instruction::CALLBUILTIN {
                        name: Builtin::Lread,
                        n: 0,
                    });

                    let LValue::Name(name) = &var.node else {
                        todo!("{:?}", var.node);
                    };

                    // Re-use the variable's existing slot if it was already
                    // declared
                    let (index, rel) = self
                        .env
                        .find_variable(name)
                        .unwrap_or_else(|| self.env.allocate_variable(name.clone()));
                    instrs.push(Instruction::STORE { rel, index });
                }

                instrs
            }
            Statement::ExitLoop => {
                let mut instrs = vec![];

                // Jump to the end of the innermost enclosing loop, per `exit`
                // semantics
                let Some(nearest_end_label) = self.env.current_loop_end() else {
                    panic!("Nowhere to jump from an early exit");
                };

                instrs.push(Instruction::JMP {
                    dest: nearest_end_label.clone(),
                });

                instrs
            }
            _ => panic!("Not implemented: {:?}", stmt),
        }
    }

    /// Determines the number of local variables in a block body
    ///
    /// Searches to the maximum depth for nested blocks
    fn count_locals(&self, body: &[Spannable<Statement>]) -> usize {
        // TODO: there is name_declarations field in FunctionDefinition
        let mut count = 0;
        for stmt in body {
            match &stmt.node {
                Statement::Declaration { .. } => count += 1,
                Statement::Conditional {
                    then_body,
                    else_body,
                    ..
                } => {
                    count += self.count_locals(then_body);
                    if let Some(else_body) = else_body {
                        count += self.count_locals(else_body);
                    }
                }
                Statement::Selection(SelectionStatement::ValueMatch {
                    cases, else_body, ..
                }) => {
                    for case in cases {
                        count += self.count_locals(&case.node.body);
                    }
                    if let Some(else_body) = else_body {
                        count += self.count_locals(else_body);
                    }
                }
                Statement::Loop(LoopStatement { header, body, .. }) => {
                    // A `for` loop introduces its iteration variable in addition
                    // to any declarations in the loop body.
                    // As well as the `repeat` loop (implicit counter variable)
                    if let LoopHeader::For { .. } | LoopHeader::Repeat { .. } = header {
                        count += 1;
                    }
                    count += self.count_locals(body);
                }
                Statement::Input {
                    text_mode: _,
                    variables,
                } => {
                    count += variables.len() as usize;
                }
                _ => {}
            }
        }
        count
    }

    fn ast_binop_to_vm_op(&self, operator: &BinaryOperator) -> Op {
        match operator {
            BinaryOperator::Power => Op::POW,
            BinaryOperator::Multiply => Op::MUL,
            BinaryOperator::Divide => Op::DIV,
            BinaryOperator::IntegerDivide => Op::IDIV,
            BinaryOperator::Remainder => Op::MOD,
            BinaryOperator::Add => Op::ADD,
            BinaryOperator::Subtract => Op::SUB,
            BinaryOperator::Greater => Op::GT,
            BinaryOperator::Less => Op::LT,
            BinaryOperator::GreaterOrEqual => Op::GEQ,
            BinaryOperator::LessOrEqual => Op::LEQ,
            BinaryOperator::Equal => Op::EQ,
            BinaryOperator::NotEqual => Op::NEQ,
            BinaryOperator::And => Op::AND,
            BinaryOperator::Or => Op::OR,
            BinaryOperator::Dot => panic!("DOT binop not implemented!"),
        }
    }
}

impl CodegenTarget for BcGen {
    fn generate(&mut self, modules: Vec<Module>) -> ModuleMap {
        // Forbid multi-module compilation for now
        if modules.len() > 1 {
            panic!("Multi-module compilation is not supported yet");
        }

        let module = &modules[0];
        let mut module_map = ModuleMap::new();

        // First emit top-level, after this call
        // the bytefile should have a main function offset
        // in public symbols
        let mut instrs = self.emit_top_level_def(&module.toplevel);

        // TODO: emit other constructs
        for func in &module.functions {
            instrs.extend(self.emit_function_def(func));
        }

        self.bytefile.add_instructions(&instrs).unwrap();

        // Map this module path to the bytefile bits
        module_map.insert(module.path.clone(), self.bytefile.encode());

        module_map
    }

    fn compile(
        &mut self,
        modules_codes: ModuleMap,
        current_dir: &PathBuf,
        dump: bool,
    ) -> Result<Vec<AbsolutGeneratedCodePath>, RunError> {
        let (module_path, bytefile) = modules_codes.iter().next().unwrap();
        let module_path = module_path.clone().0;
        let bytefile_path = module_path.with_extension("rbc");
        std::fs::write(&bytefile_path, bytefile).unwrap();

        Ok(vec![AbsolutGeneratedCodePath(bytefile_path)])
    }

    fn emit_procedure_def(
        &mut self,
        proc_def_span: &crate::ast::Spannable<crate::ast::ProcedureDefinition>,
    ) -> Vec<Instruction> {
        todo!()
    }

    fn emit_function_def(
        &mut self,
        func_def_span: &Spannable<FunctionDefinition>,
    ) -> Vec<Instruction> {
        self.env.context = Context::Function;
        self.env.locals.clear();
        self.env.local_counter = 0;
        self.env.args.clear();
        self.env.arg_counter = 0;

        // Memorize arguments
        for arg in &func_def_span.node.parameters {
            self.env.allocate_argument(arg.clone());
        }

        let func_def = &func_def_span.node;
        let locals = self.count_locals(&func_def.body);
        let mut instructions = Vec::new();

        instructions.push(Instruction::LABEL {
            name: func_def.name.as_ref().unwrap().clone(),
        });

        // Emit BEGIN of function
        let begin_instr = Instruction::BEGIN {
            args: func_def.parameters.len() as i32,
            locals: locals as i32,
        };
        instructions.push(begin_instr);

        // Emit function body
        let mut explicit_return = false;
        for stmt in &func_def.body {
            instructions.extend(self.emit_statement(stmt));
            if let Statement::ReturnFromFunction(_) = &stmt.node {
                explicit_return = true;
            }
        }

        // Append `return null` to the function if no explicit return (at the end) was found
        if !explicit_return {
            instructions.push(Instruction::NULL);
            instructions.push(Instruction::END);
        }

        self.env.context = Context::Global;

        instructions
    }

    fn emit_type_def(&mut self, type_def_span: &Spannable<TypeDefinition>) -> Vec<Instruction> {
        todo!()
    }

    fn emit_top_level_def(&mut self, top_level: &Vec<Spannable<Statement>>) -> Vec<Instruction> {
        // Top-level is just a main function, nothing special actually
        // The important thing is that top-level is evaluated on import (if used as module)
        // or immediately if its a main module

        let top_level_name = "main".to_string();
        let top_level_offset = self.bytefile.get_current_offset();

        let instrs = self.emit_function_def(&Spannable {
            node: FunctionDefinition {
                name: Some(top_level_name.clone()),
                parameters: Vec::new(),
                body: top_level.clone(),
                name_declarations: NameDeclarations {
                    foreign_names: Vec::new(),
                    own_names: Vec::new(),
                },
                variables_need_saving: HashSet::new(),
            },
            position_start: top_level
                .first()
                .map(|stmt| stmt.position_start)
                .unwrap_or(0),
            position_end: top_level.last().map(|stmt| stmt.position_end).unwrap_or(0),
        });

        // Now we can mark the current offset as main_offset
        self.bytefile.main_offset = top_level_offset as u32;

        // Also make it public
        self.bytefile.add_string(top_level_name.clone());
        self.bytefile
            .add_public_symbol(&top_level_name, self.bytefile.main_offset)
            .unwrap();

        instrs
    }
}
