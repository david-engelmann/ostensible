//! Recorded conformance for the Round 16 port slice, the
//! mutant generation of PORT8. Every case in
//! tests/conformance/mutation_cases.json states a source text
//! and the answer the landed Python gave for it, recorded by
//! the generator at tools/gen_mutation_recordings.py and never
//! written by hand. These tests run the crate's generation
//! entry over each recorded source and compare the mutant
//! lists field for field and the sources byte for byte, and
//! the refusals against the port's recorded wording. The
//! parses and differs test of the retired Python suite
//! re-expresses here as recorded answers: each recorded mutant
//! carries whether its tree equals the original's tree, read
//! by the generator off the interpreter's own tree dump, and
//! this test asserts the same equality or difference on the
//! crate's parsed trees, the no-op float shapes standing as
//! the cases whose recorded answer is equality.

use ostensible::mutations::generate_mutants;
use ostensible::pyparse::parse_module;

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

fn load_battery() -> JVal {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/conformance/mutation_cases.json"
    );
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|_| panic!("recording missing: {}", path));
    JsonParser::parse(&text)
}

#[test]
fn mutation_battery_replays() {
    let battery = load_battery();
    let cases = battery
        .get("cases")
        .expect("battery carries cases")
        .as_list();
    let mut replayed = 0usize;
    for case in cases {
        let name = case.get("name").expect("case carries a name").as_str();
        let source = case.get("source").expect("case carries a source").as_str();
        let outcome = generate_mutants(source);
        if let Some(refusal) = case.get("refusal") {
            let error = match outcome {
                Ok(_) => panic!("{}: the crate answers where the recording refuses", name),
                Err(error) => error,
            };
            assert_eq!(
                error.message,
                refusal.as_str(),
                "{}: the refusal wording differs from the recording",
                name
            );
            replayed += 1;
            continue;
        }
        let mutants = outcome.unwrap_or_else(|error| {
            panic!(
                "{}: the crate refuses where the recording answers: {}",
                name, error.message
            )
        });
        let recorded = case.get("mutants").expect("case carries mutants").as_list();
        assert_eq!(
            mutants.len(),
            recorded.len(),
            "{}: the mutant count differs from the recording",
            name
        );
        let tree_check = case
            .get("tree_check")
            .map(|flag| flag.as_bool())
            .unwrap_or(true);
        let original = if tree_check {
            Some(parse_module(source).expect("the recorded source parses"))
        } else {
            None
        };
        for (index, (mine, expected)) in mutants.iter().zip(recorded.iter()).enumerate() {
            assert_eq!(
                mine.kind,
                expected.get("kind").unwrap().as_str(),
                "{} mutant {}: kind",
                name,
                index
            );
            assert_eq!(
                mine.line,
                expected.get("line").unwrap().as_int(),
                "{} mutant {}: line",
                name,
                index
            );
            assert_eq!(
                mine.detail,
                expected.get("detail").unwrap().as_str(),
                "{} mutant {}: detail",
                name,
                index
            );
            assert_eq!(
                mine.source,
                expected.get("source").unwrap().as_str(),
                "{} mutant {}: source",
                name,
                index
            );
            if let Some(original) = &original {
                let mutant_tree = parse_module(&mine.source).unwrap_or_else(|_| {
                    panic!("{} mutant {}: the mutant source parses", name, index)
                });
                assert_eq!(
                    mutant_tree == *original,
                    expected.get("tree_equal").unwrap().as_bool(),
                    "{} mutant {}: the tree equality answer differs from the recording",
                    name,
                    index
                );
            }
        }
        replayed += 1;
    }
    assert_eq!(
        replayed, 105,
        "the battery case count stands at the figure the closing entry of PORT8-9 states"
    );
}
