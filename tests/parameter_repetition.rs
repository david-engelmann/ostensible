//! The pins of PORT19, a parameter name spelled twice
//! in one definition, over synthetic sources the suite
//! writes itself. The six refusing pins drive the scan
//! over a test case class carrying a flaggable constant
//! assertion beside the refused shape, and fail on the
//! head this slice branches from, where the parser reads
//! the file and the scan reports the finding. The three
//! standing pins hold the standings PORT19-2 keeps, on
//! both heads.

use ostensible::pyscan::scan_source;

const UNPARSEABLE: &str = "the source does not parse under the port's reading";

fn wrap(helper: &str) -> String {
    format!(
        "import unittest\n\n\n{helper}\n\nclass TestRepeat(unittest.TestCase):\n    def test_repeat(self):\n        self.assertTrue(True)\n"
    )
}

fn refusal(source: &str) -> String {
    scan_source(source, "test_case.py", None, None)
        .expect_err("the scan refuses")
        .message
}

#[test]
fn test_a_repeated_parameter_refuses_the_scan() {
    let source = wrap("def helper(a, a):\n    return a\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_repeated_parameter_across_the_slash_refuses_the_scan() {
    let source = wrap("def helper(a, b, /, b):\n    return b\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_repeated_starred_name_refuses_the_scan() {
    let source = wrap("def helper(a, *a):\n    return a\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_repeated_double_starred_name_refuses_the_scan() {
    let source = wrap("def helper(a, **a):\n    return a\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_repeated_keyword_only_parameter_refuses_the_scan() {
    let source = wrap("def helper(a, *, a):\n    return a\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_repeated_lambda_parameter_refuses_the_scan() {
    let source = wrap("helper = lambda a, a: a\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_the_same_name_in_two_definitions_still_reads() {
    let source = wrap("def helper(a):\n    return a\n\n\ndef other(a):\n    return a\n");
    let found = scan_source(&source, "test_case.py", None, None).expect("the scan reads");
    assert_eq!(found.len(), 1, "the constant finding stands: {found:?}");
    assert_eq!(found[0].kind, "constant_assert");
}

#[test]
fn test_a_name_repeated_in_a_nested_lambda_still_reads() {
    let source = wrap("def helper(a):\n    return (lambda a: a)(a)\n");
    let found = scan_source(&source, "test_case.py", None, None).expect("the scan reads");
    assert_eq!(found.len(), 1, "the constant finding stands: {found:?}");
    assert_eq!(found[0].kind, "constant_assert");
}

#[test]
fn test_distinct_names_in_every_slot_still_read() {
    let source = wrap("def helper(a, /, b, *c, d, **e):\n    return b\n");
    let found = scan_source(&source, "test_case.py", None, None).expect("the scan reads");
    assert_eq!(found.len(), 1, "the constant finding stands: {found:?}");
    assert_eq!(found[0].kind, "constant_assert");
}
