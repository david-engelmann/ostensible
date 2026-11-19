//! The pins of PORT21, a type parameter name spelled
//! twice in one list, over synthetic sources the suite
//! writes itself. The seven refusing pins drive the scan
//! over a test case class carrying a flaggable constant
//! assertion beside the refused shape, and fail on the
//! head this slice branches from, where the parser reads
//! the file and the scan reports the finding. The three
//! standing pins hold the standings PORT21-2 and
//! PORT21-3 keep, on both heads.

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
fn test_a_repeated_type_parameter_in_a_definition_refuses_the_scan() {
    let source = wrap("def helper[T, T](value):\n    return value\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_repeated_type_parameter_in_a_class_refuses_the_scan() {
    let source = wrap("class Helper[T, T]:\n    pass\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_repeated_type_parameter_in_a_type_alias_refuses_the_scan() {
    let source = wrap("type Helper[T, T] = int\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_type_parameter_repeated_as_a_tuple_refuses_the_scan() {
    let source = wrap("def helper[T, *T](value):\n    return value\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_type_parameter_repeated_as_a_specification_refuses_the_scan() {
    let source = wrap("def helper[*T, **T](value):\n    return value\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_type_parameter_repeated_under_normalization_refuses_the_scan() {
    let source = wrap("def helper[T, \u{ff34}](value):\n    return value\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_a_type_parameter_repeated_at_the_end_of_a_list_refuses_the_scan() {
    let source = wrap("class Helper[A, B, A]:\n    pass\n");
    assert_eq!(refusal(&source), UNPARSEABLE);
}

#[test]
fn test_type_parameters_distinct_in_case_stand() {
    let source = wrap("def helper[T, t](value):\n    return value\n");
    let found = scan_source(&source, "test_case.py", None, None).expect("the scan reads");
    assert_eq!(found.len(), 1, "the constant finding stands: {found:?}");
    assert_eq!(found[0].kind, "constant_assert");
}

#[test]
fn test_a_single_type_parameter_stands() {
    let source = wrap("def helper[T](value):\n    return value\n");
    let found = scan_source(&source, "test_case.py", None, None).expect("the scan reads");
    assert_eq!(found.len(), 1, "the constant finding stands: {found:?}");
    assert_eq!(found[0].kind, "constant_assert");
}

#[test]
fn test_the_same_type_parameter_name_in_two_lists_stands() {
    let source = wrap("def outer[T](value):\n    def inner[T](other):\n        return other\n    return inner(value)\n");
    let found = scan_source(&source, "test_case.py", None, None).expect("the scan reads");
    assert_eq!(found.len(), 1, "the constant finding stands: {found:?}");
    assert_eq!(found[0].kind, "constant_assert");
}
