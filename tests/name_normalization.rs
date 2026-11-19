//! The pins of PORT20, a name under its normalized
//! spelling, over synthetic sources the suite writes
//! itself. The three refusing pins drive the scan over
//! a test case class carrying a flaggable constant
//! assertion beside the refused shape, and fail on the
//! head this slice branches from, where the parser reads
//! the file under the raw spellings and the scan reports
//! the finding. The carried spelling pin fails on that
//! head on the finding's own text. The five standing
//! pins hold the standings PORT20-3 keeps, on both heads.

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
fn test_a_kelvin_spelled_parameter_pair_refuses_the_scan() {
    let source = wrap("def helper(K, \u{212a}):\n    return K\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_ligature_spelled_parameter_pair_refuses_the_scan() {
    let source = wrap("def helper(\u{fb00}, ff):\n    return ff\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_kelvin_spelled_keyword_pair_refuses_the_scan() {
    let source = wrap("probe(K=1, \u{212a}=2)\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_self_comparison_carries_the_normalized_spelling() {
    let source = "import unittest\n\n\nclass TestNames(unittest.TestCase):\n    def test_check(self):\n        assert \u{212a} == \u{212a}\n";
    let found = scan_source(source, "test_case.py", None, None).expect("the scan reads");
    let comparison = found
        .iter()
        .find(|finding| finding.kind == "self_comparison")
        .expect("the self comparison finding stands");
    assert_eq!(comparison.detail, "assertion compares K with itself");
}

#[test]
fn test_a_lone_kelvin_spelled_parameter_still_reads() {
    let source = wrap("def helper(\u{212a}):\n    return 1\n");
    let found = scan_source(&source, "test_case.py", None, None).expect("the scan reads");
    assert_eq!(found.len(), 1, "the constant finding stands: {found:?}");
    assert_eq!(found[0].kind, "constant_assert");
}

#[test]
fn test_a_fullwidth_name_reads_as_a_name() {
    let source = wrap("def helper():\n    \u{ff49}\u{ff46} = 1\n    return \u{ff49}\u{ff46}\n");
    let found = scan_source(&source, "test_case.py", None, None).expect("the scan reads");
    assert_eq!(found.len(), 1, "the constant finding stands: {found:?}");
    assert_eq!(found[0].kind, "constant_assert");
}

#[test]
fn test_a_string_carrying_the_kelvin_sign_still_reads() {
    let source = wrap("def helper():\n    return \"\u{212a}\"\n");
    let found = scan_source(&source, "test_case.py", None, None).expect("the scan reads");
    assert_eq!(found.len(), 1, "the constant finding stands: {found:?}");
    assert_eq!(found[0].kind, "constant_assert");
}

#[test]
fn test_a_name_already_in_its_normalized_form_still_reads() {
    let source = wrap("def helper(\u{1e69}x):\n    return 1\n");
    let found = scan_source(&source, "test_case.py", None, None).expect("the scan reads");
    assert_eq!(found.len(), 1, "the constant finding stands: {found:?}");
    assert_eq!(found[0].kind, "constant_assert");
}

#[test]
fn test_names_agreeing_in_neither_spelling_still_read() {
    let source = wrap("def helper(K, a\u{212a}b):\n    return K\n");
    let found = scan_source(&source, "test_case.py", None, None).expect("the scan reads");
    assert_eq!(found.len(), 1, "the constant finding stands: {found:?}");
    assert_eq!(found[0].kind, "constant_assert");
}
