// Force-link raperr so its `#[no_mangle] runtime_error_description` (a C symbol
// the runtime archive references via RAP_fatal_error) is present at link time.
extern crate raperr;

use std::env;
use std::fs::File;
use std::io::Read;
use std::path::PathBuf;

use clap::Parser;
use compiler_core::codegen::bcgen::BcGen;
use compiler_core::codegen::{CodegenTargetName, run_codegen};
use compiler_core::module::{build_dependency_graph, dump_dependency_graph};
use compiler_core::opt::deframe::DeframePass;
use compiler_core::opt::opt_pass::{OptimizationPassOpts, run_optimizations};
use compiler_core::pretty::pretty_parse_error;
use vm_core::bytefile::Bytefile;
use vm_core::decoder::Decoder;
use vm_interp::interpreter::Interpreter;

const MAX_FILE_SIZE: u64 = 1024 * 1024 * 1024; // 1GB

#[derive(Parser)]
#[command(name = "рапик", about = "Компилятор языка рапира26")]
struct Cli {
    /// Исходные файлы (.рап/.rap)
    файлы: Vec<PathBuf>,

    /// Вывести AST и выйти
    #[arg(long)]
    дамп_аст: bool,

    /// Вывести сгенерированный код и выйти
    #[arg(long)]
    дамп_код: bool,

    /// Скомпилировать и запустить программу
    #[arg(long)]
    запуск: bool,

    /// Вывести отладочную информацию о проходах оптимизации
    #[arg(long)]
    дамп_опт_дебаг: bool,

    /// Выбор бэкенда для генерации кода
    #[arg(long, value_enum, default_value_t = CodegenTargetName::RBC)]
    бэкенд: CodegenTargetName,

    /// Дамп графа зависимостей модулей
    #[arg(long)]
    дамп_граф_модулей: bool,
}

// TODO: refactor
fn run_interpreter(path: &str, dump_bytefile: bool) -> Result<(), Box<dyn std::error::Error>> {
    // Check file size
    let metadata = std::fs::metadata(path).map_err(|err| {
        eprintln!("{}", err);
        err
    })?;
    if metadata.len() >= MAX_FILE_SIZE {
        panic!("File is too large: {} > {}", metadata.len(), MAX_FILE_SIZE);
    }

    let mut file: File = File::open(path).map_err(|err| {
        eprintln!("{}", err);
        err
    })?;
    let mut content = Vec::new();
    file.read_to_end(&mut content).map_err(|err| {
        eprintln!("{}", err);
        err
    })?;

    let bytefile = Bytefile::parse(content.clone()).map_err(|err| {
        eprintln!("{}", err);
        err
    })?;

    if dump_bytefile {
        println!("{}", bytefile);
        return Ok(());
    }

    let decoder = Decoder::new(bytefile);

    // TODO: patch verifier mod declaration
    // let mut verifier = Veri::new(decoder);
    // verifier.verify().map_err(|err| {
    //     eprintln!("{}", err);
    //     err
    // })?;

    // // Move decoder from verifier to interpreter
    // let Verifier { mut decoder, .. } = verifier;

    // Reset IP to main offset
    // decoder.ip = decoder.bf.main_offset as usize;

    let mut interp = Interpreter::new(decoder);
    let _ = interp.run().map_err(|err| {
        eprintln!("{}", err);
        err
    });

    Ok(())
}

fn main() {
    let cli = Cli::parse();
    env_logger::init();

    let mut modules = Vec::new();

    // 1. Parse all input files into modules and apply optimizations
    //    that can be performed in AST for each

    for file in &cli.файлы {
        let source = std::fs::read_to_string(file).unwrap_or_else(|error| {
            eprintln!("error reading {:?}: {error}", file);
            std::process::exit(1);
        });

        let token_stream = compiler_core::lexer::Lexer::new(&source);
        let mut parser = compiler_core::parser::Parser::new(
            token_stream,
            file.canonicalize().unwrap_or_default(),
        );

        let mut program = match parser.parse_program() {
            Ok(program) => program,
            Err(error) => {
                eprintln!(
                    "{}",
                    pretty_parse_error(&source, file.to_str().unwrap(), error)
                );
                std::process::exit(1);
            }
        };

        if cli.дамп_аст {
            println!("{program:#?}");
            return;
        }

        // Apply optimizations
        run_optimizations(
            &mut program,
            &[&DeframePass],
            &OptimizationPassOpts {
                dump: cli.дамп_опт_дебаг,
            },
        )
        .unwrap_or_else(|error| {
            eprintln!("Оптимизация не справилась: {error}");
            std::process::exit(1);
        });

        modules.push(program);
    }

    // 2. Build dependency graph, topologically sort modules

    let (graph, modules) = build_dependency_graph(modules).unwrap_or_else(|e| {
        eprintln!("Ошибка в зависимостях: {e}");
        std::process::exit(1);
    });

    if cli.дамп_граф_модулей {
        dump_dependency_graph(&graph);
        std::process::exit(0);
    }

    // 3. Run codegen

    match cli.бэкенд {
        CodegenTargetName::C => {
            panic!("C backend is deprecated");
        }
        CodegenTargetName::RBC => {
            let generated_paths = run_codegen(
                &mut BcGen::new(),
                modules,
                &env::current_dir().unwrap(),
                cli.дамп_код,
            )
            .unwrap_or_else(|error| {
                eprintln!("Кодоген не справился: {error}");
                std::process::exit(1);
            });

            log::info!("Generated paths: {generated_paths:?}");

            // TODO: support >1 module
            let module = &generated_paths[0];

            run_interpreter(&module.0.to_string_lossy(), cli.дамп_код).unwrap_or_else(|error| {
                eprintln!("Интерпретатор не справился: {error}");
                std::process::exit(1);
            });

            if !cli.дамп_код {
                std::fs::remove_file(&module.0).unwrap_or_else(|e| {
                    eprintln!("Failed to remove generated file: {e}");
                });
            }
        }
    }
}
