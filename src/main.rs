use std::env;
use std::path::PathBuf;
use std::process::Command;

use clap::Parser;

#[derive(Parser)]
#[command(name = "rapira26", about = "Rapira language compiler")]
struct Cli {
    /// Source file (.rap)
    source: PathBuf,

    /// Dump the AST and exit
    #[arg(long)]
    dump_ast: bool,

    /// Emit generated C code to stdout and exit
    #[arg(long)]
    emit_c: bool,

    /// Compile and run the program
    #[arg(long)]
    run: bool,

    /// Run in REPL mode
    #[arg(long)]
    repl: bool,

    /// Enable leak checking (compile with RAP_TEST_LEAKS)
    #[arg(long)]
    check_leaks: bool,
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

    if cli.repl {
        // repl_mode();
        println!("До новых встреч!");
        return;
    }

    let source = std::fs::read_to_string(&cli.source).unwrap_or_else(|error| {
        eprintln!("error reading {:?}: {error}", cli.source);
        std::process::exit(1);
    });

    let token_stream = rapira26::lexer::Lexer::new(&source);
    let parser = rapira26::parser::Parser::new(token_stream);

    let program = match parser.parse_program() {
        Ok(program) => program,
        Err(error) => {
            eprintln!("parse error: {error}");
            std::process::exit(1);
        }
    };

    if cli.dump_ast {
        println!("{program:#?}");
        return;
    }

    let codegen = rapira26::codegen::Codegen::new().with_check_leaks(cli.check_leaks);
    let c_code = codegen.generate(&program);

    if cli.emit_c {
        print!("{c_code}");
        return;
    }

    let file_name = cli
        .source
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("a");
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
        .arg(format!("-I{}", runtime_dir.display()))
        .arg(format!("-L{}", runtime_dir.join("lib").display()))
        .arg("-lrapruntime")
        .arg("-lm")
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

    if cli.run {
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
