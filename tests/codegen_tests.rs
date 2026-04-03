//! End-to-end tests: .rap → parse → codegen → gcc → run → compare output.
//!
//! Expected output is extracted from `\ => ...` comments in each .rap file.
//! Special markers:
//!   `\ => (no output)` — no output line produced
//!   `\ => (empty line)` — an empty line is expected
//!   `\ => (empty string)` — same as empty line

use rapira26::opt::deframe::DeframePass;
use rapira26::opt::opt_pass::{OptimizationPassOpts, run_optimizations};
use rapira26::pretty::pretty_parse_error;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

/// Parse expected output from `\ => ...` comments in a Rapira source file.
fn parse_expected_output(source: &str) -> String {
    let mut lines = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("\\ =>") {
            let rest = rest.strip_prefix(' ').unwrap_or(rest);
            match rest {
                "(no output)" | "(empty string)" => {
                    // These markers mean no output line is produced
                }
                "(empty line)" => {
                    lines.push(String::new());
                }
                _ => {
                    lines.push(rest.to_string());
                }
            }
        }
    }
    let mut result = lines.join("\n");
    if !result.is_empty() {
        result.push('\n');
    }
    result
}

/// Parse stdin input from `\ <= ...` comments in a Rapira source file.
fn parse_stdin_input(source: &str) -> String {
    let mut lines = Vec::new();
    for line in source.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("\\ <=") {
            let rest = rest.strip_prefix(' ').unwrap_or(rest);
            lines.push(rest.to_string());
        }
    }
    lines.join("\n") + if lines.is_empty() { "" } else { "\n" }
}

/// Locate the runtime/ directory (relative to project root).
fn runtime_dir() -> PathBuf {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir.join("runtime")
}

/// Run a .rap file through the full pipeline and return its stdout.
fn run_rap_file(rap_path: &Path) -> Result<String, String> {
    // Read and parse
    let source = std::fs::read_to_string(rap_path)
        .map_err(|e| format!("failed to read {}: {e}", rap_path.display()))?;

    let token_stream = rapira26::lexer::Lexer::new(&source);
    let parser = rapira26::parser::Parser::new(token_stream);
    let mut program = parser
        .parse_program()
        .map_err(|e| pretty_parse_error(&source, rap_path.to_str().unwrap(), e))?;

    // Apply optimizations
    run_optimizations(
        &mut program,
        &[&DeframePass],
        &OptimizationPassOpts { dump: false },
    )
    .unwrap_or_else(|error| {
        eprintln!("Оптимизация не справилась: {error}");
        std::process::exit(1);
    });

    let codegen = rapira26::codegen::Codegen::new();
    let c_code = codegen.generate(&program, rap_path.canonicalize().unwrap().to_str().unwrap());

    // Write C to a temp file
    let temp_dir = std::env::temp_dir().join("rapira26_e2e");
    std::fs::create_dir_all(&temp_dir).map_err(|e| format!("mkdir: {e}"))?;

    let stem = rap_path.file_stem().unwrap().to_str().unwrap();
    let c_path = temp_dir.join(format!("{stem}.c"));
    let bin_path = temp_dir.join(stem);

    std::fs::write(&c_path, &c_code).map_err(|e| format!("write C: {e}"))?;

    // Compile
    let rt = runtime_dir();
    let gcc_output = Command::new("gcc")
        .arg(&c_path)
        .arg("-o")
        .arg(&bin_path)
        .arg(format!("-I{}", rt.display())) // runtime C library
        .arg(format!("-I{}", rt.join("raperr/include").display())) // runtime error C library
        .arg(format!("-L{}", rt.join("lib").display()))
        .arg(format!("-L{}", rt.join("raperr/target/release").display()))
        .arg("-lrapruntime")
        .arg("-lraperr")
        .arg("-lm")
        .output()
        .map_err(|e| format!("gcc launch: {e}"))?;

    if !gcc_output.status.success() {
        let stderr = String::from_utf8_lossy(&gcc_output.stderr);
        return Err(format!(
            "gcc failed (exit {}):\n{stderr}\n\n--- generated C ---\n{c_code}",
            gcc_output.status.code().unwrap_or(-1)
        ));
    }

    // Run with stdin piped if needed
    let stdin_input = parse_stdin_input(&source);
    let mut child = Command::new(&bin_path)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("run: {e}"))?;

    if !stdin_input.is_empty() {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin_input.as_bytes())
            .map_err(|e| format!("write stdin: {e}"))?;
    }

    let run_output = child.wait_with_output().map_err(|e| format!("wait: {e}"))?;

    if !run_output.status.success() {
        let stderr = String::from_utf8_lossy(&run_output.stderr);
        return Err(format!(
            "runtime error (exit {}):\nstderr: {stderr}\nstdout: {}",
            run_output.status.code().unwrap_or(-1),
            String::from_utf8_lossy(&run_output.stdout)
        ));
    }

    Ok(String::from_utf8_lossy(&run_output.stdout).to_string())
}

/// Run a test for a given .rap file: compile, execute, compare output.
fn assert_rap_output(filename: &str) {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let rap_path = manifest_dir.join("tests/examples").join(filename);

    let source = std::fs::read_to_string(&rap_path)
        .unwrap_or_else(|e| panic!("cannot read {filename}: {e}"));
    let expected = parse_expected_output(&source);

    let actual = run_rap_file(&rap_path).unwrap_or_else(|e| panic!("{filename} failed:\n{e}"));

    // Compare line-by-line, trimming trailing whitespace (invisible in test files)
    let actual_lines: Vec<&str> = actual.lines().map(|l| l.trim_end()).collect();
    let expected_lines: Vec<&str> = expected.lines().map(|l| l.trim_end()).collect();

    if actual_lines != expected_lines {
        // Find all mismatches
        let mismatches: Vec<(usize, String, String)> = actual_lines
            .iter()
            .zip(expected_lines.iter())
            .enumerate()
            .filter(|(_, (a, e))| a != e)
            .map(|(i, (a, e))| (i, a.to_string(), e.to_string()))
            .collect();

        panic!(
            "\n\n=== {filename}: output mismatch ===\n\
             --- expected ---\n{expected}\
             --- actual ---\n{actual}\
             --- end ---\n\
             --- mismatches ---\n{}\
             \n--- end ---\n",
            mismatches
                .iter()
                .map(|(i, a, e)| format!("{i}: expected '{e}', got '{a}'"))
                .collect::<Vec<_>>()
                .join("\n")
        );
    }
}

// ── Tests ──────────────────────────────────────────────────────

#[test]
fn e2e_01_output_and_literals() {
    assert_rap_output("01_output_and_literals.rap");
}

#[test]
fn e2e_02_arithmetic() {
    assert_rap_output("02_arithmetic.rap");
}

#[test]
fn e2e_03_text_operations() {
    assert_rap_output("03_text_operations.rap");
}

#[test]
fn e2e_04_tuple_operations() {
    assert_rap_output("04_tuple_operations.rap");
}

#[test]
fn e2e_05_conditionals() {
    assert_rap_output("05_conditionals.rap");
}

#[test]
fn e2e_06_loops() {
    assert_rap_output("06_loops.rap");
}

#[test]
fn e2e_07_procedures() {
    assert_rap_output("07_procedures.rap");
}

#[test]
fn e2e_08_functions() {
    assert_rap_output("08_functions.rap");
}

#[test]
fn e2e_09_scoping() {
    assert_rap_output("09_scoping.rap");
}

#[test]
fn e2e_10_return_parameters() {
    assert_rap_output("10_return_parameters.rap");
}

#[test]
fn e2e_11_type_checks() {
    assert_rap_output("11_type_checks.rap");
}

#[test]
fn e2e_12_spec_examples() {
    assert_rap_output("12_spec_examples.rap");
}

#[test]
fn e2e_13_input() {
    assert_rap_output("13_input.rap");
}

// ── Unit tests for the expected-output parser ──────────────────

#[cfg(test)]
mod parser_tests {
    use super::parse_expected_output;

    #[test]
    fn simple_output() {
        let source = "вывод: 42\n\\ => 42\n";
        assert_eq!(parse_expected_output(source), "42\n");
    }

    #[test]
    fn multiple_lines() {
        let source = "вывод: 1\n\\ => 1\nвывод: 2\n\\ => 2\n";
        assert_eq!(parse_expected_output(source), "1\n2\n");
    }

    #[test]
    fn no_output_marker() {
        let source = "вывод: 42\n\\ => (no output)\n";
        assert_eq!(parse_expected_output(source), "");
    }

    #[test]
    fn empty_line_marker() {
        let source = "\\ => before\n\\ => (empty line)\n\\ => after\n";
        assert_eq!(parse_expected_output(source), "before\n\nafter\n");
    }

    #[test]
    fn no_expected_lines() {
        let source = "\\ just a comment\nвывод: 42\n";
        assert_eq!(parse_expected_output(source), "");
    }
}
