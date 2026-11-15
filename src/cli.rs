//! The command line door, stated by PORT11. The door reads one
//! positional project and the flags --json and --timeout, on the
//! acceptance the port's probes recorded for the Python parser
//! it replaces, a flag abbreviated to a unique prefix, the
//! equals form for a valued flag, flags standing in any order
//! against the positional, a repeated valued flag keeping its
//! last value, a negative number reading as a value, and the
//! separator -- making every token after it positional. The
//! usage and help and error texts are the door's own, the
//! Python parser's texts standing outside parity under the
//! divergence PORT11-2 states.

use std::ffi::OsString;
use std::path::{Path, PathBuf};

use crate::report::{to_json, to_text};
use crate::runner::analyse_project;

/// The one line every usage refusal prints.
pub const USAGE: &str = "usage: ostensible [--json] [--timeout SECONDS] <project>";

const HELP: &str = "usage: ostensible [--json] [--timeout SECONDS] <project>

Name the tests that no mutation in a fixed panel can turn red.

options:
  -h, --help  show this help and exit
  --json  print the report as JSON instead of text
  --timeout SECONDS  the timeout in seconds for each single test file run, standing at 30 where no flag gives it
";

const LONG_FLAGS: [&str; 3] = ["--help", "--json", "--timeout"];

enum Parsed {
    Help,
    Run {
        project: PathBuf,
        project_text: String,
        json: bool,
        timeout: f64,
    },
    Refuse,
}

/// A token reads as a negative number, and so as a value or a
/// positional and never as an option, when it opens with a dash
/// and a digit, or a dash and a point and a digit.
fn is_negative_number(token: &str) -> bool {
    let rest = match token.strip_prefix('-') {
        Some(rest) => rest,
        None => return false,
    };
    rest.chars().next().is_some_and(|c| c.is_ascii_digit())
        || rest
            .strip_prefix('.')
            .is_some_and(|r| r.chars().next().is_some_and(|c| c.is_ascii_digit()))
}

fn is_option_like(token: &str) -> bool {
    token.len() > 1 && token.starts_with('-') && !is_negative_number(token)
}

/// Read a timeout value the way a Python float reads, the
/// spellings the two readers share. An underscore stands only
/// between two digits, as the Python reader requires, and the
/// value may carry whitespace at its ends.
fn read_timeout(text: &str) -> Option<f64> {
    let trimmed = text.trim_matches(|c: char| c.is_ascii_whitespace());
    if trimmed.is_empty() {
        return None;
    }
    let bytes = trimmed.as_bytes();
    let mut cleaned = String::with_capacity(trimmed.len());
    for (index, ch) in trimmed.chars().enumerate() {
        if ch == '_' {
            let before = index > 0 && bytes[index - 1].is_ascii_digit();
            let after = index + 1 < bytes.len() && bytes[index + 1].is_ascii_digit();
            if !before || !after {
                return None;
            }
        } else {
            cleaned.push(ch);
        }
    }
    cleaned.parse::<f64>().ok()
}

fn parse(args: &[OsString]) -> Parsed {
    let mut project: Option<(PathBuf, String)> = None;
    let mut json = false;
    let mut timeout: f64 = 30.0;
    let mut deferred_refusal = false;
    let mut separated = false;
    let mut index = 0;
    while index < args.len() {
        let arg = &args[index];
        index += 1;
        if separated {
            if project.is_none() {
                project = Some((PathBuf::from(arg), arg.to_string_lossy().into_owned()));
            } else {
                deferred_refusal = true;
            }
            continue;
        }
        let token = arg.to_str();
        let text = match token {
            Some(text) => text,
            None => {
                // A token that is not text is never a flag. It can
                // only stand as the positional, in its own bytes.
                if project.is_none() {
                    project = Some((PathBuf::from(arg), arg.to_string_lossy().into_owned()));
                } else {
                    deferred_refusal = true;
                }
                continue;
            }
        };
        if text == "--" {
            separated = true;
            continue;
        }
        if !is_option_like(text) {
            if project.is_none() {
                project = Some((PathBuf::from(arg), text.to_string()));
            } else {
                deferred_refusal = true;
            }
            continue;
        }
        if let Some(long) = text.strip_prefix("--") {
            let (name, inline) = match long.split_once('=') {
                Some((name, value)) => (format!("--{name}"), Some(value.to_string())),
                None => (text.to_string(), None),
            };
            let matched: Vec<&str> = LONG_FLAGS
                .iter()
                .filter(|flag| flag == &&name || flag.starts_with(&name))
                .copied()
                .collect();
            if matched.len() != 1 {
                deferred_refusal = true;
                continue;
            }
            match matched[0] {
                "--help" => {
                    if inline.is_some() {
                        return Parsed::Refuse;
                    }
                    return Parsed::Help;
                }
                "--json" => {
                    if inline.is_some() {
                        return Parsed::Refuse;
                    }
                    json = true;
                }
                _ => {
                    let value = match inline {
                        Some(value) => value,
                        None => {
                            if index >= args.len() {
                                return Parsed::Refuse;
                            }
                            let next = &args[index];
                            match next.to_str() {
                                Some(next_text) if !is_option_like(next_text) => {
                                    index += 1;
                                    next_text.to_string()
                                }
                                _ => return Parsed::Refuse,
                            }
                        }
                    };
                    match read_timeout(&value) {
                        Some(parsed) => timeout = parsed,
                        None => return Parsed::Refuse,
                    }
                }
            }
            continue;
        }
        // A single dash token. The help flag fires on its first
        // letter, as the parser it replaces fires it.
        if text[1..].starts_with('h') {
            return Parsed::Help;
        }
        deferred_refusal = true;
    }
    if deferred_refusal {
        return Parsed::Refuse;
    }
    match project {
        Some((project, project_text)) => Parsed::Run {
            project,
            project_text,
            json,
            timeout,
        },
        None => Parsed::Refuse,
    }
}

/// Run the door over the process arguments after the program
/// name and return the exit code the process stands at.
pub fn run(args: &[OsString]) -> i32 {
    match parse(args) {
        Parsed::Help => {
            print!("{HELP}");
            0
        }
        Parsed::Refuse => {
            eprintln!("{USAGE}");
            eprintln!("ostensible: error: the arguments could not be read");
            2
        }
        Parsed::Run {
            project,
            project_text,
            json,
            timeout,
        } => {
            if !project.is_dir() {
                eprintln!("project directory not found - {project_text}");
                return 2;
            }
            match analyse_project(&project, timeout, Path::new("python3")) {
                Ok(analysis) => {
                    if json {
                        println!("{}", to_json(&analysis));
                    } else {
                        print!("{}", to_text(&analysis));
                    }
                    0
                }
                Err(error) => {
                    eprintln!("{}", error.message);
                    2
                }
            }
        }
    }
}
