//! The ini reading of the Python configuration parser, ported
//! for the one construction the settings module uses, with no
//! interpolation and strict duplicate refusal. The port follows
//! the parser state machine as it stands. A file the parser
//! refuses comes back as a refusal. A file that cannot be read
//! at all comes back as an empty parse, because the Python read
//! entry passes over an unreadable file in silence, while a file
//! whose bytes are not text refuses the parse outright.

use std::collections::HashSet;
use std::path::Path;

/// Whether a character is whitespace to Python. The Python set
/// joins the ASCII whitespace and the file and group separators
/// to the Unicode whitespace set.
pub fn py_isspace(ch: char) -> bool {
    matches!(ch,
        '\t' | '\n' | '\u{b}' | '\u{c}' | '\r' | '\u{1c}' | '\u{1d}' | '\u{1e}' | '\u{1f}'
        | ' ' | '\u{85}' | '\u{a0}' | '\u{1680}' | '\u{2000}'..='\u{200a}' | '\u{2028}'
        | '\u{2029}' | '\u{202f}' | '\u{205f}' | '\u{3000}')
}

/// The Python strip of a string, at both ends or at the end.
pub fn py_trim(text: &str) -> String {
    text.trim_matches(py_isspace).to_string()
}

fn py_rstrip(text: &str) -> String {
    text.trim_end_matches(py_isspace).to_string()
}

/// The result of reading one ini file.
pub enum IniLoad {
    Parsed(IniFile),
    Refused,
}

/// One parsed ini file. Sections stand in file order, the default
/// section stands apart, and every value stands joined the way
/// the parser joins a multiline value at the end of its read.
pub struct IniFile {
    pub defaults: Vec<(String, String)>,
    pub sections: Vec<(String, Vec<(String, String)>)>,
}

impl IniFile {
    pub fn has_section(&self, section: &str) -> bool {
        self.sections.iter().any(|(name, _)| name == section)
    }

    fn lookup<'a>(pairs: &'a [(String, String)], key: &str) -> Option<&'a str> {
        pairs
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }

    pub fn has_option(&self, section: &str, key: &str) -> bool {
        if section == "DEFAULT" {
            return Self::lookup(&self.defaults, key).is_some();
        }
        match self.sections.iter().find(|(name, _)| name == section) {
            None => false,
            Some((_, pairs)) => {
                Self::lookup(pairs, key).is_some()
                    || Self::lookup(&self.defaults, key).is_some()
            }
        }
    }

    pub fn get(&self, section: &str, key: &str) -> Option<String> {
        if section == "DEFAULT" {
            return Self::lookup(&self.defaults, key).map(|value| value.to_string());
        }
        let (_, pairs) = self.sections.iter().find(|(name, _)| name == section)?;
        if let Some(value) = Self::lookup(pairs, key) {
            return Some(value.to_string());
        }
        Self::lookup(&self.defaults, key).map(|value| value.to_string())
    }
}

enum SectionRef {
    Defaults,
    Named(usize),
}

/// Read and parse one ini file at a path, under the rules above.
pub fn load_ini(path: &Path) -> IniLoad {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(_) => {
            return IniLoad::Parsed(IniFile {
                defaults: Vec::new(),
                sections: Vec::new(),
            })
        }
    };
    let text = match String::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => return IniLoad::Refused,
    };
    parse_ini(&text)
}

/// Parse ini text under the state machine the Python parser
/// runs. Any error the parser would raise, a duplicate section,
/// a duplicate option, a line before any section, or a line the
/// option reading cannot split, refuses the whole file.
pub fn parse_ini(text: &str) -> IniLoad {
    // The Python reader opens the file in text mode, where a
    // carriage return before a newline and a lone carriage return
    // both read as a newline.
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    let lines: Vec<&str> = normalized.split('\n').collect();

    let mut defaults: Vec<(String, Vec<String>)> = Vec::new();
    let mut sections: Vec<(String, Vec<(String, Vec<String>)>)> = Vec::new();
    let mut added_sections: HashSet<String> = HashSet::new();
    let mut added_options: HashSet<(String, String)> = HashSet::new();
    let mut current: Option<SectionRef> = None;
    let mut current_name = String::new();
    let mut optname: Option<String> = None;
    let mut indent_level: usize = 0;

    fn store(
        defaults: &mut Vec<(String, Vec<String>)>,
        sections: &mut Vec<(String, Vec<(String, Vec<String>)>)>,
        current: &SectionRef,
        key: &str,
        parts: Vec<String>,
    ) {
        let pairs = match current {
            SectionRef::Defaults => defaults,
            SectionRef::Named(index) => &mut sections[*index].1,
        };
        match pairs.iter_mut().find(|(name, _)| name == key) {
            Some((_, existing)) => *existing = parts,
            None => pairs.push((key.to_string(), parts)),
        }
    }

    fn append_part(
        defaults: &mut Vec<(String, Vec<String>)>,
        sections: &mut Vec<(String, Vec<(String, Vec<String>)>)>,
        current: &SectionRef,
        key: &str,
        part: String,
    ) {
        let pairs = match current {
            SectionRef::Defaults => defaults,
            SectionRef::Named(index) => &mut sections[*index].1,
        };
        if let Some((_, existing)) = pairs.iter_mut().find(|(name, _)| name == key) {
            existing.push(part);
        }
    }

    fn current_has_value(
        defaults: &[(String, Vec<String>)],
        sections: &[(String, Vec<(String, Vec<String>)>)],
        current: &SectionRef,
        key: &str,
    ) -> bool {
        let pairs = match current {
            SectionRef::Defaults => defaults,
            SectionRef::Named(index) => &sections[*index].1,
        };
        pairs.iter().any(|(name, _)| name == key)
    }

    for line in &lines {
        let trimmed_start = line.trim_start_matches(py_isspace);
        let comment_line = trimmed_start.starts_with('#') || trimmed_start.starts_with(';');
        let value = if comment_line {
            String::new()
        } else {
            py_trim(line)
        };
        if value.is_empty() {
            // A blank line joins the value it stands inside, as
            // an empty part. A comment line only passes over.
            if !comment_line {
                if let (Some(section), Some(key)) = (&current, &optname) {
                    if current_has_value(&defaults, &sections, section, key) {
                        append_part(&mut defaults, &mut sections, section, key, String::new());
                    }
                }
            }
            continue;
        }
        let chars: Vec<char> = line.chars().collect();
        let cur_indent = chars
            .iter()
            .position(|ch| !py_isspace(*ch))
            .unwrap_or(0);
        let is_continuation = match (&current, &optname) {
            (Some(_), Some(key)) => !key.is_empty() && cur_indent > indent_level,
            _ => false,
        };
        if is_continuation {
            let section = current.as_ref().unwrap();
            let key = optname.as_ref().unwrap();
            if current_has_value(&defaults, &sections, section, key) {
                append_part(
                    &mut defaults,
                    &mut sections,
                    section,
                    key,
                    value.clone(),
                );
            }
            continue;
        }
        indent_level = cur_indent;
        let value_chars: Vec<char> = value.chars().collect();
        let section_header = if value_chars.first() == Some(&'[') {
            // The section reading takes the header greedily to
            // the last closing bracket on the line.
            let mut header: Option<String> = None;
            for pos in (2..value_chars.len()).rev() {
                if value_chars[pos] == ']' {
                    header = Some(value_chars[1..pos].iter().collect());
                    break;
                }
            }
            header
        } else {
            None
        };
        if let Some(header) = section_header {
            if header == "DEFAULT" {
                current = Some(SectionRef::Defaults);
                current_name = header;
            } else if let Some(index) =
                sections.iter().position(|(name, _)| *name == header)
            {
                if added_sections.contains(&header) {
                    return IniLoad::Refused;
                }
                added_sections.insert(header.clone());
                current = Some(SectionRef::Named(index));
                current_name = header;
            } else {
                sections.push((header.clone(), Vec::new()));
                added_sections.insert(header.clone());
                current = Some(SectionRef::Named(sections.len() - 1));
                current_name = header;
            }
            optname = None;
            continue;
        }
        if current.is_none() {
            return IniLoad::Refused;
        }
        // An option line. The option is the text before the first
        // delimiter on the line, with its trailing whitespace
        // taken away and its case folded down.
        let delimiter = value_chars
            .iter()
            .position(|ch| *ch == '=' || *ch == ':');
        let delimiter = match delimiter {
            Some(pos) => pos,
            None => return IniLoad::Refused,
        };
        let raw_option: String = value_chars[..delimiter].iter().collect();
        if raw_option.is_empty() {
            return IniLoad::Refused;
        }
        let option = py_rstrip(&raw_option).to_lowercase();
        if added_options.contains(&(current_name.clone(), option.clone())) {
            return IniLoad::Refused;
        }
        added_options.insert((current_name.clone(), option.clone()));
        let raw_value: String = value_chars[delimiter + 1..].iter().collect();
        let optval = py_trim(&raw_value);
        let section = current.as_ref().unwrap();
        store(
            &mut defaults,
            &mut sections,
            section,
            &option,
            vec![optval],
        );
        optname = Some(option);
    }

    let join = |pairs: Vec<(String, Vec<String>)>| -> Vec<(String, String)> {
        pairs
            .into_iter()
            .map(|(key, parts)| {
                let joined = parts.join("\n");
                (key, py_rstrip(&joined))
            })
            .collect()
    };
    IniLoad::Parsed(IniFile {
        defaults: join(defaults),
        sections: sections
            .into_iter()
            .map(|(name, pairs)| (name, join(pairs)))
            .collect(),
    })
}
