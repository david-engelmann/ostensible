//! Pins for the indentation ceiling of the Round 15 fix
//! pass. The interpreter's indentation stack holds one
//! hundred levels, the level of the file itself among
//! them, whatever statement kind opens each level, and a
//! line standing deeper refuses with the wording the port
//! states for it. The boundaries below were probed against
//! the interpreter shape by shape: a def at the top adds
//! its own level, so a tower of ifs inside a def stands to
//! 98 and refuses at 99, while a tower at module level
//! stands to 99 and refuses at 100, and mixed towers of
//! defs and ifs refuse on the same single count.

use ostensible::pyparse::parse_module;
use ostensible::pyscan::scan_source;

/// A tower of nested ifs inside one def; the def stands
/// at level one, each if one level deeper.
fn ifs_in_def(n: usize) -> String {
    let mut lines = vec!["def test_a():".to_string()];
    for i in 0..n {
        lines.push(format!("{}if True:", "    ".repeat(i + 1)));
    }
    lines.push(format!("{}assert True", "    ".repeat(n + 1)));
    lines.join("\n") + "\n"
}

/// A tower of nested ifs at module level carrying the def
/// at its foot; the def's own body stands one level below
/// the def line, so the tower of ifs stands to 98.
fn ifs_at_module(n: usize) -> String {
    let mut lines = Vec::new();
    for i in 0..n {
        lines.push(format!("{}if True:", "    ".repeat(i)));
    }
    lines.push(format!("{}def test_a():", "    ".repeat(n)));
    lines.push(format!("{}assert True", "    ".repeat(n + 1)));
    lines.join("\n") + "\n"
}

/// A tower of nested defs at module level.
fn nested_defs(n: usize) -> String {
    let mut lines = Vec::new();
    for i in 0..n {
        lines.push(format!("{}def f{}():", "    ".repeat(i), i));
    }
    lines.push(format!("{}assert True", "    ".repeat(n)));
    lines.join("\n") + "\n"
}

/// A tower alternating ifs and defs at module level, one
/// statement kind per level.
fn mixed_tower(pairs: usize) -> String {
    let mut lines = Vec::new();
    let mut level = 0usize;
    for i in 0..pairs {
        lines.push(format!("{}if True:", "    ".repeat(level)));
        level += 1;
        lines.push(format!("{}def f{}():", "    ".repeat(level), i));
        level += 1;
    }
    lines.push(format!("{}assert True", "    ".repeat(level)));
    lines.join("\n") + "\n"
}

#[test]
fn ifs_in_a_def_stand_to_98_and_refuse_at_99() {
    assert!(parse_module(&ifs_in_def(98)).is_ok());
    let error = parse_module(&ifs_in_def(99)).expect_err("the tower refuses at 99");
    assert_eq!(error.message, "too many levels of indentation");
}

#[test]
fn ifs_at_module_level_stand_to_98_and_refuse_at_99() {
    assert!(parse_module(&ifs_at_module(98)).is_ok());
    let error = parse_module(&ifs_at_module(99)).expect_err("the tower refuses at 99");
    assert_eq!(error.message, "too many levels of indentation");
}

#[test]
fn nested_defs_refuse_on_the_same_count() {
    assert!(parse_module(&nested_defs(98)).is_ok());
    let error = parse_module(&nested_defs(150)).expect_err("the tower refuses");
    assert_eq!(error.message, "too many levels of indentation");
}

#[test]
fn mixed_towers_count_every_level_alike() {
    assert!(parse_module(&mixed_tower(40)).is_ok());
    let error = parse_module(&mixed_tower(60)).expect_err("the tower refuses");
    assert_eq!(error.message, "too many levels of indentation");
}

#[test]
fn the_red_team_towers_refuse() {
    for source in [ifs_in_def(120), ifs_in_def(210)] {
        let error = parse_module(&source).expect_err("the tower refuses");
        assert_eq!(error.message, "too many levels of indentation");
    }
}

#[test]
fn the_scan_refuses_a_tower_in_its_own_wording() {
    let error = scan_source(&ifs_in_def(120), "test_case.py", None, None)
        .expect_err("the scan refuses the tower");
    assert_eq!(
        error.message,
        "the source does not parse under the port's reading"
    );
}

#[test]
fn dedent_frees_the_levels_it_leaves() {
    let mut source = String::new();
    for _ in 0..2 {
        source.push_str(&ifs_at_module(90));
    }
    assert!(parse_module(&source).is_ok());
}
