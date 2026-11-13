//! The tokenizer for the Python 3.12 reading of PORT7-2.
//! It produces the logical token stream of the interpreter,
//! names, numbers, strings with their values decoded, the
//! operators, and the newline and indentation tokens the
//! grammar is stated over. A source the tokenizer refuses is
//! a parse refusal of the family PORT7-7 names, the null byte
//! and the leading byte order mark among them. Bracket
//! nesting past the interpreter's own limit of 200 refuses
//! here, the shallower limit PORT7-2 keeps inside the port
//! bound of 4,000.

use crate::pyast::{ConstVal, Pos, PyText};
use crate::pydata::lookup_name;

/// A failure to tokenize, carried up to the parse refusal.
#[derive(Debug, Clone)]
pub struct TokenError {
    pub message: String,
}

fn err(message: &str) -> TokenError {
    TokenError {
        message: message.to_string(),
    }
}

/// A string literal as the tokenizer hands it over, either a
/// decoded value or the parts of an f-string the parser
/// finishes.
#[derive(Debug, Clone)]
pub enum StrTok {
    Value(ConstVal),
    FString(Vec<FPart>),
}

/// One part of an f-string, literal text or a field.
#[derive(Debug, Clone)]
pub enum FPart {
    Lit(Vec<u32>),
    Field(FieldData),
}

/// One replacement field of an f-string.
#[derive(Debug, Clone)]
pub struct FieldData {
    /// The source text of the field expression.
    pub expr: String,
    /// The line the expression text starts on.
    pub expr_line: u32,
    /// The byte column the expression text starts at.
    pub expr_col: u32,
    /// The conversion character when one stands, s, r, or a.
    pub conversion: Option<char>,
    /// The format spec parts when a spec stands.
    pub spec: Option<Vec<FPart>>,
    /// The literal the self documenting form prefixes, the
    /// expression text as written and an equals sign.
    pub debug: Option<String>,
    /// The position of the opening brace.
    pub pos: Pos,
    /// The bracket depth standing at this field's opening
    /// brace, the enclosing code brackets and the braces of
    /// the strings around it counted in, the base the
    /// fragment's own reading counts its brackets from.
    pub bracket_depth: usize,
    /// The number of f-strings enclosing the string this
    /// field stands in, this string itself counted, the
    /// depth the fragment's nested f-strings count from.
    pub fstring_depth: usize,
}

#[derive(Debug, Clone)]
pub enum Tok {
    Name(String),
    Number(String),
    Str(StrTok),
    Op(String),
    Newline,
    Indent,
    Dedent,
    End,
}

#[derive(Debug, Clone)]
pub struct Token {
    pub kind: Tok,
    pub pos: Pos,
}

struct Scanner {
    chars: Vec<char>,
    /// Byte offset of each char in the source.
    bytes: Vec<usize>,
    /// Byte offset of the start of each line, by line index.
    line_starts: Vec<usize>,
    index: usize,
    /// The number of f-strings enclosing the text this
    /// scanner reads, handed in as the tokenize base and
    /// raised by scan_string while one string's parts are
    /// scanned, then restored.
    fstring_depth: usize,
    /// The bracket depth standing where the string now
    /// being scanned opened, set by scan_string.
    bracket_floor: usize,
    /// The net structural braces the string now being
    /// scanned has opened inside itself, field braces of
    /// its parts and of its format specs counted.
    string_braces: usize,
}

impl Scanner {
    fn new(source: &str) -> Scanner {
        let mut chars = Vec::new();
        let mut bytes = Vec::new();
        for (offset, ch) in source.char_indices() {
            chars.push(ch);
            bytes.push(offset);
        }
        bytes.push(source.len());
        let mut line_starts = vec![0usize];
        let mut index = 0usize;
        let raw = source.as_bytes();
        while index < raw.len() {
            if raw[index] == b'\n' {
                line_starts.push(index + 1);
            } else if raw[index] == b'\r' {
                if index + 1 < raw.len() && raw[index + 1] == b'\n' {
                    line_starts.push(index + 2);
                    index += 1;
                } else {
                    line_starts.push(index + 1);
                }
            }
            index += 1;
        }
        Scanner {
            chars,
            bytes,
            line_starts,
            index: 0,
            fstring_depth: 0,
            bracket_floor: 0,
            string_braces: 0,
        }
    }

    fn pos_at(&self, char_index: usize) -> Pos {
        let byte = if char_index < self.bytes.len() {
            self.bytes[char_index]
        } else {
            *self.bytes.last().unwrap_or(&0)
        };
        let line = match self.line_starts.binary_search(&byte) {
            Ok(found) => found,
            Err(insert) => insert - 1,
        };
        Pos::new((line + 1) as u32, (byte - self.line_starts[line]) as u32)
    }

    fn peek(&self) -> Option<char> {
        self.chars.get(self.index).copied()
    }

    fn peek_at(&self, ahead: usize) -> Option<char> {
        self.chars.get(self.index + ahead).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let ch = self.peek();
        if ch.is_some() {
            self.index += 1;
        }
        ch
    }

    fn at_end(&self) -> bool {
        self.index >= self.chars.len()
    }
}

/// Whether the character can start an identifier under the
/// reading the tokenizer makes, a letter or an underscore.
fn is_name_start(ch: char) -> bool {
    ch == '_' || ch.is_alphabetic()
}

/// Whether the character can continue an identifier.
fn is_name_part(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric() || ('\u{300}'..='\u{36f}').contains(&ch)
}

const OPERATORS: &[&str] = &[
    "**=", "//=", "<<=", ">>=", "...", "**", "//", "<<", ">>", "==", "!=", "<=", ">=", "->", ":=",
    "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "@=", "(", ")", "[", "]", "{", "}", ",", ":",
    ".", ";", "@", "+", "-", "*", "/", "%", "&", "|", "^", "~", "<", ">", "=",
];

/// Tokenize a whole source. The base line and the base column
/// of the first line let a fragment, the expression of an
/// f-string field, carry the positions of the source it was
/// cut from. The bracket base and the f-string base do the
/// same for the interpreter's two nesting accounts, which run
/// continuously over the whole source: the bracket ceiling
/// counts the code brackets around the fragment with the
/// fragment's own, and an f-string standing inside another
/// counts toward the nested f-string cap however many
/// fragments stand between them.
pub fn tokenize(
    source: &str,
    base_line: u32,
    base_col: u32,
    base_brackets: usize,
    base_fstrings: usize,
) -> Result<Vec<Token>, TokenError> {
    if source.contains('\0') {
        return Err(err("source code string cannot contain null bytes"));
    }
    if source.starts_with('\u{feff}') {
        return Err(err("invalid non-printable character U+FEFF"));
    }
    let mut scanner = Scanner::new(source);
    scanner.fstring_depth = base_fstrings;
    let mut tokens: Vec<Token> = Vec::new();
    let mut indents: Vec<(u32, u32)> = vec![(0, 0)];
    let mut depth: usize = 0;
    let mut line_start = true;
    // Assigned at every physical line end before the macro
    // below reads it, so it stands declared without a value.
    let mut pending_newline_pos: Pos;

    // Emit the tokens of one logical line end.
    macro_rules! end_logical_line {
        () => {
            tokens.push(Token {
                kind: Tok::Newline,
                pos: pending_newline_pos,
            })
        };
    }

    while !scanner.at_end() {
        if line_start && depth == 0 {
            // Read the indentation of the next physical line,
            // as the expanded column and as the raw column
            // the characters themselves count, the pair the
            // interpreter compares when it refuses a line
            // whose indentation mixes tabs and spaces at one
            // level.
            let mut col: u32 = 0;
            let mut altcol: u32 = 0;
            let mut scan = scanner.index;
            let mut blank = true;
            while scan < scanner.chars.len() {
                match scanner.chars[scan] {
                    ' ' => {
                        col += 1;
                        altcol += 1;
                    }
                    '\t' => {
                        col = (col / 8 + 1) * 8;
                        altcol += 1;
                    }
                    '\u{c}' => {
                        col = 0;
                        altcol = 0;
                    }
                    '\r' | '\n' => break,
                    '#' => break,
                    _ => {
                        blank = false;
                        break;
                    }
                }
                scan += 1;
            }
            if blank {
                // Blank and comment only lines pass over.
                while let Some(ch) = scanner.peek() {
                    if ch == '\n' {
                        scanner.bump();
                        break;
                    }
                    scanner.bump();
                }
                continue;
            }
            let (current, current_alt) = *indents.last().unwrap();
            if col > current {
                // The interpreter's indentation stack holds
                // one hundred levels, the level of the file
                // itself among them, and a line standing
                // deeper refuses as a syntax error.
                if indents.len() >= 100 {
                    return Err(err("too many levels of indentation"));
                }
                indents.push((col, altcol));
                tokens.push(Token {
                    kind: Tok::Indent,
                    pos: scanner.pos_at(scanner.index),
                });
            } else if col < current {
                while indents.last().unwrap().0 > col {
                    indents.pop();
                    tokens.push(Token {
                        kind: Tok::Dedent,
                        pos: scanner.pos_at(scanner.index),
                    });
                }
                let (landed, landed_alt) = *indents.last().unwrap();
                if landed != col {
                    return Err(err("unindent does not match any outer indentation level"));
                }
                if landed_alt != altcol {
                    return Err(err("inconsistent use of tabs and spaces in indentation"));
                }
            } else if current_alt != altcol {
                return Err(err("inconsistent use of tabs and spaces in indentation"));
            }
            line_start = false;
            // Skip the leading whitespace itself.
            while matches!(scanner.peek(), Some(' ') | Some('\t') | Some('\u{c}')) {
                scanner.bump();
            }
        }

        let ch = scanner.peek().unwrap();
        let pos = scanner.pos_at(scanner.index);
        match ch {
            '\n' | '\r' => {
                // A physical line end. Inside brackets it is
                // no token at all, outside it ends the logical
                // line.
                scanner.bump();
                if ch == '\r' && scanner.peek() == Some('\n') {
                    scanner.bump();
                }
                if depth == 0 {
                    pending_newline_pos = pos;
                    end_logical_line!();
                    line_start = true;
                }
            }
            '\\' => {
                // An explicit line joining.
                scanner.bump();
                match scanner.peek() {
                    Some('\n') => {
                        scanner.bump();
                    }
                    Some('\r') => {
                        scanner.bump();
                        if scanner.peek() == Some('\n') {
                            scanner.bump();
                        }
                    }
                    _ => {
                        return Err(err(
                            "unexpected character after line continuation character",
                        ))
                    }
                }
            }
            '#' => {
                while let Some(next) = scanner.peek() {
                    if next == '\n' || next == '\r' {
                        break;
                    }
                    scanner.bump();
                }
            }
            ' ' | '\t' | '\u{c}' => {
                scanner.bump();
            }
            '(' | '[' | '{' => {
                depth += 1;
                if base_brackets + depth > 200 {
                    return Err(err("too many nested parentheses"));
                }
                tokens.push(Token {
                    kind: Tok::Op(ch.to_string()),
                    pos,
                });
                scanner.bump();
            }
            ')' | ']' | '}' => {
                if depth == 0 {
                    return Err(err("unmatched closing bracket"));
                }
                depth -= 1;
                tokens.push(Token {
                    kind: Tok::Op(ch.to_string()),
                    pos,
                });
                scanner.bump();
            }
            '"' | '\'' => {
                let token = scan_string(&mut scanner, None, base_brackets + depth)?;
                tokens.push(Token {
                    kind: Tok::Str(token),
                    pos,
                });
            }
            _ if is_name_start(ch) => {
                // A name, or the prefix of a string literal.
                let start = scanner.index;
                while let Some(next) = scanner.peek() {
                    if is_name_part(next) {
                        scanner.bump();
                    } else {
                        break;
                    }
                }
                let word: String = scanner.chars[start..scanner.index].iter().collect();
                if matches!(scanner.peek(), Some('"') | Some('\'')) && is_string_prefix(&word) {
                    let token = scan_string(&mut scanner, Some(&word), base_brackets + depth)?;
                    tokens.push(Token {
                        kind: Tok::Str(token),
                        pos,
                    });
                } else {
                    tokens.push(Token {
                        kind: Tok::Name(word),
                        pos,
                    });
                }
            }
            _ if ch.is_ascii_digit()
                || (ch == '.'
                    && matches!(scanner.peek_at(1), Some(next) if next.is_ascii_digit())) =>
            {
                let text = scan_number(&mut scanner);
                tokens.push(Token {
                    kind: Tok::Number(text),
                    pos,
                });
            }
            _ => {
                let rest: String = scanner.chars[scanner.index..].iter().take(3).collect();
                let mut matched: Option<&str> = None;
                for op in OPERATORS {
                    if rest.starts_with(op) {
                        matched = Some(op);
                        break;
                    }
                }
                match matched {
                    Some(op) => {
                        for _ in 0..op.chars().count() {
                            scanner.bump();
                        }
                        tokens.push(Token {
                            kind: Tok::Op(op.to_string()),
                            pos,
                        });
                    }
                    None => {
                        return Err(err("invalid character in source"));
                    }
                }
            }
        }
    }

    if depth != 0 {
        return Err(err("unexpected end of file inside brackets"));
    }
    if !line_start {
        tokens.push(Token {
            kind: Tok::Newline,
            pos: scanner.pos_at(scanner.index),
        });
    }
    while indents.len() > 1 {
        indents.pop();
        tokens.push(Token {
            kind: Tok::Dedent,
            pos: scanner.pos_at(scanner.index),
        });
    }
    tokens.push(Token {
        kind: Tok::End,
        pos: scanner.pos_at(scanner.index),
    });
    // Apply the fragment base to every position, the
    // positions inside string parts as well as the token
    // positions themselves: a field's expression base and
    // brace position are stamped in the scanner's own
    // coordinates while the string is scanned, and a
    // nested field read from them without the base would
    // lose the line the string stands on.
    if base_line != 1 || base_col != 0 {
        for token in tokens.iter_mut() {
            token.pos = adjusted(token.pos, base_line, base_col);
            if let Tok::Str(StrTok::FString(parts)) = &mut token.kind {
                for part in parts.iter_mut() {
                    adjust_part(part, base_line, base_col);
                }
            }
        }
    }
    Ok(tokens)
}

/// Carry one string part's stamped positions onto the
/// fragment base the token positions move by.
fn adjust_part(part: &mut FPart, base_line: u32, base_col: u32) {
    if let FPart::Field(field) = part {
        let expr = adjusted(
            Pos::new(field.expr_line, field.expr_col),
            base_line,
            base_col,
        );
        field.expr_line = expr.line;
        field.expr_col = expr.col;
        field.pos = adjusted(field.pos, base_line, base_col);
        if let Some(spec) = &mut field.spec {
            for part in spec.iter_mut() {
                adjust_part(part, base_line, base_col);
            }
        }
    }
}

fn adjusted(pos: Pos, base_line: u32, base_col: u32) -> Pos {
    if pos.line == 1 {
        Pos::new(base_line, base_col + pos.col)
    } else {
        Pos::new(base_line + pos.line - 1, pos.col)
    }
}

fn is_string_prefix(word: &str) -> bool {
    let lower = word.to_lowercase();
    matches!(
        lower.as_str(),
        "r" | "b" | "f" | "u" | "rb" | "br" | "rf" | "fr" | "bf" | "fb"
    )
}

/// Scan a number literal's text, underscores and all.
fn scan_number(scanner: &mut Scanner) -> String {
    let start = scanner.index;
    if scanner.peek() == Some('0')
        && matches!(
            scanner.peek_at(1),
            Some('x') | Some('X') | Some('o') | Some('O') | Some('b') | Some('B')
        )
    {
        scanner.bump();
        scanner.bump();
        while let Some(ch) = scanner.peek() {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                scanner.bump();
            } else {
                break;
            }
        }
    } else {
        while let Some(ch) = scanner.peek() {
            if ch.is_ascii_digit() || ch == '_' {
                scanner.bump();
            } else {
                break;
            }
        }
        if scanner.peek() == Some('.') {
            scanner.bump();
            while let Some(ch) = scanner.peek() {
                if ch.is_ascii_digit() || ch == '_' {
                    scanner.bump();
                } else {
                    break;
                }
            }
        }
        if matches!(scanner.peek(), Some('e') | Some('E')) {
            let save = scanner.index;
            scanner.bump();
            if matches!(scanner.peek(), Some('+') | Some('-')) {
                scanner.bump();
            }
            if matches!(scanner.peek(), Some(next) if next.is_ascii_digit()) {
                while let Some(ch) = scanner.peek() {
                    if ch.is_ascii_digit() || ch == '_' {
                        scanner.bump();
                    } else {
                        break;
                    }
                }
            } else {
                scanner.index = save;
            }
        }
    }
    if matches!(scanner.peek(), Some('j') | Some('J')) {
        scanner.bump();
    }
    scanner.chars[start..scanner.index].iter().collect()
}

struct StrSpec {
    raw: bool,
    bytes: bool,
    fstring: bool,
}

/// Scan one string literal starting at the quote, the prefix
/// already read when one stands. Adjacent literals are the
/// parser's to join, so this scans exactly one literal.
fn scan_string(
    scanner: &mut Scanner,
    prefix: Option<&str>,
    bracket_floor: usize,
) -> Result<StrTok, TokenError> {
    scanner.bracket_floor = bracket_floor;
    scanner.string_braces = 0;
    let lower = prefix.unwrap_or("").to_lowercase();
    let spec = StrSpec {
        raw: lower.contains('r'),
        bytes: lower.contains('b'),
        fstring: lower.contains('f'),
    };
    let quote = scanner.bump().expect("a quote stands");
    let triple = scanner.peek() == Some(quote) && scanner.peek_at(1) == Some(quote);
    if triple {
        scanner.bump();
        scanner.bump();
    }
    if spec.fstring {
        // The interpreter's cap on nested f-strings: a chain
        // of one hundred and fifty refuses at the tokenizer,
        // however the nesting is reached, through replacement
        // fields or through format specs.
        let level = scanner.fstring_depth + 1;
        if level > 149 {
            return Err(err("too many nested f-strings"));
        }
        let saved = scanner.fstring_depth;
        scanner.fstring_depth = level;
        let parts = scan_fstring_parts(scanner, quote, triple, spec.raw);
        scanner.fstring_depth = saved;
        return Ok(StrTok::FString(parts?));
    }
    let mut text: Vec<u32> = Vec::new();
    let mut bytes: Vec<u8> = Vec::new();
    loop {
        let ch = match scanner.peek() {
            Some(ch) => ch,
            None => return Err(err("unterminated string literal")),
        };
        if ch == quote {
            if triple {
                if scanner.peek_at(1) == Some(quote) && scanner.peek_at(2) == Some(quote) {
                    scanner.bump();
                    scanner.bump();
                    scanner.bump();
                    break;
                }
                // A bare quote inside a triple quoted string.
                scanner.bump();
                push_char(&spec, &mut text, &mut bytes, ch)?;
                continue;
            }
            scanner.bump();
            break;
        }
        if ch == '\n' || ch == '\r' {
            if !triple {
                return Err(err("unterminated string literal"));
            }
            // A line end inside a literal stands in the
            // value as one line feed, a carriage return and
            // its line feed together as much as either alone,
            // the interpreter's normalization of the source's
            // line ends inside a literal.
            scanner.bump();
            if ch == '\r' && scanner.peek() == Some('\n') {
                scanner.bump();
            }
            push_char(&spec, &mut text, &mut bytes, '\n')?;
            continue;
        }
        if ch == '\\' && !spec.raw {
            scanner.bump();
            decode_escape(scanner, &spec, &mut text, &mut bytes)?;
            continue;
        }
        if ch == '\\' && spec.raw {
            // A backslash in a raw string still keeps the
            // quote from ending the string, and both
            // characters stand in the value. A line end
            // standing after the backslash normalizes the
            // way any line end in the literal does, to the
            // one line feed.
            scanner.bump();
            push_char(&spec, &mut text, &mut bytes, '\\')?;
            match scanner.peek() {
                Some('\r') => {
                    scanner.bump();
                    if scanner.peek() == Some('\n') {
                        scanner.bump();
                    }
                    push_char(&spec, &mut text, &mut bytes, '\n')?;
                }
                Some(next) => {
                    scanner.bump();
                    push_char(&spec, &mut text, &mut bytes, next)?;
                }
                None => {}
            }
            continue;
        }
        scanner.bump();
        push_char(&spec, &mut text, &mut bytes, ch)?;
    }
    if spec.bytes {
        Ok(StrTok::Value(ConstVal::Bytes(bytes)))
    } else {
        Ok(StrTok::Value(ConstVal::Str(PyText::from_codepoints(text))))
    }
}

fn push_char(
    spec: &StrSpec,
    text: &mut Vec<u32>,
    bytes: &mut Vec<u8>,
    ch: char,
) -> Result<(), TokenError> {
    if spec.bytes {
        // A bytes literal holds literal ASCII characters
        // alone. A literal character past ASCII cannot
        // stand in one, where an escape may still name
        // any byte, so the refusal fires here rather
        // than the character truncating to one byte.
        if (ch as u32) > 0x7f {
            return Err(err("character past ASCII in a bytes literal"));
        }
        bytes.push(ch as u8);
    } else {
        text.push(ch as u32);
    }
    Ok(())
}

fn push_codepoint(
    spec: &StrSpec,
    text: &mut Vec<u32>,
    bytes: &mut Vec<u8>,
    cp: u32,
) -> Result<(), TokenError> {
    if spec.bytes {
        if cp > 0xff {
            return Err(err("character out of range in a bytes literal"));
        }
        bytes.push(cp as u8);
    } else {
        text.push(cp);
    }
    Ok(())
}

/// Decode one escape after its backslash, in a string or a
/// bytes literal according to the spec.
fn decode_escape(
    scanner: &mut Scanner,
    spec: &StrSpec,
    text: &mut Vec<u32>,
    bytes: &mut Vec<u8>,
) -> Result<(), TokenError> {
    let ch = match scanner.bump() {
        Some(ch) => ch,
        None => return Err(err("unterminated string literal")),
    };
    match ch {
        '\n' => Ok(()),
        '\r' => {
            if scanner.peek() == Some('\n') {
                scanner.bump();
            }
            Ok(())
        }
        '\\' => push_codepoint(spec, text, bytes, '\\' as u32),
        '\'' => push_codepoint(spec, text, bytes, '\'' as u32),
        '"' => push_codepoint(spec, text, bytes, '"' as u32),
        'a' => push_codepoint(spec, text, bytes, 7),
        'b' => push_codepoint(spec, text, bytes, 8),
        'f' => push_codepoint(spec, text, bytes, 12),
        'n' => push_codepoint(spec, text, bytes, 10),
        'r' => push_codepoint(spec, text, bytes, 13),
        't' => push_codepoint(spec, text, bytes, 9),
        'v' => push_codepoint(spec, text, bytes, 11),
        'x' => {
            let hi = scanner.bump().and_then(|c| c.to_digit(16));
            let lo = scanner.bump().and_then(|c| c.to_digit(16));
            match (hi, lo) {
                (Some(hi), Some(lo)) => push_codepoint(spec, text, bytes, hi * 16 + lo),
                _ => Err(err("truncated hex escape")),
            }
        }
        '0'..='7' => {
            let mut value = ch.to_digit(8).unwrap();
            for _ in 0..2 {
                match scanner.peek() {
                    Some(next @ '0'..='7') => {
                        scanner.bump();
                        value = value * 8 + next.to_digit(8).unwrap();
                    }
                    _ => break,
                }
            }
            if spec.bytes {
                bytes.push((value & 0xff) as u8);
                Ok(())
            } else {
                text.push(value);
                Ok(())
            }
        }
        'u' if !spec.bytes => {
            let mut value: u32 = 0;
            for _ in 0..4 {
                match scanner.bump().and_then(|c| c.to_digit(16)) {
                    Some(digit) => value = value * 16 + digit,
                    None => return Err(err("truncated unicode escape")),
                }
            }
            text.push(value);
            Ok(())
        }
        'U' if !spec.bytes => {
            let mut value: u32 = 0;
            for _ in 0..8 {
                match scanner.bump().and_then(|c| c.to_digit(16)) {
                    Some(digit) => value = value * 16 + digit,
                    None => return Err(err("truncated unicode escape")),
                }
            }
            if value > 0x10ffff {
                return Err(err("illegal Unicode character"));
            }
            text.push(value);
            Ok(())
        }
        'N' if !spec.bytes => {
            if scanner.bump() != Some('{') {
                return Err(err("malformed named character escape"));
            }
            let mut name = String::new();
            loop {
                match scanner.bump() {
                    Some('}') => break,
                    Some(next) => name.push(next),
                    None => return Err(err("malformed named character escape")),
                }
            }
            match lookup_name(&name) {
                Some(found) => {
                    text.push(found as u32);
                    Ok(())
                }
                None => Err(err("undefined character name")),
            }
        }
        other => {
            // An escape the language does not define keeps
            // both of its characters. The character after
            // the backslash is still a literal character of
            // the source, so a bytes literal refuses it
            // past ASCII the way it refuses it standing
            // alone, under the same wording.
            if spec.bytes && (other as u32) > 0x7f {
                return Err(err("character past ASCII in a bytes literal"));
            }
            push_codepoint(spec, text, bytes, '\\' as u32)?;
            push_codepoint(spec, text, bytes, other as u32)
        }
    }
}

/// Scan the parts of an f-string after its opening quote.
fn scan_fstring_parts(
    scanner: &mut Scanner,
    quote: char,
    triple: bool,
    raw: bool,
) -> Result<Vec<FPart>, TokenError> {
    let mut parts: Vec<FPart> = Vec::new();
    let mut literal: Vec<u32> = Vec::new();
    loop {
        let ch = match scanner.peek() {
            Some(ch) => ch,
            None => return Err(err("unterminated f-string")),
        };
        if ch == quote {
            if triple {
                if scanner.peek_at(1) == Some(quote) && scanner.peek_at(2) == Some(quote) {
                    scanner.bump();
                    scanner.bump();
                    scanner.bump();
                    break;
                }
                scanner.bump();
                literal.push(ch as u32);
                continue;
            }
            scanner.bump();
            break;
        }
        if (ch == '\n' || ch == '\r') && !triple {
            return Err(err("unterminated f-string"));
        }
        match ch {
            '\r' | '\n' => {
                // The literal text of an f-string carries
                // the interpreter's line end normalization,
                // in raw strings as in plain ones.
                scanner.bump();
                if ch == '\r' && scanner.peek() == Some('\n') {
                    scanner.bump();
                }
                literal.push('\n' as u32);
            }
            '{' => {
                if scanner.peek_at(1) == Some('{') {
                    scanner.bump();
                    scanner.bump();
                    literal.push('{' as u32);
                    continue;
                }
                if !literal.is_empty() {
                    parts.push(FPart::Lit(std::mem::take(&mut literal)));
                }
                // A field's brace counts on the one bracket
                // account with the code around the string.
                scanner.string_braces += 1;
                if scanner.bracket_floor + scanner.string_braces > 200 {
                    return Err(err("too many nested parentheses"));
                }
                let field = scan_field(scanner, raw, 1);
                scanner.string_braces -= 1;
                parts.push(FPart::Field(field?));
            }
            '}' => {
                if scanner.peek_at(1) == Some('}') {
                    scanner.bump();
                    scanner.bump();
                    literal.push('}' as u32);
                    continue;
                }
                return Err(err("f-string: single '}' is not allowed"));
            }
            '\\' if !raw => {
                // A brace after a backslash keeps its
                // structural reading, as it does in a spec,
                // and the backslash stands alone in the
                // value. Every other escape decodes whole.
                if matches!(scanner.peek_at(1), Some('{') | Some('}')) {
                    scanner.bump();
                    literal.push('\\' as u32);
                    continue;
                }
                scanner.bump();
                let mut text: Vec<u32> = Vec::new();
                let mut bytes: Vec<u8> = Vec::new();
                let spec = StrSpec {
                    raw: false,
                    bytes: false,
                    fstring: true,
                };
                decode_escape(scanner, &spec, &mut text, &mut bytes)?;
                literal.extend(text);
            }
            _ => {
                scanner.bump();
                literal.push(ch as u32);
            }
        }
    }
    if !literal.is_empty() {
        parts.push(FPart::Lit(literal));
    }
    Ok(parts)
}

/// Skip a string literal standing inside a field expression,
/// starting at its prefix or quote, returning nothing. The
/// scanner index lands past its closing quote.
fn skip_string(scanner: &mut Scanner) -> Result<(), TokenError> {
    // Read a prefix when one stands.
    let mut index = scanner.index;
    while index < scanner.chars.len() && is_name_part(scanner.chars[index]) {
        index += 1;
    }
    if index < scanner.chars.len()
        && (scanner.chars[index] == '"' || scanner.chars[index] == '\'')
        && index > scanner.index
    {
        let word: String = scanner.chars[scanner.index..index].iter().collect();
        if !is_string_prefix(&word) {
            return Err(err("not a string"));
        }
        scanner.index = index;
    }
    let quote = scanner.bump().ok_or_else(|| err("unterminated string"))?;
    if quote != '"' && quote != '\'' {
        return Err(err("not a string"));
    }
    let triple = scanner.peek() == Some(quote) && scanner.peek_at(1) == Some(quote);
    if triple {
        scanner.bump();
        scanner.bump();
    }
    loop {
        let ch = match scanner.bump() {
            Some(ch) => ch,
            None => return Err(err("unterminated string")),
        };
        if ch == '\\' {
            scanner.bump();
            continue;
        }
        if ch == quote {
            if triple {
                if scanner.peek() == Some(quote) && scanner.peek_at(1) == Some(quote) {
                    scanner.bump();
                    scanner.bump();
                    return Ok(());
                }
                continue;
            }
            return Ok(());
        }
        if (ch == '\n' || ch == '\r') && !triple {
            return Err(err("unterminated string"));
        }
    }
}

/// Scan one replacement field, the opening brace standing at
/// the scanner. The expression text is captured raw, with its
/// start position, for the parser to read as a fragment.
fn scan_field(
    scanner: &mut Scanner,
    raw: bool,
    field_depth: usize,
) -> Result<FieldData, TokenError> {
    let brace_pos = scanner.pos_at(scanner.index);
    scanner.bump(); // the opening brace
    let mut expr = String::new();
    let expr_start = scanner.index;
    let mut depth: usize = 0;
    let mut lambda_colons: usize = 0;
    let mut conversion: Option<char> = None;
    let mut spec: Option<Vec<FPart>> = None;
    let mut debug: Option<String> = None;
    loop {
        let ch = match scanner.peek() {
            Some(ch) => ch,
            None => return Err(err("unterminated f-string field")),
        };
        match ch {
            '"' | '\'' => {
                let start = scanner.index;
                skip_string(scanner)?;
                expr.extend(&scanner.chars[start..scanner.index]);
            }
            _ if is_name_start(ch) => {
                let start = scanner.index;
                while let Some(next) = scanner.peek() {
                    if is_name_part(next) {
                        scanner.bump();
                    } else {
                        break;
                    }
                }
                let word: String = scanner.chars[start..scanner.index].iter().collect();
                if matches!(scanner.peek(), Some('"') | Some('\'')) && is_string_prefix(&word) {
                    skip_string(scanner)?;
                    expr.extend(&scanner.chars[start..scanner.index]);
                } else {
                    if word == "lambda" && depth == 0 {
                        lambda_colons += 1;
                    }
                    expr.push_str(&word);
                }
            }
            '(' | '[' | '{' => {
                depth += 1;
                expr.push(ch);
                scanner.bump();
            }
            ')' | ']' => {
                depth = depth.saturating_sub(1);
                expr.push(ch);
                scanner.bump();
            }
            '}' if depth == 0 => {
                scanner.bump();
                break;
            }
            '}' => {
                depth -= 1;
                expr.push(ch);
                scanner.bump();
            }
            ':' if depth == 0 && lambda_colons == 0 => {
                scanner.bump();
                spec = Some(scan_spec(scanner, raw, field_depth)?);
                break;
            }
            ':' if depth == 0 => {
                lambda_colons -= 1;
                expr.push(ch);
                scanner.bump();
            }
            '!' if depth == 0 => {
                let conv = scanner.peek_at(1);
                let after = scanner.peek_at(2);
                if matches!(conv, Some('s') | Some('r') | Some('a'))
                    && matches!(after, Some(':') | Some('}'))
                {
                    conversion = Some(conv.unwrap());
                    scanner.bump();
                    scanner.bump();
                    if after == Some(':') {
                        scanner.bump();
                        spec = Some(scan_spec(scanner, raw, field_depth)?);
                    } else {
                        scanner.bump();
                    }
                    break;
                }
                expr.push(ch);
                scanner.bump();
            }
            '=' if depth == 0 => {
                // The self documenting form, a single equals
                // sign that no operator owns.
                let prev = expr.chars().rev().find(|c| !c.is_whitespace());
                let next = scanner.peek_at(1);
                if !matches!(
                    prev,
                    Some('=') | Some('<') | Some('>') | Some('!') | Some(':')
                ) && next != Some('=')
                {
                    // The debug text runs through the
                    // whitespace standing after the equals
                    // sign, which the interpreter keeps as
                    // part of the spelled text.
                    let mut tail = String::new();
                    scanner.bump();
                    while matches!(
                        scanner.peek(),
                        Some(' ') | Some('\t') | Some('\n') | Some('\r')
                    ) {
                        tail.push(scanner.bump().unwrap());
                    }
                    debug = Some(format!("{}={}", expr, tail));
                    // A conversion may follow the equals sign.
                    if scanner.peek() == Some('!') {
                        let conv = scanner.peek_at(1);
                        let after = scanner.peek_at(2);
                        if matches!(conv, Some('s') | Some('r') | Some('a'))
                            && matches!(after, Some(':') | Some('}'))
                        {
                            conversion = Some(conv.unwrap());
                            scanner.bump();
                            scanner.bump();
                            if after == Some(':') {
                                scanner.bump();
                                spec = Some(scan_spec(scanner, raw, field_depth)?);
                            } else {
                                scanner.bump();
                            }
                            return finish_field(
                                expr, expr_start, scanner, conversion, spec, debug, brace_pos,
                            );
                        }
                    }
                    if scanner.peek() == Some(':') {
                        scanner.bump();
                        spec = Some(scan_spec(scanner, raw, field_depth)?);
                    } else if scanner.peek() == Some('}') {
                        scanner.bump();
                    } else {
                        return Err(err("f-string: expecting '}' after the equals sign"));
                    }
                    break;
                }
                expr.push(ch);
                scanner.bump();
            }
            '#' => {
                // A comment inside a field of a multiline
                // f-string runs to the end of its line.
                while let Some(next) = scanner.peek() {
                    expr.push(next);
                    scanner.bump();
                    if next == '\n' {
                        break;
                    }
                }
            }
            _ => {
                expr.push(ch);
                scanner.bump();
            }
        }
    }
    finish_field(
        expr, expr_start, scanner, conversion, spec, debug, brace_pos,
    )
}

#[allow(clippy::too_many_arguments)]
fn finish_field(
    expr: String,
    expr_start: usize,
    scanner: &mut Scanner,
    conversion: Option<char>,
    spec: Option<Vec<FPart>>,
    debug: Option<String>,
    brace_pos: Pos,
) -> Result<FieldData, TokenError> {
    let pos = scanner.pos_at(expr_start);
    Ok(FieldData {
        expr,
        expr_line: pos.line,
        expr_col: pos.col,
        conversion,
        spec,
        debug,
        pos: brace_pos,
        bracket_depth: scanner.bracket_floor + scanner.string_braces,
        fstring_depth: scanner.fstring_depth,
    })
}

/// Scan a format spec, the f-string parts standing between
/// the colon and the closing brace of a field. The closing
/// brace is consumed. Literal text in a spec carries its
/// decoded value, exactly as the body literal of the string
/// does, because the interpreter decodes the escapes of a
/// spec in raw and plain strings alike. The braces keep
/// their structural reading over the decoding, so a brace
/// standing after a backslash still opens or closes a
/// field and the backslash stands alone in the value, and
/// a single opening brace always opens a nested field,
/// where a doubled pair opens one whose expression the
/// second brace begins. One corner of the raw reading
/// stands apart, measured against the interpreter. In a
/// raw string the name escape of a spec fails to decode
/// there, and the scan refuses it in the port's wording.
fn scan_spec(
    scanner: &mut Scanner,
    raw: bool,
    field_depth: usize,
) -> Result<Vec<FPart>, TokenError> {
    let mut parts: Vec<FPart> = Vec::new();
    let mut literal: Vec<u32> = Vec::new();
    loop {
        let ch = match scanner.peek() {
            Some(ch) => ch,
            None => return Err(err("unterminated f-string spec")),
        };
        match ch {
            '\r' | '\n' => {
                // Spec text is literal text of the string
                // and normalizes its line ends the same way.
                scanner.bump();
                if ch == '\r' && scanner.peek() == Some('\n') {
                    scanner.bump();
                }
                literal.push('\n' as u32);
            }
            '{' => {
                if !literal.is_empty() {
                    parts.push(FPart::Lit(std::mem::take(&mut literal)));
                }
                // Fields nested through format specs chain
                // three deep inside one string, the
                // interpreter's own bound on the shape; a
                // fourth refuses at the tokenizer. A nested
                // string begins the count again, as the
                // field expressions it stands in are read as
                // their own fragments.
                let next_depth = field_depth + 1;
                if next_depth > 3 {
                    return Err(err("f-string: expressions nested too deeply"));
                }
                scanner.string_braces += 1;
                if scanner.bracket_floor + scanner.string_braces > 200 {
                    return Err(err("too many nested parentheses"));
                }
                let field = scan_field(scanner, raw, next_depth);
                scanner.string_braces -= 1;
                parts.push(FPart::Field(field?));
            }
            '}' => {
                scanner.bump();
                break;
            }
            '\\' => {
                match scanner.peek_at(1) {
                    // A brace after a backslash keeps its
                    // structural reading, and the backslash
                    // stands alone in the value.
                    Some('{') | Some('}') => {
                        scanner.bump();
                        literal.push('\\' as u32);
                    }
                    Some('N') if raw && scanner.peek_at(2) == Some('{') => {
                        return Err(err("malformed named character escape"));
                    }
                    _ => {
                        scanner.bump();
                        let mut text: Vec<u32> = Vec::new();
                        let mut bytes: Vec<u8> = Vec::new();
                        let spec = StrSpec {
                            raw: false,
                            bytes: false,
                            fstring: true,
                        };
                        decode_escape(scanner, &spec, &mut text, &mut bytes)?;
                        literal.extend(text);
                    }
                }
            }
            _ => {
                scanner.bump();
                literal.push(ch as u32);
            }
        }
    }
    if !literal.is_empty() {
        parts.push(FPart::Lit(literal));
    } else if matches!(parts.last(), Some(FPart::Field(_))) {
        // A spec ending at a field end carries the trailing
        // empty constant the interpreter's reading appends.
        parts.push(FPart::Lit(Vec::new()));
    }
    Ok(parts)
}
