//! Pins for the escape readings of f-string bodies and
//! format specs, from the Round 15 fix pass. The
//! interpreter decodes the escapes of a format spec in
//! raw and plain strings alike, and a brace keeps its
//! structural reading after a backslash in a body and in
//! a spec alike. The conformance battery records none of
//! these shapes, so they are pinned directly here, each
//! against the interpreter's reading of the same source.
//! Where the port's spelling departs from the
//! interpreter's on the spec surface, the pin names the
//! port's stated spelling, the one the amendment on the
//! carrying of string constants in CONTRACT.md states.

use ostensible::pyparse::parse_module;
use ostensible::pyunparse::unparse_module;

fn module_spelling(source: &str) -> String {
    let module = parse_module(source).expect("the source parses");
    unparse_module(&module)
}

fn backslashes(count: usize) -> String {
    "\\".repeat(count)
}

#[test]
fn spec_newline_escape_decodes_and_spells_back() {
    assert_eq!(module_spelling("x = f'{v:\\n}'\n"), "x = f'{v:\\n}'");
}

#[test]
fn spec_hex_escape_decodes_to_its_character() {
    assert_eq!(module_spelling("x = f'{v:\\x41}'\n"), "x = f'{v:A}'");
}

#[test]
fn spec_quote_escape_spells_with_one_backslash() {
    assert_eq!(module_spelling("x = f'{v:\\'}'\n"), "x = f'{v:\\'}'");
}

#[test]
fn spec_undefined_escape_keeps_its_backslash() {
    // The value is a backslash and the letter after it.
    // The port spells the backslash quadrupled on this
    // surface, its stated standing, where the interpreter
    // doubles it.
    let source = ["x = f'{v:", &backslashes(1), "q}'\n"].concat();
    let expected = ["x = f'{v:", &backslashes(4), "q}'"].concat();
    assert_eq!(module_spelling(&source), expected);
}

#[test]
fn spec_backslash_pair_spells_quadrupled_in_the_port() {
    // The source spec holds two escapes, so the value is
    // two backslashes. The interpreter doubles each one
    // in its spelling and the port quadruples each one,
    // the departure the amendment states.
    let source = ["x = f\"{v:", &backslashes(4), "}\"\n"].concat();
    let expected = ["x = f'{v:", &backslashes(8), "}'"].concat();
    assert_eq!(module_spelling(&source), expected);
}

#[test]
fn spec_backslash_before_the_close_stands_alone() {
    // The closing brace after a backslash still closes
    // the spec, and the backslash stands alone in the
    // value, spelled quadrupled on this surface.
    let source = ["x = f'{v:", &backslashes(1), "}'\n"].concat();
    let expected = ["x = f'{v:", &backslashes(4), "}'"].concat();
    assert_eq!(module_spelling(&source), expected);
}

#[test]
fn spec_double_open_brace_opens_a_nested_field() {
    // A doubled opening brace inside a spec is no escape
    // there. The first brace opens a nested field and the
    // second begins its expression, an empty set display
    // here, where the body of a string reads the pair as
    // one literal brace.
    assert_eq!(module_spelling("x = f'{v:{{}}}'\n"), "x = f'{v:{ {}}}'");
}

#[test]
fn spec_open_brace_pair_left_open_refuses() {
    assert!(parse_module("x = f'{v:{{'\n").is_err());
}

#[test]
fn nested_field_spec_surrogate_spells_at_its_own_level() {
    // The surrogate stands in the spec of the nested
    // field, and that spec takes the escape form at its
    // own level, two backslashes and the escape text.
    assert_eq!(
        module_spelling("x = f'{v:{w:\\ud800}}'\n"),
        "x = f'{v:{w:\\\\ud800}}'"
    );
}

#[test]
fn raw_spec_escape_still_decodes() {
    // A raw string keeps its body text raw and still
    // decodes the escapes of its specs, the reading the
    // interpreter gives, and the spelling drops the raw
    // prefix as the interpreter's unparse does.
    assert_eq!(module_spelling("x = rf'{v:\\n}'\n"), "x = f'{v:\\n}'");
}

#[test]
fn raw_spec_name_escape_refuses() {
    // The one corner where a raw spec fails to decode in
    // the interpreter, the name escape, and the port
    // refuses the source with it.
    assert!(parse_module("x = rf'{v:\\N{LATIN CAPITAL LETTER A}}'\n").is_err());
}

#[test]
fn body_backslash_before_an_open_brace_keeps_the_field() {
    // The brace after a backslash in a body still opens
    // its field, and the backslash stands alone in the
    // literal before it.
    assert_eq!(module_spelling("x = f'\\{v}'\n"), "x = f'\\\\{v}'");
}

#[test]
fn body_backslash_before_a_close_brace_refuses() {
    // The brace after a backslash in a body still closes,
    // and a close with no field open refuses the source.
    assert!(parse_module("x = f'a\\}b'\n").is_err());
}
