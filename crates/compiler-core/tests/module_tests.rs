use std::path::PathBuf;

use compiler_core::{
    lexer::Lexer,
    module::{DependencyError, build_dependency_graph},
    parser::Parser,
};

fn parse_module(path: &str, source: &str) -> compiler_core::module::Module {
    let path = PathBuf::from(path);
    let lexer = Lexer::new(source);
    Parser::new(lexer, path.clone())
        .parse_program()
        .unwrap_or_else(|error| {
            panic!("failed to parse {path:?}: {error:?}");
        })
}

#[test]
fn resolves_imported_functions_and_orders_dependencies_first() {
    let math = parse_module("/project/math.rap", "функ удвоить(х)\n  возврат х * 2\n");
    let main = parse_module(
        "/project/main.rap",
        "подкл \"math\" (удвоить)\nвывод: удвоить(21)\n",
    );

    let (_, modules) = build_dependency_graph(vec![main, math]).unwrap();

    assert_eq!(modules[0].name.get(), "math");
    assert_eq!(modules[1].name.get(), "main");
    assert_eq!(modules[1].resolved_imports["удвоить"].get(), "math");
}

#[test]
fn rejects_missing_imported_definition() {
    let math = parse_module("/project/math.rap", "функ удвоить(х)\n  возврат х * 2\n");
    let main = parse_module("/project/main.rap", "подкл \"math\" (нет_такой)\n");

    let error = build_dependency_graph(vec![main, math]).unwrap_err();
    assert_eq!(
        error,
        DependencyError::DefinitionNotFound {
            module: "math".to_string(),
            definition: "нет_такой".to_string(),
        }
    );
}

#[test]
fn rejects_duplicate_module_names() {
    let first = parse_module("/first/math.rap", "");
    let second = parse_module("/second/math.rap", "");

    let error = build_dependency_graph(vec![first, second]).unwrap_err();
    assert_eq!(error, DependencyError::DuplicateModule("math".to_string()));
}

#[test]
fn rejects_circular_imports() {
    let first = parse_module(
        "/project/first.rap",
        "подкл \"second\" (вторая)\nфунк первая()\n  возврат вторая()\n",
    );
    let second = parse_module(
        "/project/second.rap",
        "подкл \"first\" (первая)\nфунк вторая()\n  возврат первая()\n",
    );

    let error = build_dependency_graph(vec![first, second]).unwrap_err();
    assert_eq!(error, DependencyError::CyclicDependency);
}
