//! PORT11 conformance. The recorded battery at
//! tests/conformance/cli_cases.json states synthetic projects
//! and the argument list, exit code and standard and error
//! streams the Python command line gave for each, recorded by
//! tools/gen_cli_recordings.py driving that command line before
//! it retired and never edited by hand. Every case is rebuilt
//! in a scratch folder, the port's own binary is run from that
//! folder with the recorded arguments, and the exit code and
//! both streams are compared byte for byte.

use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, PartialEq)]
enum JVal {
    Str(String),
    Int(i64),
    Bool(bool),
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

    fn as_int(&self) -> i64 {
        match self {
            JVal::Int(value) => *value,
            _ => panic!("expected an integer"),
        }
    }
}

struct Parser<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Parser<'a> {
    fn parse(text: &'a str) -> JVal {
        let mut parser = Parser {
            bytes: text.as_bytes(),
            at: 0,
        };
        let value = parser.value();
        parser.skip_space();
        assert_eq!(parser.at, parser.bytes.len(), "trailing text in battery");
        value
    }

    fn skip_space(&mut self) {
        while self.at < self.bytes.len() && self.bytes[self.at].is_ascii_whitespace() {
            self.at += 1;
        }
    }

    fn value(&mut self) -> JVal {
        self.skip_space();
        match self.bytes[self.at] {
            b'{' => {
                self.at += 1;
                let mut pairs = Vec::new();
                loop {
                    self.skip_space();
                    if self.bytes[self.at] == b'}' {
                        self.at += 1;
                        return JVal::Object(pairs);
                    }
                    let key = match self.value() {
                        JVal::Str(text) => text,
                        _ => panic!("object key is not a string"),
                    };
                    self.skip_space();
                    assert_eq!(self.bytes[self.at], b':');
                    self.at += 1;
                    let value = self.value();
                    pairs.push((key, value));
                    self.skip_space();
                    if self.bytes[self.at] == b',' {
                        self.at += 1;
                    }
                }
            }
            b'[' => {
                self.at += 1;
                let mut items = Vec::new();
                loop {
                    self.skip_space();
                    if self.bytes[self.at] == b']' {
                        self.at += 1;
                        return JVal::List(items);
                    }
                    items.push(self.value());
                    self.skip_space();
                    if self.bytes[self.at] == b',' {
                        self.at += 1;
                    }
                }
            }
            b'"' => JVal::Str(self.string()),
            b't' => {
                self.at += 4;
                JVal::Bool(true)
            }
            b'f' => {
                self.at += 5;
                JVal::Bool(false)
            }
            _ => {
                let start = self.at;
                while self.at < self.bytes.len()
                    && (self.bytes[self.at].is_ascii_digit()
                        || matches!(self.bytes[self.at], b'-' | b'+' | b'.' | b'e' | b'E'))
                {
                    self.at += 1;
                }
                let text = std::str::from_utf8(&self.bytes[start..self.at]).expect("number text");
                JVal::Int(text.parse().expect("integer in battery"))
            }
        }
    }

    fn string(&mut self) -> String {
        assert_eq!(self.bytes[self.at], b'"');
        self.at += 1;
        let mut out = String::new();
        loop {
            let byte = self.bytes[self.at];
            self.at += 1;
            match byte {
                b'"' => return out,
                b'\\' => {
                    let escape = self.bytes[self.at];
                    self.at += 1;
                    match escape {
                        b'n' => out.push('\n'),
                        b't' => out.push('\t'),
                        b'r' => out.push('\r'),
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'u' => {
                            let hex = std::str::from_utf8(&self.bytes[self.at..self.at + 4])
                                .expect("hex text");
                            let code = u32::from_str_radix(hex, 16).expect("hex value");
                            self.at += 4;
                            out.push(char::from_u32(code).expect("unicode escape"));
                        }
                        _ => panic!("unknown escape in battery"),
                    }
                }
                _ => {
                    let mut buffer = [0u8; 4];
                    buffer[0] = byte;
                    let width = if byte >= 0xF0 {
                        3
                    } else if byte >= 0xE0 {
                        2
                    } else if byte >= 0xC0 {
                        1
                    } else {
                        0
                    };
                    for slot in buffer.iter_mut().skip(1).take(width) {
                        *slot = self.bytes[self.at];
                        self.at += 1;
                    }
                    out.push_str(
                        std::str::from_utf8(&buffer[..width + 1]).expect("utf8 in battery"),
                    );
                }
            }
        }
    }
}

fn battery() -> JVal {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("conformance")
        .join("cli_cases.json");
    let text = std::fs::read_to_string(&path).expect("the cli battery reads");
    Parser::parse(&text)
}

#[test]
fn test_the_cli_battery_replays_byte_for_byte() {
    let battery = battery();
    assert_eq!(
        battery.get("format").map(|v| v.as_str()),
        Some("ostensible-port11-cases/1")
    );
    let cases = battery.get("cases").expect("cases stand").as_list();
    assert_eq!(cases.len(), 5, "the battery carries five cases");
    let scratch_root: PathBuf =
        std::env::temp_dir().join(format!("ostensible-port11-replay-{}", std::process::id()));
    for case in cases {
        let name = case.get("name").expect("case name").as_str();
        let scratch = scratch_root.join(name);
        std::fs::create_dir_all(&scratch).expect("case scratch builds");
        let files = case.get("files").expect("case files");
        let argv: Vec<String> = case
            .get("argv")
            .expect("case argv")
            .as_list()
            .iter()
            .map(|v| v.as_str().to_string())
            .collect();
        if let JVal::Object(entries) = files {
            let folder = scratch.join(&argv[0]);
            for (rel, content) in entries {
                let target = folder.join(rel);
                if let Some(parent) = target.parent() {
                    std::fs::create_dir_all(parent).expect("case folders build");
                }
                std::fs::write(&target, content.as_str()).expect("case files write");
            }
        }
        let output = Command::new(env!("CARGO_BIN_EXE_ostensible"))
            .args(&argv)
            .current_dir(&scratch)
            .output()
            .expect("the door runs");
        assert_eq!(
            output.status.code(),
            Some(case.get("exit").expect("case exit").as_int() as i32),
            "case {name} exit"
        );
        assert_eq!(
            output.stdout,
            case.get("stdout").expect("case stdout").as_str().as_bytes(),
            "case {name} stdout"
        );
        assert_eq!(
            output.stderr,
            case.get("stderr").expect("case stderr").as_str().as_bytes(),
            "case {name} stderr"
        );
        std::fs::remove_dir_all(&scratch).expect("case scratch clears");
    }
    let _ = std::fs::remove_dir_all(&scratch_root);
    let _ = Path::new("unused");
}
