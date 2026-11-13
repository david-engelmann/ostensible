//! The glob reading of the Python fnmatch module, reproduced at
//! the level of its translation. The translation the Python
//! module builds is a regular expression with an unusual shape.
//! Interior star and fixed pairings become atomic groups that
//! commit to the earliest place the fixed part matches, and the
//! character class rewriting removes empty ranges and escapes set
//! operations. This port builds the same program of atoms and
//! runs it with the same commitments, so a name matches here
//! exactly where the Python reading matches it.

/// One member of a character class.
#[derive(Debug, Clone, PartialEq)]
enum ClassItem {
    Ch(char),
    Range(char, char),
}

/// One pattern atom, matching exactly one character or none.
#[derive(Debug, Clone, PartialEq)]
enum Atom {
    Ch(char),
    Any,
    Class {
        negated: bool,
        items: Vec<ClassItem>,
    },
    Never,
}

/// One step of a translated pattern.
#[derive(Debug, Clone, PartialEq)]
enum Node {
    Atom(Atom),
    /// A star with the pattern still open after it, at the tail.
    Star,
    /// An interior star and fixed pairing. The star grows from
    /// nothing upward and the fixed atoms are tried at each
    /// length, and the first length at which they match commits,
    /// with no return to a longer growth when the rest fails.
    AtomicStar(Vec<Atom>),
}

fn atom_matches(atom: &Atom, ch: char) -> bool {
    match atom {
        Atom::Ch(expected) => *expected == ch,
        Atom::Any => true,
        Atom::Never => false,
        Atom::Class { negated, items } => {
            let mut found = false;
            for item in items {
                match item {
                    ClassItem::Ch(member) => {
                        if *member == ch {
                            found = true;
                        }
                    }
                    ClassItem::Range(low, high) => {
                        if *low <= ch && ch <= *high {
                            found = true;
                        }
                    }
                }
            }
            found != *negated
        }
    }
}

/// Parse the body of a character class, after the translation has
/// rewritten it, into class items. A backslash quotes the next
/// character, and a hyphen between two characters joins them
/// into a range.
fn parse_class_body(body: &[char]) -> Vec<ClassItem> {
    let mut items = Vec::new();
    let mut i = 0;
    let read_char = |body: &[char], i: &mut usize| -> char {
        if body[*i] == '\\' && *i + 1 < body.len() {
            let ch = body[*i + 1];
            *i += 2;
            ch
        } else {
            let ch = body[*i];
            *i += 1;
            ch
        }
    };
    while i < body.len() {
        let first = read_char(body, &mut i);
        if i < body.len() && body[i] == '-' && i + 1 < body.len() {
            i += 1;
            let last = read_char(body, &mut i);
            items.push(ClassItem::Range(first, last));
        } else {
            items.push(ClassItem::Ch(first));
        }
    }
    items
}

/// Translate one character class starting just after its opening
/// bracket. Returns the atom and the index just past the class,
/// or None when the bracket never closes and stands as a literal.
fn translate_class(pat: &[char], i: usize) -> Option<(Atom, usize)> {
    let n = pat.len();
    let mut j = i;
    if j < n && pat[j] == '!' {
        j += 1;
    }
    if j < n && pat[j] == ']' {
        j += 1;
    }
    while j < n && pat[j] != ']' {
        j += 1;
    }
    if j >= n {
        return None;
    }
    let mut stuff: Vec<char> = pat[i..j].to_vec();
    if !stuff.contains(&'-') {
        let mut doubled = Vec::new();
        for ch in &stuff {
            if *ch == '\\' {
                doubled.push('\\');
            }
            doubled.push(*ch);
        }
        stuff = doubled;
    } else {
        let mut chunks: Vec<Vec<char>> = Vec::new();
        let mut start = i;
        let mut k: i64 = if pat[i] == '!' {
            i as i64 + 2
        } else {
            i as i64 + 1
        };
        loop {
            let mut found: i64 = -1;
            let mut probe = k;
            while probe < j as i64 {
                if pat[probe as usize] == '-' {
                    found = probe;
                    break;
                }
                probe += 1;
            }
            if found < 0 {
                break;
            }
            chunks.push(pat[start..found as usize].to_vec());
            start = (found + 1) as usize;
            k = found + 3;
        }
        let chunk: Vec<char> = pat[start..j].to_vec();
        if !chunk.is_empty() {
            chunks.push(chunk);
        } else {
            chunks.last_mut().unwrap().push('-');
        }
        let mut idx = chunks.len() - 1;
        while idx >= 1 {
            let prev_last = *chunks[idx - 1].last().unwrap();
            let this_first = chunks[idx][0];
            if prev_last > this_first {
                let mut merged: Vec<char> = chunks[idx - 1][..chunks[idx - 1].len() - 1].to_vec();
                merged.extend_from_slice(&chunks[idx][1..]);
                chunks[idx - 1] = merged;
                chunks.remove(idx);
            }
            idx -= 1;
        }
        let mut joined: Vec<char> = Vec::new();
        for (position, chunk) in chunks.iter().enumerate() {
            if position > 0 {
                joined.push('-');
            }
            for ch in chunk {
                if *ch == '\\' {
                    joined.push('\\');
                }
                if *ch == '-' {
                    joined.push('\\');
                }
                joined.push(*ch);
            }
        }
        stuff = joined;
    }
    let mut escaped: Vec<char> = Vec::new();
    for ch in &stuff {
        if *ch == '&' || *ch == '~' || *ch == '|' {
            escaped.push('\\');
        }
        escaped.push(*ch);
    }
    let stuff = escaped;
    if stuff.is_empty() {
        return Some((Atom::Never, j + 1));
    }
    if stuff == ['!'] {
        return Some((Atom::Any, j + 1));
    }
    if stuff[0] == '!' {
        let body = &stuff[1..];
        return Some((
            Atom::Class {
                negated: true,
                items: parse_class_body(body),
            },
            j + 1,
        ));
    }
    if stuff[0] == '^' || stuff[0] == '[' {
        let mut body: Vec<char> = vec!['\\'];
        body.extend_from_slice(&stuff);
        return Some((
            Atom::Class {
                negated: false,
                items: parse_class_body(&body),
            },
            j + 1,
        ));
    }
    Some((
        Atom::Class {
            negated: false,
            items: parse_class_body(&stuff),
        },
        j + 1,
    ))
}

/// Translate a pattern into the program of nodes the Python
/// translation describes.
fn translate(pat: &[char]) -> Vec<Node> {
    let n = pat.len();
    let mut pieces: Vec<Piece> = Vec::new();
    let mut i = 0;
    while i < n {
        let c = pat[i];
        i += 1;
        if c == '*' {
            if !matches!(pieces.last(), Some(Piece::Star)) {
                pieces.push(Piece::Star);
            }
        } else if c == '?' {
            pieces.push(Piece::Atom(Atom::Any));
        } else if c == '[' {
            match translate_class(pat, i) {
                Some((atom, next)) => {
                    pieces.push(Piece::Atom(atom));
                    i = next;
                }
                None => {
                    pieces.push(Piece::Atom(Atom::Ch('[')));
                }
            }
        } else {
            pieces.push(Piece::Atom(Atom::Ch(c)));
        }
    }
    let mut nodes: Vec<Node> = Vec::new();
    let mut i = 0;
    while i < pieces.len() {
        match &pieces[i] {
            Piece::Atom(atom) => {
                nodes.push(Node::Atom(atom.clone()));
                i += 1;
            }
            Piece::Star => break,
        }
    }
    while i < pieces.len() {
        // pieces[i] is a star here, as stars and atoms alternate
        // in runs and every run boundary was left at a star.
        i += 1;
        if i == pieces.len() {
            nodes.push(Node::Star);
            break;
        }
        let mut fixed: Vec<Atom> = Vec::new();
        while i < pieces.len() {
            match &pieces[i] {
                Piece::Atom(atom) => {
                    fixed.push(atom.clone());
                    i += 1;
                }
                Piece::Star => break,
            }
        }
        if i == pieces.len() {
            nodes.push(Node::Star);
            for atom in fixed {
                nodes.push(Node::Atom(atom));
            }
        } else {
            nodes.push(Node::AtomicStar(fixed));
        }
    }
    nodes
}

#[derive(Debug, Clone, PartialEq)]
enum Piece {
    Star,
    Atom(Atom),
}

fn atoms_match_at(atoms: &[Atom], text: &[char], pos: usize) -> bool {
    if pos + atoms.len() > text.len() {
        return false;
    }
    atoms
        .iter()
        .enumerate()
        .all(|(shift, atom)| atom_matches(atom, text[pos + shift]))
}

fn run(nodes: &[Node], text: &[char], pos: usize) -> bool {
    if nodes.is_empty() {
        return pos == text.len();
    }
    match &nodes[0] {
        Node::Atom(atom) => {
            pos < text.len() && atom_matches(atom, text[pos]) && run(&nodes[1..], text, pos + 1)
        }
        Node::Star => {
            let mut growth = 0;
            loop {
                if run(&nodes[1..], text, pos + growth) {
                    return true;
                }
                if pos + growth >= text.len() {
                    return false;
                }
                growth += 1;
            }
        }
        Node::AtomicStar(atoms) => {
            let mut growth = 0;
            loop {
                if pos + growth + atoms.len() > text.len() {
                    return false;
                }
                if atoms_match_at(atoms, text, pos + growth) {
                    return run(&nodes[1..], text, pos + growth + atoms.len());
                }
                growth += 1;
            }
        }
    }
}

/// The fnmatchcase reading. The name matches the pattern as the
/// glob the Python module translates it to, anchored at both
/// ends, with every character standing for itself in the name.
pub fn fnmatchcase(name: &str, pattern: &str) -> bool {
    let name_chars: Vec<char> = name.chars().collect();
    let pat_chars: Vec<char> = pattern.chars().collect();
    let nodes = translate(&pat_chars);
    run(&nodes, &name_chars, 0)
}
