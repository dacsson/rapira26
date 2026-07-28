//! Bytecode generation for rapira virtual machine
//!
//! The bytecode itself is an adopted/reworked version of LaMa VM bytecode.
//! To explore the bytecode format, see the `vm-core` crate.

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

use vm_core::{
    bytecode::{Builtin, Instruction, LWRITE_NEWLINE_FLAG, Op, UnaryOp, ValueRel},
    bytefile::Bytefile,
};

use crate::{
    ast::{
        BinaryOperator, Expr, FunctionDefinition, LValue, Literal, LoopStatement, NameDeclarations,
        SelectionStatement, Spannable, Statement, TypeDefinition, UnaryOperator,
    },
    codegen::{AbsolutGeneratedCodePath, CodegenTarget, ModuleMap, RunError},
    module::Module,
};

const BUILTIN_FUNCS: [(&str, Builtin); 9] = [
    ("abs", Builtin::Abs),
    ("абс", Builtin::Abs),
    ("sign", Builtin::Sign),
    ("знак", Builtin::Sign),
    ("корень", Builtin::Sqrt),
    ("sqrt", Builtin::Sqrt),
    ("целч", Builtin::Floor),
    ("окрч", Builtin::Round),
    ("индекс", Builtin::Index),
];

#[derive(Debug, Clone, PartialEq, Eq)]
enum Context {
    Global,
    Function,
}

struct Env {
    /// Locals counter
    local_counter: i32,
    /// Globals counter
    global_counter: i32,
    /// Mapping from local variable names to their bytecode index
    locals: HashMap<String, i32>,
    /// Mapping from global variable names to their bytecode index
    globals: HashMap<String, i32>,
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
                local_counter: 0,
                global_counter: 0,
                locals: HashMap::new(),
                globals: HashMap::new(),
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

                if let Some(&(_, builtin)) = BUILTIN_FUNCS.iter().find(|(n, _)| name == n) {
                    // A builtin call
                    for argument in arguments {
                        instrs.extend(self.emit_expr(argument));
                    }
                    instrs.push(Instruction::CALLBUILTIN {
                        name: builtin, // why do need to clone this?
                        n: arguments.len() as i32,
                    });
                } else {
                    panic!("Function call not implemented");
                }
            }
            Expr::Name(name) => {
                // Load variable by index
                let Some(&index) = self
                    .env
                    .locals
                    .get(name)
                    .or_else(|| self.env.globals.get(name))
                else {
                    panic!("Unknown local variable: {}", name);
                };

                let is_local = self.env.context == Context::Function;
                let rel = if is_local {
                    ValueRel::Local
                } else {
                    ValueRel::Global
                };

                instrs.push(Instruction::LOAD {
                    rel: rel,
                    index: index,
                });
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

                // For Slice, CALLBUILTIN.n is a bound-presence bitmask rather
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

                instrs.push(Instruction::CALLBUILTIN {
                    name: Builtin::Slice,
                    n: bounds,
                });
            }
            _ => panic!("Not implemented: {:?}", expr),
        }

        instrs
    }

    fn emit_statement(&mut self, stmt_span: &Spannable<Statement>) -> Vec<Instruction> {
        let stmt = &stmt_span.node;

        match stmt {
            Statement::Declaration { target, value } => {
                let mut instrs = self.emit_expr(value);

                let LValue::Name(name) = &target.node else {
                    panic!("Not implemented: {:?}", target.node);
                };

                let (index, rel) = self.env.allocate_variable(name.clone());
                instrs.push(Instruction::STORE { rel, index });
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
                Statement::Assignment { .. } => count += 1,
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
                Statement::Loop(LoopStatement { body, .. }) => {
                    count += self.count_locals(body);
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
        self.emit_top_level_def(&module.toplevel);

        // Map this module path to the bytefile bits
        module_map.insert(module.path.clone(), self.bytefile.encode());

        module_map
    }

    fn compile(
        &mut self,
        modules_codes: ModuleMap,
        current_dir: &PathBuf,
        flags: &[String],
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
    ) {
        todo!()
    }

    fn emit_function_def(&mut self, func_def_span: &Spannable<FunctionDefinition>) {
        let func_def = &func_def_span.node;
        let locals = self.count_locals(&func_def.body);
        let mut instructions = Vec::new();

        // Emit BEGIN of function
        let begin_instr = Instruction::BEGIN {
            args: func_def.parameters.len() as i32,
            locals: locals as i32,
        };
        instructions.push(begin_instr);

        // Emit function body
        for stmt in &func_def.body {
            instructions.extend(self.emit_statement(stmt));
        }

        // Emit END of function
        instructions.push(Instruction::END);

        instructions.iter().for_each(|instr| {
            self.bytefile.add_instruction(instr).unwrap();
        });
    }

    fn emit_type_def(&mut self, type_def_span: &Spannable<TypeDefinition>) {
        todo!()
    }

    fn emit_top_level_def(&mut self, top_level: &Vec<Spannable<Statement>>) {
        // Top-level is just a main function, nothing special actually
        // The important thing is that top-level is evaluated on import (if used as module)
        // or immediately if its a main module (look at docs for more info)

        let top_level_name = "main".to_string();
        let locals = self.count_locals(top_level);
        let begin_instr_length = self.bytefile.get_current_offset() + 1 /* 1 byte for BEGIN */ +
            std::mem::size_of::<i32>() /* byte size of `args` */ + std::mem::size_of::<i32>() /* byte size of `locals` */;
        let top_level_offset = self.bytefile.get_current_offset();

        self.emit_function_def(&Spannable {
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
    }
}
