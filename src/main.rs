use std::env;
use std::path::PathBuf;
use std::process::Command;

use clap::Parser;
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

    /// Флаги для передачи компилятору Cи
    #[arg(long)]
    си_флаги: Vec<String>,

    /// Вывести отладочную информацию о проходах оптимизации
    #[arg(long)]
    дамп_опт_дебаг: bool,
}

// TODO:
/// Repl mode works like this:
/// - Reads a line of input from the user
/// - Parses and compiles the input
/// - Executes the compiled code
/// - The next line user has entered is added to the program and compiled again
///     Therefore we need to save all input as a single program
/// - Loops until the user exits
// fn repl_mode() {
//     let mut source = String::new();
//     loop {
//         let mut input = String::new();
//         std::io::stdin().read_line(&mut input).unwrap();
//         // source.push_str(&input);

//         let token_stream = rapira26::lexer::Lexer::new(&input);
//         let parser = rapira26::parser::Parser::new(token_stream);

//         let
//     }
// }

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
    let parser = rapira26::parser::Parser::new(token_stream);

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
    let codegen = rapira26::codegen::Codegen::new().with_check_leaks(cli.вкл_проверку_утечек);
    let c_code = codegen.generate(&program, cli.файл.canonicalize().unwrap().to_str().unwrap());

    if cli.дамп_си {
        print!("{c_code}");
        return;
    }

    let file_name = cli.файл.file_name().and_then(|n| n.to_str()).unwrap_or("a");
    let c_path = env::current_dir()
        .unwrap()
        .join(PathBuf::from(file_name).with_extension("c"));

    let binary_path = env::current_dir()
        .unwrap()
        .join(PathBuf::from(file_name).with_extension(""));

    std::fs::write(&c_path, &c_code).unwrap_or_else(|error| {
        eprintln!("error writing {:?}: {error}", c_path);
        std::process::exit(1);
    });

    let runtime_dir = find_runtime_dir();

    let status = Command::new("gcc")
        .arg(&c_path)
        .arg("-o")
        .arg(&binary_path)
        .arg(format!("-I{}", runtime_dir.display())) // runtime C library
        .arg(format!(
            "-I{}",
            runtime_dir.join("raperr/include").display()
        )) // runtime error C library
        .arg(format!("-L{}", runtime_dir.join("lib").display()))
        .arg(format!(
            "-L{}",
            runtime_dir.join("raperr/target/release").display()
        ))
        .arg("-lrapruntime")
        .arg("-lraperr")
        .arg("-lm")
        .args(cli.си_флаги)
        .status()
        .unwrap_or_else(|error| {
            eprintln!("failed to run gcc: {error}");
            std::process::exit(1);
        });

    if !status.success() {
        eprintln!("gcc failed with {status}");
        std::process::exit(1);
    }

    // Clean up generated C file
    if let Err(error) = std::fs::remove_file(&c_path) {
        eprintln!("failed to remove {:?}: {error}", c_path);
    }

    if cli.запуск {
        let status = Command::new(&binary_path).status().unwrap_or_else(|error| {
            eprintln!("failed to run {:?}: {error}", binary_path);
            std::process::exit(1);
        });

        // Clean up generated binary
        if let Err(error) = std::fs::remove_file(&binary_path) {
            eprintln!("failed to remove {:?}: {error}", binary_path);
        }

        std::process::exit(status.code().unwrap_or(1));
    }
}

/// Find the runtime/ directory containing librapruntime.a and headers.
/// Checks:
/// 1. $RAPIRA_RUNTIME env var
/// 2. Next to the compiler binary
/// 3. ./runtime/ in cwd (development)
fn find_runtime_dir() -> PathBuf {
    if let Ok(dir) = std::env::var("RAPIRA_RUNTIME") {
        let candidate = PathBuf::from(dir);
        if candidate.join("lib/librapruntime.a").exists() {
            return candidate;
        }
    }

    if let Ok(exe) = std::env::current_exe() {
        let candidate = exe.parent().unwrap().join("runtime");
        if candidate.join("lib/librapruntime.a").exists() {
            return candidate;
        }
    }

    let candidate = PathBuf::from("runtime");
    if candidate.join("lib/librapruntime.a").exists() {
        return candidate;
    }

    eprintln!(
        "error: cannot find librapruntime.a (tried $RAPIRA_RUNTIME, next to binary, and ./runtime/)\n\
         hint: run 'make' in the runtime/ directory first"
    );
    std::process::exit(1);
}
