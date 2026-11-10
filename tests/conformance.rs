//! Recorded conformance for the Rust port slice 1. Every case in
//! the recordings under tests/conformance/ states an input and
//! the answer the landed Python gave for it, and these tests
//! build the input again and compare the crate answer against
//! the recording exactly. The recordings were produced by the
//! probe generator in the round scratch, never written by hand.

use ostensible::ini::IniLoad;
use ostensible::jsonvalue::JsonValue;
use ostensible::model::{
    aggregate_outcomes, Analysis, FileVerdict, FunctionVerdict, Mutant, MutantOutcome,
    StaticFinding,
};
use ostensible::pyvalue::{Offset, PyValue};
use ostensible::settings::{
    file_matches, load_collection_settings_with_source, name_matches, split_words,
};
use ostensible::{report, settings};
use std::path::PathBuf;

// A small JSON reader for the recordings. Numbers land as
// integers or floats by their spelling, strings resolve their
// escapes including surrogate pairs, and objects keep order.
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

    fn parse_value(&mut self) -> JVal {
        match self.peek().expect("JSON value") {
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
                        JVal::Str(key) => key,
                        _ => panic!("object key must be a string"),
                    };
                    self.skip_ws();
                    assert_eq!(self.peek(), Some(':'));
                    self.pos += 1;
                    self.skip_ws();
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
                        other => panic!("bad object tail {:?}", other),
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
                    self.skip_ws();
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
                        other => panic!("bad array tail {:?}", other),
                    }
                }
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
        for ch in word.chars() {
            assert_eq!(self.peek(), Some(ch), "bad literal");
            self.pos += 1;
        }
    }

    fn parse_string(&mut self) -> String {
        assert_eq!(self.peek(), Some('"'));
        self.pos += 1;
        let mut out = String::new();
        loop {
            let ch = self.chars[self.pos];
            self.pos += 1;
            match ch {
                '"' => return out,
                '\\' => {
                    let escaped = self.chars[self.pos];
                    self.pos += 1;
                    match escaped {
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
                                assert_eq!(self.chars[self.pos], '\\');
                                assert_eq!(self.chars[self.pos + 1], 'u');
                                self.pos += 2;
                                let second = self.parse_hex4();
                                let code = 0x10000
                                    + (((first - 0xd800) as u32) << 10)
                                    + (second - 0xdc00) as u32;
                                out.push(char::from_u32(code).expect("astral code"));
                            } else {
                                match char::from_u32(first as u32) {
                                    Some(ch) => out.push(ch),
                                    None => panic!("lone surrogate in recording"),
                                }
                            }
                        }
                        other => panic!("bad escape {}", other),
                    }
                }
                ch => out.push(ch),
            }
        }
    }

    fn parse_hex4(&mut self) -> u16 {
        let mut value: u16 = 0;
        for _ in 0..4 {
            let digit = self.chars[self.pos].to_digit(16).expect("hex digit");
            self.pos += 1;
            value = value * 16 + digit as u16;
        }
        value
    }

    fn parse_number(&mut self) -> JVal {
        let start = self.pos;
        while matches!(self.peek(), Some(ch) if ch.is_ascii_digit() || matches!(ch, '-' | '+' | '.' | 'e' | 'E')) {
            self.pos += 1;
        }
        let text: String = self.chars[start..self.pos].iter().collect();
        if text.contains('.') || text.contains('e') || text.contains('E') {
            JVal::Float(text.parse().expect("float"))
        } else {
            JVal::Int(text.parse().expect("int"))
        }
    }
}

impl JVal {
    fn get(&self, key: &str) -> &JVal {
        match self {
            JVal::Object(pairs) => pairs
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value)
                .unwrap_or_else(|| panic!("missing key {}", key)),
            _ => panic!("not an object"),
        }
    }

    fn as_str(&self) -> &str {
        match self {
            JVal::Str(text) => text,
            _ => panic!("not a string: {:?}", self),
        }
    }

    fn as_list(&self) -> &[JVal] {
        match self {
            JVal::List(items) => items,
            _ => panic!("not a list"),
        }
    }

    fn as_bool(&self) -> bool {
        match self {
            JVal::Bool(flag) => *flag,
            _ => panic!("not a bool"),
        }
    }

    fn as_int(&self) -> i64 {
        match self {
            JVal::Int(number) => *number,
            _ => panic!("not an int"),
        }
    }

    fn is_null(&self) -> bool {
        matches!(self, JVal::Null)
    }
}

fn load_recording(path: &str) -> JVal {
    let text = std::fs::read_to_string(path)
        .unwrap_or_else(|_| panic!("recording missing: {}", path));
    JsonParser::parse(&text)
}

// The report recordings.

fn build_outcome(spec: &JVal) -> MutantOutcome {
    MutantOutcome {
        source_path: spec.get("source_path").as_str().to_string(),
        mutant: Mutant {
            kind: spec.get("kind").as_str().to_string(),
            line: spec.get("line").as_int(),
            detail: spec.get("detail").as_str().to_string(),
        },
        killed: spec.get("killed").as_bool(),
        timed_out: spec.get("timed_out").as_bool(),
    }
}

fn build_function(spec: &JVal) -> FunctionVerdict {
    FunctionVerdict {
        name: spec.get("name").as_str().to_string(),
        baseline_ok: spec.get("baseline_ok").as_bool(),
        outcomes: spec
            .get("outcomes")
            .as_list()
            .iter()
            .map(build_outcome)
            .collect(),
        baseline_timed_out: spec.get("baseline_timed_out").as_bool(),
    }
}

fn build_verdict(spec: &JVal) -> FileVerdict {
    FileVerdict {
        path: spec.get("path").as_str().to_string(),
        test_functions: spec
            .get("test_functions")
            .as_list()
            .iter()
            .map(|item| item.as_str().to_string())
            .collect(),
        baseline_ok: spec.get("baseline_ok").as_bool(),
        outcomes: spec
            .get("outcomes")
            .as_list()
            .iter()
            .map(build_outcome)
            .collect(),
        functions: spec
            .get("functions")
            .as_list()
            .iter()
            .map(build_function)
            .collect(),
        whole_file: spec.get("whole_file").as_bool(),
        no_tests: spec.get("no_tests").as_bool(),
        partial_selection: spec.get("partial_selection").as_bool(),
    }
}

fn build_analysis(spec: &JVal) -> Analysis {
    Analysis {
        project: spec.get("project").as_str().to_string(),
        static_findings: spec
            .get("static_findings")
            .as_list()
            .iter()
            .map(|item| StaticFinding {
                kind: item.get("kind").as_str().to_string(),
                path: item.get("path").as_str().to_string(),
                line: item.get("line").as_int(),
                detail: item.get("detail").as_str().to_string(),
            })
            .collect(),
        verdicts: spec
            .get("verdicts")
            .as_list()
            .iter()
            .map(build_verdict)
            .collect(),
        collection_settings: spec
            .get("collection_settings")
            .as_list()
            .iter()
            .map(|pair| {
                let pair = pair.as_list();
                (
                    pair[0].as_str().to_string(),
                    pair[1]
                        .as_list()
                        .iter()
                        .map(|item| item.as_str().to_string())
                        .collect(),
                )
            })
            .collect(),
        settings_source: match spec.get("settings_source") {
            JVal::Null => None,
            value => Some(value.as_str().to_string()),
        },
    }
}

fn canon(value: &JsonValue) -> JVal {
    match value {
        JsonValue::Str(text) => JVal::Str(text.clone()),
        JsonValue::Int(number) => JVal::Int(*number),
        JsonValue::Bool(flag) => JVal::Bool(*flag),
        JsonValue::List(items) => JVal::List(items.iter().map(canon).collect()),
        JsonValue::Object(pairs) => {
            let mut pairs: Vec<(String, JVal)> = pairs
                .iter()
                .map(|(key, item)| (key.clone(), canon(item)))
                .collect();
            pairs.sort_by(|a, b| a.0.cmp(&b.0));
            JVal::Object(pairs)
        }
    }
}

fn canon_jval(value: &JVal) -> JVal {
    match value {
        JVal::List(items) => JVal::List(items.iter().map(canon_jval).collect()),
        JVal::Object(pairs) => {
            let mut pairs: Vec<(String, JVal)> = pairs
                .iter()
                .map(|(key, item)| (key.clone(), canon_jval(item)))
                .collect();
            pairs.sort_by(|a, b| a.0.cmp(&b.0));
            JVal::Object(pairs)
        }
        other => other.clone(),
    }
}

#[test]
fn report_recordings_render_byte_identical() {
    let doc = load_recording(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/conformance/report_cases.json"
    ));
    let cases = doc.get("cases").as_list();
    assert!(cases.len() >= 20, "the report set thinned out");
    for case in cases {
        let name = case.get("name").as_str();
        let analysis = build_analysis(case.get("given"));
        assert_eq!(
            report::to_json(&analysis),
            case.get("expected_json").as_str(),
            "json differs in case {}",
            name
        );
        assert_eq!(
            report::to_text(&analysis),
            case.get("expected_text").as_str(),
            "text differs in case {}",
            name
        );
    }
}

#[test]
fn aggregate_recordings_fold_as_recorded() {
    let doc = load_recording(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/conformance/report_cases.json"
    ));
    for case in doc.get("aggregate_cases").as_list() {
        let name = case.get("name").as_str();
        let functions: Vec<FunctionVerdict> = case
            .get("given_functions")
            .as_list()
            .iter()
            .map(build_function)
            .collect();
        let folded = aggregate_outcomes(&functions);
        let rendered = JsonValue::List(
            folded.iter().map(|item| item.to_dict()).collect(),
        );
        assert_eq!(
            canon(&rendered),
            canon_jval(case.get("expected_outcomes")),
            "fold differs in case {}",
            name
        );
    }
}

// The settings recordings.

fn base64_decode(text: &str) -> Vec<u8> {
    fn value(ch: u8) -> u8 {
        match ch {
            b'A'..=b'Z' => ch - b'A',
            b'a'..=b'z' => ch - b'a' + 26,
            b'0'..=b'9' => ch - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => panic!("bad base64"),
        }
    }
    let bytes: Vec<u8> = text.bytes().filter(|b| *b != b'=').collect();
    let mut out = Vec::new();
    for chunk in bytes.chunks(4) {
        let mut acc: u32 = 0;
        for (index, byte) in chunk.iter().enumerate() {
            acc |= (value(*byte) as u32) << (18 - 6 * index);
        }
        let keep = match chunk.len() {
            4 => 3,
            3 => 2,
            2 => 1,
            _ => 0,
        };
        let all = acc.to_be_bytes();
        out.extend_from_slice(&all[1..1 + keep]);
    }
    out
}

fn fresh_base(tag: &str) -> PathBuf {
    let base = std::env::temp_dir().join(format!(
        "ostensible-port1-{}-{}",
        std::process::id(),
        tag
    ));
    if base.exists() {
        std::fs::remove_dir_all(&base).unwrap();
    }
    std::fs::create_dir_all(&base).unwrap();
    base
}

fn expected_setting(spec: &JVal, key: &str) -> Option<Vec<String>> {
    match spec.get(key) {
        JVal::Null => None,
        value => Some(
            value
                .as_list()
                .iter()
                .map(|item| item.as_str().to_string())
                .collect(),
        ),
    }
}

#[test]
fn settings_load_recordings_resolve_as_recorded() {
    let doc = load_recording(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/conformance/settings_cases.json"
    ));
    let cases = doc.get("load_cases").as_list();
    assert!(cases.len() >= 40, "the load set thinned out");
    for (index, case) in cases.iter().enumerate() {
        let name = case.get("name").as_str();
        let base = fresh_base(&format!("load{}", index));
        match case.get("files") {
            JVal::Object(files) => {
                for (relative, content) in files {
                    let target = base.join(relative);
                    std::fs::create_dir_all(target.parent().unwrap()).unwrap();
                    match content {
                        JVal::Str(text) => {
                            std::fs::write(&target, text).unwrap()
                        }
                        JVal::Object(spec) => {
                            if let Some(encoded) =
                                spec.iter().find(|(k, _)| k == "b64")
                            {
                                let bytes =
                                    base64_decode(encoded.1.as_str());
                                std::fs::write(&target, bytes).unwrap();
                            } else if let Some(link) =
                                spec.iter().find(|(k, _)| k == "link")
                            {
                                #[cfg(unix)]
                                std::os::unix::fs::symlink(
                                    link.1.as_str(),
                                    &target,
                                )
                                .unwrap();
                            } else {
                                panic!("bad file spec in {}", name);
                            }
                        }
                        _ => panic!("bad file content in {}", name),
                    }
                }
            }
            _ => panic!("files must be an object"),
        }
        let ask = base.join(case.get("ask").as_str());
        std::fs::create_dir_all(&ask).unwrap();
        let (found, source) = load_collection_settings_with_source(&ask);
        let expected = case.get("expected_settings");
        for key in settings::KEYS {
            assert_eq!(
                found.get(key).cloned(),
                expected_setting(expected, key),
                "setting {} differs in case {}",
                key,
                name
            );
        }
        let non_defaults: Vec<(String, Vec<String>)> = case
            .get("expected_non_defaults")
            .as_list()
            .iter()
            .map(|pair| {
                let pair = pair.as_list();
                (
                    pair[0].as_str().to_string(),
                    pair[1]
                        .as_list()
                        .iter()
                        .map(|item| item.as_str().to_string())
                        .collect(),
                )
            })
            .collect();
        assert_eq!(
            found.non_defaults(),
            non_defaults,
            "non defaults differ in case {}",
            name
        );
        let expected_source = case.get("expected_source");
        if expected_source.is_null() {
            assert!(
                source.is_none(),
                "source should be none in case {} but was {:?}",
                name,
                source
            );
        } else {
            let wanted = expected_source
                .as_str()
                .replace("{BASE}", &base.to_string_lossy());
            assert_eq!(
                source.map(|path| path.to_string_lossy().into_owned()),
                Some(wanted),
                "source differs in case {}",
                name
            );
        }
        std::fs::remove_dir_all(&base).ok();
    }
}

#[test]
fn name_and_file_recordings_match_as_recorded() {
    let doc = load_recording(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/conformance/settings_cases.json"
    ));
    for case in doc.get("name_cases").as_list() {
        let patterns: Vec<String> = case
            .get("patterns")
            .as_list()
            .iter()
            .map(|item| item.as_str().to_string())
            .collect();
        assert_eq!(
            name_matches(case.get("name").as_str(), &patterns),
            case.get("expected").as_bool(),
            "name case {:?} against {:?}",
            case.get("name").as_str(),
            patterns
        );
    }
    for case in doc.get("file_cases").as_list() {
        let patterns: Vec<String> = case
            .get("patterns")
            .as_list()
            .iter()
            .map(|item| item.as_str().to_string())
            .collect();
        assert_eq!(
            file_matches(
                case.get("relative").as_str(),
                case.get("filename").as_str(),
                &patterns
            ),
            case.get("expected").as_bool(),
            "file case {:?}",
            case.get("relative").as_str()
        );
    }
}

fn spec_to_value(spec: &JVal) -> PyValue {
    match spec.get("t").as_str() {
        "str" => PyValue::Str(spec.get("v").as_str().to_string()),
        "int" => PyValue::Int(spec.get("v").as_str().to_string()),
        "float" => PyValue::Float(spec.get("v").as_str().parse().unwrap()),
        "bool" => PyValue::Bool(spec.get("v").as_bool()),
        "list" => PyValue::List(
            spec.get("v").as_list().iter().map(spec_to_value).collect(),
        ),
        "table" => PyValue::Table(
            spec.get("v")
                .as_list()
                .iter()
                .map(|pair| {
                    let pair = pair.as_list();
                    (pair[0].as_str().to_string(), spec_to_value(&pair[1]))
                })
                .collect(),
        ),
        "date" => {
            let parts = spec.get("v").as_list();
            PyValue::Date {
                year: parts[0].as_int(),
                month: parts[1].as_int() as u32,
                day: parts[2].as_int() as u32,
            }
        }
        "time" => {
            let parts = spec.get("v").as_list();
            PyValue::Time {
                hour: parts[0].as_int() as u32,
                minute: parts[1].as_int() as u32,
                second: parts[2].as_int() as u32,
                micros: parts[3].as_int() as u32,
            }
        }
        "datetime" => {
            let parts = spec.get("v").as_list();
            let offset = match &parts[7] {
                JVal::Null => None,
                JVal::Str(text) if text == "utc" => Some(Offset::Utc),
                JVal::Int(seconds) => Some(Offset::FixedSeconds(*seconds as i32)),
                _ => panic!("bad offset spec"),
            };
            PyValue::DateTime {
                year: parts[0].as_int(),
                month: parts[1].as_int() as u32,
                day: parts[2].as_int() as u32,
                hour: parts[3].as_int() as u32,
                minute: parts[4].as_int() as u32,
                second: parts[5].as_int() as u32,
                micros: parts[6].as_int() as u32,
                offset,
            }
        }
        other => panic!("bad spec kind {}", other),
    }
}

#[test]
fn split_recordings_split_as_recorded() {
    let doc = load_recording(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/conformance/settings_cases.json"
    ));
    for case in doc.get("split_cases").as_list() {
        let value = spec_to_value(case.get("spec"));
        let expected: Vec<String> = case
            .get("expected")
            .as_list()
            .iter()
            .map(|item| item.as_str().to_string())
            .collect();
        assert_eq!(
            split_words(&value),
            expected,
            "split differs for spec {}",
            case.get("spec").get("t").as_str()
        );
    }
}

#[test]
fn ini_refusals_and_readings_agree_with_the_parser() {
    // A second door into the ini reading beside the settings
    // recordings, pinning the refusal shape beside the read shape
    // at the level the port claims.
    let base = fresh_base("ini");
    let good = base.join("good.ini");
    std::fs::write(&good, "[pytest]\npython_functions = qa_\n").unwrap();
    match ostensible::ini::load_ini(&good) {
        IniLoad::Parsed(parser) => {
            assert!(parser.has_section("pytest"));
            assert_eq!(
                parser.get("pytest", "python_functions").as_deref(),
                Some("qa_")
            );
        }
        IniLoad::Refused => panic!("a plain file refused"),
    }
    let broken = base.join("broken.ini");
    std::fs::write(&broken, "[pytest]\na = 1\na = 2\n").unwrap();
    assert!(matches!(
        ostensible::ini::load_ini(&broken),
        IniLoad::Refused
    ));
    let missing = base.join("missing.ini");
    match ostensible::ini::load_ini(&missing) {
        IniLoad::Parsed(parser) => assert!(!parser.has_section("pytest")),
        IniLoad::Refused => panic!("a missing file must read as empty"),
    }
    std::fs::remove_dir_all(&base).ok();
}
