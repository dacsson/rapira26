//! Tests for the interpreter/bytecode evaluation
//!
//! Dont forget to change MAX_OPERAND_STACK_SIZE if
//! you want to run this tests, otherwise the interpreter
//! will panic due to stack overflow.
//! TODO: change that behaviour

use vm_core::bytecode::*;
use vm_core::bytefile::Bytefile;
use vm_core::decoder::Decoder;
use vm_interp::interpreter::Interpreter;

fn prepare_bytefile(program: &[Instruction]) -> Decoder {
    let mut bytefile = Bytefile::new();
    bytefile.main_offset = 0;
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

/// Test minimal evaluation functionality of the interpreter
#[test]
fn eval_int_binops() -> Result<(), Box<dyn std::error::Error>> {
    let mut programs = Vec::new();
    for i in 1u8..=13u8 {
        let program = vec![
            Instruction::CONST { value: 2 },
            Instruction::CONST { value: 3 },
            Instruction::BINOP {
                op: Op::try_from(i).unwrap(),
            },
        ];
        programs.push(program);
    }

    // tests on 0
    programs.push(vec![
        Instruction::CONST { value: 0 },
        Instruction::CONST { value: 0 },
        Instruction::BINOP { op: Op::AND },
    ]);

    programs.push(vec![
        Instruction::CONST { value: 0 },
        Instruction::CONST { value: 1 },
        Instruction::BINOP { op: Op::OR },
    ]);

    programs.push(vec![
        Instruction::CONST { value: 0 },
        Instruction::CONST { value: 0 },
        Instruction::BINOP { op: Op::OR },
    ]);

    // equality
    programs.push(vec![
        Instruction::CONST { value: 1 },
        Instruction::CONST { value: 1 },
        Instruction::BINOP { op: Op::EQ },
    ]);

    programs.push(vec![
        Instruction::CONST { value: 1 },
        Instruction::CONST { value: 1 },
        Instruction::BINOP { op: Op::NEQ },
    ]);

    let expected_results = vec![
        5,  // 2 + 3 = 5
        -1, // 2 - 3 = 5
        6,  // 2 * 3 = 6
        0,  // 2 / 3 = 0
        2,  // 2 % 3 = 2
        1,  // 2 < 3 = 1
        1,  // 2 <= 3 = 1
        0,  // 2 > 3 = 0
        0,  // 2 >= 3 = 0
        0,  // 2 == 3 = 0
        1,  // 2 != 3 = 1
        1,  // 2 && 3 = 1
        1,  // 2 != 3 = 1
        0,  // 0 && 0 = 0
        1,  // 0 || 1 = 1
        0,  // 0 || 0 = 0
        1,  // 1 == 1 = 1
        0,  // 1 != 1 = 0
    ];

    assert_eq!(programs.len(), expected_results.len());

    for (program, expected) in programs.into_iter().zip(expected_results) {
        let decoder = prepare_bytefile(&program);
        let mut interp = Interpreter::new(decoder);
        interp.run()?;

        let top = interp.peek().unwrap();

        assert_eq!(top.unbox(), expected);
    }

    Ok(())
}
