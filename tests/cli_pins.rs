//! PORT11 pins. The door's own wordings and the acceptance
//! edges the recorded battery cannot carry, proven over the
//! built binary and the committed demo project, which the door
//! reads and never writes.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use ostensible::cli::USAGE;

fn door(args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_ostensible"))
        .args(args)
        .output()
        .expect("the door runs")
}

fn demo_project() -> String {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("demo_project");
    path.to_str().expect("demo path is text").to_string()
}

fn demo_file(name: &str) -> Vec<u8> {
    let path: PathBuf = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join(name);
    std::fs::read(&path).expect("demo file reads")
}

fn stderr_text(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).expect("stderr is text")
}

fn stdout_text(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).expect("stdout is text")
}

#[test]
fn test_help_prints_the_usage_and_exits_zero() {
    let output = door(&["--help"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout_text(&output).starts_with(USAGE));
}

#[test]
fn test_a_missing_project_argument_is_a_usage_refusal() {
    let output = door(&[]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr_text(&output).contains(USAGE));
}

#[test]
fn test_an_unknown_flag_is_a_usage_refusal() {
    let demo = demo_project();
    let output = door(&[&demo, "--bogus"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr_text(&output).contains(USAGE));
}

#[test]
fn test_a_missing_timeout_value_is_a_usage_refusal() {
    let demo = demo_project();
    let output = door(&[&demo, "--timeout"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr_text(&output).contains(USAGE));
}

#[test]
fn test_a_non_float_timeout_is_a_usage_refusal() {
    let demo = demo_project();
    let output = door(&[&demo, "--timeout", "abc"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr_text(&output).contains(USAGE));
}

#[test]
fn test_an_extra_positional_is_a_usage_refusal() {
    let demo = demo_project();
    let output = door(&[&demo, "extra"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr_text(&output).contains(USAGE));
}

#[test]
fn test_an_abbreviated_flag_stands_for_the_flag() {
    let demo = demo_project();
    let output = door(&[&demo, "--tim", "5", "--j"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout_text(&output).starts_with('{'));
}

#[test]
fn test_the_equals_form_reads_the_timeout() {
    let demo = demo_project();
    let output = door(&[&demo, "--timeout=30"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(stdout_text(&output).starts_with("cantfail report"));
}

#[test]
fn test_a_repeated_timeout_keeps_its_last_value() {
    let demo = demo_project();
    let output = door(&[&demo, "--timeout", "0", "--timeout", "30"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(output.stdout, demo_file("demo_report.txt"));
}

#[test]
fn test_the_separator_makes_the_next_token_the_project() {
    let output = door(&["--", "--json"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr_text(&output),
        "project directory not found - --json\n"
    );
}

#[test]
fn test_a_negative_timeout_value_is_read_as_a_value() {
    let demo = demo_project();
    let output = door(&[&demo, "--timeout", "-1"]);
    assert_eq!(output.status.code(), Some(0));
    let text = stdout_text(&output);
    assert!(text.contains("project - demo_project"));
    assert!(text.contains("inconclusive"));
}

#[test]
fn test_a_missing_project_directory_refuses_with_its_words() {
    let output = door(&["no_such_project_here"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(
        stderr_text(&output),
        "project directory not found - no_such_project_here\n"
    );
}

#[test]
fn test_the_demo_reports_regenerate_byte_identical() {
    let demo = demo_project();
    let text = door(&[&demo]);
    assert_eq!(text.status.code(), Some(0));
    assert_eq!(text.stdout, demo_file("demo_report.txt"));
    let json = door(&[&demo, "--json"]);
    assert_eq!(json.status.code(), Some(0));
    assert_eq!(json.stdout, demo_file("demo_report.json"));
}
