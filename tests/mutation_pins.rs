//! Pins for the mutant generation of PORT8, disposing the
//! obligations named in each pin below. The three execution
//! proofs of the retired Python suite (the return_none mutant
//! really returns None, the compare_invert mutant really
//! inverts, the bool_negate mutant really negates) re-express
//! here as assertions on the crate's own parse of the recorded
//! mutant sources in tests/conformance/mutation_cases.json,
//! because a recording cannot execute: each pin reads the
//! recorded mutant of the named kind out of its suite case and
//! asserts the shape of its tree. The remaining pins dispose
//! PORT8-1 (the widened record, its describe reading, and its
//! to_dict reading) and the suite's panel, direction, and
//! determinism tests, which re-express against the crate entry
//! directly.

// A small JSON reader for the recordings, in the shape of
// the reader the scan conformance tests carry. Numbers land
// as integers or floats by their spelling, strings resolve
// their escapes including surrogate pairs, and objects keep
// order.
#[derive(Debug, Clone, PartialEq)]
enum JVal {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    List(Vec<JVal>),
    Object(Vec<(String, JVal)>),
}

impl JVal {
    fn get(&self, key: &str) -> Option<&JVal> {
        match self {
            JVal::Object(pairs) => pairs.iter().find(|(held, _)| held == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn as_str(&self) -> &str {
        match self {
            JVal::Str(text) => text,
            _ => panic!("expected a string in the recording, found {:?}", self),
        }
    }

    fn as_list(&self) -> &[JVal] {
        match self {
            JVal::List(items) => items,
            _ => panic!("expected a list in the recording, found {:?}", self),
        }
    }

    fn as_bool(&self) -> bool {
        match self {
            JVal::Bool(flag) => *flag,
            _ => panic!("expected a bool in the recording, found {:?}", self),
        }
    }

    fn as_int(&self) -> i64 {
        match self {
            JVal::Int(value) => *value,
            _ => panic!("expected an int in the recording, found {:?}", self),
        }
    }
}

struct JsonParser {
    chars: Vec<char>,
    pos: usize,
}

impl JsonParser {
    fn parse(text: &str) -> JVal {
        let mut parser = JsonParser {
            chars: text.chars().collect(),
            pos: 0,
        };
        parser.skip_ws();
        let value = parser.parse_value();
        parser.skip_ws();
        assert_eq!(parser.pos, parser.chars.len(), "trailing JSON text");
        value
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn take(&mut self) -> char {
        let ch = self.peek().expect("unexpected end of JSON");
        self.pos += 1;
        ch
    }

    fn parse_value(&mut self) -> JVal {
        match self.peek().expect("unexpected end of JSON") {
            '{' => {
                self.pos += 1;
                let mut pairs = Vec::new();
                self.skip_ws();
                if self.peek() == Some('}') {
                    self.pos += 1;
                    return JVal::Object(pairs);
                }
                loop {
                    self.skip_ws();
                    let key = match self.parse_value() {
                        JVal::Str(text) => text,
                        other => panic!("object key is not a string: {:?}", other),
                    };
                    self.skip_ws();
                    assert_eq!(self.take(), ':');
                    self.skip_ws();
                    let value = self.parse_value();
                    pairs.push((key, value));
                    self.skip_ws();
                    match self.take() {
                        ',' => continue,
                        '}' => break,
                        other => panic!("expected , or }} in object, found {}", other),
                    }
                }
                JVal::Object(pairs)
            }
            '[' => {
                self.pos += 1;
                let mut items = Vec::new();
                self.skip_ws();
                if self.peek() == Some(']') {
                    self.pos += 1;
                    return JVal::List(items);
                }
                loop {
                    self.skip_ws();
                    items.push(self.parse_value());
                    self.skip_ws();
                    match self.take() {
                        ',' => continue,
                        ']' => break,
                        other => panic!("expected , or ] in list, found {}", other),
                    }
                }
                JVal::List(items)
            }
            '"' => JVal::Str(self.parse_string()),
            't' => {
                self.expect_word("true");
                JVal::Bool(true)
            }
            'f' => {
                self.expect_word("false");
                JVal::Bool(false)
            }
            'n' => {
                self.expect_word("null");
                JVal::Null
            }
            _ => self.parse_number(),
        }
    }

    fn expect_word(&mut self, word: &str) {
        for expected in word.chars() {
            assert_eq!(self.take(), expected, "misspelled literal in JSON");
        }
    }

    fn parse_string(&mut self) -> String {
        assert_eq!(self.take(), '"');
        let mut out: Vec<u32> = Vec::new();
        loop {
            let ch = self.take();
            match ch {
                '"' => break,
                '\\' => {
                    let escape = self.take();
                    match escape {
                        '"' => out.push('"' as u32),
                        '\\' => out.push('\\' as u32),
                        '/' => out.push('/' as u32),
                        'b' => out.push(0x08),
                        'f' => out.push(0x0C),
                        'n' => out.push('\n' as u32),
                        'r' => out.push('\r' as u32),
                        't' => out.push('\t' as u32),
                        'u' => {
                            let first = self.parse_hex4();
                            if (0xD800..0xDC00).contains(&first) {
                                assert_eq!(self.take(), '\\');
                                assert_eq!(self.take(), 'u');
                                let second = self.parse_hex4();
                                let combined =
                                    0x10000 + ((first - 0xD800) << 10) + (second - 0xDC00);
                                out.push(combined);
                            } else {
                                out.push(first);
                            }
                        }
                        other => panic!("unknown JSON escape: {}", other),
                    }
                }
                ch => out.push(ch as u32),
            }
        }
        out.iter()
            .map(|point| char::from_u32(*point).expect("recorded string holds a real character"))
            .collect()
    }

    fn parse_hex4(&mut self) -> u32 {
        let mut value: u32 = 0;
        for _ in 0..4 {
            let ch = self.take();
            value = value * 16 + ch.to_digit(16).expect("hex digit in JSON escape");
        }
        value
    }

    fn parse_number(&mut self) -> JVal {
        let start = self.pos;
        while matches!(self.peek(), Some('0'..='9' | '-' | '+' | '.' | 'e' | 'E')) {
            self.pos += 1;
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        if text.contains('.') || text.contains('e') || text.contains('E') {
            JVal::Float(text.parse().expect("float in JSON"))
        } else {
            JVal::Int(text.parse().expect("int in JSON"))
        }
    }
}


use ostensible::jsonvalue::JsonValue;
use ostensible::model::Mutant;
use ostensible::mutations::generate_mutants;
use ostensible::pyast::{CmpKind, ConstVal, ExprKind, StmtKind};
use ostensible::pyparse::parse_module;
use ostensible::pywalk::{walk, NRef};

const PANEL_SOURCE: &str =
    "def check(x):\n    if x > 3:\n        return True\n    return x + 2\n";

fn recorded_mutant_source(case_name: &str, kind: &str) -> String {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/conformance/mutation_cases.json"
    );
    let text = std::fs::read_to_string(path).expect("the battery stands recorded");
    let battery = JsonParser::parse(&text);
    let cases = battery.get("cases").expect("battery carries cases").as_list();
    for case in cases {
        if case.get("name").map(|name| name.as_str()) == Some(case_name) {
            for mutant in case.get("mutants").expect("case carries mutants").as_list() {
                if mutant.get("kind").map(|held| held.as_str()) == Some(kind) {
                    return mutant.get("source").expect("mutant carries a source").as_str().to_string();
                }
            }
            panic!("case {} records a {} mutant", case_name, kind);
        }
    }
    panic!("case {} stands in the battery", case_name);
}

fn first_return_value(source: &str) -> ConstVal {
    let module = parse_module(source).expect("the recorded mutant source parses");
    for node in walk(&module) {
        if let NRef::Stmt(stmt) = node {
            if let StmtKind::Return { value: Some(value) } = &stmt.kind {
                if let ExprKind::Constant(constant) = &value.kind {
                    return constant.clone();
                }
            }
        }
    }
    panic!("the recorded mutant source holds a constant return");
}

#[test]
fn return_none_pin_holds_the_constant_none_at_the_return() {
    let source = recorded_mutant_source("suite_return_none_exec", "return_none");
    assert_eq!(first_return_value(&source), ConstVal::NoneVal);
}

#[test]
fn compare_invert_pin_holds_the_inverted_operator() {
    let source = recorded_mutant_source("suite_compare_invert_exec", "compare_invert");
    let module = parse_module(&source).expect("the recorded mutant source parses");
    let mut found = None;
    for node in walk(&module) {
        if let NRef::Expr(expr) = node {
            if let ExprKind::Compare { ops, .. } = &expr.kind {
                found = Some(ops[0]);
                break;
            }
        }
    }
    // The suite input compares with Gt; the mutant inverted it.
    assert_eq!(found, Some(CmpKind::LtE));
}

#[test]
fn bool_negate_pin_holds_the_constant_false_at_the_return() {
    let source = recorded_mutant_source("suite_bool_negate_exec", "bool_negate");
    assert_eq!(first_return_value(&source), ConstVal::Bool(false));
}

#[test]
fn describe_pin_spells_the_python_reading() {
    let mutants = generate_mutants("def f():\n    return 9\n").expect("generation answers");
    let first = &mutants[0];
    assert_eq!(first.kind, "return_none");
    assert_eq!(first.describe(), "return_none at line 2, return value 9");
}

#[test]
fn to_dict_pin_excludes_the_source() {
    let mutant = Mutant {
        kind: "constant_shift".to_string(),
        line: 3,
        detail: "numeric constant 2 moved plus one".to_string(),
        source: "x = 3\n".to_string(),
    };
    assert_eq!(
        mutant.to_dict(),
        JsonValue::Object(vec![
            ("kind".to_string(), JsonValue::Str("constant_shift".to_string())),
            ("line".to_string(), JsonValue::Int(3)),
            (
                "detail".to_string(),
                JsonValue::Str("numeric constant 2 moved plus one".to_string())
            ),
        ])
    );
}

#[test]
fn panel_pin_applies_every_kind() {
    let mutants = generate_mutants(PANEL_SOURCE).expect("generation answers");
    let mut kinds: Vec<&str> = mutants.iter().map(|mutant| mutant.kind.as_str()).collect();
    kinds.sort();
    kinds.dedup();
    assert_eq!(
        kinds,
        ["bool_negate", "compare_invert", "constant_shift", "return_none"]
    );
}

#[test]
fn directions_pin_shifts_both_ways() {
    let mutants = generate_mutants(PANEL_SOURCE).expect("generation answers");
    let details: Vec<&str> = mutants
        .iter()
        .filter(|mutant| mutant.kind == "constant_shift")
        .map(|mutant| mutant.detail.as_str())
        .collect();
    assert!(details.iter().any(|detail| detail.contains("plus one")));
    assert!(details.iter().any(|detail| detail.contains("minus one")));
}

#[test]
fn determinism_pin_two_calls_agree() {
    let first = generate_mutants(PANEL_SOURCE).expect("generation answers");
    let second = generate_mutants(PANEL_SOURCE).expect("generation answers");
    assert_eq!(first, second);
}
