//! The pin of PORT9-4. The crate carries one string repr
//! implementation under PORT9-1, and this pin spells a
//! panel of strings through the two surfaces that reach
//! it by different roads: the value world's repr, through
//! the value constructors, and the unparse of a module
//! returning each string as a literal, through the parser
//! and the unparse. The two spellings must agree character
//! for character; the pin fails wherever a future change
//! re-forks the reading or misroutes either surface.

use ostensible::pyparse::parse_module;
use ostensible::pyunparse::unparse_module;
use ostensible::pyvalue::{py_repr, PyValue};

/// Spell a string as source text the parser decodes back
/// to the same string, escaping every character outside
/// the plain letters, digits, and space by its codepoint,
/// so the source form never depends on the spelling under
/// test.
fn source_form(text: &str) -> String {
    let mut out = String::new();
    for ch in text.chars() {
        let cp = ch as u32;
        if ch.is_ascii_alphanumeric() || ch == ' ' {
            out.push(ch);
        } else if cp < 0x100 {
            out.push_str(&format!("\\x{:02x}", cp));
        } else if cp < 0x10000 {
            out.push_str(&format!("\\u{:04x}", cp));
        } else {
            out.push_str(&format!("\\U{:08x}", cp));
        }
    }
    out
}

fn panel() -> Vec<String> {
    vec![
        String::new(),
        "plain".to_string(),
        "it's".to_string(),
        "say \"hi\"".to_string(),
        "both ' and \"".to_string(),
        "back\\slash".to_string(),
        "new\nline".to_string(),
        "tab\there".to_string(),
        "carriage\rreturn".to_string(),
        "\u{0}\u{1b}\u{7f}\u{a0}\u{ad}\u{2028}\u{2029}\u{3000}\u{e000}\u{f8ff}\u{fffd}".to_string(),
        "\u{10000}\u{10ffff}\u{1f600}".to_string(),
        "trailing space ".to_string(),
    ]
}

#[test]
fn test_value_repr_and_unparse_repr_agree_on_a_panel() {
    for text in panel() {
        let through_value = py_repr(&PyValue::Str(text.clone()));
        let source = format!("VALUE = '{}'", source_form(&text));
        let module = parse_module(&source).expect("the panel source parses");
        let through_unparse = unparse_module(&module);
        assert_eq!(
            through_unparse,
            format!("VALUE = {}", through_value),
            "the two surfaces spell {text:?} differently"
        );
    }
}
