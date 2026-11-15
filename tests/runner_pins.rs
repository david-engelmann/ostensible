//! PORT10 pins. The standings the battery cannot carry: the
//! refusal where a run cannot be started at all, and the
//! discovery passing over a dotted folder whole.

use ostensible::analyse_project;
use std::path::PathBuf;

fn scratch(name: &str) -> PathBuf {
    let root = std::env::temp_dir().join(format!(
        "ostensible-port10-pins-{}-{name}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).expect("pin scratch builds");
    root
}

#[test]
fn an_unstartable_run_refuses_in_the_ports_own_wording() {
    let root = scratch("unstartable");
    let project = root.join("proj");
    std::fs::create_dir_all(&project).expect("project builds");
    std::fs::write(
        project.join("calc.py"),
        "def add(a, b):\n    return a + b\n",
    )
    .expect("source writes");
    std::fs::write(
        project.join("test_calc.py"),
        "from calc import add\n\n\ndef test_add():\n    assert add(2, 3) == 5\n",
    )
    .expect("tests write");
    let error = analyse_project(
        &project,
        20.0,
        PathBuf::from("/nonexistent/python").as_path(),
    )
    .expect_err("an unstartable run must refuse");
    assert_eq!(error.message, "the pytest run could not be started");
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn a_dotted_folder_is_passed_over_whole() {
    let root = scratch("dotted");
    let project = root.join("proj");
    let hidden = project.join(".hidden");
    std::fs::create_dir_all(&hidden).expect("project builds");
    std::fs::write(
        hidden.join("test_calc.py"),
        "def test_nothing():\n    assert True\n",
    )
    .expect("hidden tests write");
    std::fs::write(
        project.join("calc.py"),
        "def add(a, b):\n    return a + b\n",
    )
    .expect("source writes");
    let analysis = analyse_project(&project, 20.0, PathBuf::from("python3").as_path())
        .expect("the analysis answers");
    assert!(analysis.verdicts.is_empty());
    assert!(analysis.static_findings.is_empty());
    let _ = std::fs::remove_dir_all(&root);
}
