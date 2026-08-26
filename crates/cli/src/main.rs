// Force-link raperr so its `#[no_mangle] runtime_error_description` (a C symbol
// the runtime archive references via RAP_fatal_error) is present at link time.
extern crate raperr;

use std::env;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::{collections::HashSet, collections::VecDeque};

use clap::Parser;
use compiler_core::codegen::CodegenTargetName;
use compiler_core::codegen::bcgen::BcGen;
use compiler_core::codegen::run_codegen;
use compiler_core::module::{build_dependency_graph, dump_dependency_graph};
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

    /// Дополнительный каталог поиска модулей (можно указать несколько раз)
    #[arg(
        long = "путь-модулей",
        visible_alias = "module-path",
        value_name = "КАТАЛОГ"
    )]
    пути_модулей: Vec<PathBuf>,

    /// Каталог стандартной библиотеки (иначе RAPIRA_STD или каталог установки)
    #[arg(long = "путь-стд", visible_alias = "stdlib", value_name = "КАТАЛОГ")]
    путь_стд: Option<PathBuf>,
}

fn run_interpreter_bytes(
    content: Vec<u8>,
    dump_bytefile: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let bytefile = Bytefile::parse(content).map_err(|err| {
        eprintln!("{}", err);
        err
    })?;

    if dump_bytefile {
        println!("{}", bytefile);
        return Ok(());
    }

    let decoder = Decoder::new(bytefile);
    let mut interp = Interpreter::new(decoder);
    let _ = interp.run().map_err(|err| {
        eprintln!("{}", err);
        err
    });

    Ok(())
}

#[derive(Debug)]
struct ModuleResolver {
    module_paths: Vec<PathBuf>,
    std_path: Option<PathBuf>,
}

impl ModuleResolver {
    fn from_cli(cli: &Cli) -> Self {
        let mut module_paths = cli.пути_модулей.clone();
        if let Some(paths) = env::var_os("RAPIRA_PATH") {
            module_paths.extend(env::split_paths(&paths));
        }

        let std_path = cli
            .путь_стд
            .clone()
            .or_else(|| env::var_os("RAPIRA_STD").map(PathBuf::from))
            .or_else(installed_std_path);

        Self {
            module_paths,
            std_path,
        }
    }

    fn resolve(&self, importer: &Path, module_name: &str) -> Option<PathBuf> {
        let local_path = importer.parent().unwrap_or_else(|| Path::new("."));
        let roots = std::iter::once(local_path)
            .chain(self.module_paths.iter().map(PathBuf::as_path))
            .chain(self.std_path.iter().map(PathBuf::as_path));

        roots
            .flat_map(|root| module_candidates(root, module_name))
            .find(|candidate| candidate.is_file())
    }
}

fn module_candidates(root: &Path, module_name: &str) -> Vec<PathBuf> {
    let requested = root.join(module_name);
    let candidates = if requested.extension().is_some() {
        vec![requested]
    } else {
        vec![
            requested.with_extension("rap"),
            requested.with_extension("рап"),
        ]
    };

    candidates
}

fn installed_std_path() -> Option<PathBuf> {
    let executable = env::current_exe().ok()?;
    let binary_dir = executable.parent()?;
    let prefix = binary_dir.parent()?;

    [
        binary_dir.join("std"),
        prefix.join("lib/rapira26/std"),
        prefix.join("share/rapira26/std"),
    ]
    .into_iter()
    .find(|path| path.is_dir())
}

fn main() {
    let cli = Cli::parse();
    env_logger::init();

    let module_resolver = ModuleResolver::from_cli(&cli);

    let mut modules = Vec::new();
    let mut pending_files = VecDeque::from(cli.файлы.clone());
    let mut parsed_files = HashSet::new();

    // 1. Parse all input files into modules and apply optimizations
    //    that can be performed in AST for each

    while let Some(file) = pending_files.pop_front() {
        #[cfg(target_os = "wasi")]
        let canonical_file = file.clone();

        #[cfg(not(target_os = "wasi"))]
        let canonical_file = file.canonicalize().unwrap_or_else(|error| {
            eprintln!("error resolving {:?}: {error}", file);
            std::process::exit(1);
        });
        if !parsed_files.insert(canonical_file.clone()) {
            continue;
        }

        let source = std::fs::read_to_string(&canonical_file).unwrap_or_else(|error| {
            eprintln!("error reading {:?}: {error}", file);
            std::process::exit(1);
        });

        let token_stream = compiler_core::lexer::Lexer::new(&source);
        let mut parser = compiler_core::parser::Parser::new(token_stream, canonical_file.clone());

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

        for imported_module in program.imported_module_names() {
            if let Some(imported_path) = module_resolver.resolve(&canonical_file, imported_module) {
                pending_files.push_back(imported_path);
            }
        }

        // Apply optimizations
        // run_optimizations(
        //     &mut program,
        //     &[&DeframePass],
        //     &OptimizationPassOpts {
        //         dump: cli.дамп_опт_дебаг,
        //     },
        // )
        // .unwrap_or_else(|error| {
        //     eprintln!("Оптимизация не справилась: {error}");
        //     std::process::exit(1);
        // });

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
            let bytefile = run_codegen(&mut BcGen::new(), modules)
                .into_values()
                .next()
                .unwrap_or_else(|| {
                    eprintln!("Кодоген не создал байткод");
                    std::process::exit(1);
                });
            run_interpreter_bytes(bytefile, cli.дамп_код).unwrap_or_else(|error| {
                eprintln!("Интерпретатор не справился: {error}");
                std::process::exit(1);
            });
        }
    }
}
