//! Pins for the lone surrogate carrying of the Round 15 fix
//! pass. Every standing here was probed against the landed
//! Python at f51c07c^ before the fix: a lone surrogate in a
//! string constant stands as its own value, a real U+FFFD
//! stands as itself, and the two never compare equal, in
//! constant values, in unparse spellings, and in finding
//! details alike.

use ostensible::pyparse::parse_module;
use ostensible::pyscan::scan_source;
use ostensible::pyunparse::unparse_module;

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

fn module_spelling(source: &str) -> String {
    let module = parse_module(source).expect("the source parses");
    unparse_module(&module)
}

const HELPER_SURROGATE: &str = r"def helper():
    m.fetch.return_value = '\ud800'
    return m


def test_a():
    svc = helper()
";

#[test]
fn cross_pair_earns_no_finding() {
    let source = format!("{}    assert svc.fetch() == '\\ufffd'\n", HELPER_SURROGATE);
    assert_eq!(scan(&source), Vec::new());
}

#[test]
fn same_surrogate_flags_with_the_surrogate_spelled() {
    let source = format!("{}    assert svc.fetch() == '\\ud800'\n", HELPER_SURROGATE);
    let findings = scan(&source);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].0, "mock_return_value");
    assert!(
        findings[0].1.ends_with(r"'\ud800'"),
        "detail spells the surrogate as CPython spells it: {}",
        findings[0].1
    );
    assert!(
        !findings[0].1.contains('\u{fffd}'),
        "detail carries no replacement character: {}",
        findings[0].1
    );
}

#[test]
fn true_replacement_character_flags_as_itself() {
    let source = "def helper():\n    m.fetch.return_value = '\\ufffd'\n    return m\n\n\ndef test_a():\n    svc = helper()\n    assert svc.fetch() == '\\ufffd'\n";
    let findings = scan(source);
    assert_eq!(findings.len(), 1);
    assert!(
        findings[0].1.ends_with("'\u{fffd}'"),
        "detail carries the replacement character itself: {}",
        findings[0].1
    );
}

#[test]
fn lim01_shape_flags_only_the_surrogate_assert() {
    let source = format!(
        "{}    assert svc.fetch() == '\\ufffd'\n    assert svc.fetch() == '\\ud800'\n",
        HELPER_SURROGATE
    );
    let findings = scan(&source);
    assert_eq!(findings.len(), 1);
    assert!(findings[0].1.ends_with(r"'\ud800'"));
}

#[test]
fn self_comparison_reads_surrogates_apart() {
    let cross = "def test_a():\n    assert '\\ud800' == '\\ufffd'\n";
    assert_eq!(scan(cross), Vec::new());
    let same = "def test_a():\n    assert '\\ud800' == '\\ud800'\n";
    let findings = scan(same);
    assert_eq!(findings.len(), 1);
    assert_eq!(findings[0].0, "self_comparison");
    assert_eq!(findings[0].1, r"assertion compares '\ud800' with itself");
}

#[test]
fn surrogate_pair_escape_reads_as_two_lone_surrogates() {
    let same = r"def test_a():
    assert '\ud83d\ude00' == '\ud83d\ude00'
";
    let findings = scan(same);
    assert_eq!(findings.len(), 1);
    assert_eq!(
        findings[0].1,
        r"assertion compares '\ud83d\ude00' with itself"
    );
    let versus_emoji = "def test_a():\n    assert '\\ud83d\\ude00' == '\\U0001F600'\n";
    assert_eq!(scan(versus_emoji), Vec::new());
}

#[test]
fn unparse_spellings_carry_the_surrogate_apart() {
    assert_eq!(module_spelling("x = '\\ud800'\n"), r"x = '\ud800'");
    assert_eq!(module_spelling("x = '\\ufffd'\n"), "x = '\u{fffd}'");
    assert_eq!(
        module_spelling("x = '\\ud83d\\ude00'\n"),
        r"x = '\ud83d\ude00'"
    );
    assert_eq!(module_spelling("x = f'\\ud800'\n"), r"x = f'\ud800'");
    assert_eq!(
        module_spelling("x = 'a\\ud800b\\ufffd'\n"),
        "x = 'a\\ud800b\u{fffd}'"
    );
}
