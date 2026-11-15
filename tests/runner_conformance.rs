//! PORT10 conformance. The recorded battery at
//! tests/conformance/runner_cases.json states synthetic projects
//! and the analysis dictionary the restored Python machinery
//! returned for each, recorded by
//! tests/conformance/gen_runner_recordings.py and never edited
//! by hand. Every case is rebuilt and analysed by the port, and
//! the analysis dictionary is compared field for field and value
//! for value against the recording, both sides rendered in the
//! canonical sorted spelling. The analysed project must also
//! stand byte identical after the run.

use ostensible::analyse_project;
use ostensible::jsonvalue::{dumps_sorted_indented, JsonValue};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, PartialEq)]
enum JVal {
    Str(String),
    Int(i64),
    Float(f64),
    Bool(bool),
    Null,
    List(Vec<JVal>),
    Object(Vec<(String, JVal)>),
}

impl JVal {
    fn get(&self, key: &str) -> Option<&JVal> {
        match self {
            JVal::Object(pairs) => pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    fn as_str(&self) -> &str {
        match self {
            JVal::Str(text) => text,
            _ => panic!("expected a string"),
        }
    }

    fn as_list(&self) -> &[JVal] {
        match self {
            JVal::List(items) => items,
            _ => panic!("expected a list"),
        }
    }

    fn as_f64(&self) -> f64 {
        match self {
            JVal::Float(value) => *value,
            JVal::Int(value) => *value as f64,
            _ => panic!("expected a number"),
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

    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(' ' | '\t' | '\n' | '\r')) {
            self.pos += 1;
        }
    }

    fn parse_value(&mut self) -> JVal {
        self.skip_ws();
        match self.peek().expect("value expected") {
            '{' => {
                self.pos += 1;
                let mut pairs = Vec::new();
                self.skip_ws();
                if self.peek() == Some('}') {
                    self.pos += 1;
                    return JVal::Object(pairs);
                }
                loop {
                    let key = match self.parse_value() {
                        JVal::Str(text) => text,
                        _ => panic!("object key must be a string"),
                    };
                    self.skip_ws();
                    assert_eq!(self.peek(), Some(':'), "colon expected");
                    self.pos += 1;
                    let value = self.parse_value();
                    pairs.push((key, value));
                    self.skip_ws();
                    match self.peek() {
                        Some(',') => {
                            self.pos += 1;
                        }
                        Some('}') => {
                            self.pos += 1;
                            return JVal::Object(pairs);
                        }
                        other => panic!("unexpected object continuation: {:?}", other),
                    }
                }
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
                    items.push(self.parse_value());
                    self.skip_ws();
                    match self.peek() {
                        Some(',') => {
                            self.pos += 1;
                        }
                        Some(']') => {
                            self.pos += 1;
                            return JVal::List(items);
                        }
                        other => panic!("unexpected list continuation: {:?}", other),
                    }
                }
            }
            '"' => JVal::Str(self.parse_string()),
            't' => {
                self.pos += 4;
                JVal::Bool(true)
            }
            'f' => {
                self.pos += 5;
                JVal::Bool(false)
            }
            'n' => {
                self.pos += 4;
                JVal::Null
            }
            _ => self.parse_number(),
        }
    }

    fn parse_string(&mut self) -> String {
        assert_eq!(self.peek(), Some('"'));
        self.pos += 1;
        let mut out = String::new();
        loop {
            let ch = self.peek().expect("string runs to the end");
            self.pos += 1;
            match ch {
                '"' => return out,
                '\\' => {
                    let esc = self.peek().expect("escape expected");
                    self.pos += 1;
                    match esc {
                        '"' => out.push('"'),
                        '\\' => out.push('\\'),
                        '/' => out.push('/'),
                        'b' => out.push('\u{8}'),
                        'f' => out.push('\u{c}'),
                        'n' => out.push('\n'),
                        'r' => out.push('\r'),
                        't' => out.push('\t'),
                        'u' => {
                            let first = self.parse_hex4();
                            if (0xd800..0xdc00).contains(&first) {
                                assert_eq!(self.peek(), Some('\\'));
                                self.pos += 1;
                                assert_eq!(self.peek(), Some('u'));
                                self.pos += 1;
                                let second = self.parse_hex4();
                                let code = 0x10000u32
                                    + (((first as u32) - 0xd800) << 10)
                                    + ((second as u32) - 0xdc00);
                                out.push(char::from_u32(code).expect("valid code point"));
                            } else {
                                out.push(char::from_u32(first as u32).expect("valid code point"));
                            }
                        }
                        other => panic!("bad escape: {}", other),
                    }
                }
                ch => out.push(ch),
            }
        }
    }

    fn parse_hex4(&mut self) -> u16 {
        let text: String = self.chars[self.pos..self.pos + 4].iter().collect();
        self.pos += 4;
        u16::from_str_radix(&text, 16).expect("hex escape")
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

fn to_json_value(value: &JVal) -> JsonValue {
    match value {
        JVal::Str(text) => JsonValue::Str(text.clone()),
        JVal::Int(number) => JsonValue::Int(*number),
        JVal::Bool(flag) => JsonValue::Bool(*flag),
        JVal::List(items) => JsonValue::List(items.iter().map(to_json_value).collect()),
        JVal::Object(pairs) => JsonValue::Object(
            pairs
                .iter()
                .map(|(key, item)| (key.clone(), to_json_value(item)))
                .collect(),
        ),
        JVal::Float(_) | JVal::Null => {
            panic!("the recorded analysis dictionary carries no float or null")
        }
    }
}

fn load_battery() -> JVal {
    let path = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/conformance/runner_cases.json"
    );
    let text =
        std::fs::read_to_string(path).unwrap_or_else(|_| panic!("recording missing: {}", path));
    JsonParser::parse(&text)
}

fn write_project(root: &Path, files: &JVal) {
    let pairs = match files {
        JVal::Object(pairs) => pairs,
        _ => panic!("case files must be an object"),
    };
    for (relative, text) in pairs {
        let target = root.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).expect("project folders build");
        }
        std::fs::write(&target, text.as_str()).expect("project files write");
    }
}

fn project_bytes(root: &Path) -> Vec<(String, Vec<u8>)> {
    let mut found = Vec::new();
    let mut pending = vec![root.to_path_buf()];
    while let Some(folder) = pending.pop() {
        for entry in std::fs::read_dir(&folder).expect("project reads") {
            let entry = entry.expect("project entry reads");
            let path = entry.path();
            if entry.file_type().expect("entry kind").is_dir() {
                pending.push(path);
            } else {
                let relative = path
                    .strip_prefix(root)
                    .expect("under the project")
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned())
                    .collect::<Vec<String>>()
                    .join("/");
                found.push((relative, std::fs::read(&path).expect("file reads")));
            }
        }
    }
    found.sort();
    found
}

#[test]
fn runner_battery_replays() {
    let battery = load_battery();
    assert_eq!(
        battery
            .get("format")
            .expect("battery carries a format")
            .as_str(),
        "ostensible-port10-cases/1"
    );
    let cases = battery
        .get("cases")
        .expect("battery carries cases")
        .as_list();
    assert_eq!(cases.len(), 10, "the battery's own case count");
    let interpreter = PathBuf::from(
        std::env::var("OSTENSIBLE_TEST_PYTHON").unwrap_or_else(|_| "python3".to_string()),
    );
    let scratch =
        std::env::temp_dir().join(format!("ostensible-port10-replay-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).expect("replay scratch builds");
    let mut replayed = 0usize;
    for case in cases {
        let name = case.get("name").expect("case carries a name").as_str();
        let timeout = case
            .get("timeout")
            .expect("case carries a timeout")
            .as_f64();
        let files = case.get("files").expect("case carries files");
        let project = scratch.join(name);
        std::fs::create_dir_all(&project).expect("case project builds");
        write_project(&project, files);
        let before = project_bytes(&project);
        let analysis = analyse_project(&project, timeout, &interpreter).unwrap_or_else(|error| {
            panic!(
                "{}: the port refuses where the recording answers: {}",
                name, error.message
            )
        });
        let expected = to_json_value(case.get("expected").expect("case carries the expected"));
        assert_eq!(
            dumps_sorted_indented(&analysis.to_dict()),
            dumps_sorted_indented(&expected),
            "{}: the analysis differs from the recording",
            name
        );
        let after = project_bytes(&project);
        assert_eq!(before, after, "{}: the analysed project moved", name);
        replayed += 1;
    }
    let _ = std::fs::remove_dir_all(&scratch);
    assert_eq!(replayed, 10);
}
