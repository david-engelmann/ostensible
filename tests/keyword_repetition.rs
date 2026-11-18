//! The pins of PORT18, a keyword argument spelled twice
//! in one call, over synthetic sources the suite writes
//! itself. The four refusing pins drive the scan over a
//! test case class carrying a flaggable constant
//! assertion beside the refused shape, and fail on the
//! head this slice branches from, where the parser reads
//! the file and the scan reports the finding. The three
//! standing pins hold the standings PORT18-2 keeps, on
//! both heads.

use ostensible::pyscan::scan_source;

const UNPARSEABLE: &str = "the source does not parse under the port's reading";

fn wrap(body: &str) -> String {
    format!(
        "import unittest\n\n\nclass TestRepeat(unittest.TestCase):\n    def test_repeat(self):\n        self.assertTrue(True)\n{body}"
    )
}

fn refusal(source: &str) -> String {
    scan_source(source, "test_case.py", None, None)
        .expect_err("the scan refuses")
        .message
}

#[test]
fn test_a_repeated_keyword_refuses_the_scan() {
    let source = wrap("        f(x=1, x=2)\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_repeated_keyword_standing_apart_refuses_the_scan() {
    let source = wrap("        f(x=1, y=2, x=3)\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_repeated_keyword_across_a_starred_and_an_unpacking_refuses_the_scan() {
    let source = wrap("        f(x=1, *b, **c, x=2)\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_repeated_keyword_in_a_class_head_refuses_the_scan() {
    let source = "import unittest\n\n\nclass TestRepeat(unittest.TestCase, metaclass=type, metaclass=type):\n    def test_repeat(self):\n        self.assertTrue(True)\n";
    assert_eq!(refusal(source), UNPARSEABLE);
}

#[test]
fn test_an_unpacking_keyword_repeated_still_reads() {
    let source = wrap("        f(**a, **b)\n");
    let found = scan_source(&source, "test_case.py", None, None).expect("the scan reads");
    assert_eq!(found.len(), 1, "the constant finding stands: {found:?}");
    assert_eq!(found[0].kind, "constant_assert");
}

#[test]
fn test_the_same_keyword_in_two_calls_still_reads() {
    let source = wrap("        f(x=1)\n        g(x=2)\n");
    let found = scan_source(&source, "test_case.py", None, None).expect("the scan reads");
    assert_eq!(found.len(), 1, "the constant finding stands: {found:?}");
    assert_eq!(found[0].kind, "constant_assert");
}

#[test]
fn test_a_keyword_beside_a_starred_and_an_unpacking_still_reads() {
    let source = wrap("        f(1, *b, x=2, **c)\n");
    let found = scan_source(&source, "test_case.py", None, None).expect("the scan reads");
    assert_eq!(found.len(), 1, "the constant finding stands: {found:?}");
    assert_eq!(found[0].kind, "constant_assert");
}
