//! The pins of PORT14, the false and none assertion
//! spellings, over synthetic sources the suite writes
//! itself. Each flagged pin drives the scan over a test
//! case class whose constant assertion is spelled as a
//! method call, and the boundary pins beside them hold
//! the lines PORT14-3 draws, the identity reading, the
//! set aside spellings, and the governing rule.

use ostensible::pyscan::scan_source;

fn findings(source: &str) -> Vec<(String, String)> {
    scan_source(source, "test_case.py", None, None)
        .expect("the scan reads")
        .into_iter()
        .map(|finding| (finding.kind, finding.detail))
        .collect()
}

#[test]
fn test_an_assert_false_on_the_constant_false_is_flagged() {
    let source = "import unittest\n\n\nclass TestConstant(unittest.TestCase):\n    def test_constant(self):\n        self.assertFalse(False)\n";
    let found = findings(source);
    assert_eq!(found.len(), 1, "one finding stands: {found:?}");
    assert_eq!(found[0].0, "constant_assert");
    assert_eq!(found[0].1, "assertion on the constant false value");
}

#[test]
fn test_an_assert_is_none_on_the_constant_none_is_flagged() {
    let source = "import unittest\n\n\nclass TestConstant(unittest.TestCase):\n    def test_constant(self):\n        self.assertIsNone(None)\n";
    let found = findings(source);
    assert_eq!(found.len(), 1, "one finding stands: {found:?}");
    assert_eq!(found[0].0, "constant_assert");
    assert_eq!(found[0].1, "assertion on the constant none value");
}

#[test]
fn test_an_assert_false_with_a_message_argument_is_flagged() {
    // The message argument stands outside the reading, as
    // it does for the truth method, and the site stands
    // over the first positional argument alone.
    let source = "import unittest\n\n\nclass TestConstant(unittest.TestCase):\n    def test_constant(self):\n        self.assertFalse(False, \"the flag stands clear\")\n";
    let found = findings(source);
    assert_eq!(found.len(), 1, "one finding stands: {found:?}");
    assert_eq!(found[0].0, "constant_assert");
    assert_eq!(found[0].1, "assertion on the constant false value");
}

#[test]
fn test_an_assert_is_none_with_a_message_argument_is_flagged() {
    let source = "import unittest\n\n\nclass TestConstant(unittest.TestCase):\n    def test_constant(self):\n        self.assertIsNone(None, \"the result stands missing\")\n";
    let found = findings(source);
    assert_eq!(found.len(), 1, "one finding stands: {found:?}");
    assert_eq!(found[0].0, "constant_assert");
    assert_eq!(found[0].1, "assertion on the constant none value");
}

#[test]
fn test_an_assert_false_on_zero_is_not_flagged() {
    // The constant zero passes an assertFalse at runtime
    // as a falsy constant, and the identity reading takes
    // no notice of it, as it takes no notice of the
    // constant one at the truth method.
    let source = "import unittest\n\n\nclass TestConstant(unittest.TestCase):\n    def test_constant(self):\n        self.assertFalse(0)\n";
    let found = findings(source);
    assert!(found.is_empty(), "no finding stands: {found:?}");
}

#[test]
fn test_an_assert_false_on_a_value_is_not_flagged() {
    let source = "import unittest\n\n\nclass TestConstant(unittest.TestCase):\n    def test_constant(self):\n        flag = compute_flag()\n        self.assertFalse(flag)\n";
    let found = findings(source);
    assert!(found.is_empty(), "no finding stands: {found:?}");
}

#[test]
fn test_an_assert_is_none_on_a_value_is_not_flagged() {
    let source = "import unittest\n\n\nclass TestConstant(unittest.TestCase):\n    def test_constant(self):\n        result = compute_result()\n        self.assertIsNone(result)\n";
    let found = findings(source);
    assert!(found.is_empty(), "no finding stands: {found:?}");
}

#[test]
fn test_an_assert_is_not_none_on_a_constant_is_not_flagged() {
    // The negation of the none method names no constant
    // by identity and stands outside the reading whole,
    // over constants as over values.
    let source = "import unittest\n\n\nclass TestConstant(unittest.TestCase):\n    def test_constant(self):\n        self.assertIsNotNone(5)\n";
    let found = findings(source);
    assert!(found.is_empty(), "no finding stands: {found:?}");
}

#[test]
fn test_an_assert_not_equal_comparing_a_value_with_itself_is_not_flagged() {
    // The comparison sites stand for the equality reading
    // alone, and a negation over a self comparison names
    // the opposite defect from the one the scan reports.
    let source = "import unittest\n\n\nclass TestTotal(unittest.TestCase):\n    def test_total(self):\n        total = 5\n        self.assertNotEqual(total, total)\n";
    let found = findings(source);
    assert!(found.is_empty(), "no finding stands: {found:?}");
}

#[test]
fn test_an_assert_false_outside_a_test_function_is_not_flagged() {
    let source = "import unittest\n\n\nclass HelperChecks(unittest.TestCase):\n    def check_constant(self):\n        self.assertFalse(False)\n";
    let found = findings(source);
    assert!(found.is_empty(), "no finding stands: {found:?}");
}

#[test]
fn test_an_assert_false_with_no_argument_forms_no_site() {
    let source = "import unittest\n\n\nclass TestConstant(unittest.TestCase):\n    def test_constant(self):\n        self.assertFalse()\n";
    let found = findings(source);
    assert!(found.is_empty(), "no finding stands: {found:?}");
}

#[test]
fn test_an_assert_is_none_with_no_argument_forms_no_site() {
    let source = "import unittest\n\n\nclass TestConstant(unittest.TestCase):\n    def test_constant(self):\n        self.assertIsNone()\n";
    let found = findings(source);
    assert!(found.is_empty(), "no finding stands: {found:?}");
}
