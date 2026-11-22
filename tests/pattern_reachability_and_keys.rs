//! The pins of PORT24, the unreachable alternative and
//! the duplicated mapping key, over synthetic sources the
//! suite writes itself. The fourteen refusing pins drive
//! the scan over a test case class carrying a flaggable
//! constant assertion beside the refused shape, and fail
//! on the head this slice branches from, where the parser
//! reads the file and the scan reports the finding. The
//! eight standing pins hold the standings PORT24-1,
//! PORT24-2 and PORT24-4 keep, on both heads.

use ostensible::pyscan::scan_source;

const UNPARSEABLE: &str = "the source does not parse under the port's reading";

fn wrap(helper: &str) -> String {
    format!(
        "import unittest\n\n\n{helper}\n\nclass TestReachability(unittest.TestCase):\n    def test_reachability(self):\n        self.assertTrue(True)\n"
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
fn test_a_wildcard_alternative_ahead_of_a_literal_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case _ | 1:\n            return 0\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_capture_ahead_of_an_alternative_binding_the_same_name_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case y | [y]:\n            return y\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_wildcard_alternative_ahead_of_another_wildcard_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case _ | _:\n            return 0\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_an_unreachable_alternative_inside_a_sequence_pattern_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case [(_ | 1), x]:\n            return x\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_an_unreachable_alternative_inside_a_class_pattern_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case Point(y | [y]):\n            return y\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_an_as_pattern_wrapping_a_capture_ahead_of_its_siblings_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case (y as z) | [y, z]:\n            return z\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_capture_alternative_standing_between_agreeing_alternatives_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case [y] | y | [y]:\n            return y\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_string_key_spelled_twice_in_one_mapping_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case {\"a\": y, 'a': z}:\n            return y\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_an_integer_key_against_a_boolean_key_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case {1: y, True: z}:\n            return y\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_an_integer_key_against_its_float_spelling_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case {0x10: y, 16.0: z}:\n            return y\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_negative_integer_key_spelled_twice_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case {-1: y, -1: z}:\n            return y\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_complex_key_against_its_respelling_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case {1j: y, 1.0j: z}:\n            return y\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_duplicated_key_inside_a_nested_mapping_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case {\"k\": {\"a\": y, \"a\": z}}:\n            return y\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_the_none_key_spelled_twice_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case {None: y, None: z}:\n            return y\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_wildcard_alternative_standing_last_stands() {
    let source = wrap("def helper(value):\n    match value:\n        case 1 | _:\n            return 0\n    return None\n");
    stands(&source);
}

#[test]
fn test_a_capture_alternative_standing_last_stands() {
    let source = wrap("def helper(value):\n    match value:\n        case [y] | y:\n            return y\n    return None\n");
    stands(&source);
}

#[test]
fn test_an_as_pattern_wrapping_a_literal_stands() {
    let source = wrap("def helper(value):\n    match value:\n        case (1 as z) | [z]:\n            return z\n    return None\n");
    stands(&source);
}

#[test]
fn test_two_literal_alternatives_stand() {
    let source = wrap("def helper(value):\n    match value:\n        case 1 | 2:\n            return 0\n    return None\n");
    stands(&source);
}

#[test]
fn test_a_dotted_key_spelled_twice_stands() {
    let source = wrap("def helper(value):\n    match value:\n        case {C.A: y, C.A: z}:\n            return y\n    return None\n");
    stands(&source);
}

#[test]
fn test_an_integer_key_against_a_complex_key_stands() {
    let source = wrap("def helper(value):\n    match value:\n        case {1: y, 1j: z}:\n            return y\n    return None\n");
    stands(&source);
}

#[test]
fn test_a_string_key_against_a_bytes_key_stands() {
    let source = wrap("def helper(value):\n    match value:\n        case {\"a\": y, b\"a\": z}:\n            return y\n    return None\n");
    stands(&source);
}

#[test]
fn test_the_same_key_in_the_alternatives_of_one_pattern_stands() {
    let source = wrap("def helper(value):\n    match value:\n        case {\"a\": y} | {\"a\": y}:\n            return y\n    return None\n");
    stands(&source);
}
