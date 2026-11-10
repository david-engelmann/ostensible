//! Pins for the CodeRabbit findings on the Round 15
//! pull request, each verified against the CPython 3.12
//! oracle before it was pinned. The battery recordings
//! cannot hold these shapes: the first turns on node id
//! assignment inside one parse, which no recorded answer
//! names, and the other two are refusals of sources the
//! generator never produced. Every expectation here is
//! the interpreter's behaviour on the same source, probed
//! directly, and each discriminating pin fails on the
//! head before the fix.

use ostensible::pyparse::parse_module;
use ostensible::pyscan::scan_source;
use ostensible::pywalk::{node_registry, walk};

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

fn ids_all_unique(source: &str) -> bool {
    let module = parse_module(source).expect("the source parses");
    node_registry(&module).len() == walk(&module).len()
}

#[test]
fn fragment_field_ids_do_not_repeat_the_modules() {
    // A field expression parsed from its fragment must
    // take ids from the module's own run. Before the fix
    // the fragment parser restarted its counter at one,
    // and this module walked 14 nodes over 11 ids.
    let source = "def test_a():\n    assert True\n\ndef helper():\n    x = f'{a.b.c}'\n";
    assert!(ids_all_unique(source));
}

#[test]
fn spec_field_ids_do_not_repeat_the_modules() {
    // The second fragment call site, the field standing
    // inside a format spec, owes the same run of ids.
    let source = "def helper():\n    x = f'{a:{b.c.d}}'\n";
    assert!(ids_all_unique(source));
}

#[test]
fn constant_assert_stands_beside_an_fstring_helper() {
    // The landed Python reports the constant assert on
    // this module. Before the fix the port reported
    // nothing: the helper's fragment nodes took the ids
    // of the assert's own nodes, the parent map keyed on
    // those ids sent the assert's climb into the helper,
    // and the gate read it as standing outside a test.
    let source = "def test_a():\n    assert True\n\ndef helper():\n    x = f'{a.b.c}'\n";
    assert_eq!(
        scan(source),
        vec![(
            "constant_assert".to_string(),
            "assertion on the constant true value".to_string()
        )]
    );
}

#[test]
fn constant_assert_stands_beside_a_plain_helper() {
    // The control: with no f-string in the helper the
    // finding stood on both heads, so the pin above
    // discriminates on the fragment parse, not the shape.
    let source = "def test_a():\n    assert True\n\ndef helper():\n    x = 'plain'\n";
    assert_eq!(scan(source).len(), 1);
}

#[test]
fn a_field_expression_counts_toward_the_port_bound() {
    // The nesting of a field expression stands inside
    // the nesting of the expression holding the string.
    // Two thousand minuses around the string and two
    // thousand inside the field pass the bound of 4,000
    // together though each side stands under it alone.
    // Before the fix the fragment parser restarted its
    // depth at zero and the parse answered.
    let source = format!("x = {}f'{{{}a}}'\n", "-".repeat(2100), "-".repeat(2000));
    match parse_module(&source) {
        Ok(_) => panic!("the parse answers past the bound"),
        Err(error) => assert_eq!(error.message, "nesting past the port bound"),
    }
}

#[test]
fn a_field_expression_inside_the_bound_still_answers() {
    // The guard against an over-wide refusal: the same
    // shape with the two sides summing under the bound
    // parses whole.
    let source = format!("x = {}f'{{{}a}}'\n", "-".repeat(2000), "-".repeat(1500));
    parse_module(&source).expect("the parse answers inside the bound");
}

#[test]
fn a_positional_without_default_after_one_with_refuses() {
    // The interpreter refuses both spellings with its
    // own text; the port refuses in its own wording
    // under the PORT7-7 family.
    for source in ["def f(a=1, b): pass\n", "x = lambda a=1, b: 0\n"] {
        let error = parse_module(source).expect_err("the parse refuses");
        assert_eq!(
            error.message,
            "a positional parameter without a default stands after one with a default"
        );
    }
}

#[test]
fn the_default_rule_spans_the_positional_only_slash() {
    // The slash splits posonly from ordinary positional
    // parameters without splitting the default rule,
    // which the interpreter reads over the combined run.
    for source in ["def f(a=1, /, b): pass\n", "def f(a, b=2, /, c): pass\n"] {
        assert!(parse_module(source).is_err(), "refuses: {source}");
    }
}

#[test]
fn the_default_rule_leaves_the_keyword_only_mix_free() {
    // Keyword only parameters mix defaults freely on
    // both sides of the rule, and a defaulted positional
    // before the star leaves the keyword only run free.
    for source in [
        "def f(a, b=2): pass\n",
        "def f(*, a=1, b): pass\n",
        "def f(a=1, *args, b): pass\n",
        "def f(a=1, *, b): pass\n",
    ] {
        parse_module(source).expect("the parse answers");
    }
}

#[test]
fn the_default_rule_refusal_reaches_the_scan() {
    // The refusal is a member of the PORT7-7 family, so
    // the scan entry returns it as a refusal rather than
    // answering for a source the interpreter refuses.
    // The scan states every parse refusal in its one
    // standing wording; the specific wording stands at
    // the parse entry, pinned above.
    let error = scan_source("def f(a=1, b): pass\n", "test_case.py", None, None)
        .expect_err("the scan refuses");
    assert_eq!(
        error.message,
        "the source does not parse under the port's reading"
    );
}

#[test]
fn a_bytes_literal_holding_a_character_past_ascii_refuses() {
    // A literal character past ASCII cannot stand in a
    // bytes literal. The interpreter refuses the shape;
    // before the fix the port accepted it and truncated
    // the character to one byte. The escaped spelling of
    // the same bytes is legal and stands as the control.
    for source in [
        "x = b'café'\n",
        "x = b'€'\n",
        "x = b'caf\u{80}'\n",
        "x = rb'café'\n",
        "x = b'''café'''\n",
        "x = b'\\é'\n",
    ] {
        let error = parse_module(source).expect_err("the parse refuses");
        assert_eq!(error.message, "character past ASCII in a bytes literal");
    }
}

#[test]
fn the_escaped_bytes_spelling_still_answers() {
    // The same byte values written as escapes are the
    // legal spelling of a bytes literal and parse whole.
    for source in [
        "x = b'caf\\xc3\\xa9'\n",
        "x = b'caf\\x7f'\n",
        "x = b'\\xff'\n",
    ] {
        parse_module(source).expect("the parse answers");
    }
}
