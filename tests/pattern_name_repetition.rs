//! The pins of PORT22, a capture name spelled twice in
//! one pattern, over synthetic sources the suite writes
//! itself. The twelve refusing pins drive the scan over a
//! test case class carrying a flaggable constant
//! assertion beside the refused shape, and fail on the
//! head this slice branches from, where the parser reads
//! the file and the scan reports the finding. The four
//! standing pins hold the standings PORT22-1 and PORT22-3
//! keep, on both heads. A fifth standing pin, named for
//! alternatives binding different name sets standing
//! under this slice, stood here until PORT23 took that
//! judgement and retired it, as PORT23's build amendment
//! records.

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

fn stands(source: &str) {
    let found = scan_source(source, "test_case.py", None, None).expect("the scan reads");
    assert_eq!(found.len(), 1, "the constant finding stands: {found:?}");
    assert_eq!(found[0].kind, "constant_assert");
}

#[test]
fn test_a_capture_name_spelled_twice_in_a_sequence_pattern_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case [x, x]:\n            return x\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_capture_name_spelled_plainly_and_starred_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case [x, *x]:\n            return x\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_capture_name_repeated_at_two_nesting_levels_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case [[x], x]:\n            return x\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_capture_name_spelled_twice_in_a_mapping_pattern_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case {\"a\": x, \"b\": x}:\n            return x\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_capture_name_spelled_twice_in_a_class_pattern_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case SomeClass(a=x, b=x):\n            return x\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_mapping_rest_name_repeating_a_capture_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case {\"a\": x, **x}:\n            return x\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_an_as_name_repeating_a_capture_of_its_own_pattern_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case [x] as x:\n            return x\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_capture_name_repeated_under_normalization_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case {\"a\": \u{212a}, \"b\": K}:\n            return K\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_name_repeated_inside_one_alternative_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case [x, x] | [y]:\n            return y\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_an_as_name_outside_an_alternation_repeating_a_capture_inside_it_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case ([x] | [y]) as x:\n            return x\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_capture_name_repeated_across_sibling_alternations_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case [([x] | (x,)), ([x] | (x,))]:\n            return x\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_starred_name_spelled_twice_refuses_the_scan() {
    let source = wrap("def helper(value):\n    match value:\n        case [*x, *x]:\n            return x\n    return None\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_wildcard_repeated_in_a_pattern_stands() {
    let source = wrap("def helper(value):\n    match value:\n        case [_, _]:\n            return 1\n    return None\n");
    stands(&source);
}

#[test]
fn test_the_same_name_in_different_alternatives_stands() {
    let source = wrap("def helper(value):\n    match value:\n        case [x] | [x]:\n            return x\n    return None\n");
    stands(&source);
}

#[test]
fn test_the_same_name_in_different_cases_stands() {
    let source = wrap("def helper(value):\n    match value:\n        case [x]:\n            return x\n        case [x, y]:\n            return y\n    return None\n");
    stands(&source);
}

#[test]
fn test_two_starred_names_in_one_sequence_stand_under_this_slice() {
    let source = wrap("def helper(value):\n    match value:\n        case [*x, *y]:\n            return x\n    return None\n");
    stands(&source);
}
