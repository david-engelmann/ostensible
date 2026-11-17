//! The pins of PORT15, the keyword forms of the taken
//! assertion spellings, over synthetic sources the suite
//! writes itself. Each flagged pin drives the scan over a
//! test case class whose assertion stands in keyword
//! arguments of the parameters' own names, and the
//! boundary pins beside them hold the lines PORT15-3 and
//! PORT15-5 draw, the binding discipline, the places a
//! starred or an unnamed keyword leaves unread, and the
//! governing rule. Two pins reverse a standing answer of
//! the positional reading, the duplicated place and the
//! starred pair, and those two fail on the contract tree
//! beside the eight keyword pins, as PORT15-6 states.

use ostensible::pyscan::scan_source;

fn findings(source: &str) -> Vec<(String, String)> {
    scan_source(source, "test_case.py", None, None)
        .expect("the scan reads")
        .into_iter()
        .map(|finding| (finding.kind, finding.detail))
        .collect()
}

const HELPER: &str = "from unittest.mock import Mock\n\n\ndef make_client():\n    svc = Mock()\n    svc.fetch.return_value = 42\n    return svc\n\n\n";

#[test]
fn test_an_assert_equal_with_keyword_arguments_comparing_a_value_with_itself_is_flagged() {
    let source = "import unittest\n\n\nclass TestTotal(unittest.TestCase):\n    def test_total(self):\n        total = 5\n        self.assertEqual(first=total, second=total)\n";
    let found = findings(source);
    assert_eq!(found.len(), 1, "one finding stands: {found:?}");
    assert_eq!(found[0].0, "self_comparison");
    assert_eq!(found[0].1, "assertion compares total with itself");
}

#[test]
fn test_an_assert_equal_with_a_mixed_keyword_argument_comparing_a_value_with_itself_is_flagged() {
    let source = "import unittest\n\n\nclass TestTotal(unittest.TestCase):\n    def test_total(self):\n        total = 5\n        self.assertEqual(total, second=total)\n";
    let found = findings(source);
    assert_eq!(found.len(), 1, "one finding stands: {found:?}");
    assert_eq!(found[0].0, "self_comparison");
    assert_eq!(found[0].1, "assertion compares total with itself");
}

#[test]
fn test_an_assert_is_with_keyword_arguments_comparing_a_value_with_itself_is_flagged() {
    // The identity method names its places expr1 and
    // expr2, not the equality method's names, and the
    // site reads them under those names alone.
    let source = "import unittest\n\n\nclass TestTotal(unittest.TestCase):\n    def test_total(self):\n        total = 5\n        self.assertIs(expr1=total, expr2=total)\n";
    let found = findings(source);
    assert_eq!(found.len(), 1, "one finding stands: {found:?}");
    assert_eq!(found[0].0, "self_comparison");
    assert_eq!(found[0].1, "assertion compares total with itself");
}

#[test]
fn test_an_assert_equal_with_keyword_arguments_on_a_configured_mock_call_is_flagged() {
    // The direct form spelled in keyword arguments: the
    // mock stands configured in the test itself and the
    // assertion names its call in the first place.
    let source = "import unittest\nfrom unittest.mock import Mock\n\n\nclass TestFetch(unittest.TestCase):\n    def test_fetch(self):\n        svc = Mock()\n        svc.fetch.return_value = 42\n        self.assertEqual(first=svc.fetch(), second=42)\n";
    let found = findings(source);
    assert_eq!(found.len(), 1, "one finding stands: {found:?}");
    assert_eq!(found[0].0, "mock_return_value");
    assert!(
        found[0].1.contains(
            "assertion checks a call on a mock against the return value the mock was assigned, 42"
        ),
        "the direct form carries its wording: {}",
        found[0].1
    );
}

#[test]
fn test_an_assert_equal_with_keyword_arguments_on_a_helper_mock_call_is_flagged() {
    // The helper form spelled in keyword arguments, read
    // by the helper and fixture pass through the one site
    // reading both passes share.
    let source = format!(
        "{HELPER}import unittest\n\n\nclass TestFetch(unittest.TestCase):\n    def test_fetch(self):\n        client = make_client()\n        self.assertEqual(first=client.fetch(), second=42)\n"
    );
    let found = findings(&source);
    assert_eq!(found.len(), 1, "one finding stands: {found:?}");
    assert_eq!(found[0].0, "mock_return_value");
    assert!(
        found[0].1.contains("a helper or fixture in the same file"),
        "the helper form names its source: {}",
        found[0].1
    );
}

#[test]
fn test_an_assert_true_with_a_keyword_argument_on_the_constant_true_is_flagged() {
    let source = "import unittest\n\n\nclass TestConstant(unittest.TestCase):\n    def test_constant(self):\n        self.assertTrue(expr=True)\n";
    let found = findings(source);
    assert_eq!(found.len(), 1, "one finding stands: {found:?}");
    assert_eq!(found[0].0, "constant_assert");
    assert_eq!(found[0].1, "assertion on the constant true value");
}

#[test]
fn test_an_assert_false_with_a_keyword_argument_on_the_constant_false_is_flagged() {
    let source = "import unittest\n\n\nclass TestConstant(unittest.TestCase):\n    def test_constant(self):\n        self.assertFalse(expr=False)\n";
    let found = findings(source);
    assert_eq!(found.len(), 1, "one finding stands: {found:?}");
    assert_eq!(found[0].0, "constant_assert");
    assert_eq!(found[0].1, "assertion on the constant false value");
}

#[test]
fn test_an_assert_is_none_with_a_keyword_argument_on_the_constant_none_is_flagged() {
    let source = "import unittest\n\n\nclass TestConstant(unittest.TestCase):\n    def test_constant(self):\n        self.assertIsNone(obj=None)\n";
    let found = findings(source);
    assert_eq!(found.len(), 1, "one finding stands: {found:?}");
    assert_eq!(found[0].0, "constant_assert");
    assert_eq!(found[0].1, "assertion on the constant none value");
}

#[test]
fn test_an_assert_equal_with_a_duplicated_place_is_not_flagged() {
    // The first place stands claimed by the positional
    // argument and by the keyword argument of its name
    // together, and the run refuses the call for the
    // duplication and compares neither expression, so
    // the binding reading leaves the place unread and no
    // site forms. The positional reading this slice
    // narrows flagged the shape.
    let source = "import unittest\n\n\nclass TestTotal(unittest.TestCase):\n    def test_total(self):\n        total = 5\n        self.assertEqual(total, total, first=5)\n";
    let found = findings(source);
    assert!(found.is_empty(), "no finding stands: {found:?}");
}

#[test]
fn test_an_assert_equal_with_a_starred_argument_is_not_flagged() {
    // A starred argument hands the method the items of
    // its expansion, and the run compares those items or
    // refuses the call for their count, never the starred
    // expression with itself, so no place a starred
    // argument stands at is read. The positional reading
    // this slice narrows flagged the shape.
    let source = "import unittest\n\n\npair = (7, 7)\n\n\nclass TestTotal(unittest.TestCase):\n    def test_total(self):\n        self.assertEqual(*pair, *pair)\n";
    let found = findings(source);
    assert!(found.is_empty(), "no finding stands: {found:?}");
}

#[test]
fn test_an_assert_equal_with_a_keyword_argument_against_a_different_literal_is_not_flagged() {
    let source = format!(
        "{HELPER}import unittest\n\n\nclass TestFetch(unittest.TestCase):\n    def test_fetch(self):\n        client = make_client()\n        self.assertEqual(first=client.fetch(), second=43)\n"
    );
    let found = findings(&source);
    assert!(found.is_empty(), "no finding stands: {found:?}");
}

#[test]
fn test_an_assert_true_with_a_keyword_argument_on_a_value_is_not_flagged() {
    // The identity reading governs a constant read from a
    // keyword argument exactly as it governs one read
    // positionally, and a computed value is no constant.
    let source = "import unittest\n\n\nclass TestConstant(unittest.TestCase):\n    def test_constant(self):\n        flag = compute_flag()\n        self.assertTrue(expr=flag)\n";
    let found = findings(source);
    assert!(found.is_empty(), "no finding stands: {found:?}");
}

#[test]
fn test_an_assert_is_with_the_keyword_names_of_assert_equal_is_not_flagged() {
    // The names first and second are the equality
    // method's places and name no place of the identity
    // method, whose places are expr1 and expr2, so they
    // stand outside the reading and no site forms.
    let source = "import unittest\n\n\nclass TestTotal(unittest.TestCase):\n    def test_total(self):\n        total = 5\n        self.assertIs(first=total, second=total)\n";
    let found = findings(source);
    assert!(found.is_empty(), "no finding stands: {found:?}");
}

#[test]
fn test_an_assert_true_with_an_unnamed_keyword_forms_no_site() {
    // The double star form carries the places invisibly,
    // so a place that would be read from a keyword
    // argument is not read while one stands in the call.
    let source = "import unittest\n\n\nclass TestConstant(unittest.TestCase):\n    def test_constant(self):\n        self.assertTrue(expr=True, **{})\n";
    let found = findings(source);
    assert!(found.is_empty(), "no finding stands: {found:?}");
}

#[test]
fn test_an_assert_equal_with_keyword_arguments_outside_a_test_function_is_not_flagged() {
    let source = "import unittest\n\n\nclass HelperChecks(unittest.TestCase):\n    def check_total(self):\n        total = 5\n        self.assertEqual(first=total, second=total)\n";
    let found = findings(source);
    assert!(found.is_empty(), "no finding stands: {found:?}");
}

#[test]
fn test_an_assert_equal_with_the_message_keyword_comparing_a_value_with_itself_is_flagged() {
    // The message keyword stands outside the reading, as
    // the message argument has stood since PORT13, and
    // the site stands over the positional places alone.
    let source = "import unittest\n\n\nclass TestTotal(unittest.TestCase):\n    def test_total(self):\n        total = 5\n        self.assertEqual(total, total, msg=\"the totals stand\")\n";
    let found = findings(source);
    assert_eq!(found.len(), 1, "one finding stands: {found:?}");
    assert_eq!(found[0].0, "self_comparison");
    assert_eq!(found[0].1, "assertion compares total with itself");
}
