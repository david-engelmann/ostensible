//! The dynamic value type the TOML reader builds, carrying the
//! text forms the Python language gives those values, because the
//! settings reading takes the text of a stated value through the
//! Python str conversion. Integers are held as their canonical
//! decimal text, since Python integers are unbounded and the
//! settings reading only ever takes their text.

/// The offset of an offset date time. A zero offset stands as the
/// Python timezone utc value, whose text and repr forms differ
/// from a spelled fixed offset of zero only in the repr.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Offset {
    Utc,
    FixedSeconds(i32),
}

/// One dynamic value, mirroring the values the Python toml reader
/// builds from a document.
#[derive(Debug, Clone, PartialEq)]
pub enum PyValue {
    Str(String),
    Int(String),
    Float(f64),
    Bool(bool),
    List(Vec<PyValue>),
    Table(Vec<(String, PyValue)>),
    Date {
        year: i64,
        month: u32,
        day: u32,
    },
    Time {
        hour: u32,
        minute: u32,
        second: u32,
        micros: u32,
    },
    DateTime {
        year: i64,
        month: u32,
        day: u32,
        hour: u32,
        minute: u32,
        second: u32,
        micros: u32,
        offset: Option<Offset>,
    },
}

impl PyValue {
    /// The entry of a table under a key, or None.
    pub fn get(&self, key: &str) -> Option<&PyValue> {
        match self {
            PyValue::Table(entries) => entries
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    pub fn is_table(&self) -> bool {
        matches!(self, PyValue::Table(_))
    }
}

fn format_offset(offset: Offset) -> String {
    match offset {
        Offset::Utc => "+00:00".to_string(),
        Offset::FixedSeconds(total) => {
            let sign = if total < 0 { "-" } else { "+" };
            let rest = total.abs();
            let hours = rest / 3600;
            let minutes = (rest % 3600) / 60;
            let seconds = rest % 60;
            if seconds != 0 {
                format!("{}{:02}:{:02}:{:02}", sign, hours, minutes, seconds)
            } else {
                format!("{}{:02}:{:02}", sign, hours, minutes)
            }
        }
    }
}

fn format_time(hour: u32, minute: u32, second: u32, micros: u32) -> String {
    let mut out = format!("{:02}:{:02}:{:02}", hour, minute, second);
    if micros != 0 {
        out.push_str(&format!(".{:06}", micros));
    }
    out
}

/// The Python str form of a float, which is its repr form. The
/// digits are the shortest digit string that round trips, taken
/// from the Rust exponent rendering, and the choice between the
/// fixed and the exponent spelling follows the Python rule over
/// the decimal exponent of the value.
pub fn py_float_str(value: f64) -> String {
    if value.is_nan() {
        return "nan".to_string();
    }
    if value.is_infinite() {
        return if value < 0.0 {
            "-inf".to_string()
        } else {
            "inf".to_string()
        };
    }
    if value == 0.0 {
        return if value.is_sign_negative() {
            "-0.0".to_string()
        } else {
            "0.0".to_string()
        };
    }
    let negative = value < 0.0;
    let rendered = format!("{:e}", value.abs());
    let (mantissa, exponent) = rendered.split_once('e').unwrap();
    let exp: i32 = exponent.parse().unwrap();
    let digits: String = mantissa.chars().filter(|ch| *ch != '.').collect();
    let mut out = String::new();
    if negative {
        out.push('-');
    }
    if (-4..16).contains(&exp) {
        let point = exp + 1;
        if point >= digits.len() as i32 {
            out.push_str(&digits);
            for _ in 0..(point - digits.len() as i32) {
                out.push('0');
            }
            out.push_str(".0");
        } else if point > 0 {
            let point = point as usize;
            out.push_str(&digits[..point]);
            out.push('.');
            out.push_str(&digits[point..]);
        } else {
            out.push_str("0.");
            for _ in 0..(-point) {
                out.push('0');
            }
            out.push_str(&digits);
        }
    } else {
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        out.push('e');
        out.push(if exp < 0 { '-' } else { '+' });
        let magnitude = exp.abs();
        if magnitude < 10 {
            out.push('0');
        }
        out.push_str(&magnitude.to_string());
    }
    out
}

fn timedelta_repr(total_seconds: i64, micros: i64) -> String {
    let mut days = total_seconds.div_euclid(86400);
    let mut seconds = total_seconds.rem_euclid(86400);
    let mut micros = micros;
    if micros < 0 {
        seconds -= 1;
        micros += 1_000_000;
    }
    if seconds < 0 {
        days -= 1;
        seconds += 86400;
    }
    let mut fields: Vec<String> = Vec::new();
    if days != 0 {
        fields.push(format!("days={}", days));
    }
    if seconds != 0 {
        fields.push(format!("seconds={}", seconds));
    }
    if micros != 0 {
        fields.push(format!("microseconds={}", micros));
    }
    if fields.is_empty() {
        "datetime.timedelta(0)".to_string()
    } else {
        format!("datetime.timedelta({})", fields.join(", "))
    }
}

fn offset_repr(offset: Offset) -> String {
    match offset {
        Offset::Utc => "datetime.timezone.utc".to_string(),
        Offset::FixedSeconds(total) => {
            if total == 0 {
                "datetime.timezone.utc".to_string()
            } else {
                format!("datetime.timezone({})", timedelta_repr(total as i64, 0))
            }
        }
    }
}

pub fn py_str(value: &PyValue) -> String {
    match value {
        PyValue::Str(text) => text.clone(),
        PyValue::Int(text) => text.clone(),
        PyValue::Float(number) => py_float_str(*number),
        PyValue::Bool(flag) => {
            if *flag {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        PyValue::Date { year, month, day } => {
            format!("{:04}-{:02}-{:02}", year, month, day)
        }
        PyValue::Time {
            hour,
            minute,
            second,
            micros,
        } => format_time(*hour, *minute, *second, *micros),
        PyValue::DateTime {
            year,
            month,
            day,
            hour,
            minute,
            second,
            micros,
            offset,
        } => {
            let mut out = format!(
                "{:04}-{:02}-{:02} {}",
                year,
                month,
                day,
                format_time(*hour, *minute, *second, *micros)
            );
            if let Some(offset) = offset {
                out.push_str(&format_offset(*offset));
            }
            out
        }
        PyValue::List(_) | PyValue::Table(_) => py_repr(value),
    }
}

/// The repr of a string the value world carries, through
/// the one implementation in pydata under PORT9-1, which
/// takes the string's codepoints.
fn repr_str(text: &str) -> String {
    let points: Vec<u32> = text.chars().map(|ch| ch as u32).collect();
    crate::pydata::str_repr(&points)
}

/// The Python repr form of a value.
pub fn py_repr(value: &PyValue) -> String {
    match value {
        PyValue::Str(text) => repr_str(text),
        PyValue::Date { year, month, day } => {
            format!("datetime.date({}, {}, {})", year, month, day)
        }
        PyValue::Time {
            hour,
            minute,
            second,
            micros,
        } => {
            let mut fields = vec![hour.to_string(), minute.to_string()];
            if *second != 0 || *micros != 0 {
                fields.push(second.to_string());
            }
            if *micros != 0 {
                fields.push(micros.to_string());
            }
            format!("datetime.time({})", fields.join(", "))
        }
        PyValue::DateTime {
            year,
            month,
            day,
            hour,
            minute,
            second,
            micros,
            offset,
        } => {
            let mut fields = vec![
                year.to_string(),
                month.to_string(),
                day.to_string(),
                hour.to_string(),
                minute.to_string(),
            ];
            if *second != 0 || *micros != 0 {
                fields.push(second.to_string());
            }
            if *micros != 0 {
                fields.push(micros.to_string());
            }
            if let Some(offset) = offset {
                fields.push(format!("tzinfo={}", offset_repr(*offset)));
            }
            format!("datetime.datetime({})", fields.join(", "))
        }
        PyValue::List(items) => {
            let parts: Vec<String> = items.iter().map(py_repr).collect();
            format!("[{}]", parts.join(", "))
        }
        PyValue::Table(entries) => {
            let parts: Vec<String> = entries
                .iter()
                .map(|(key, item)| format!("{}: {}", repr_str(key), py_repr(item)))
                .collect();
            format!("{{{}}}", parts.join(", "))
        }
        other => py_str(other),
    }
}
