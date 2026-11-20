//! The pins of PORT23, the alternatives of one pattern
//! binding different names, over synthetic sources the
//! suite writes itself. The ten refusing pins drive the
//! scan over a test case class carrying a flaggable
//! constant assertion beside the refused shape, and fail
//! on the head this slice branches from, where the parser
//! reads the file and the scan reports the finding. The
//! seven standing pins hold the standings PORT23-1 and
//! PORT23-3 keep, on both heads.

use ostensible::pyscan::scan_source;

const UNPARSEABLE: &str = "the source does not parse under the port's reading";

fn wrap(helper: &str) -> String {
    format!(
        "import unittest\n\n\n{helper}\n\nclass TestAlternatives(unittest.TestCase):\n    def test_alternatives(self):\n        self.assertTrue(True)\n"
    )
}

fn refusal(source: &str) -> String {
    scan_source(source, "test_case.py", None, None)
        .expect_err("the scan refuses")
        .message
}

fn stands(source: &str) {
    let found = scan_source(source, "test_case.py", None, None).expect("the scan reads");
    assert_eq!(found.len(), 1, "the constant finding stands: {found:?}");
    assert_eq!(found[0].kind, "constant_assert");
}

#[test]
fn test_alternatives_binding_different_names_in_sequence_patterns_refuse_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case [x] | [y]:\n            return x\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_alternatives_binding_different_counts_of_names_refuse_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case [x] | [x, y]:\n            return x\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_an_alternative_binding_no_name_against_one_binding_a_name_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case [] | [x]:\n            return x\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_value_alternative_against_a_capture_alternative_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case 1 | [x]:\n            return x\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_mapping_alternative_against_a_sequence_alternative_binding_another_name_refuses_the_scan(
) {
    let source = wrap("def helper(value):\n    match value:\n        case {\"a\": x} | [y]:\n            return x\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_alternatives_differing_under_an_as_name_refuse_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case ([x] | [y]) as z:\n            return z\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_as_names_inside_alternatives_differing_refuse_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case ([x] as w) | ([x] as v):\n            return w\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_nested_alternation_lending_its_names_to_a_differing_outer_alternative_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case [[x] | [x]] | [y]:\n            return x\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_alternatives_differing_after_normalization_refuse_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case [x] | [\u{212a}]:\n            return x\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_wildcard_alternative_against_a_capture_alternative_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case [x] | _:\n            return x\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_alternatives_binding_the_same_name_stand() {
    let source = wrap("def helper(value):\n    match value:\n        case [x] | [x]:\n            return x\n    return None\n");
    stands(&source);
}

#[test]
fn test_alternatives_binding_the_same_names_in_the_other_order_stand() {
    let source = wrap("def helper(value):\n    match value:\n        case [x, y] | [y, x]:\n            return x\n    return None\n");
    stands(&source);
}

#[test]
fn test_a_mapping_alternative_and_a_sequence_alternative_binding_the_same_name_stand() {
    let source = wrap("def helper(value):\n    match value:\n        case {\"a\": x} | [x]:\n            return x\n    return None\n");
    stands(&source);
}

#[test]
fn test_a_starred_name_and_a_plain_name_binding_the_same_name_stand() {
    let source = wrap("def helper(value):\n    match value:\n        case [*x] | [x]:\n            return x\n    return None\n");
    stands(&source);
}

#[test]
fn test_alternatives_agreeing_after_normalization_stand() {
    let source = wrap("def helper(value):\n    match value:\n        case [K] | [\u{212a}]:\n            return K\n    return None\n");
    stands(&source);
}

#[test]
fn test_a_bare_capture_standing_last_among_equals_stands() {
    let source = wrap("def helper(value):\n    match value:\n        case [x] | x:\n            return x\n    return None\n");
    stands(&source);
}

#[test]
fn test_alternatives_binding_no_names_stand() {
    let source = wrap("def helper(value):\n    match value:\n        case 1 | 2:\n            return 1\n    return None\n");
    stands(&source);
}
