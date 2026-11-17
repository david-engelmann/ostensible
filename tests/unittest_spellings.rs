//! The pins of PORT13, the unittest assertion spellings,
//! over synthetic sources the suite writes itself. Each
//! flagged pin drives the scan over a test case class whose
//! assertion is spelled as a method call, beside the same
//! assertion in the statement spelling where the pair makes
//! the claim plain.

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
fn test_an_assert_equal_on_a_configured_mock_call_is_flagged() {
    // The direct form: the mock stands configured in the
    // test itself and the assertion names its call.
    let source = "import unittest\nfrom unittest.mock import Mock\n\n\nclass TestFetch(unittest.TestCase):\n    def test_fetch(self):\n        svc = Mock()\n        svc.fetch.return_value = 42\n        self.assertEqual(svc.fetch(), 42)\n";
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
fn test_an_assert_equal_on_a_helper_mock_call_is_flagged() {
    let source = format!(
        "{HELPER}import unittest\n\n\nclass TestFetch(unittest.TestCase):\n    def test_fetch(self):\n        client = make_client()\n        self.assertEqual(client.fetch(), 42)\n"
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
fn test_an_assert_is_on_a_configured_mock_call_is_flagged() {
    let source = format!(
        "{HELPER}import unittest\n\n\nclass TestFetch(unittest.TestCase):\n    def test_fetch(self):\n        client = make_client()\n        self.assertIs(client.fetch(), 42)\n"
    );
    let found = findings(&source);
    assert_eq!(found.len(), 1, "one finding stands: {found:?}");
    assert_eq!(found[0].0, "mock_return_value");
}

#[test]
fn test_an_assert_equal_comparing_a_value_with_itself_is_flagged() {
    let source = "import unittest\n\n\nclass TestTotal(unittest.TestCase):\n    def test_total(self):\n        total = 5\n        self.assertEqual(total, total)\n";
    let found = findings(source);
    assert_eq!(found.len(), 1, "one finding stands: {found:?}");
    assert_eq!(found[0].0, "self_comparison");
    assert_eq!(found[0].1, "assertion compares total with itself");
}

#[test]
fn test_an_assert_true_on_the_constant_true_is_flagged() {
    let source = "import unittest\n\n\nclass TestConstant(unittest.TestCase):\n    def test_constant(self):\n        self.assertTrue(True)\n";
    let found = findings(source);
    assert_eq!(found.len(), 1, "one finding stands: {found:?}");
    assert_eq!(found[0].0, "constant_assert");
    assert_eq!(found[0].1, "assertion on the constant true value");
}

#[test]
fn test_an_assert_equal_against_a_different_literal_is_not_flagged() {
    let source = format!(
        "{HELPER}import unittest\n\n\nclass TestFetch(unittest.TestCase):\n    def test_fetch(self):\n        client = make_client()\n        self.assertEqual(client.fetch(), 43)\n"
    );
    let found = findings(&source);
    assert!(found.is_empty(), "no finding stands: {found:?}");
}

#[test]
fn test_an_assert_equal_outside_a_test_function_is_not_flagged() {
    let source = format!(
        "{HELPER}import unittest\n\n\nclass HelperChecks(unittest.TestCase):\n    def check_fetch(self):\n        client = make_client()\n        self.assertEqual(client.fetch(), 42)\n"
    );
    let found = findings(&source);
    assert!(found.is_empty(), "no finding stands: {found:?}");
}

#[test]
fn test_an_assert_equal_with_one_argument_forms_no_site() {
    let source = "import unittest\n\n\nclass TestTotal(unittest.TestCase):\n    def test_total(self):\n        total = 5\n        self.assertEqual(total)\n";
    let found = findings(source);
    assert!(found.is_empty(), "no finding stands: {found:?}");
}
