//! The TOML reading, ported to the validity judgment the Python
//! toml reader passes. The settings reading asks only whether a
//! file parses and what tables it builds, so this reader builds
//! the same values and refuses the same documents, including the
//! rules for defining a table twice, for a table a dotted key has
//! claimed, for the contents of an inline table standing frozen,
//! and for an array of tables taking its settings from its last
//! element under a later header.
//!
//! Integers are unbounded in the Python reading, so an integer
//! value is held as its canonical decimal text, computed here
//! from whichever literal form states it.

use crate::pyvalue::{Offset, PyValue};
use std::path::Path;

/// Parse a TOML file at a path, or return None where the Python
/// reader would refuse it, whether for its bytes or its text.
pub fn load_toml(path: &Path) -> Option<PyValue> {
    let bytes = std::fs::read(path).ok()?;
    let text = String::from_utf8(bytes).ok()?;
    parse_toml(&text).ok()
}

#[derive(Debug)]
pub struct TomlError;

type TomlResult<T> = Result<T, TomlError>;

#[derive(Debug, Clone, Copy, PartialEq)]
enum Origin {
    Root,
    Header,
    Implicit,
    Inline,
}

#[derive(Debug, Clone)]
struct Tab {
    entries: Vec<(String, Node)>,
    origin: Origin,
    dotted: bool,
    frozen: bool,
    explicit: bool,
}

impl Tab {
    fn new(origin: Origin) -> Tab {
        Tab {
            entries: Vec::new(),
            origin,
            dotted: false,
            frozen: false,
            explicit: false,
        }
    }

    fn get_mut(&mut self, key: &str) -> Option<&mut Node> {
        self.entries
            .iter_mut()
            .find(|(name, _)| name == key)
            .map(|(_, node)| node)
    }

    fn contains(&self, key: &str) -> bool {
        self.entries.iter().any(|(name, _)| name == key)
    }
}

#[derive(Debug, Clone)]
enum Node {
    Table(Tab),
    StaticArray(Vec<Node>),
    AoT(Vec<Tab>),
    Scalar(PyValue),
}

fn freeze(node: &mut Node) {
    match node {
        Node::Table(tab) => {
            tab.frozen = true;
            for (_, child) in tab.entries.iter_mut() {
                freeze(child);
            }
        }
        Node::StaticArray(items) => {
            for item in items.iter_mut() {
                freeze(item);
            }
        }
        Node::AoT(elements) => {
            for tab in elements.iter_mut() {
                tab.frozen = true;
                for (_, child) in tab.entries.iter_mut() {
                    freeze(child);
                }
            }
        }
        Node::Scalar(_) => {}
    }
}

/// Descend from a table through path parts, creating implicit
/// tables where the path is silent, crossing an array of tables
/// into its last element, and refusing frozen tables, static
/// arrays, and scalars on the way.
fn descend<'a>(tab: &'a mut Tab, parts: &[String]) -> TomlResult<&'a mut Tab> {
    if parts.is_empty() {
        return Ok(tab);
    }
    let key = &parts[0];
    if !tab.contains(key) {
        tab.entries
            .push((key.clone(), Node::Table(Tab::new(Origin::Implicit))));
    }
    match tab.get_mut(key).unwrap() {
        Node::Table(inner) => {
            if inner.frozen {
                return Err(TomlError);
            }
            descend(inner, &parts[1..])
        }
        Node::AoT(elements) => {
            let last = elements.last_mut().unwrap();
            if last.frozen {
                return Err(TomlError);
            }
            descend(last, &parts[1..])
        }
        _ => Err(TomlError),
    }
}

/// Open a header over a path from the root.
fn apply_header(root: &mut Tab, parts: &[String], array: bool) -> TomlResult<()> {
    let (last, parents) = parts.split_last().unwrap();
    let parent = descend(root, parents)?;
    if array {
        if !parent.contains(last) {
            parent
                .entries
                .push((last.clone(), Node::AoT(vec![Tab::new(Origin::Header)])));
            return Ok(());
        }
        match parent.get_mut(last).unwrap() {
            Node::AoT(elements) => {
                elements.push(Tab::new(Origin::Header));
                Ok(())
            }
            _ => Err(TomlError),
        }
    } else {
        if !parent.contains(last) {
            parent
                .entries
                .push((last.clone(), Node::Table(Tab::new(Origin::Header))));
            return Ok(());
        }
        match parent.get_mut(last).unwrap() {
            Node::Table(tab) if tab.origin == Origin::Implicit && !tab.dotted => {
                tab.origin = Origin::Header;
                Ok(())
            }
            _ => Err(TomlError),
        }
    }
}

/// Settle the claims the dotted keys of the sections so far
/// have made, the act a new header performs before it is read.
/// A table a dotted key has claimed and no header has yet
/// settled stands barred from that act on, both to a header
/// that would open it and to a dotted key that would cross it,
/// the reading the Python reader gives the namespaces its
/// dotted keys build once a later header intervenes.
fn finalize_claims(tab: &mut Tab) {
    if tab.dotted && !tab.explicit {
        tab.explicit = true;
    }
    for (_, node) in tab.entries.iter_mut() {
        finalize_node(node);
    }
}

fn finalize_node(node: &mut Node) {
    match node {
        Node::Table(tab) => finalize_claims(tab),
        Node::StaticArray(items) => {
            for item in items.iter_mut() {
                finalize_node(item);
            }
        }
        Node::AoT(elements) => {
            for tab in elements.iter_mut() {
                finalize_claims(tab);
            }
        }
        Node::Scalar(_) => {}
    }
}

/// Descend through the intermediate parts of a dotted key inside
/// one table, creating the tables it names and marking each as
/// claimed by a dotted key, so no later header opens one. A
/// table a header defined, and a table an earlier section
/// claimed by dotted keys once a later header settled the
/// claim, refuse the crossing, because the Python reader holds
/// the namespaces a header defined apart from the namespaces
/// dotted keys build.
fn descend_dotted<'a>(tab: &'a mut Tab, parts: &[String]) -> TomlResult<&'a mut Tab> {
    if parts.is_empty() {
        return Ok(tab);
    }
    if tab.frozen {
        return Err(TomlError);
    }
    let key = &parts[0];
    if !tab.contains(key) {
        tab.entries
            .push((key.clone(), Node::Table(Tab::new(Origin::Implicit))));
    }
    match tab.get_mut(key).unwrap() {
        Node::Table(inner) => {
            if inner.frozen {
                return Err(TomlError);
            }
            if inner.origin == Origin::Header || inner.explicit {
                return Err(TomlError);
            }
            inner.dotted = true;
            descend_dotted(inner, &parts[1..])
        }
        _ => Err(TomlError),
    }
}

/// Apply one key and value assignment under a context path.
fn apply_keyval(
    root: &mut Tab,
    context: &[String],
    parts: &[String],
    value: Node,
) -> TomlResult<()> {
    let base = descend(root, context)?;
    let (last, parents) = parts.split_last().unwrap();
    let target = descend_dotted(base, parents)?;
    if target.contains(last) {
        return Err(TomlError);
    }
    let mut value = value;
    freeze(&mut value);
    target.entries.push((last.clone(), value));
    Ok(())
}

fn node_to_value(node: &Node) -> PyValue {
    match node {
        Node::Scalar(value) => value.clone(),
        Node::Table(tab) => PyValue::Table(
            tab.entries
                .iter()
                .map(|(key, child)| (key.clone(), node_to_value(child)))
                .collect(),
        ),
        Node::StaticArray(items) => PyValue::List(items.iter().map(node_to_value).collect()),
        Node::AoT(elements) => PyValue::List(
            elements
                .iter()
                .map(|tab| {
                    PyValue::Table(
                        tab.entries
                            .iter()
                            .map(|(key, child)| (key.clone(), node_to_value(child)))
                            .collect(),
                    )
                })
                .collect(),
        ),
    }
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn peek_at(&self, shift: usize) -> Option<char> {
        self.chars.get(self.pos + shift).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek();
        if ch.is_some() {
            self.pos += 1;
        }
        ch
    }

    fn expect(&mut self, ch: char) -> TomlResult<()> {
        if self.bump() == Some(ch) {
            Ok(())
        } else {
            Err(TomlError)
        }
    }

    /// Skip spaces and tabs, never a newline.
    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(' ') | Some('\t')) {
            self.pos += 1;
        }
    }

    /// Skip one comment, the cursor standing on its opening
    /// mark, and refuse the characters the Python reader refuses
    /// in a comment, the control characters other than the tab
    /// and the delete character. The cursor stops on the newline
    /// that ends the comment or at the end of the document.
    fn skip_comment(&mut self) -> TomlResult<()> {
        self.pos += 1;
        while let Some(ch) = self.peek() {
            if ch == '\n' {
                return Ok(());
            }
            if (ch as u32) < 0x20 && ch != '\t' || ch == '\u{7f}' {
                return Err(TomlError);
            }
            self.pos += 1;
        }
        Ok(())
    }

    /// Skip whitespace, comments, and newlines, as an array does.
    fn skip_ws_comments_newlines(&mut self) -> TomlResult<()> {
        loop {
            match self.peek() {
                Some(' ') | Some('\t') | Some('\n') => {
                    self.pos += 1;
                }
                Some('#') => {
                    self.skip_comment()?;
                }
                _ => return Ok(()),
            }
        }
    }

    /// After a statement, take an optional comment and then the
    /// end of the line or of the document.
    fn finish_line(&mut self) -> TomlResult<()> {
        self.skip_ws();
        if self.peek() == Some('#') {
            self.skip_comment()?;
        }
        match self.peek() {
            None => Ok(()),
            Some('\n') => {
                self.pos += 1;
                Ok(())
            }
            _ => Err(TomlError),
        }
    }

    fn parse_key_part(&mut self) -> TomlResult<String> {
        match self.peek() {
            Some('"') => self.parse_basic_string(false),
            Some('\'') => self.parse_literal_string(false),
            _ => {
                let start = self.pos;
                while let Some(ch) = self.peek() {
                    if ch.is_ascii_alphanumeric() || ch == '_' || ch == '-' {
                        self.pos += 1;
                    } else {
                        break;
                    }
                }
                if self.pos == start {
                    return Err(TomlError);
                }
                Ok(self.chars[start..self.pos].iter().collect())
            }
        }
    }

    fn parse_key(&mut self) -> TomlResult<Vec<String>> {
        let mut parts = Vec::new();
        loop {
            self.skip_ws();
            parts.push(self.parse_key_part()?);
            self.skip_ws();
            if self.peek() == Some('.') {
                self.pos += 1;
            } else {
                return Ok(parts);
            }
        }
    }

    fn parse_basic_string(&mut self, multiline_ok: bool) -> TomlResult<String> {
        // The opening quote stands at the cursor.
        if multiline_ok && self.peek_at(1) == Some('"') && self.peek_at(2) == Some('"') {
            self.pos += 3;
            return self.parse_multiline_basic();
        }
        self.pos += 1;
        let mut out = String::new();
        loop {
            let ch = self.bump().ok_or(TomlError)?;
            match ch {
                '"' => return Ok(out),
                '\\' => {
                    let escaped = self.parse_escape()?;
                    out.push(escaped);
                }
                '\n' => return Err(TomlError),
                ch if (ch as u32) < 0x20 && ch != '\t' => return Err(TomlError),
                '\u{7f}' => return Err(TomlError),
                ch => out.push(ch),
            }
        }
    }

    fn parse_escape(&mut self) -> TomlResult<char> {
        let ch = self.bump().ok_or(TomlError)?;
        let code = match ch {
            'b' => '\u{8}',
            't' => '\t',
            'n' => '\n',
            'f' => '\u{c}',
            'r' => '\r',
            '"' => '"',
            '\\' => '\\',
            'u' => {
                let value = self.parse_hex(4)?;
                char::from_u32(value).ok_or(TomlError)?
            }
            'U' => {
                let value = self.parse_hex(8)?;
                char::from_u32(value).ok_or(TomlError)?
            }
            _ => return Err(TomlError),
        };
        Ok(code)
    }

    fn parse_hex(&mut self, digits: usize) -> TomlResult<u32> {
        let mut value: u32 = 0;
        for _ in 0..digits {
            let ch = self.bump().ok_or(TomlError)?;
            let digit = ch.to_digit(16).ok_or(TomlError)?;
            value = value * 16 + digit;
        }
        Ok(value)
    }

    fn parse_multiline_basic(&mut self) -> TomlResult<String> {
        // A newline hard against the opening quotes is trimmed.
        if self.peek() == Some('\n') {
            self.pos += 1;
        }
        let mut out = String::new();
        loop {
            let ch = self.peek().ok_or(TomlError)?;
            if ch == '"' {
                let mut run = 0;
                while self.peek_at(run) == Some('"') {
                    run += 1;
                }
                if run < 3 {
                    for _ in 0..run {
                        out.push('"');
                    }
                    self.pos += run;
                    continue;
                }
                if run <= 5 {
                    for _ in 0..(run - 3) {
                        out.push('"');
                    }
                    self.pos += run;
                    return Ok(out);
                }
                self.pos += 3;
                return Ok(out);
            }
            if ch == '\\' {
                // A line ending backslash trims the whitespace
                // and newlines that follow it.
                let mut probe = self.pos + 1;
                while matches!(self.chars.get(probe), Some(' ') | Some('\t')) {
                    probe += 1;
                }
                if self.chars.get(probe) == Some(&'\n') {
                    probe += 1;
                    while matches!(self.chars.get(probe), Some(' ') | Some('\t') | Some('\n')) {
                        probe += 1;
                    }
                    self.pos = probe;
                    continue;
                }
                self.pos += 1;
                let escaped = self.parse_escape()?;
                out.push(escaped);
                continue;
            }
            if (ch as u32) < 0x20 && ch != '\t' && ch != '\n' {
                return Err(TomlError);
            }
            if ch == '\u{7f}' {
                return Err(TomlError);
            }
            out.push(ch);
            self.pos += 1;
        }
    }

    fn parse_literal_string(&mut self, multiline_ok: bool) -> TomlResult<String> {
        if multiline_ok && self.peek_at(1) == Some('\'') && self.peek_at(2) == Some('\'') {
            self.pos += 3;
            return self.parse_multiline_literal();
        }
        self.pos += 1;
        let mut out = String::new();
        loop {
            let ch = self.bump().ok_or(TomlError)?;
            match ch {
                '\'' => return Ok(out),
                '\n' => return Err(TomlError),
                ch if (ch as u32) < 0x20 && ch != '\t' => return Err(TomlError),
                '\u{7f}' => return Err(TomlError),
                ch => out.push(ch),
            }
        }
    }

    fn parse_multiline_literal(&mut self) -> TomlResult<String> {
        if self.peek() == Some('\n') {
            self.pos += 1;
        }
        let mut out = String::new();
        loop {
            let ch = self.peek().ok_or(TomlError)?;
            if ch == '\'' {
                let mut run = 0;
                while self.peek_at(run) == Some('\'') {
                    run += 1;
                }
                if run < 3 {
                    for _ in 0..run {
                        out.push('\'');
                    }
                    self.pos += run;
                    continue;
                }
                if run <= 5 {
                    for _ in 0..(run - 3) {
                        out.push('\'');
                    }
                    self.pos += run;
                    return Ok(out);
                }
                self.pos += 3;
                return Ok(out);
            }
            if (ch as u32) < 0x20 && ch != '\t' && ch != '\n' {
                return Err(TomlError);
            }
            if ch == '\u{7f}' {
                return Err(TomlError);
            }
            out.push(ch);
            self.pos += 1;
        }
    }
}

/// Whether a digit run holds its underscores the way the grammar
/// states, one underscore between two digits and none at an end.
fn digit_run_ok(run: &str) -> bool {
    if run.is_empty() {
        return false;
    }
    let bytes: Vec<char> = run.chars().collect();
    if bytes[0] == '_' || bytes[bytes.len() - 1] == '_' {
        return false;
    }
    let mut previous_underscore = false;
    for ch in &bytes {
        if *ch == '_' {
            if previous_underscore {
                return false;
            }
            previous_underscore = true;
        } else {
            previous_underscore = false;
        }
    }
    true
}

/// Multiply a decimal digit string by a small radix and add a
/// digit, the arithmetic the unbounded integer reading needs.
fn digits_mul_add(digits: &mut Vec<u8>, radix: u32, add: u32) {
    let mut carry = add;
    for digit in digits.iter_mut().rev() {
        let value = (*digit as u32) * radix + carry;
        *digit = (value % 10) as u8;
        carry = value / 10;
    }
    while carry > 0 {
        digits.insert(0, (carry % 10) as u8);
        carry /= 10;
    }
}

fn radix_digits_to_decimal(text: &str, radix: u32) -> String {
    let mut digits: Vec<u8> = vec![0];
    for ch in text.chars() {
        if ch == '_' {
            continue;
        }
        let value = ch.to_digit(radix).unwrap();
        digits_mul_add(&mut digits, radix, value);
    }
    let rendered: String = digits
        .iter()
        .map(|digit| char::from_digit(*digit as u32, 10).unwrap())
        .collect();
    let trimmed = rendered.trim_start_matches('0');
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 0,
    }
}

struct DateParts {
    year: i64,
    month: u32,
    day: u32,
}

struct TimeParts {
    hour: u32,
    minute: u32,
    second: u32,
    micros: u32,
}

impl Parser {
    fn take_digits(&mut self, count: usize) -> TomlResult<Vec<u32>> {
        let mut out = Vec::new();
        for _ in 0..count {
            let ch = self.bump().ok_or(TomlError)?;
            out.push(ch.to_digit(10).ok_or(TomlError)?);
        }
        Ok(out)
    }

    fn parse_date(&mut self) -> TomlResult<DateParts> {
        let year_digits = self.take_digits(4)?;
        self.expect('-')?;
        let month_digits = self.take_digits(2)?;
        self.expect('-')?;
        let day_digits = self.take_digits(2)?;
        let fold = |digits: &[u32]| digits.iter().fold(0i64, |acc, d| acc * 10 + *d as i64);
        let year = fold(&year_digits);
        let month = fold(&month_digits) as u32;
        let day = fold(&day_digits) as u32;
        if year < 1 || !(1..=12).contains(&month) || day < 1 || day > days_in_month(year, month) {
            return Err(TomlError);
        }
        Ok(DateParts { year, month, day })
    }

    fn parse_time(&mut self) -> TomlResult<TimeParts> {
        let hour_digits = self.take_digits(2)?;
        self.expect(':')?;
        let minute_digits = self.take_digits(2)?;
        self.expect(':')?;
        let second_digits = self.take_digits(2)?;
        let fold = |digits: &[u32]| digits.iter().fold(0u32, |acc, d| acc * 10 + d);
        let hour = fold(&hour_digits);
        let minute = fold(&minute_digits);
        let second = fold(&second_digits);
        if hour > 23 || minute > 59 || second > 59 {
            return Err(TomlError);
        }
        let mut micros: u32 = 0;
        if self.peek() == Some('.') {
            self.pos += 1;
            let start = self.pos;
            while matches!(self.peek(), Some(ch) if ch.is_ascii_digit()) {
                self.pos += 1;
            }
            if self.pos == start {
                return Err(TomlError);
            }
            let frac: String = self.chars[start..self.pos].iter().collect();
            let mut text: String = frac.chars().take(6).collect();
            while text.len() < 6 {
                text.push('0');
            }
            micros = text.parse().unwrap();
        }
        Ok(TimeParts {
            hour,
            minute,
            second,
            micros,
        })
    }

    /// A value that begins with digits. A date shape commits the
    /// value to the datetime reading, and a time shape to the
    /// time reading, and anything else is read as a number.
    fn parse_numeric_or_datetime(&mut self) -> TomlResult<PyValue> {
        let four_then_dash = matches!(self.peek_at(0), Some(ch) if ch.is_ascii_digit())
            && matches!(self.peek_at(1), Some(ch) if ch.is_ascii_digit())
            && matches!(self.peek_at(2), Some(ch) if ch.is_ascii_digit())
            && matches!(self.peek_at(3), Some(ch) if ch.is_ascii_digit())
            && self.peek_at(4) == Some('-');
        if four_then_dash {
            let date = self.parse_date()?;
            let separator = self.peek();
            let time_follows = match separator {
                Some('T') | Some('t') => true,
                Some(' ') => {
                    matches!(self.peek_at(1), Some(ch) if ch.is_ascii_digit())
                        && matches!(self.peek_at(2), Some(ch) if ch.is_ascii_digit())
                        && self.peek_at(3) == Some(':')
                }
                _ => false,
            };
            if !time_follows {
                return Ok(PyValue::Date {
                    year: date.year,
                    month: date.month,
                    day: date.day,
                });
            }
            self.pos += 1;
            let time = self.parse_time()?;
            let offset = match self.peek() {
                Some('Z') | Some('z') => {
                    self.pos += 1;
                    Some(Offset::Utc)
                }
                Some(sign @ ('+' | '-')) => {
                    self.pos += 1;
                    let hours = self.take_digits(2)?;
                    self.expect(':')?;
                    let minutes = self.take_digits(2)?;
                    let hours = hours.iter().fold(0i64, |acc, d| acc * 10 + *d as i64);
                    let minutes = minutes.iter().fold(0i64, |acc, d| acc * 10 + *d as i64);
                    if hours > 23 || minutes > 59 {
                        return Err(TomlError);
                    }
                    let total = (hours * 3600 + minutes * 60) as i32;
                    Some(Offset::FixedSeconds(if sign == '-' {
                        -total
                    } else {
                        total
                    }))
                }
                _ => None,
            };
            return Ok(PyValue::DateTime {
                year: date.year,
                month: date.month,
                day: date.day,
                hour: time.hour,
                minute: time.minute,
                second: time.second,
                micros: time.micros,
                offset,
            });
        }
        let two_then_colon = matches!(self.peek_at(0), Some(ch) if ch.is_ascii_digit())
            && matches!(self.peek_at(1), Some(ch) if ch.is_ascii_digit())
            && self.peek_at(2) == Some(':');
        if two_then_colon {
            let time = self.parse_time()?;
            return Ok(PyValue::Time {
                hour: time.hour,
                minute: time.minute,
                second: time.second,
                micros: time.micros,
            });
        }
        self.parse_number()
    }

    fn parse_number(&mut self) -> TomlResult<PyValue> {
        let start = self.pos;
        while let Some(ch) = self.peek() {
            if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '+' | '-' | '.') {
                self.pos += 1;
            } else {
                break;
            }
        }
        let token: String = self.chars[start..self.pos].iter().collect();
        number_from_token(&token)
    }

    fn parse_array(&mut self) -> TomlResult<Node> {
        self.expect('[')?;
        let mut items = Vec::new();
        loop {
            self.skip_ws_comments_newlines()?;
            if self.peek() == Some(']') {
                self.pos += 1;
                return Ok(Node::StaticArray(items));
            }
            let value = self.parse_value()?;
            items.push(value);
            self.skip_ws_comments_newlines()?;
            match self.bump() {
                Some(',') => continue,
                Some(']') => return Ok(Node::StaticArray(items)),
                _ => return Err(TomlError),
            }
        }
    }

    fn parse_inline_table(&mut self) -> TomlResult<Node> {
        self.expect('{')?;
        let mut tab = Tab::new(Origin::Inline);
        self.skip_ws();
        if self.peek() == Some('}') {
            self.pos += 1;
            return Ok(Node::Table(tab));
        }
        loop {
            self.skip_ws();
            if self.peek() == Some('\n') {
                return Err(TomlError);
            }
            let parts = self.parse_key()?;
            self.skip_ws();
            self.expect('=')?;
            self.skip_ws();
            let value = self.parse_value()?;
            let (last, parents) = parts.split_last().unwrap();
            let target = descend_dotted(&mut tab, parents)?;
            if target.contains(last) {
                return Err(TomlError);
            }
            target.entries.push((last.clone(), value));
            self.skip_ws();
            match self.bump() {
                Some(',') => {
                    // The pair stands complete at its comma, and
                    // its value freezes there, so no later pair
                    // of this table descends into it or extends
                    // it, the reading the Python reader gives a
                    // completed pair of an inline table.
                    let inserted = target.get_mut(last).unwrap();
                    freeze(inserted);
                    continue;
                }
                Some('}') => return Ok(Node::Table(tab)),
                _ => return Err(TomlError),
            }
        }
    }

    fn parse_value(&mut self) -> TomlResult<Node> {
        match self.peek() {
            Some('"') => Ok(Node::Scalar(PyValue::Str(self.parse_basic_string(true)?))),
            Some('\'') => Ok(Node::Scalar(PyValue::Str(self.parse_literal_string(true)?))),
            Some('[') => self.parse_array(),
            Some('{') => self.parse_inline_table(),
            Some('t') => {
                if self.chars[self.pos..].starts_with(&['t', 'r', 'u', 'e']) {
                    self.pos += 4;
                    Ok(Node::Scalar(PyValue::Bool(true)))
                } else {
                    Err(TomlError)
                }
            }
            Some('f') => {
                if self.chars[self.pos..].starts_with(&['f', 'a', 'l', 's', 'e']) {
                    self.pos += 5;
                    Ok(Node::Scalar(PyValue::Bool(false)))
                } else {
                    Err(TomlError)
                }
            }
            Some(ch) if ch.is_ascii_digit() || ch == '+' || ch == '-' => {
                // A signed or unsigned inf or nan stands here, and
                // the number reading takes the rest.
                let rest: String = self.chars[self.pos..].iter().take(4).collect();
                if rest.starts_with("inf")
                    || rest.starts_with("nan")
                    || rest.starts_with("+inf")
                    || rest.starts_with("-inf")
                    || rest.starts_with("+nan")
                    || rest.starts_with("-nan")
                {
                    return Ok(Node::Scalar(self.parse_number()?));
                }
                if ch.is_ascii_digit() {
                    Ok(Node::Scalar(self.parse_numeric_or_datetime()?))
                } else {
                    Ok(Node::Scalar(self.parse_number()?))
                }
            }
            Some('i') | Some('n') => Ok(Node::Scalar(self.parse_number()?)),
            _ => Err(TomlError),
        }
    }
}

fn number_from_token(token: &str) -> TomlResult<PyValue> {
    let (sign, body) = match token.strip_prefix('-') {
        Some(rest) => (-1i32, rest),
        None => match token.strip_prefix('+') {
            Some(rest) => (1i32, rest),
            None => (1i32, token),
        },
    };
    if body == "inf" {
        return Ok(PyValue::Float(if sign < 0 {
            f64::NEG_INFINITY
        } else {
            f64::INFINITY
        }));
    }
    if body == "nan" {
        return Ok(PyValue::Float(f64::NAN));
    }
    if let Some(hex) = body.strip_prefix("0x") {
        if token != body {
            return Err(TomlError);
        }
        if !digit_run_ok(hex) || !hex.chars().all(|ch| ch.is_ascii_hexdigit() || ch == '_') {
            return Err(TomlError);
        }
        return Ok(PyValue::Int(radix_digits_to_decimal(hex, 16)));
    }
    if let Some(oct) = body.strip_prefix("0o") {
        if token != body {
            return Err(TomlError);
        }
        if !digit_run_ok(oct) || !oct.chars().all(|ch| matches!(ch, '0'..='7' | '_')) {
            return Err(TomlError);
        }
        return Ok(PyValue::Int(radix_digits_to_decimal(oct, 8)));
    }
    if let Some(bin) = body.strip_prefix("0b") {
        if token != body {
            return Err(TomlError);
        }
        if !digit_run_ok(bin) || !bin.chars().all(|ch| matches!(ch, '0' | '1' | '_')) {
            return Err(TomlError);
        }
        return Ok(PyValue::Int(radix_digits_to_decimal(bin, 2)));
    }
    // A decimal integer or float.
    let (mantissa, exponent) = match body.find(['e', 'E']) {
        Some(index) => (&body[..index], Some(&body[index + 1..])),
        None => (body, None),
    };
    let (int_part, frac_part) = match mantissa.find('.') {
        Some(index) => (&mantissa[..index], Some(&mantissa[index + 1..])),
        None => (mantissa, None),
    };
    if int_part.is_empty() || !digit_run_ok(int_part) {
        return Err(TomlError);
    }
    let int_digits: String = int_part.chars().filter(|ch| *ch != '_').collect();
    if int_digits.len() > 1 && int_digits.starts_with('0') {
        return Err(TomlError);
    }
    if !int_digits.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(TomlError);
    }
    if let Some(frac) = frac_part {
        if !digit_run_ok(frac) || !frac.chars().all(|ch| ch.is_ascii_digit() || ch == '_') {
            return Err(TomlError);
        }
    }
    if let Some(exp) = exponent {
        let exp_digits = exp
            .strip_prefix('-')
            .or_else(|| exp.strip_prefix('+'))
            .unwrap_or(exp);
        if !digit_run_ok(exp_digits)
            || !exp_digits
                .chars()
                .all(|ch| ch.is_ascii_digit() || ch == '_')
        {
            return Err(TomlError);
        }
    }
    if frac_part.is_none() && exponent.is_none() {
        let mut text = int_digits;
        if text.chars().all(|ch| ch == '0') {
            return Ok(PyValue::Int("0".to_string()));
        }
        if sign < 0 {
            text.insert(0, '-');
        }
        return Ok(PyValue::Int(text));
    }
    let cleaned: String = token.chars().filter(|ch| *ch != '_').collect();
    let value: f64 = cleaned.parse().map_err(|_| TomlError)?;
    Ok(PyValue::Float(value))
}

/// Parse a whole TOML document from its text. The text is taken
/// with its carriage returns folded into its newlines first, the
/// way the Python reader takes the document.
pub fn parse_toml(text: &str) -> TomlResult<PyValue> {
    let normalized = text.replace("\r\n", "\n");
    let mut parser = Parser {
        chars: normalized.chars().collect(),
        pos: 0,
    };
    let mut root = Tab::new(Origin::Root);
    let mut context: Vec<String> = Vec::new();
    loop {
        // Document level whitespace and comments.
        loop {
            match parser.peek() {
                Some(' ') | Some('\t') | Some('\n') => {
                    parser.pos += 1;
                }
                Some('#') => {
                    parser.skip_comment()?;
                }
                _ => break,
            }
        }
        match parser.peek() {
            None => break,
            Some('[') => {
                finalize_claims(&mut root);
                parser.pos += 1;
                let array = if parser.peek() == Some('[') {
                    parser.pos += 1;
                    true
                } else {
                    false
                };
                parser.skip_ws();
                let parts = parser.parse_key()?;
                parser.skip_ws();
                parser.expect(']')?;
                if array {
                    parser.expect(']')?;
                }
                apply_header(&mut root, &parts, array)?;
                context = parts;
                parser.finish_line()?;
            }
            Some(_) => {
                let parts = parser.parse_key()?;
                parser.skip_ws();
                parser.expect('=')?;
                parser.skip_ws();
                let value = parser.parse_value()?;
                apply_keyval(&mut root, &context, &parts, value)?;
                parser.finish_line()?;
            }
        }
    }
    Ok(node_to_value(&Node::Table(root)))
}
