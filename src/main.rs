use std::env;
use std::path::PathBuf;

use clap::Parser;
use rapira26::codegen::{CodegenTargetName, run_codegen};
use rapira26::opt::deframe::DeframePass;
use rapira26::opt::opt_pass::{OptimizationPassOpts, run_optimizations};
use rapira26::pretty::pretty_parse_error;

#[derive(Parser)]
#[command(name = "рапик", about = "Компилятор языка рапира26")]
struct Cli {
    /// Исходный файл (.рап/.rap)
    файл: PathBuf,

    /// Вывести AST и выйти
    #[arg(long)]
    дамп_аст: bool,

    /// Вывести сгенерированный C код и выйти
    #[arg(long)]
    дамп_си: bool,

    /// Скомпилировать и запустить программу
    #[arg(long)]
    запуск: bool,

    /// Запуск в режиме ПИВИС/REPL
    #[arg(long)]
    пивис: bool,

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
}

fn main() {
    let cli = Cli::parse();

    if cli.пивис {
        // repl_mode();
        println!("До новых встреч!");
        return;
    }

    let source = std::fs::read_to_string(&cli.файл).unwrap_or_else(|error| {
        eprintln!("error reading {:?}: {error}", cli.файл);
        std::process::exit(1);
    });

    let token_stream = rapira26::lexer::Lexer::new(&source);
    let mut parser = rapira26::parser::Parser::new(
        token_stream,
        cli.файл
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("<неизвестный модуль>"),
    );

    let mut program = match parser.parse_program() {
        Ok(program) => program,
        Err(error) => {
            eprintln!(
                "{}",
                pretty_parse_error(&source, cli.файл.to_str().unwrap(), error)
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

    // Run codegen
    match cli.бэкенд {
        CodegenTargetName::C => {
            run_codegen(
                &mut rapira26::codegen::cgen::CGen::new().with_check_leaks(cli.вкл_проверку_утечек),
                &program,
                &cli.файл.canonicalize().unwrap(),
                &env::current_dir().unwrap(),
                cli.флаги.as_slice(),
                cli.запуск,
                cli.дамп_си,
            )
            .unwrap_or_else(|error| {
                eprintln!("Кодоген не справился: {error}");
                std::process::exit(1);
            });
        }
    }
}
