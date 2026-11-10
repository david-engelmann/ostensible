//! Unicode data the port readings lean on, carried as data
//! files beside this module and read at first use. The
//! printable ranges state which codepoints CPython's repr
//! leaves unescaped, and the name table states the character
//! names the string escape reading resolves, both generated
//! from the interpreter's own unicode database under the
//! generator standing in the round scratch. Names the
//! database forms algorithmically, the Hangul syllables and
//! the ideographs whose name ends in their own codepoint, are
//! resolved by rule here instead of standing in the table.

use std::collections::HashMap;
use std::sync::OnceLock;

static PRINTABLE: OnceLock<Vec<(u32, u32)>> = OnceLock::new();
static NAMES: OnceLock<HashMap<String, u32>> = OnceLock::new();

fn printable_ranges() -> &'static Vec<(u32, u32)> {
    PRINTABLE.get_or_init(|| {
        let text = include_str!("printable_ranges.txt");
        let mut ranges = Vec::new();
        for line in text.lines() {
            if let Some((lo, hi)) = line.split_once('-') {
                let lo = u32::from_str_radix(lo, 16).unwrap_or(0);
                let hi = u32::from_str_radix(hi, 16).unwrap_or(0);
                ranges.push((lo, hi));
            }
        }
        ranges
    })
}

/// Return true when CPython's repr leaves the character
/// unescaped, the printable reading of the interpreter.
pub fn is_printable(ch: char) -> bool {
    let cp = ch as u32;
    let ranges = printable_ranges();
    let mut lo = 0usize;
    let mut hi = ranges.len();
    while lo < hi {
        let mid = (lo + hi) / 2;
        let (start, end) = ranges[mid];
        if cp < start {
            hi = mid;
        } else if cp > end {
            lo = mid + 1;
        } else {
            return true;
        }
    }
    false
}

fn name_table() -> &'static HashMap<String, u32> {
    NAMES.get_or_init(|| {
        let text = include_str!("unicode_names.txt");
        let mut map = HashMap::new();
        for line in text.lines() {
            if let Some((name, hex)) = line.rsplit_once(' ') {
                if let Ok(cp) = u32::from_str_radix(hex, 16) {
                    map.insert(name.to_string(), cp);
                }
            }
        }
        map
    })
}

/// Normalize a character name the loose way the interpreter's
/// lookup reads it, upper case with spaces and hyphens
/// standing outside the comparison.
fn normalize_name(name: &str) -> String {
    name.chars()
        .filter(|ch| *ch != ' ' && *ch != '-')
        .flat_map(|ch| ch.to_uppercase())
        .collect()
}

const HANGUL_INITIALS: [&str; 19] = [
    "G", "GG", "N", "D", "DD", "R", "M", "B", "BB", "S", "SS", "", "J", "JJ", "C", "K", "T", "P",
    "H",
];
const HANGUL_MEDIALS: [&str; 21] = [
    "A", "AE", "YA", "YAE", "EO", "E", "YEO", "YE", "O", "YO", "U", "YU", "WA", "WAE", "OE", "WO",
    "WE", "WI", "EU", "YI", "I",
];
const HANGUL_FINALS: [&str; 28] = [
    "", "G", "GG", "GS", "N", "NJ", "NH", "D", "L", "LG", "LM", "LB", "LS", "LT", "LP", "LH", "M",
    "B", "BS", "S", "SS", "NG", "J", "C", "K", "T", "P", "H",
];

/// Resolve an algorithmic Hangul syllable name to its
/// codepoint, or None when the name is not one.
fn hangul_lookup(normalized: &str) -> Option<u32> {
    let rest = normalized.strip_prefix("HANGULSYLLABLE")?;
    // Try each initial, then a medial, then an optional final,
    // the syllable the three parts compose.
    for (ii, initial) in HANGUL_INITIALS.iter().enumerate() {
        let after = if initial.is_empty() {
            rest
        } else {
            match rest.strip_prefix(initial) {
                Some(after) => after,
                None => continue,
            }
        };
        for (mi, medial) in HANGUL_MEDIALS.iter().enumerate() {
            let tail = match after.strip_prefix(medial) {
                Some(tail) => tail,
                None => continue,
            };
            for (fi, final_) in HANGUL_FINALS.iter().enumerate() {
                let matches = if fi == 0 {
                    tail.is_empty()
                } else {
                    tail == *final_
                };
                if matches {
                    let code = 0xAC00 + ((ii as u32 * 21) + mi as u32) * 28 + fi as u32;
                    return Some(code);
                }
            }
        }
    }
    None
}

/// Resolve a character name to its codepoint, the reading the
/// string escape for a named character makes, or None when
/// the name stands unknown.
pub fn lookup_name(name: &str) -> Option<char> {
    let normalized = normalize_name(name);
    if let Some(cp) = name_table().get(&normalized) {
        return char::from_u32(*cp);
    }
    if let Some(cp) = hangul_lookup(&normalized) {
        return char::from_u32(cp);
    }
    // Names the database forms from the codepoint itself, the
    // unified and compatibility ideographs, the Tangut
    // ideographs, and the Khitan characters.
    for prefix in [
        "CJKUNIFIEDIDEOGRAPH",
        "CJKCOMPATIBILITYIDEOGRAPH",
        "TANGUTIDEOGRAPH",
        "KHITANCHARACTER",
        "IDEOGRAPH",
    ] {
        if let Some(hex) = normalized.strip_prefix(prefix) {
            if let Ok(cp) = u32::from_str_radix(hex, 16) {
                if char::from_u32(cp).is_some() {
                    return char::from_u32(cp);
                }
            }
        }
    }
    None
}
