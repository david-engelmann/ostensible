//! The pins of PORT16, method comparison sites inside
//! expressions, over synthetic sources the suite writes
//! itself. Each flagged pin drives the scan over a test
//! case class whose assertion stands inside a larger
//! expression in the helper form, and the boundary pins
//! beside them hold the lines PORT16-4 draws, the lambda
//! body, the constant spelling, and the statement form
//! reading once. The nine flagged pins fail on the head
//! this slice branches from, where the helper and fixture
//! reading reads a method call only where it stands as a
//! statement, and the three standing pins hold on both
//! heads.

use ostensible::pyscan::scan_source;

fn findings(source: &str) -> Vec<(String, String)> {
    scan_source(source, "test_case.py", None, None)
        .expect("the scan reads")
        .into_iter()
        .map(|finding| (finding.kind, finding.detail))
        .collect()
}

const HELPER: &str = "from unittest.mock import Mock\n\n\ndef make_client():\n    svc = Mock()\n    svc.fetch.return_value = 42\n    return svc\n\n\n";

fn fetch_source(body: &str) -> String {
    format!(
        "{HELPER}import unittest\n\n\nclass TestFetch(unittest.TestCase):\n    def test_fetch(self):\n        client = make_client()\n{body}"
    )
}

fn assert_helper_finding(found: &[(String, String)]) {
    assert_eq!(found.len(), 1, "one finding stands: {found:?}");
    assert_eq!(found[0].0, "mock_return_value");
    assert!(
        found[0].1.contains("a helper or fixture in the same file"),
        "the helper form names its source: {}",
        found[0].1
    );
    assert!(
        found[0].1.contains("42"),
        "the detail names the configured value: {}",
        found[0].1
    );
}

#[test]
fn test_a_method_comparison_in_an_assignment_value_is_flagged() {
    let source = fetch_source("        result = self.assertEqual(client.fetch(), 42)\n");
    assert_helper_finding(&findings(&source));
}

#[test]
fn test_a_method_comparison_as_a_call_argument_is_flagged() {
    let source = fetch_source("        print(self.assertEqual(client.fetch(), 42))\n");
    assert_helper_finding(&findings(&source));
}

#[test]
fn test_a_method_comparison_as_a_boolean_operand_is_flagged() {
    let source = fetch_source("        self.assertEqual(client.fetch(), 42) or True\n");
    assert_helper_finding(&findings(&source));
}

#[test]
fn test_a_method_comparison_as_a_return_value_is_flagged() {
    let source = fetch_source("        return self.assertEqual(client.fetch(), 42)\n");
    assert_helper_finding(&findings(&source));
}

#[test]
fn test_a_method_comparison_inside_an_assert_test_is_flagged() {
    let source = fetch_source("        assert self.assertEqual(client.fetch(), 42) is None\n");
    assert_helper_finding(&findings(&source));
}

#[test]
fn test_a_method_comparison_inside_an_if_test_is_flagged() {
    let source = fetch_source(
        "        if self.assertEqual(client.fetch(), 42) is None:\n            pass\n",
    );
    assert_helper_finding(&findings(&source));
}

#[test]
fn test_a_keyword_method_comparison_in_an_assignment_value_is_flagged() {
    let source =
        fetch_source("        result = self.assertEqual(first=client.fetch(), second=42)\n");
    assert_helper_finding(&findings(&source));
}

#[test]
fn test_a_method_comparison_nested_in_another_assertion_is_flagged() {
    let source =
        fetch_source("        self.assertEqual(self.assertEqual(client.fetch(), 42), None)\n");
    assert_helper_finding(&findings(&source));
}

#[test]
fn test_a_method_comparison_in_an_assignment_inside_a_nested_function_is_flagged() {
    let source = fetch_source(
        "        def check():\n            result = self.assertEqual(client.fetch(), 42)\n",
    );
    assert_helper_finding(&findings(&source));
}

#[test]
fn test_a_method_comparison_in_a_statement_value_still_reads_once() {
    let source = fetch_source("        self.assertEqual(client.fetch(), 42)\n");
    assert_helper_finding(&findings(&source));
}

#[test]
fn test_a_method_comparison_in_a_lambda_body_is_not_flagged() {
    let source = fetch_source("        check = lambda: self.assertEqual(client.fetch(), 42)\n");
    let found = findings(&source);
    assert!(found.is_empty(), "no finding stands: {found:?}");
}

#[test]
fn test_a_constant_spelling_in_an_assignment_value_earns_no_helper_finding() {
    let source = fetch_source("        result = self.assertTrue(True)\n");
    let found = findings(&source);
    assert_eq!(found.len(), 1, "one finding stands: {found:?}");
    assert_eq!(found[0].0, "constant_assert");
}
