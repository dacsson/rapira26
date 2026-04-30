use std::env;
use std::path::PathBuf;

use clap::Parser;
use rapira26::codegen::cgen::CGen;
use rapira26::codegen::{CodegenTargetName, run_codegen};
use rapira26::module::{build_dependency_graph, dump_dependency_graph};
use rapira26::opt::deframe::DeframePass;
use rapira26::opt::opt_pass::{OptimizationPassOpts, run_optimizations};
use rapira26::pretty::pretty_parse_error;

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

    /// Включить проверку утечек (компилируй с RAP_TEST_LEAKS)
    #[arg(long)]
    вкл_проверку_утечек: bool,

    /// Флаги для передачи компилятору бэкенда
    #[arg(long)]
    флаги: Vec<String>,

    /// Вывести отладочную информацию о проходах оптимизации
    #[arg(long)]
    дамп_опт_дебаг: bool,

    /// Выбор бэкенда для генерации кода
    #[arg(long, value_enum, default_value_t = CodegenTargetName::C)]
    бэкенд: CodegenTargetName,

    /// Дамп графа зависимостей модулей
    #[arg(long)]
    дамп_граф_модулей: bool,
}

fn main() {
    let cli = Cli::parse();

    let mut modules = Vec::new();

    // 1. Parse all input files into modules and apply optimizations
    //    that can be performed in AST for each

    for file in &cli.файлы {
        let source = std::fs::read_to_string(file).unwrap_or_else(|error| {
            eprintln!("error reading {:?}: {error}", file);
            std::process::exit(1);
        });

        let filename = file
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_else(|| {
                eprintln!("Упс, ошибка: неизвестный моудль");
                std::process::exit(1);
            });

        let token_stream = rapira26::lexer::Lexer::new(&source);
        let mut parser = rapira26::parser::Parser::new(
            token_stream,
            filename,
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
            run_codegen(
                &mut CGen::new().with_check_leaks(cli.вкл_проверку_утечек),
                modules,
                &env::current_dir().unwrap(),
                cli.флаги.as_slice(),
                cli.запуск,
                cli.дамп_код,
            )
            .unwrap_or_else(|error| {
                eprintln!("Кодоген не справился: {error}");
                std::process::exit(1);
            });
        }
    }
}
