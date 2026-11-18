//! The pins of PORT17, a positional argument standing
//! after a keyword argument, over synthetic sources the
//! suite writes itself. The three refusing pins drive the
//! scan over a test case class carrying a flaggable
//! constant assertion beside the refused shape, and fail
//! on the head this slice branches from, where the parser
//! reads the file and the scan reports the finding. The
//! three standing pins hold the standings PORT17-2 keeps
//! and the boundary PORT17-3 states, on both heads.

use ostensible::pyscan::scan_source;

const UNPARSEABLE: &str = "the source does not parse under the port's reading";

fn wrap(body: &str) -> String {
    format!(
        "import unittest\n\n\nclass TestOrder(unittest.TestCase):\n    def test_order(self):\n        self.assertTrue(True)\n{body}"
    )
}

fn refusal(source: &str) -> String {
    scan_source(source, "test_case.py", None, None)
        .expect_err("the scan refuses")
        .message
}

#[test]
fn test_a_positional_argument_after_a_keyword_refuses_the_scan() {
    let source = wrap("        f(x=1, 2)\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_positional_argument_after_an_unpacking_refuses_the_scan() {
    let source = wrap("        f(**a, 2)\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_positional_base_after_a_keyword_refuses_the_scan() {
    let source = "import unittest\n\n\nclass TestOrder(unittest.TestCase, metaclass=type, 2):\n    def test_order(self):\n        self.assertTrue(True)\n";
    assert_eq!(refusal(source), UNPARSEABLE);
}

#[test]
fn test_a_starred_argument_after_a_keyword_still_reads() {
    let source = wrap("        f(a=1, *b)\n");
    let found = scan_source(&source, "test_case.py", None, None).expect("the scan reads");
    assert_eq!(found.len(), 1, "the constant finding stands: {found:?}");
    assert_eq!(found[0].kind, "constant_assert");
}

#[test]
fn test_a_keyword_argument_after_positionals_still_reads() {
    let source = wrap("        f(1, a=2)\n");
    let found = scan_source(&source, "test_case.py", None, None).expect("the scan reads");
    assert_eq!(found.len(), 1, "the constant finding stands: {found:?}");
    assert_eq!(found[0].kind, "constant_assert");
}

#[test]
fn test_a_repeated_keyword_still_reads_the_file() {
    let source = wrap("        f(x=1, x=2)\n");
    let found = scan_source(&source, "test_case.py", None, None).expect("the scan reads");
    assert_eq!(found.len(), 1, "the constant finding stands: {found:?}");
    assert_eq!(found[0].kind, "constant_assert");
}
