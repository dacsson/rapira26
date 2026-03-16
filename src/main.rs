use std::path::PathBuf;

fn main() {
    let source_path: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            eprintln!("usage: rapira26 <source.rap>");
            std::process::exit(1);
        });

    let source = std::fs::read_to_string(&source_path).unwrap_or_else(|error| {
        eprintln!("error reading {:?}: {error}", source_path);
        std::process::exit(1);
    });

    let token_stream = rapira26::lexer::Lexer::new(&source);
    let parser = rapira26::parser::Parser::new(token_stream);

    match parser.parse_program() {
        Ok(program) => {
            println!("{program:#?}");
        }
        Err(error) => {
            eprintln!("parse error: {error}");
            std::process::exit(1);
        }
    }
}
