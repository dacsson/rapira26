//! Tests for interpreter/bytecode evaluation.

use crate::RAP_TAG_MASK;
use crate::interpreter::Interpreter;
use vm_core::bytecode::*;
use vm_core::bytefile::Bytefile;
use vm_core::decoder::Decoder;

// The runtime exposes process-global GC stack pointers, so two Interpreter
// instances cannot safely execute concurrently in the same test process.
static INTERPRETER_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn prepare_bytefile(program: &[Instruction], globals: u32) -> Decoder {
    let mut bytefile = Bytefile::new();
    bytefile.main_offset = 0;
    bytefile.global_area_size = globals;
    bytefile.add_string("main".to_string());
    bytefile.add_public_symbol("main", 0).unwrap();

    bytefile
        .add_instruction(&Instruction::BEGIN { args: 0, locals: 0 })
        .unwrap();
    for instr in program {
        bytefile.add_instruction(instr).unwrap();
    }
    bytefile.add_instruction(&Instruction::END).unwrap();

    Decoder::new(bytefile)
}

fn evaluate(program: &[Instruction]) -> Result<usize, Box<dyn std::error::Error>> {
    evaluate_with_globals(program, 0)
}

fn evaluate_with_globals(
    program: &[Instruction],
    globals: u32,
) -> Result<usize, Box<dyn std::error::Error>> {
    let _guard = INTERPRETER_LOCK.lock().unwrap();
    let decoder = prepare_bytefile(program, globals);
    let mut interp = Interpreter::new(decoder);
    interp.run_with_result().map_err(Into::into)
}

fn prepare_bytefile_with_locals(program: &[Instruction], globals: u32, locals: i32) -> Decoder {
    let mut bytefile = Bytefile::new();
    bytefile.main_offset = 0;
    bytefile.global_area_size = globals;
    bytefile.add_string("main".to_string());
    bytefile.add_public_symbol("main", 0).unwrap();

    bytefile
        .add_instruction(&Instruction::BEGIN { args: 0, locals })
        .unwrap();
    for instr in program {
        bytefile.add_instruction(instr).unwrap();
    }
    bytefile.add_instruction(&Instruction::END).unwrap();

    Decoder::new(bytefile)
}

fn evaluate_with_locals(
    program: &[Instruction],
    locals: u32,
) -> Result<usize, Box<dyn std::error::Error>> {
    let _guard = INTERPRETER_LOCK.lock().unwrap();
    let decoder = prepare_bytefile_with_locals(program, 0, locals as i32);
    let mut interp = Interpreter::new(decoder);
    interp.run_with_result().map_err(Into::into)
}

fn smi(raw: usize) -> i64 {
    (raw as i64) >> 32
}

fn boolean(raw: usize) -> bool {
    ((raw >> 2) & 1) != 0
}

fn binary(left: i32, op: Op, right: i32) -> Vec<Instruction> {
    vec![
        Instruction::CONST { value: left },
        Instruction::CONST { value: right },
        Instruction::BINOP { op },
    ]
}

#[test]
fn eval_integer_arithmetic() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (2, Op::ADD, 3, 5),
        (2, Op::SUB, 3, -1),
        (3, Op::MUL, 4, 12),
        (6, Op::DIV, 2, 3),
        (7, Op::IDIV, 2, 3),
        (-7, Op::IDIV, 2, -4),
        (7, Op::MOD, 2, 1),
        (2, Op::POW, 10, 1024),
    ];

    for (left, op, right, expected) in cases {
        assert_eq!(smi(evaluate(&binary(left, op, right))?), expected);
    }

    Ok(())
}

#[test]
fn eval_comparisons() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (2, Op::LT, 3, true),
        (3, Op::LEQ, 3, true),
        (3, Op::GT, 2, true),
        (2, Op::GEQ, 3, false),
        (3, Op::EQ, 3, true),
        (3, Op::NEQ, 3, false),
    ];

    for (left, op, right, expected) in cases {
        assert_eq!(boolean(evaluate(&binary(left, op, right))?), expected);
    }

    Ok(())
}

#[test]
fn eval_boolean_and_unary_operations() -> Result<(), Box<dyn std::error::Error>> {
    let cases = [
        (true, Op::AND, false, false),
        (true, Op::OR, false, true),
        (false, Op::OR, false, false),
    ];

    for (left, op, right, expected) in cases {
        let program = [
            Instruction::BOOL { value: left },
            Instruction::BOOL { value: right },
            Instruction::BINOP { op },
        ];
        assert_eq!(boolean(evaluate(&program)?), expected);
    }

    let negated = evaluate(&[
        Instruction::CONST { value: 7 },
        Instruction::UNARY {
            op: UnaryOp::Negate,
        },
    ])?;
    assert_eq!(smi(negated), -7);

    for (value, expected) in [(true, false), (false, true)] {
        let result = evaluate(&[
            Instruction::BOOL { value },
            Instruction::UNARY { op: UnaryOp::Not },
        ])?;
        assert_eq!(boolean(result), expected);
    }

    Ok(())
}

#[test]
fn eval_float_and_null_equality() -> Result<(), Box<dyn std::error::Error>> {
    let floats_equal = evaluate(&[
        Instruction::CONSTF { value: 3.5 },
        Instruction::CONSTF { value: 3.5 },
        Instruction::BINOP { op: Op::EQ },
    ])?;
    assert!(boolean(floats_equal));

    let nulls_equal = evaluate(&[
        Instruction::NULL,
        Instruction::NULL,
        Instruction::BINOP { op: Op::EQ },
    ])?;
    assert!(boolean(nulls_equal));

    Ok(())
}

#[test]
fn eval_stack_operations() -> Result<(), Box<dyn std::error::Error>> {
    let duplicated = evaluate(&[
        Instruction::CONST { value: 4 },
        Instruction::DUP,
        Instruction::BINOP { op: Op::ADD },
    ])?;
    assert_eq!(smi(duplicated), 8);

    let swapped_then_dropped = evaluate(&[
        Instruction::CONST { value: 1 },
        Instruction::CONST { value: 2 },
        Instruction::SWAP,
        Instruction::DROP,
    ])?;
    assert_eq!(smi(swapped_then_dropped), 2);

    Ok(())
}

#[test]
fn eval_global_store_and_load() -> Result<(), Box<dyn std::error::Error>> {
    let result = evaluate_with_globals(
        &[
            Instruction::CONST { value: 42 },
            Instruction::STORE {
                rel: ValueRel::Global,
                index: 0,
            },
            Instruction::LOAD {
                rel: ValueRel::Global,
                index: 0,
            },
        ],
        1,
    )?;

    assert_eq!(smi(result), 42);
    Ok(())
}

#[test]
fn eval_tuple_length_and_element_lookup() -> Result<(), Box<dyn std::error::Error>> {
    let length = evaluate(&[
        Instruction::CONST { value: 2 },
        Instruction::CONST { value: 3 },
        Instruction::CONST { value: 5 },
        Instruction::TUPLE { n: 3 },
        Instruction::CALLBUILTIN {
            name: Builtin::Llength,
            n: 0,
        },
    ])?;
    assert_eq!(smi(length), 3);

    let element = evaluate(&[
        Instruction::CONST { value: 2 },
        Instruction::CONST { value: 3 },
        Instruction::CONST { value: 5 },
        Instruction::TUPLE { n: 3 },
        Instruction::CONST { value: 1 },
        Instruction::ELEM,
    ])?;
    assert_eq!(smi(element), 3);

    Ok(())
}

#[test]
fn eval_tuple_assignment_keeps_returned_aggregate_alive() -> Result<(), Box<dyn std::error::Error>>
{
    let element = evaluate(&[
        Instruction::CONST { value: 9 },
        Instruction::CONST { value: 0 },
        Instruction::CONST { value: 1 },
        Instruction::CONST { value: 2 },
        Instruction::TUPLE { n: 2 },
        Instruction::STA,
        Instruction::CONST { value: 0 },
        Instruction::ELEM,
    ])?;
    assert_eq!(smi(element), 9);

    Ok(())
}

#[test]
fn eval_integer_math_builtins() -> Result<(), Box<dyn std::error::Error>> {
    let result = evaluate(&[
        Instruction::CONST { value: -7 },
        Instruction::CALLBUILTIN {
            name: Builtin::Abs,
            n: 0,
        },
    ])?;
    assert_eq!(smi(result), 7);

    Ok(())
}

#[test]
fn eval_complex_logical_expr() -> Result<(), Box<dyn std::error::Error>> {
    let and = evaluate(&[
        Instruction::CONST { value: 1 },
        Instruction::CONST { value: 0 },
        Instruction::BINOP { op: Op::GT },
        Instruction::CONST { value: 2 },
        Instruction::CONST { value: 3 },
        Instruction::BINOP { op: Op::LEQ },
        Instruction::BINOP { op: Op::AND },
    ])?;

    assert!(((and as u32) & RAP_TAG_MASK) == 0x1);
    assert!(boolean(and));

    let chained = evaluate(&[
        Instruction::CONST { value: 1 },
        Instruction::CONST { value: 0 },
        Instruction::BINOP { op: Op::GT },
        Instruction::CONST { value: 2 },
        Instruction::CONST { value: 3 },
        Instruction::BINOP { op: Op::LEQ },
        Instruction::BINOP { op: Op::AND },
        Instruction::CONST { value: 1 },
        Instruction::CONST { value: 0 },
        Instruction::BINOP { op: Op::LEQ },
        Instruction::CONST { value: 2 },
        Instruction::CONST { value: 3 },
        Instruction::BINOP { op: Op::GEQ },
        Instruction::BINOP { op: Op::AND },
        Instruction::BINOP { op: Op::OR },
    ])?;

    assert!(((chained as u32) & RAP_TAG_MASK) == 0x1);
    assert!(boolean(chained));

    Ok(())
}

#[test]
fn eval_super_binop_local_local() -> Result<(), Box<dyn std::error::Error>> {
    // BINOP_LOCAL_LOCAL computes `local[lhs] op local[rhs]` and pushes the result.
    let result = evaluate_with_locals(
        &[
            // local[0] = 4, local[1] = 5
            Instruction::CONST { value: 4 },
            Instruction::STORE {
                rel: ValueRel::Local,
                index: 0,
            },
            Instruction::CONST { value: 5 },
            Instruction::STORE {
                rel: ValueRel::Local,
                index: 1,
            },
            Instruction::BINOP_LOCAL_LOCAL {
                rhs_index: 1,
                lhs_index: 0,
                op: Op::MUL,
            },
        ],
        2,
    )?;
    assert_eq!(smi(result), 20);

    // The instruction order is `lhs op rhs`, mirroring the plain BINOP.
    let subtracted = evaluate_with_locals(
        &[
            // local[0] = 10, local[1] = 3
            Instruction::CONST { value: 10 },
            Instruction::STORE {
                rel: ValueRel::Local,
                index: 0,
            },
            Instruction::CONST { value: 3 },
            Instruction::STORE {
                rel: ValueRel::Local,
                index: 1,
            },
            Instruction::BINOP_LOCAL_LOCAL {
                rhs_index: 1,
                lhs_index: 0,
                op: Op::SUB,
            },
        ],
        2,
    )?;
    assert_eq!(smi(subtracted), 7);

    Ok(())
}

#[test]
fn eval_super_binop_local_const() -> Result<(), Box<dyn std::error::Error>> {
    // BINOP_LOCAL_CONST computes `local[index] op value` (mirroring the source
    // pattern LOAD local; CONST value; BINOP) and pushes the result.
    const VALUE: i32 = 10;
    for (op, expected) in [
        (Op::ADD, (4 + VALUE) as i64),
        (Op::SUB, (4 - VALUE) as i64),
        (Op::MUL, (4 * VALUE) as i64),
        (Op::IDIV, (4 / VALUE) as i64),
    ] {
        let op_name = format!("{op:#?}");
        let result = evaluate_with_locals(
            &[
                // local[0] = 4
                Instruction::CONST { value: 4 },
                Instruction::STORE {
                    rel: ValueRel::Local,
                    index: 0,
                },
                Instruction::BINOP_LOCAL_CONST {
                    index: 0,
                    op,
                    value: VALUE,
                },
            ],
            1,
        )?;
        assert_eq!(smi(result), expected, "op={op_name}");
    }

    Ok(())
}

#[test]
fn eval_super_binop_local_local_store() -> Result<(), Box<dyn std::error::Error>> {
    // BINOP_LOCAL_LOCAL_STORE fuses `LOAD lhs; LOAD rhs; BINOP; STORE dst`: it
    // stores `local[lhs] op local[rhs]` into local[dst] and leaves the stack
    // unchanged (matching the original four instruction's net effect). Load the
    // destination slot back to prove the store persisted.
    let result = evaluate_with_locals(
        &[
            // local[0] = 6, local[1] = 7
            Instruction::CONST { value: 6 },
            Instruction::STORE {
                rel: ValueRel::Local,
                index: 0,
            },
            Instruction::CONST { value: 7 },
            Instruction::STORE {
                rel: ValueRel::Local,
                index: 1,
            },
            Instruction::BINOP_LOCAL_LOCAL_STORE {
                rhs_index: 1,
                lhs_index: 0,
                dst_index: 2,
                op: Op::MUL,
            },
            Instruction::LOAD {
                rel: ValueRel::Local,
                index: 2,
            },
        ],
        3,
    )?;
    assert_eq!(smi(result), 42);

    Ok(())
}

#[test]
fn eval_super_binop_local_const_store() -> Result<(), Box<dyn std::error::Error>> {
    // BINOP_LOCAL_CONST_STORE fuses `LOAD idx; CONST value; BINOP; STORE dst`:
    // it stores `local[index] op value` into local[dst] and leaves the stack
    // unchanged. Load the destination slot back to prove the store persisted.
    let result = evaluate_with_locals(
        &[
            // local[0] = 5
            Instruction::CONST { value: 5 },
            Instruction::STORE {
                rel: ValueRel::Local,
                index: 0,
            },
            Instruction::BINOP_LOCAL_CONST_STORE {
                index: 0,
                dst_index: 1,
                op: Op::POW,
                value: 2,
            },
            Instruction::LOAD {
                rel: ValueRel::Local,
                index: 1,
            },
        ],
        2,
    )?;
    // 5 ^ 2 = 25
    assert_eq!(smi(result), 25_i64);

    Ok(())
}
