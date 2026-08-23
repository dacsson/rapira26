//! End-to-end tests:
//!   source file -> рапик binary (parse -> codegen -> RBC -> interpreter) -> compare output.
//!
//! Expected output is extracted from `\ => ...` comments in each source file.
//! Special markers:
//!   `\ => (no output)` — no output line produced
//!   `\ => (empty line)` — an empty line is expected
//!   `\ => (empty string)` — same as empty line

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

/// Run a .rap file through the full pipeline (рапик binary) and return its stdout.
fn run_rap_file(rap_path: &Path) -> Result<String, String> {
    let source = std::fs::read_to_string(rap_path)
        .map_err(|e| format!("failed to read {}: {e}", rap_path.display()))?;

    // Copy source to a temp dir so generated .rbc files don't pollute the tree
    let temp_dir = std::env::temp_dir().join("rapira26_e2e");
    std::fs::create_dir_all(&temp_dir).map_err(|e| format!("mkdir: {e}"))?;

    let filename = rap_path.file_name().unwrap();
    let temp_source = temp_dir.join(filename);
    std::fs::copy(rap_path, &temp_source).map_err(|e| format!("copy: {e}"))?;

    // set std path explicitely
    unsafe {
        std::env::set_var(
            "RAPIRA_STD",
            format!("{}/../../std/", env!("CARGO_MANIFEST_DIR")),
        );

        println!("std path: {}", std::env::var("RAPIRA_STD").unwrap())
    };

    let binary = env!("CARGO_BIN_EXE_рапик");
    let stdin_input = parse_stdin_input(&source);

    let mut child = Command::new(binary)
        .arg(&temp_source)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("spawn рапик: {e}"))?;

    if !stdin_input.is_empty() {
        child
            .stdin
            .take()
            .unwrap()
            .write_all(stdin_input.as_bytes())
            .map_err(|e| format!("write stdin: {e}"))?;
    }
    drop(child.stdin.take());

    let output = child.wait_with_output().map_err(|e| format!("wait: {e}"))?;

    // Clean up generated .rbc file
    let rbc_path = temp_source.with_extension("rbc");
    let _ = std::fs::remove_file(&rbc_path);

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(format!(
            "рапик failed (exit {}):\nstderr: {stderr}\nstdout: {stdout}",
            output.status.code().unwrap_or(-1),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

/// Run a test for a given .rap file: compile, execute, compare output.
fn assert_rap_output(filename: &str) {
    let project_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .to_path_buf();
    let rap_path = project_root.join("examples").join(filename);

    let source = std::fs::read_to_string(&rap_path)
        .unwrap_or_else(|e| panic!("cannot read {filename}: {e}"));
    let expected = parse_expected_output(&source);

    let actual = run_rap_file(&rap_path).unwrap_or_else(|e| panic!("{filename} failed:\n{e}"));

    // Compare line-by-line, trimming trailing whitespace (invisible in test files)
    let actual_lines: Vec<&str> = actual.lines().map(|l| l.trim_end()).collect();
    let expected_lines: Vec<&str> = expected.lines().map(|l| l.trim_end()).collect();

    if actual_lines != expected_lines {
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
fn e2e_07_functions() {
    assert_rap_output("07_functions.rap");
}

#[test]
fn e2e_08_type_checks() {
    assert_rap_output("08_type_checks.rap");
}

#[test]
fn e2e_09_spec_examples() {
    assert_rap_output("09_spec_examples.rap");
}

#[test]
fn e2e_10_input() {
    assert_rap_output("10_input.rap");
}

#[test]
fn e2e_11_user_types() {
    assert_rap_output("11_user_types.rap");
}

#[test]
fn e2e_static_linked_modules() {
    let temp_dir = std::env::temp_dir().join(format!("rapira26_modules_{}", std::process::id()));
    std::fs::create_dir_all(&temp_dir).unwrap();

    let base_path = temp_dir.join("base.rap");
    let math_path = temp_dir.join("math.rap");
    let main_path = temp_dir.join("main.rap");
    std::fs::write(
        &base_path,
        "вывод: \"base\"\nсмещение := 1\nфунк увеличить(х)\n  возврат х + смещение\n",
    )
    .unwrap();
    std::fs::write(
        &math_path,
        "подкл \"base\" (увеличить)\nвывод: \"math\"\nфунк вычислить(х)\n  возврат увеличить(х) * 2\n",
    )
    .unwrap();
    std::fs::write(
        &main_path,
        "подкл \"math\" (вычислить)\nвывод: \"main\"\nвывод: вычислить(41)\n",
    )
    .unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_рапик"))
        .arg(&main_path)
        .output()
        .unwrap();

    let _ = std::fs::remove_file(main_path.with_extension("rbc"));
    let _ = std::fs::remove_dir_all(&temp_dir);

    assert!(
        output.status.success(),
        "module program failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout),
        "base\nmath\nmain\n84\n"
    );
}

#[test]
fn e2e_module_from_std_path() {
    let temp_dir =
        std::env::temp_dir().join(format!("rapira26_std_modules_{}", std::process::id()));
    let app_dir = temp_dir.join("app");
    let std_dir = temp_dir.join("std");
    std::fs::create_dir_all(&app_dir).unwrap();
    std::fs::create_dir_all(&std_dir).unwrap();

    let main_path = app_dir.join("main.rap");
    std::fs::write(std_dir.join("база.рап"), "функ индекс()\n  возврат 26\n").unwrap();
    std::fs::write(&main_path, "подкл \"база\" (индекс)\nвывод: индекс()\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_рапик"))
        .arg("--путь-стд")
        .arg(&std_dir)
        .arg(&main_path)
        .output()
        .unwrap();

    let _ = std::fs::remove_file(main_path.with_extension("rbc"));
    let _ = std::fs::remove_dir_all(&temp_dir);

    assert!(
        output.status.success(),
        "stdlib module program failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "26\n");
}

#[test]
fn e2e_module_from_rapira_std() {
    let temp_dir = std::env::temp_dir().join(format!("rapira26_std_env_{}", std::process::id()));
    let app_dir = temp_dir.join("app");
    let std_dir = temp_dir.join("std");
    std::fs::create_dir_all(&app_dir).unwrap();
    std::fs::create_dir_all(&std_dir).unwrap();

    let main_path = app_dir.join("main.rap");
    std::fs::write(std_dir.join("база.рап"), "функ индекс()\n  возврат 2026\n").unwrap();
    std::fs::write(&main_path, "подкл \"база\" (индекс)\nвывод: индекс()\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_рапик"))
        .env("RAPIRA_STD", &std_dir)
        .arg(&main_path)
        .output()
        .unwrap();

    let _ = std::fs::remove_file(main_path.with_extension("rbc"));
    let _ = std::fs::remove_dir_all(&temp_dir);

    assert!(
        output.status.success(),
        "RAPIRA_STD module program failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "2026\n");
}

#[test]
fn e2e_local_module_shadows_rapira_std() {
    let temp_dir = std::env::temp_dir().join(format!("rapira26_std_shadow_{}", std::process::id()));
    let app_dir = temp_dir.join("app");
    let std_dir = temp_dir.join("std");
    std::fs::create_dir_all(&app_dir).unwrap();
    std::fs::create_dir_all(&std_dir).unwrap();

    let main_path = app_dir.join("main.rap");
    std::fs::write(
        app_dir.join("база.рап"),
        "функ источник()\n  возврат \"local\"\n",
    )
    .unwrap();
    std::fs::write(
        std_dir.join("база.рап"),
        "функ источник()\n  возврат \"std\"\n",
    )
    .unwrap();
    std::fs::write(&main_path, "подкл \"база\" (источник)\nвывод: источник()\n").unwrap();

    let output = Command::new(env!("CARGO_BIN_EXE_рапик"))
        .env("RAPIRA_STD", &std_dir)
        .arg(&main_path)
        .output()
        .unwrap();

    let _ = std::fs::remove_file(main_path.with_extension("rbc"));
    let _ = std::fs::remove_dir_all(&temp_dir);

    assert!(
        output.status.success(),
        "stdlib shadowing program failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(String::from_utf8_lossy(&output.stdout), "local\n");
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
