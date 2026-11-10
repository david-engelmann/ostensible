//! Pins for the surrogate corners the conformance battery
//! cannot record, from the Round 15 fix pass. The battery
//! writes its recordings as UTF-8 text, so a source whose
//! recorded spelling would hold a raw lone surrogate, the
//! format spec literal of a field, can never stand in it.
//! These shapes are pinned directly instead, with the
//! spellings the interpreter gives for the same sources.
//! What the port spells on that one surface is the escape
//! form, the value carried whole and the spelling apart
//! from every literal backslash text, as the amendment to
//! PORT7-7 in CONTRACT.md states.

use ostensible::pyparse::parse_module;
use ostensible::pyscan::scan_source;
use ostensible::pyunparse::unparse_module;

fn module_spelling(source: &str) -> String {
    let module = parse_module(source).expect("the source parses");
    unparse_module(&module)
}

fn scan(source: &str) -> Vec<(String, String)> {
    scan_source(source, "test_case.py", None, None)
        .map(|findings| {
            findings
                .iter()
                .map(|finding| (finding.kind.clone(), finding.detail.clone()))
                .collect()
        })
        .expect("the scan answers")
}

#[test]
fn spec_literal_spells_the_escape_form() {
    // The interpreter's own unparse of this source carries
    // the raw codepoint in the spec. The port's spelling
    // carries two backslashes and the escape text, the
    // corner the amendment names. The value is the
    // surrogate either way.
    assert_eq!(
        module_spelling("x = f'{value:\\ud800}'\n"),
        "x = f'{value:\\\\ud800}'"
    );
}

#[test]
fn spec_backslash_text_spells_apart_from_the_surrogate() {
    // The spec value here is the six characters of the
    // escape text itself. Its spelling carries four
    // backslashes before the same letters, the port's
    // standing on this surface, so the text value never
    // reads as the surrogate's spelling.
    assert_eq!(
        module_spelling("x = f'{value:\\\\ud800}'\n"),
        "x = f'{value:\\\\\\\\ud800}'"
    );
}

#[test]
fn spec_nested_field_literal_spells_the_escape_form() {
    assert_eq!(
        module_spelling("x = f'{value:{width}\\ud800}'\n"),
        "x = f'{value:{width}\\\\ud800}'"
    );
}

#[test]
fn r05c_control_flags_on_neither_side() {
    // A helper configuring a genuine U+10000 against an
    // assert of the two surrogate escapes that would spell
    // it: the values differ, so no finding stands.
    let source = "def helper():\n    m.fetch.return_value = '\\U00010000'\n    return m\n\n\ndef test_a():\n    svc = helper()\n    assert svc.fetch() == '\\ud800\\udc00'\n";
    assert_eq!(scan(source), Vec::new());
}

#[test]
fn concatenated_fold_keeps_the_surrogate_apart() {
    // Implicit concatenation folds the two literals into
    // one constant whose spelling keeps the surrogate in
    // escape form beside the raw replacement character,
    // and the mock reading flags the folded value.
    let source = "def helper():\n    m.fetch.return_value = '\\ud800' '\u{fffd}'\n    return m\n\n\ndef test_a():\n    svc = helper()\n    assert svc.fetch() == '\\ud800\u{fffd}'\n";
    let findings = scan(source);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].0, "mock_return_value");
    assert!(
        findings[0].1.ends_with("'\\ud800\u{fffd}'"),
        "detail spells the folded constant: {}",
        findings[0].1
    );
}

#[test]
fn r05d_configured_surrogate_against_genuine_replacement_flags_nothing() {
    // The red team's r05d shape, the one place the
    // narrowing changed an answer set. A helper configures
    // the escape form and the assert holds one genuine
    // replacement character. The spellings differ, so no
    // finding stands, the reading the interpreter gives.
    let source = "def make():\n    m = Fake()\n    m.get.return_value = '\\ud800'\n    return m\n\ndef test_a():\n    svc = make()\n    assert svc.get() == '\u{fffd}'\n";
    assert_eq!(scan(source), Vec::new());
}

#[test]
fn docstring_spells_the_surrogate_in_escape_form() {
    let source = "def test_a():\n    \"\"\"\\ud800 doc.\"\"\"\n    assert True\n";
    assert_eq!(
        module_spelling(source),
        "def test_a():\n    \"\"\"\\ud800 doc.\"\"\"\n    assert True"
    );
}
