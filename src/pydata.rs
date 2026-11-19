//! Unicode data the port readings lean on, carried as data
//! files beside this module and read at first use. The
//! printable ranges state which codepoints CPython's repr
//! leaves unescaped, and the name table states the character
//! names the string escape reading resolves, and the normalization
//! tables state the decomposition, combining class, and composition
//! data the name normalization of PORT20 drives, all generated
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

/// The printable reading of one codepoint. A lone
/// surrogate is never printable, as the interpreter's
/// category reading states, and a codepoint past the
/// Unicode range never stands as a character at all.
pub fn is_printable_cp(point: u32) -> bool {
    match char::from_u32(point) {
        Some(ch) => is_printable(ch),
        None => false,
    }
}

/// The escape spelling of one codepoint the repr
/// cannot stand as itself, on the interpreter's tiers.
pub fn unicode_escape_cp(point: u32) -> String {
    match point {
        0x5c => "\\\\".to_string(),
        0x0a => "\\n".to_string(),
        0x0d => "\\r".to_string(),
        0x09 => "\\t".to_string(),
        _ => {
            if point < 0x100 {
                format!("\\x{:02x}", point)
            } else if point < 0x10000 {
                format!("\\u{:04x}", point)
            } else {
                format!("\\U{:08x}", point)
            }
        }
    }
}

/// The Python repr form of a string, over codepoints,
/// with its quote choice and its escapes. This is the
/// one string repr implementation the crate carries,
/// under PORT9-1. The unparse spells through it and the
/// value world's repr converts its strings to
/// codepoints and calls it, so no second copy stands.
pub fn str_repr(value: &[u32]) -> String {
    let quote = if value.contains(&0x27) && !value.contains(&0x22) {
        '"'
    } else {
        '\''
    };
    let mut out = String::new();
    out.push(quote);
    for point in value {
        if *point == quote as u32 || *point == 0x5c {
            out.push('\\');
            out.push(char::from_u32(*point).unwrap());
        } else if *point == 0x0a {
            out.push_str("\\n");
        } else if *point == 0x0d {
            out.push_str("\\r");
        } else if *point == 0x09 {
            out.push_str("\\t");
        } else if is_printable_cp(*point) {
            out.push(char::from_u32(*point).unwrap());
        } else {
            out.push_str(&unicode_escape_cp(*point));
        }
    }
    out.push(quote);
    out
}

static NFKC_DECOMP: OnceLock<HashMap<u32, Vec<char>>> = OnceLock::new();
static NFKC_CCC: OnceLock<HashMap<u32, u8>> = OnceLock::new();
static NFKC_COMP: OnceLock<HashMap<(char, char), char>> = OnceLock::new();

fn nfkc_decomp() -> &'static HashMap<u32, Vec<char>> {
    NFKC_DECOMP.get_or_init(|| {
        let text = include_str!("nfkc_tables.txt");
        let mut map = HashMap::new();
        for line in text.lines() {
            let mut fields = line.split_whitespace();
            if fields.next() != Some("D") {
                continue;
            }
            let cp = fields
                .next()
                .map(|field| u32::from_str_radix(field, 16).unwrap_or(0))
                .unwrap_or(0);
            let parts: Vec<char> = fields
                .filter_map(|field| u32::from_str_radix(field, 16).ok())
                .filter_map(char::from_u32)
                .collect();
            if !parts.is_empty() {
                map.insert(cp, parts);
            }
        }
        map
    })
}

fn nfkc_ccc() -> &'static HashMap<u32, u8> {
    NFKC_CCC.get_or_init(|| {
        let text = include_str!("nfkc_tables.txt");
        let mut map = HashMap::new();
        for line in text.lines() {
            let mut fields = line.split_whitespace();
            if fields.next() != Some("C") {
                continue;
            }
            if let (Some(cp), Some(class)) = (fields.next(), fields.next()) {
                if let (Ok(cp), Ok(class)) = (u32::from_str_radix(cp, 16), class.parse::<u8>()) {
                    map.insert(cp, class);
                }
            }
        }
        map
    })
}

fn nfkc_comp() -> &'static HashMap<(char, char), char> {
    NFKC_COMP.get_or_init(|| {
        let text = include_str!("nfkc_tables.txt");
        let mut map = HashMap::new();
        for line in text.lines() {
            let mut fields = line.split_whitespace();
            if fields.next() != Some("P") {
                continue;
            }
            let parsed: Vec<char> = fields
                .filter_map(|field| u32::from_str_radix(field, 16).ok())
                .filter_map(char::from_u32)
                .collect();
            if parsed.len() == 3 {
                map.insert((parsed[0], parsed[1]), parsed[2]);
            }
        }
        map
    })
}

fn combining_class(ch: char) -> u8 {
    nfkc_ccc().get(&(ch as u32)).copied().unwrap_or(0)
}

const HANGUL_S_BASE: u32 = 0xac00;
const HANGUL_L_BASE: u32 = 0x1100;
const HANGUL_V_BASE: u32 = 0x1161;
const HANGUL_T_BASE: u32 = 0x11a7;
const HANGUL_L_COUNT: u32 = 19;
const HANGUL_V_COUNT: u32 = 21;
const HANGUL_T_COUNT: u32 = 28;
const HANGUL_N_COUNT: u32 = HANGUL_V_COUNT * HANGUL_T_COUNT;
const HANGUL_S_COUNT: u32 = HANGUL_L_COUNT * HANGUL_N_COUNT;

fn decompose_char(ch: char, out: &mut Vec<char>) {
    let cp = ch as u32;
    if (HANGUL_S_BASE..HANGUL_S_BASE + HANGUL_S_COUNT).contains(&cp) {
        let s = cp - HANGUL_S_BASE;
        let lead = char::from_u32(HANGUL_L_BASE + s / HANGUL_N_COUNT).unwrap();
        let vowel = char::from_u32(HANGUL_V_BASE + (s % HANGUL_N_COUNT) / HANGUL_T_COUNT).unwrap();
        out.push(lead);
        out.push(vowel);
        let tail = s % HANGUL_T_COUNT;
        if tail != 0 {
            out.push(char::from_u32(HANGUL_T_BASE + tail).unwrap());
        }
        return;
    }
    match nfkc_decomp().get(&cp) {
        Some(parts) => {
            for &part in parts.iter() {
                decompose_char(part, out);
            }
        }
        None => out.push(ch),
    }
}

/// Return the name under its normalized form, the
/// compatibility normalization the arbiter applies to
/// every identifier it reads, NFKC at Unicode 15.0.0
/// over the data standing in nfkc_tables.txt. A name
/// standing in plain ASCII stands already normalized
/// and returns as written.
pub fn normalize_identifier(raw: &str) -> String {
    if raw.is_ascii() {
        return raw.to_string();
    }
    let mut seq: Vec<char> = Vec::new();
    for ch in raw.chars() {
        decompose_char(ch, &mut seq);
    }
    for i in 1..seq.len() {
        let class = combining_class(seq[i]);
        if class == 0 {
            continue;
        }
        let mut j = i;
        while j > 0 && combining_class(seq[j - 1]) > class {
            seq.swap(j, j - 1);
            j -= 1;
        }
    }
    let mut out: Vec<char> = Vec::new();
    let mut last_starter = 0usize;
    let mut last_class = 0u8;
    for (index, ch) in seq.into_iter().enumerate() {
        if index == 0 {
            out.push(ch);
            continue;
        }
        let class = combining_class(ch);
        let mut composed = None;
        if last_class == 0 || last_class < class {
            let starter = out[last_starter];
            let s_cp = starter as u32;
            let cp = ch as u32;
            if (HANGUL_S_BASE..HANGUL_S_BASE + HANGUL_S_COUNT).contains(&s_cp)
                && cp > HANGUL_T_BASE
                && cp < HANGUL_T_BASE + HANGUL_T_COUNT
                && (s_cp - HANGUL_S_BASE) % HANGUL_T_COUNT == 0
            {
                composed = char::from_u32(s_cp + (cp - HANGUL_T_BASE));
            } else if (HANGUL_L_BASE..HANGUL_L_BASE + HANGUL_L_COUNT).contains(&s_cp)
                && (HANGUL_V_BASE..HANGUL_V_BASE + HANGUL_V_COUNT).contains(&cp)
            {
                composed = char::from_u32(
                    HANGUL_S_BASE
                        + ((s_cp - HANGUL_L_BASE) * HANGUL_V_COUNT + (cp - HANGUL_V_BASE))
                            * HANGUL_T_COUNT,
                );
            } else {
                composed = nfkc_comp().get(&(starter, ch)).copied();
            }
        }
        match composed {
            Some(joined) => {
                out[last_starter] = joined;
            }
            None => {
                out.push(ch);
                if class == 0 {
                    last_starter = out.len() - 1;
                }
                last_class = class;
            }
        }
    }
    out.into_iter().collect()
}
