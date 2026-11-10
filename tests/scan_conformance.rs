//! Recorded conformance for the Round 15 port slice, the
//! static scan and the source parser it carries. Every case
//! in tests/conformance/scan_cases.json states an input and
//! the answers the landed Python gave for it, recorded by
//! the generator in the round scratch and never written by
//! hand. These tests build the inputs again and compare the
//! crate answers against the recordings exactly, finding
//! lists field for field, item lists word for word, and
//! unparse spellings byte for byte.

use ostensible::model::StaticFinding;
use ostensible::pyscan::{scan_file, scan_source, test_function_names, top_level_test_functions};
use ostensible::pyselect::{has_unselected_test_shapes, selectable_test_items};
use ostensible::pyunparse::{unparse_expr, unparse_module};
use ostensible::pywalk::{walk, NRef};
use ostensible::settings::CollectionSettings;

// A small JSON reader for the recordings, in the shape of
// the reader the slice 1 conformance tests carry. Numbers
// land as integers or floats by their spelling, strings
// resolve their escapes including surrogate pairs, and
// objects keep order.
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

    fn is_refused(&self) -> bool {
        self.get("refused").is_some()
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
                        _ => panic!("object keys are strings"),
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
                        other => panic!("unexpected object separator {}", other),
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
                        other => panic!("unexpected list separator {}", other),
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
        for ch in word.chars() {
            assert_eq!(self.take(), ch);
        }
    }

    fn parse_string(&mut self) -> String {
        assert_eq!(self.take(), '"');
        let mut out = String::new();
        loop {
            let ch = self.take();
            match ch {
                '"' => break,
                '\\' => {
                    let escape = self.take();
                    match escape {
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
                                assert_eq!(self.take(), '\\');
                                assert_eq!(self.take(), 'u');
                                let second = self.parse_hex4();
                                let combined = 0x10000
                                    + (((first - 0xd800) as u32) << 10)
                                    + (second as u32 - 0xdc00);
                                out.push(char::from_u32(combined).expect("valid surrogate pair"));
                            } else {
                                out.push(
                                    char::from_u32(first as u32)
                                        .expect("valid codepoint in recording"),
                                );
                            }
                        }
                        other => panic!("unexpected escape {}", other),
                    }
                }
                _ => out.push(ch),
            }
        }
        out
    }

    fn parse_hex4(&mut self) -> u16 {
        let mut value: u16 = 0;
        for _ in 0..4 {
            let ch = self.take();
            value = value * 16 + ch.to_digit(16).expect("hex digit") as u16;
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
            JVal::Float(text.parse().expect("float in recording"))
        } else {
            JVal::Int(text.parse().expect("int in recording"))
        }
    }
}

fn load_recording() -> JVal {
    let path = format!(
        "{}/tests/conformance/scan_cases.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("the recording at {} does not read: {}", path, error));
    JsonParser::parse(&text)
}

fn build_settings(record: Option<&JVal>) -> Option<CollectionSettings> {
    let record = record?;
    if matches!(record, JVal::Null) {
        return None;
    }
    let words = |key: &str| -> Option<Vec<String>> {
        match record.get(key) {
            Some(JVal::Null) | None => None,
            Some(JVal::List(items)) => {
                Some(items.iter().map(|item| item.as_str().to_string()).collect())
            }
            _ => panic!("settings words are lists or null"),
        }
    };
    Some(CollectionSettings {
        python_functions: words("python_functions"),
        python_classes: words("python_classes"),
        python_files: words("python_files"),
    })
}

fn supply_of(case: &JVal) -> Option<Vec<String>> {
    match case.get("supply") {
        Some(JVal::Null) | None => None,
        Some(JVal::List(items)) => {
            Some(items.iter().map(|item| item.as_str().to_string()).collect())
        }
        _ => panic!("supply is a list or null"),
    }
}

fn compare_findings(actual: &[StaticFinding], expected: &JVal, context: &str) {
    let expected = expected.as_list();
    assert_eq!(
        actual.len(),
        expected.len(),
        "{}: finding count differs, crate gave {:?}",
        context,
        actual
    );
    for (index, (mine, recorded)) in actual.iter().zip(expected.iter()).enumerate() {
        assert_eq!(
            mine.kind,
            recorded.get("kind").unwrap().as_str(),
            "{}: finding {} kind",
            context,
            index
        );
        assert_eq!(
            mine.path,
            recorded.get("path").unwrap().as_str(),
            "{}: finding {} path",
            context,
            index
        );
        assert_eq!(
            mine.line,
            recorded.get("line").unwrap().as_int(),
            "{}: finding {} line",
            context,
            index
        );
        assert_eq!(
            mine.detail,
            recorded.get("detail").unwrap().as_str(),
            "{}: finding {} detail",
            context,
            index
        );
    }
}

fn strings_of(record: &JVal, key: &str) -> Vec<String> {
    record
        .get(key)
        .unwrap()
        .as_list()
        .iter()
        .map(|item| item.as_str().to_string())
        .collect()
}

#[test]
fn scan_cases_replay() {
    let recording = load_recording();
    let sources = recording.get("sources").unwrap().as_list();
    let cases = recording.get("cases").unwrap().as_list();
    let mut checked = 0usize;
    for (index, case) in cases.iter().enumerate() {
        let family = case.get("family").unwrap().as_str();
        let context = format!("case {} of family {}", index, family);
        let source_record = &sources[case.get("source").unwrap().as_int() as usize];
        let source = source_record.get("text").unwrap().as_str();
        let settings = build_settings(case.get("settings"));
        let settings_ref = settings.as_ref();
        let port_bound = case
            .get("port_bound_refusal")
            .map(|flag| flag.as_bool())
            .unwrap_or(false);
        if let Some(scan) = case.get("scan") {
            let path = case.get("path").unwrap().as_str();
            let supply = supply_of(case);
            let result = scan_source(source, path, settings_ref, supply.as_deref());
            if port_bound || scan.is_refused() {
                assert!(
                    result.is_err(),
                    "{}: scan must refuse, crate gave {:?}",
                    context,
                    result.map(|findings| findings.len())
                );
            } else {
                let findings = result.unwrap_or_else(|error| {
                    panic!("{}: scan refused unexpectedly: {}", context, error.message)
                });
                compare_findings(&findings, scan.get("findings").unwrap(), &context);
            }
        }
        if let Some(items) = case.get("items") {
            let result = selectable_test_items(source, settings_ref);
            if port_bound || items.is_refused() {
                assert!(result.is_err(), "{}: selector must refuse", context);
            } else {
                let actual = result.unwrap_or_else(|error| {
                    panic!(
                        "{}: selector refused unexpectedly: {}",
                        context, error.message
                    )
                });
                assert_eq!(actual, strings_of(items, "items"), "{}: items", context);
            }
        }
        if let Some(shapes) = case.get("shapes") {
            let used = strings_of(shapes, "items");
            let result = has_unselected_test_shapes(source, &used, settings_ref);
            if port_bound || shapes.is_refused() {
                assert!(result.is_err(), "{}: shapes must refuse", context);
            } else {
                let actual = result.unwrap_or_else(|error| {
                    panic!(
                        "{}: shapes refused unexpectedly: {}",
                        context, error.message
                    )
                });
                assert_eq!(
                    actual,
                    shapes.get("answer").unwrap().as_bool(),
                    "{}: shapes answer",
                    context
                );
            }
        }
        if let Some(names) = case.get("names") {
            let result = test_function_names(source);
            if port_bound || names.is_refused() {
                assert!(result.is_err(), "{}: names must refuse", context);
            } else {
                let actual = result.unwrap_or_else(|error| {
                    panic!("{}: names refused unexpectedly: {}", context, error.message)
                });
                assert_eq!(actual, strings_of(names, "names"), "{}: names", context);
            }
        }
        if let Some(top) = case.get("top") {
            let result = top_level_test_functions(source);
            if port_bound || top.is_refused() {
                assert!(result.is_err(), "{}: top level must refuse", context);
            } else {
                let actual = result.unwrap_or_else(|error| {
                    panic!(
                        "{}: top level refused unexpectedly: {}",
                        context, error.message
                    )
                });
                assert_eq!(actual, strings_of(top, "names"), "{}: top level", context);
            }
        }
        checked += 1;
    }
    assert!(checked > 400, "the battery carries {} cases", checked);
}

/// The spellings of one source under the port, the module
/// whole, the first recorded spellings' worth of expression
/// spellings in walk order, and the full expression count,
/// computed on a deep stack thread because the deep sources
/// recurse as far as they nest. Err stands for a parse
/// refusal under the port.
fn spellings_of(source: &str, keep: usize) -> Result<(String, Vec<String>, usize), ()> {
    let text = source.to_string();
    let worker = std::thread::Builder::new()
        .stack_size(512 * 1024 * 1024)
        .spawn(move || {
            let module = ostensible::pyparse::parse_module(&text).map_err(|_| ())?;
            let module_spelling = unparse_module(&module);
            let mut spellings: Vec<String> = Vec::new();
            let mut total = 0usize;
            for node in walk(&module) {
                if let NRef::Expr(expr) = node {
                    total += 1;
                    if spellings.len() < keep {
                        spellings.push(unparse_expr(expr));
                    }
                }
            }
            Ok((module_spelling, spellings, total))
        })
        .expect("spelling thread");
    worker.join().expect("spelling thread finished")
}

#[test]
fn unparse_cases_replay() {
    let recording = load_recording();
    let sources = recording.get("sources").unwrap().as_list();
    let mut spellings = 0usize;
    for (index, source_record) in sources.iter().enumerate() {
        let context = format!("source {}", index);
        let source = source_record.get("text").unwrap().as_str();
        let port_bound = source_record
            .get("port_bound_refusal")
            .map(|flag| flag.as_bool())
            .unwrap_or(false);
        let expected_exprs = source_record
            .get("exprs")
            .map(|exprs| exprs.as_list().len())
            .unwrap_or(0);
        let outcome = spellings_of(source, expected_exprs);
        if source_record.is_refused() || port_bound {
            assert!(
                outcome.is_err(),
                "{}: the source must refuse under the port",
                context
            );
            continue;
        }
        let (module_spelling, actual_exprs, actual_total) =
            outcome.unwrap_or_else(|_| panic!("{}: parse refused unexpectedly", context));
        let expected_module = source_record.get("module").unwrap().as_str();
        assert_eq!(
            module_spelling, expected_module,
            "{}: module spelling",
            context
        );
        let expected = source_record.get("exprs").unwrap().as_list();
        if expected.len() < 600 {
            // Below the generator's per source spelling cap
            // the expression count itself is pinned. At the
            // cap the recordings carry the first spellings
            // only, and the port's count may run past them.
            assert_eq!(
                actual_total,
                expected.len(),
                "{}: expression count",
                context
            );
        } else {
            assert!(
                actual_total >= expected.len(),
                "{}: expression count below the recorded cap",
                context
            );
        }
        for (position, (mine, recorded)) in actual_exprs.iter().zip(expected.iter()).enumerate() {
            assert_eq!(
                mine,
                recorded.as_str(),
                "{}: expression {} spelling",
                context,
                position
            );
            spellings += 1;
        }
    }
    assert!(
        spellings > 1000,
        "the battery carries {} spellings",
        spellings
    );
}

#[test]
fn scan_file_cases_replay() {
    let recording = load_recording();
    let file_cases = recording.get("file_cases").unwrap().as_list();
    let scratch = std::env::temp_dir().join("ostensible-scan-conformance");
    std::fs::create_dir_all(&scratch).expect("scratch dir for scan file cases");
    for (index, case) in file_cases.iter().enumerate() {
        let family = case.get("family").unwrap().as_str();
        let context = format!("file case {} of family {}", index, family);
        let bytes: Vec<u8> = case
            .get("bytes")
            .unwrap()
            .as_list()
            .iter()
            .map(|byte| byte.as_int() as u8)
            .collect();
        let path = scratch.join(format!("case_{}.py", index));
        std::fs::write(&path, &bytes).expect("write the recorded bytes");
        let settings = build_settings(case.get("settings"));
        let supply = supply_of(case);
        let expected = case.get("scan_file").unwrap();
        let result = scan_file(
            &path,
            case.get("display").unwrap().as_str(),
            settings.as_ref(),
            supply.as_deref(),
        );
        if expected.is_refused() {
            assert!(
                result.is_err(),
                "{}: scan_file must refuse, crate gave {:?}",
                context,
                result.map(|findings| findings.len())
            );
        } else {
            let findings = result.unwrap_or_else(|error| {
                panic!(
                    "{}: scan_file refused unexpectedly: {}",
                    context, error.message
                )
            });
            compare_findings(&findings, expected.get("findings").unwrap(), &context);
        }
    }
}

#[test]
fn demo_report_findings_match() {
    let root = env!("CARGO_MANIFEST_DIR");
    let report_text = std::fs::read_to_string(format!("{}/examples/demo_report.json", root))
        .expect("the demo report reads");
    let report = JsonParser::parse(&report_text);
    let recorded = report.get("static_findings").unwrap().as_list();
    let mut union: Vec<StaticFinding> = Vec::new();
    for name in ["test_calc.py", "test_greeter.py"] {
        let path = format!("{}/examples/demo_project/{}", root, name);
        let findings =
            scan_file(std::path::Path::new(&path), name, None, None).expect("the demo files scan");
        union.extend(findings);
    }
    union.sort_by(|a, b| {
        (a.path.as_str(), a.line, a.kind.as_str()).cmp(&(b.path.as_str(), b.line, b.kind.as_str()))
    });
    assert_eq!(
        union.len(),
        recorded.len(),
        "the demo findings count differs from the shipped report"
    );
    for (mine, expected) in union.iter().zip(recorded.iter()) {
        assert_eq!(mine.kind, expected.get("kind").unwrap().as_str());
        assert_eq!(mine.path, expected.get("path").unwrap().as_str());
        assert_eq!(mine.line, expected.get("line").unwrap().as_int());
        assert_eq!(mine.detail, expected.get("detail").unwrap().as_str());
    }
}
