use rapira26::ast::*;
use rapira26::lexer::Lexer;
use rapira26::parser::Parser;

fn parse(source: &str) -> Program {
    let lexer = Lexer::new(source);
    let parser = Parser::new(lexer);
    parser.parse_program().unwrap_or_else(|error| {
        panic!("parse error in {source:?}: {error}")
    })
}

fn parse_first_statement(source: &str) -> Statement {
    let program = parse(source);
    assert!(!program.units.is_empty(), "expected at least one program unit");
    match program.units.into_iter().next().unwrap() {
        ProgramUnit::Statement(statement) => statement,
        other => panic!("expected statement, got {other:?}"),
    }
}

fn parse_first_procedure(source: &str) -> ProcedureDefinition {
    let program = parse(source);
    match program.units.into_iter().next().unwrap() {
        ProgramUnit::ProcedureDefinition(def) => def,
        other => panic!("expected procedure definition, got {other:?}"),
    }
}

fn parse_first_function(source: &str) -> FunctionDefinition {
    let program = parse(source);
    match program.units.into_iter().next().unwrap() {
        ProgramUnit::FunctionDefinition(def) => def,
        other => panic!("expected function definition, got {other:?}"),
    }
}

// ── Literals ────────────────────────────────────────────────────────────────

#[test]
fn parse_output_integer() {
    let statement = parse_first_statement("вывод: 42");
    match statement {
        Statement::Output { no_newline, values } => {
            assert!(!no_newline);
            assert_eq!(values.len(), 1);
            assert!(matches!(*values[0], Expr::Literal(Literal::Integer(42))));
        }
        other => panic!("expected Output, got {other:?}"),
    }
}

#[test]
fn parse_output_text() {
    let statement = parse_first_statement("вывод: \"hello\"");
    match statement {
        Statement::Output { values, .. } => {
            assert_eq!(values.len(), 1);
            match &*values[0] {
                Expr::Literal(Literal::Text(text)) => assert_eq!(text, "hello"),
                other => panic!("expected text literal, got {other:?}"),
            }
        }
        other => panic!("expected Output, got {other:?}"),
    }
}

#[test]
fn parse_output_boolean_constants() {
    let statement = parse_first_statement("вывод: да, нет, пусто");
    match statement {
        Statement::Output { values, .. } => {
            assert_eq!(values.len(), 3);
            assert!(matches!(*values[0], Expr::Literal(Literal::Boolean(true))));
            assert!(matches!(*values[1], Expr::Literal(Literal::Boolean(false))));
            assert!(matches!(*values[2], Expr::Literal(Literal::Null)));
        }
        other => panic!("expected Output, got {other:?}"),
    }
}

#[test]
fn parse_output_no_newline() {
    let statement = parse_first_statement("вывод бпс: 1");
    match statement {
        Statement::Output { no_newline, .. } => assert!(no_newline),
        other => panic!("expected Output, got {other:?}"),
    }
}

#[test]
fn parse_output_empty() {
    let statement = parse_first_statement("вывод:");
    match statement {
        Statement::Output { values, .. } => assert!(values.is_empty()),
        other => panic!("expected Output, got {other:?}"),
    }
}

// ── Arithmetic expressions ──────────────────────────────────────────────────

#[test]
fn parse_binary_addition() {
    let statement = parse_first_statement("вывод: 1 + 2");
    match statement {
        Statement::Output { values, .. } => {
            match &*values[0] {
                Expr::BinaryOp { operator, left, right } => {
                    assert_eq!(*operator, BinaryOperator::Add);
                    assert!(matches!(**left, Expr::Literal(Literal::Integer(1))));
                    assert!(matches!(**right, Expr::Literal(Literal::Integer(2))));
                }
                other => panic!("expected BinaryOp, got {other:?}"),
            }
        }
        other => panic!("expected Output, got {other:?}"),
    }
}

#[test]
fn parse_operator_precedence_mul_over_add() {
    // 1 + 2 * 3 should parse as 1 + (2 * 3)
    let statement = parse_first_statement("вывод: 1 + 2 * 3");
    match statement {
        Statement::Output { values, .. } => {
            match &*values[0] {
                Expr::BinaryOp { operator, left, right } => {
                    assert_eq!(*operator, BinaryOperator::Add);
                    assert!(matches!(**left, Expr::Literal(Literal::Integer(1))));
                    assert!(matches!(**right, Expr::BinaryOp { operator: BinaryOperator::Multiply, .. }));
                }
                other => panic!("expected BinaryOp(Add), got {other:?}"),
            }
        }
        other => panic!("expected Output, got {other:?}"),
    }
}

#[test]
fn parse_power_right_associative() {
    // 2 ** 3 ** 2 should parse as 2 ** (3 ** 2)
    let statement = parse_first_statement("вывод: 2 ** 3 ** 2");
    match statement {
        Statement::Output { values, .. } => {
            match &*values[0] {
                Expr::BinaryOp { operator, left, right } => {
                    assert_eq!(*operator, BinaryOperator::Power);
                    assert!(matches!(**left, Expr::Literal(Literal::Integer(2))));
                    match &**right {
                        Expr::BinaryOp { operator, .. } => assert_eq!(*operator, BinaryOperator::Power),
                        other => panic!("expected BinaryOp(Power), got {other:?}"),
                    }
                }
                other => panic!("expected BinaryOp(Power), got {other:?}"),
            }
        }
        other => panic!("expected Output, got {other:?}"),
    }
}

#[test]
fn parse_unary_negate() {
    let statement = parse_first_statement("вывод: -7");
    match statement {
        Statement::Output { values, .. } => {
            assert!(matches!(&*values[0], Expr::UnaryOp { operator: UnaryOperator::Negate, .. }));
        }
        other => panic!("expected Output, got {other:?}"),
    }
}

#[test]
fn parse_length_operator() {
    let statement = parse_first_statement("вывод: #\"hello\"");
    match statement {
        Statement::Output { values, .. } => {
            assert!(matches!(&*values[0], Expr::UnaryOp { operator: UnaryOperator::Length, .. }));
        }
        other => panic!("expected Output, got {other:?}"),
    }
}

#[test]
fn parse_logical_operators() {
    let statement = parse_first_statement("вывод: да и нет или да");
    match statement {
        Statement::Output { values, .. } => {
            // Should parse as (да и нет) или да
            assert!(matches!(&*values[0], Expr::BinaryOp { operator: BinaryOperator::Or, .. }));
        }
        other => panic!("expected Output, got {other:?}"),
    }
}

#[test]
fn parse_logical_not() {
    let statement = parse_first_statement("вывод: не да");
    match statement {
        Statement::Output { values, .. } => {
            assert!(matches!(&*values[0], Expr::UnaryOp { operator: UnaryOperator::Not, .. }));
        }
        other => panic!("expected Output, got {other:?}"),
    }
}

#[test]
fn parse_parenthesised_expression() {
    // (1 + 2) * 3 — parens override precedence
    let statement = parse_first_statement("вывод: (1 + 2) * 3");
    match statement {
        Statement::Output { values, .. } => {
            match &*values[0] {
                Expr::BinaryOp { operator, left, .. } => {
                    assert_eq!(*operator, BinaryOperator::Multiply);
                    assert!(matches!(**left, Expr::BinaryOp { operator: BinaryOperator::Add, .. }));
                }
                other => panic!("expected BinaryOp(Multiply), got {other:?}"),
            }
        }
        other => panic!("expected Output, got {other:?}"),
    }
}

// ── Assignment ──────────────────────────────────────────────────────────────

#[test]
fn parse_simple_assignment() {
    let statement = parse_first_statement("X := 42");
    match statement {
        Statement::Assignment { target, value } => {
            assert!(matches!(target, LValue::Name(ref name) if name == "X"));
            assert!(matches!(*value, Expr::Literal(Literal::Integer(42))));
        }
        other => panic!("expected Assignment, got {other:?}"),
    }
}

#[test]
fn parse_subscript_assignment() {
    let statement = parse_first_statement("X[1] := 5");
    match statement {
        Statement::Assignment { target, .. } => {
            assert!(matches!(target, LValue::Subscript { .. }));
        }
        other => panic!("expected Assignment, got {other:?}"),
    }
}

#[test]
fn parse_slice_assignment() {
    let statement = parse_first_statement("X[1:3] := \"abc\"");
    match statement {
        Statement::Assignment { target, .. } => {
            assert!(matches!(target, LValue::Slice { .. }));
        }
        other => panic!("expected Assignment, got {other:?}"),
    }
}

// ── Conditionals ────────────────────────────────────────────────────────────

#[test]
fn parse_if_then() {
    let statement = parse_first_statement("если да то вывод: 1 все");
    match statement {
        Statement::Conditional { else_body, .. } => {
            assert!(else_body.is_none());
        }
        other => panic!("expected Conditional, got {other:?}"),
    }
}

#[test]
fn parse_if_then_else() {
    let statement = parse_first_statement("если да то вывод: 1 иначе вывод: 2 все");
    match statement {
        Statement::Conditional { else_body, .. } => {
            assert!(else_body.is_some());
        }
        other => panic!("expected Conditional, got {other:?}"),
    }
}

// ── Selection (выбор) ───────────────────────────────────────────────────────

#[test]
fn parse_selection_value_match() {
    let statement = parse_first_statement(
        "выбор X при 1: вывод: \"один\" при 2: вывод: \"два\" иначе вывод: \"другое\" все"
    );
    match statement {
        Statement::Selection(SelectionStatement::ValueMatch { cases, else_body, .. }) => {
            assert_eq!(cases.len(), 2);
            assert!(else_body.is_some());
        }
        other => panic!("expected Selection(ValueMatch), got {other:?}"),
    }
}

#[test]
fn parse_selection_condition_list() {
    let statement = parse_first_statement(
        "выбор при да: вывод: 1 при нет: вывод: 2 все"
    );
    match statement {
        Statement::Selection(SelectionStatement::ConditionList { cases, .. }) => {
            assert_eq!(cases.len(), 2);
        }
        other => panic!("expected Selection(ConditionList), got {other:?}"),
    }
}

// ── Loops ───────────────────────────────────────────────────────────────────

#[test]
fn parse_infinite_loop() {
    let statement = parse_first_statement("цикл выход кц");
    match statement {
        Statement::Loop(LoopStatement { header: LoopHeader::Infinite, .. }) => {}
        other => panic!("expected Loop(Infinite), got {other:?}"),
    }
}

#[test]
fn parse_repeat_loop() {
    let statement = parse_first_statement("повтор 5 цикл вывод: 1 кц");
    match statement {
        Statement::Loop(LoopStatement { header: LoopHeader::Repeat(_), .. }) => {}
        other => panic!("expected Loop(Repeat), got {other:?}"),
    }
}

#[test]
fn parse_for_loop_full() {
    let statement = parse_first_statement("для I от 1 до 10 шаг 2 цикл вывод: I кц");
    match statement {
        Statement::Loop(LoopStatement { header: LoopHeader::For { variable, from, to, step }, .. }) => {
            assert_eq!(variable, "I");
            assert!(from.is_some());
            assert!(to.is_some());
            assert!(step.is_some());
        }
        other => panic!("expected Loop(For), got {other:?}"),
    }
}

#[test]
fn parse_for_loop_minimal() {
    let statement = parse_first_statement("для I до 5 цикл кц");
    match statement {
        Statement::Loop(LoopStatement { header: LoopHeader::For { variable, from, to, step }, .. }) => {
            assert_eq!(variable, "I");
            assert!(from.is_none());
            assert!(to.is_some());
            assert!(step.is_none());
        }
        other => panic!("expected Loop(For), got {other:?}"),
    }
}

#[test]
fn parse_while_condition() {
    let statement = parse_first_statement("пока да цикл кц");
    match statement {
        Statement::Loop(LoopStatement { while_condition, .. }) => {
            assert!(while_condition.is_some());
        }
        other => panic!("expected Loop with while_condition, got {other:?}"),
    }
}

#[test]
fn parse_post_condition() {
    let statement = parse_first_statement("цикл вывод: 1 кц по да");
    match statement {
        Statement::Loop(LoopStatement { post_condition, .. }) => {
            assert!(post_condition.is_some());
        }
        other => panic!("expected Loop with post_condition, got {other:?}"),
    }
}

// ── Procedure call ──────────────────────────────────────────────────────────

#[test]
fn parse_procedure_call_with_вызов() {
    let statement = parse_first_statement("вызов ПРИВЕТ()");
    match statement {
        Statement::ProcedureCall { arguments, .. } => {
            assert!(arguments.is_empty());
        }
        other => panic!("expected ProcedureCall, got {other:?}"),
    }
}

#[test]
fn parse_procedure_call_by_name() {
    let statement = parse_first_statement("ПРИВЕТ(1, 2)");
    match statement {
        Statement::ProcedureCall { arguments, .. } => {
            assert_eq!(arguments.len(), 2);
        }
        other => panic!("expected ProcedureCall, got {other:?}"),
    }
}

#[test]
fn parse_procedure_call_inout_arg() {
    let statement = parse_first_statement("ОБМЕН(<=X, <=Y)");
    match statement {
        Statement::ProcedureCall { arguments, .. } => {
            assert_eq!(arguments.len(), 2);
            assert!(matches!(&arguments[0], CallArgument::InOut(_)));
            assert!(matches!(&arguments[1], CallArgument::InOut(_)));
        }
        other => panic!("expected ProcedureCall, got {other:?}"),
    }
}

// ── Input ───────────────────────────────────────────────────────────────────

#[test]
fn parse_input_statement() {
    let statement = parse_first_statement("ввод: X");
    match statement {
        Statement::Input { text_mode, variables } => {
            assert!(!text_mode);
            assert_eq!(variables.len(), 1);
        }
        other => panic!("expected Input, got {other:?}"),
    }
}

#[test]
fn parse_input_text_mode() {
    let statement = parse_first_statement("ввод текста: X");
    match statement {
        Statement::Input { text_mode, .. } => assert!(text_mode),
        other => panic!("expected Input, got {other:?}"),
    }
}

// ── Definitions ─────────────────────────────────────────────────────────────

#[test]
fn parse_simple_procedure() {
    let proc_def = parse_first_procedure("проц ПРИВЕТ () вывод: \"hello\" конец");
    assert_eq!(proc_def.name, Some("ПРИВЕТ".to_string()));
    assert!(proc_def.parameters.is_empty());
    assert_eq!(proc_def.body.len(), 1);
}

#[test]
fn parse_procedure_with_params() {
    let proc_def = parse_first_procedure("проц ТЕСТ (A, =>B, <=C) конец");
    assert_eq!(proc_def.parameters.len(), 3);
    assert!(matches!(&proc_def.parameters[0], ProcParameter::Input(name) if name == "A"));
    assert!(matches!(&proc_def.parameters[1], ProcParameter::Input(name) if name == "B"));
    assert!(matches!(&proc_def.parameters[2], ProcParameter::InOut(name) if name == "C"));
}

#[test]
fn parse_procedure_with_name_declarations() {
    let proc_def = parse_first_procedure("проц ТЕСТ () свои: X, Y чужие: Z конец");
    assert_eq!(proc_def.name_declarations.own_names, vec!["X", "Y"]);
    assert_eq!(proc_def.name_declarations.foreign_names, vec!["Z"]);
}

#[test]
fn parse_simple_function() {
    let func_def = parse_first_function("функ ОДИН () возврат 1 конец");
    assert_eq!(func_def.name, Some("ОДИН".to_string()));
    assert!(func_def.parameters.is_empty());
    assert_eq!(func_def.body.len(), 1);
}

#[test]
fn parse_function_with_params() {
    let func_def = parse_first_function("функ КВАДРАТ (N) возврат N * N конец");
    assert_eq!(func_def.parameters, vec!["N"]);
}

// ── Return ──────────────────────────────────────────────────────────────────

#[test]
fn parse_return_from_procedure() {
    let proc_def = parse_first_procedure(
        "проц ТЕСТ (N) если N <= 0 то возврат все вывод: N конец"
    );
    // возврат in procedure body should be ReturnFromProcedure
    match &proc_def.body[0] {
        Statement::Conditional { then_body, .. } => {
            assert!(matches!(&then_body[0], Statement::ReturnFromProcedure));
        }
        other => panic!("expected Conditional, got {other:?}"),
    }
}

#[test]
fn parse_return_from_function() {
    let func_def = parse_first_function("функ ОДИН () возврат 1 конец");
    match &func_def.body[0] {
        Statement::ReturnFromFunction(expr) => {
            assert!(matches!(**expr, Expr::Literal(Literal::Integer(1))));
        }
        other => panic!("expected ReturnFromFunction, got {other:?}"),
    }
}

// ── Tuple ───────────────────────────────────────────────────────────────────

#[test]
fn parse_empty_tuple() {
    let statement = parse_first_statement("вывод: <* *>");
    match statement {
        Statement::Output { values, .. } => {
            assert!(matches!(&*values[0], Expr::TupleConstruct(elements) if elements.is_empty()));
        }
        other => panic!("expected Output, got {other:?}"),
    }
}

#[test]
fn parse_tuple_with_elements() {
    let statement = parse_first_statement("вывод: <* 1, 2, 3 *>");
    match statement {
        Statement::Output { values, .. } => {
            match &*values[0] {
                Expr::TupleConstruct(elements) => assert_eq!(elements.len(), 3),
                other => panic!("expected TupleConstruct, got {other:?}"),
            }
        }
        other => panic!("expected Output, got {other:?}"),
    }
}

// ── Subscript and slice in expressions ──────────────────────────────────────

#[test]
fn parse_subscript_expression() {
    let statement = parse_first_statement("вывод: X[1]");
    match statement {
        Statement::Output { values, .. } => {
            assert!(matches!(&*values[0], Expr::Subscript { .. }));
        }
        other => panic!("expected Output, got {other:?}"),
    }
}

#[test]
fn parse_slice_expression() {
    let statement = parse_first_statement("вывод: X[1:3]");
    match statement {
        Statement::Output { values, .. } => {
            assert!(matches!(&*values[0], Expr::Slice { .. }));
        }
        other => panic!("expected Output, got {other:?}"),
    }
}

#[test]
fn parse_function_call_expression() {
    let statement = parse_first_statement("вывод: КВАДРАТ(7)");
    match statement {
        Statement::Output { values, .. } => {
            assert!(matches!(&*values[0], Expr::FunctionCall { .. }));
        }
        other => panic!("expected Output, got {other:?}"),
    }
}

// ── ExitLoop ────────────────────────────────────────────────────────────────

#[test]
fn parse_exit_loop() {
    let statement = parse_first_statement("цикл выход кц");
    match statement {
        Statement::Loop(LoopStatement { body, .. }) => {
            assert!(matches!(&body[0], Statement::ExitLoop));
        }
        other => panic!("expected Loop, got {other:?}"),
    }
}

// ── Multi-statement programs ────────────────────────────────────────────────

#[test]
fn parse_multiple_statements() {
    let program = parse("X := 1\nY := 2\nвывод: X + Y");
    assert_eq!(program.units.len(), 3);
}

#[test]
fn parse_mixed_definitions_and_statements() {
    let program = parse("функ Ф () возврат 1 конец\nвывод: Ф()");
    assert_eq!(program.units.len(), 2);
    assert!(matches!(&program.units[0], ProgramUnit::FunctionDefinition(_)));
    assert!(matches!(&program.units[1], ProgramUnit::Statement(_)));
}

// ── Example .rap files ──────────────────────────────────────────────────────

#[test]
fn parse_example_01_output_and_literals() {
    let source = std::fs::read_to_string("tests/examples/01_output_and_literals.rap").unwrap();
    let _program = parse(&source);
}

#[test]
fn parse_example_02_arithmetic() {
    let source = std::fs::read_to_string("tests/examples/02_arithmetic.rap").unwrap();
    let _program = parse(&source);
}

#[test]
fn parse_example_05_conditionals() {
    let source = std::fs::read_to_string("tests/examples/05_conditionals.rap").unwrap();
    let _program = parse(&source);
}

#[test]
fn parse_example_06_loops() {
    let source = std::fs::read_to_string("tests/examples/06_loops.rap").unwrap();
    let _program = parse(&source);
}

#[test]
fn parse_example_07_procedures() {
    let source = std::fs::read_to_string("tests/examples/07_procedures.rap").unwrap();
    let _program = parse(&source);
}

#[test]
fn parse_example_08_functions() {
    let source = std::fs::read_to_string("tests/examples/08_functions.rap").unwrap();
    let _program = parse(&source);
}

#[test]
fn parse_example_03_text_operations() {
    let source = std::fs::read_to_string("tests/examples/03_text_operations.rap").unwrap();
    let _program = parse(&source);
}

#[test]
fn parse_example_04_tuple_operations() {
    let source = std::fs::read_to_string("tests/examples/04_tuple_operations.rap").unwrap();
    let _program = parse(&source);
}

#[test]
fn parse_example_09_scoping() {
    let source = std::fs::read_to_string("tests/examples/09_scoping.rap").unwrap();
    let _program = parse(&source);
}

#[test]
fn parse_example_10_return_parameters() {
    let source = std::fs::read_to_string("tests/examples/10_return_parameters.rap").unwrap();
    let _program = parse(&source);
}

#[test]
fn parse_example_11_type_checks() {
    let source = std::fs::read_to_string("tests/examples/11_type_checks.rap").unwrap();
    let _program = parse(&source);
}

#[test]
fn parse_example_12_spec_examples() {
    let source = std::fs::read_to_string("tests/examples/12_spec_examples.rap").unwrap();
    let _program = parse(&source);
}
