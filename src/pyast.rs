//! The Python 3.12 syntax tree the static scan reads, ported
//! under PORT7-2. Node shapes, field orders, and positions
//! reproduce the tree CPython's ast.parse builds for the same
//! text. Positions are the line number and the column offset
//! as a UTF-8 byte offset, the pair CPython assigns.

/// A source position, the line number and the UTF-8 byte
/// column offset CPython assigns a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Pos {
    pub line: u32,
    pub col: u32,
}

impl Pos {
    pub fn new(line: u32, col: u32) -> Pos {
        Pos { line, col }
    }
}

/// The text of a string constant, carried as the codepoints
/// the interpreter's string holds. A Rust string cannot hold
/// a lone surrogate, and an escape in the source can name
/// one, so the codepoints stand as numbers and a value in
/// the surrogate range stands as itself, apart from every
/// real character. No real character carries a codepoint in
/// that range, so the carrying cannot collide with one, the
/// stray mark discipline the neighbouring crates state. A
/// real replacement character stands as its own codepoint,
/// apart from every surrogate.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct PyText {
    codepoints: Vec<u32>,
}

impl PyText {
    /// Carry the codepoints of a Rust string, every one of
    /// them a real character.
    pub fn from_text(text: &str) -> PyText {
        PyText {
            codepoints: text.chars().map(|ch| ch as u32).collect(),
        }
    }

    /// Carry decoded codepoints whole, lone surrogates
    /// among them.
    pub fn from_codepoints(codepoints: Vec<u32>) -> PyText {
        PyText { codepoints }
    }

    /// The codepoints the text holds, in order.
    pub fn codepoints(&self) -> &[u32] {
        &self.codepoints
    }

    /// Append another text whole.
    pub fn push_text(&mut self, other: &PyText) {
        self.codepoints.extend_from_slice(&other.codepoints);
    }

    /// Whether the text holds no codepoint.
    pub fn is_empty(&self) -> bool {
        self.codepoints.is_empty()
    }

    /// The plain string the text spells when every
    /// codepoint is a real character, or None when a lone
    /// surrogate stands in it.
    pub fn to_plain_string(&self) -> Option<String> {
        self.codepoints
            .iter()
            .map(|point| char::from_u32(*point))
            .collect()
    }
}

/// A constant value as the parser folds it. Integers stand as
/// their canonical decimal spelling, computed exactly at any
/// size, because the unparse reading spells an integer the
/// way CPython spells its value.
#[derive(Debug, Clone, PartialEq)]
pub enum ConstVal {
    NoneVal,
    Bool(bool),
    Int(String),
    Float(f64),
    Complex(f64, f64),
    Str(PyText),
    Bytes(Vec<u8>),
    Ellipsis,
}

/// Convert the digits of an integer literal in the given base
/// to the canonical decimal spelling of its value. The digits
/// stand without underscores or a base prefix. Returns None on
/// a digit outside the base.
pub fn int_literal_to_decimal(digits: &str, base: u32) -> Option<String> {
    if digits.is_empty() {
        // A base prefix with no digit after it is no
        // literal at all.
        return None;
    }
    if base == 10 {
        let trimmed = digits.trim_start_matches('0');
        if trimmed.is_empty() {
            return Some("0".to_string());
        }
        return Some(trimmed.to_string());
    }
    // Little endian base 2^32 magnitude, multiply and add.
    let mut mag: Vec<u32> = vec![0];
    for ch in digits.chars() {
        let value = ch.to_digit(base)?;
        let mut carry = value as u64;
        for limb in mag.iter_mut() {
            let next = (*limb as u64) * (base as u64) + carry;
            *limb = next as u32;
            carry = next >> 32;
        }
        while carry > 0 {
            mag.push(carry as u32);
            carry >>= 32;
        }
    }
    while mag.len() > 1 && mag[mag.len() - 1] == 0 {
        mag.pop();
    }
    if mag.iter().all(|limb| *limb == 0) {
        return Some("0".to_string());
    }
    // Repeated division by 10^9, most significant chunk last.
    let mut chunks: Vec<u32> = Vec::new();
    let mut current = mag;
    while !(current.len() == 1 && current[0] == 0) {
        let mut remainder: u64 = 0;
        for index in (0..current.len()).rev() {
            let value = (remainder << 32) | (current[index] as u64);
            current[index] = (value / 1_000_000_000) as u32;
            remainder = value % 1_000_000_000;
        }
        while current.len() > 1 && current[current.len() - 1] == 0 {
            current.pop();
        }
        chunks.push(remainder as u32);
    }
    let mut out = String::new();
    for (index, chunk) in chunks.iter().rev().enumerate() {
        if index == 0 {
            out.push_str(&chunk.to_string());
        } else {
            out.push_str(&format!("{:09}", chunk));
        }
    }
    Some(out)
}

/// The shortest digits of a finite nonzero float, as the pair
/// of its decimal digit string and the position of the decimal
/// point, the reading CPython's repr is built on. The value is
/// digits times ten to the power decpt minus the digit count.
fn float_digits(value: f64) -> (String, i32) {
    let spelled = format!("{:e}", value.abs());
    let (mantissa, exponent) = spelled
        .split_once('e')
        .expect("lower exp spelling carries an exponent");
    let exponent: i32 = exponent.parse().expect("exponent is a number");
    let digits: String = mantissa.chars().filter(|ch| *ch != '.').collect();
    (digits, exponent + 1)
}

/// Spell a float the way CPython's repr spells it. When dot0
/// is false the integral fixed form carries no trailing point
/// zero, which is the form the parts of a complex spelling
/// take.
pub fn float_repr_mode(value: f64, dot0: bool) -> String {
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
        let body = if dot0 { "0.0" } else { "0" };
        return if value.is_sign_negative() {
            format!("-{}", body)
        } else {
            body.to_string()
        };
    }
    let (digits, decpt) = float_digits(value);
    let sign = if value < 0.0 { "-" } else { "" };
    let count = digits.len() as i32;
    if decpt <= -4 || decpt > 16 {
        let mut out = String::new();
        out.push_str(sign);
        out.push_str(&digits[..1]);
        if digits.len() > 1 {
            out.push('.');
            out.push_str(&digits[1..]);
        }
        let exponent = decpt - 1;
        out.push('e');
        out.push(if exponent < 0 { '-' } else { '+' });
        out.push_str(&format!("{:02}", exponent.abs()));
        return out;
    }
    let mut out = String::new();
    out.push_str(sign);
    if decpt <= 0 {
        out.push_str("0.");
        for _ in 0..(-decpt) {
            out.push('0');
        }
        out.push_str(&digits);
    } else if decpt >= count {
        out.push_str(&digits);
        for _ in 0..(decpt - count) {
            out.push('0');
        }
        if dot0 {
            out.push_str(".0");
        }
    } else {
        let at = decpt as usize;
        out.push_str(&digits[..at]);
        out.push('.');
        out.push_str(&digits[at..]);
    }
    out
}

/// Spell a float the way CPython's repr spells it.
pub fn float_repr(value: f64) -> String {
    float_repr_mode(value, true)
}

/// Spell a complex constant the way CPython's repr spells it.
/// A constant the parser folds is always pure imaginary in
/// practice, but the full form stands here for completeness.
pub fn complex_repr(real: f64, imag: f64) -> String {
    if real == 0.0 && !real.is_sign_negative() {
        return format!("{}j", float_repr_mode(imag, false));
    }
    let sign = if imag.is_sign_negative() || (imag == 0.0 && imag.is_sign_negative()) {
        "-"
    } else {
        "+"
    };
    format!(
        "({}{}{}j)",
        float_repr_mode(real, false),
        sign,
        float_repr_mode(imag.abs(), false)
    )
}

/// Binary operators in the grammar's spelling order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinOpKind {
    Add,
    Sub,
    Mult,
    MatMult,
    Div,
    Mod,
    Pow,
    LShift,
    RShift,
    BitOr,
    BitXor,
    BitAnd,
    FloorDiv,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryKind {
    Invert,
    Not,
    UAdd,
    USub,
}

/// Boolean operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BoolKind {
    And,
    Or,
}

/// Comparison operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpKind {
    Eq,
    NotEq,
    Lt,
    LtE,
    Gt,
    GtE,
    Is,
    IsNot,
    In,
    NotIn,
}

/// One function argument, with its annotation when stated.
#[derive(Debug, Clone)]
pub struct Arg {
    pub id: usize,
    pub pos: Pos,
    pub name: String,
    pub annotation: Option<Box<Expr>>,
}

/// The argument list of a definition or a lambda, in the field
/// order CPython states for the arguments node.
#[derive(Debug, Clone)]
pub struct Arguments {
    pub id: usize,
    pub posonlyargs: Vec<Arg>,
    pub args: Vec<Arg>,
    pub vararg: Option<Arg>,
    pub kwonlyargs: Vec<Arg>,
    pub kw_defaults: Vec<Option<Expr>>,
    pub kwarg: Option<Arg>,
    pub defaults: Vec<Expr>,
}

/// A keyword argument of a call or a class definition. A None
/// name is the double star form.
#[derive(Debug, Clone)]
pub struct Keyword {
    pub id: usize,
    pub pos: Pos,
    pub arg: Option<String>,
    pub value: Expr,
}

/// One comprehension clause.
#[derive(Debug, Clone)]
pub struct Comprehension {
    pub id: usize,
    pub pos: Pos,
    pub target: Expr,
    pub iter: Expr,
    pub ifs: Vec<Expr>,
    pub is_async: bool,
}

/// A type parameter of a definition, class, or alias.
#[derive(Debug, Clone)]
pub struct TypeParam {
    pub id: usize,
    pub pos: Pos,
    pub kind: TypeParamKind,
}

#[derive(Debug, Clone)]
pub enum TypeParamKind {
    TypeVar {
        name: String,
        bound: Option<Box<Expr>>,
    },
    TypeVarTuple {
        name: String,
    },
    ParamSpec {
        name: String,
    },
}

/// An expression node. The id is unique inside one parsed
/// module and stands in for the node identity the Python
/// readings take from the object itself.
#[derive(Debug, Clone)]
pub struct Expr {
    pub id: usize,
    pub pos: Pos,
    pub kind: ExprKind,
}

#[derive(Debug, Clone)]
pub enum ExprKind {
    Constant(ConstVal),
    Name(String),
    Attribute {
        value: Box<Expr>,
        attr: String,
    },
    Call {
        func: Box<Expr>,
        args: Vec<Expr>,
        keywords: Vec<Keyword>,
    },
    BinOp {
        left: Box<Expr>,
        op: BinOpKind,
        right: Box<Expr>,
    },
    UnaryOp {
        op: UnaryKind,
        operand: Box<Expr>,
    },
    BoolOp {
        op: BoolKind,
        values: Vec<Expr>,
    },
    Compare {
        left: Box<Expr>,
        ops: Vec<CmpKind>,
        comparators: Vec<Expr>,
    },
    IfExp {
        test: Box<Expr>,
        body: Box<Expr>,
        orelse: Box<Expr>,
    },
    Subscript {
        value: Box<Expr>,
        slice: Box<Expr>,
    },
    Slice {
        lower: Option<Box<Expr>>,
        upper: Option<Box<Expr>>,
        step: Option<Box<Expr>>,
    },
    List {
        elts: Vec<Expr>,
    },
    Tuple {
        elts: Vec<Expr>,
    },
    Set {
        elts: Vec<Expr>,
    },
    Dict {
        keys: Vec<Option<Expr>>,
        values: Vec<Expr>,
    },
    Starred {
        value: Box<Expr>,
    },
    Await {
        value: Box<Expr>,
    },
    Yield {
        value: Option<Box<Expr>>,
    },
    YieldFrom {
        value: Box<Expr>,
    },
    Lambda {
        args: Box<Arguments>,
        body: Box<Expr>,
    },
    ListComp {
        elt: Box<Expr>,
        generators: Vec<Comprehension>,
    },
    SetComp {
        elt: Box<Expr>,
        generators: Vec<Comprehension>,
    },
    GeneratorExp {
        elt: Box<Expr>,
        generators: Vec<Comprehension>,
    },
    DictComp {
        key: Box<Expr>,
        value: Box<Expr>,
        generators: Vec<Comprehension>,
    },
    JoinedStr {
        values: Vec<Expr>,
    },
    FormattedValue {
        value: Box<Expr>,
        conversion: i32,
        format_spec: Option<Box<Expr>>,
    },
    NamedExpr {
        target: Box<Expr>,
        value: Box<Expr>,
    },
}

/// One with item, the context expression and its binding.
#[derive(Debug, Clone)]
pub struct WithItem {
    pub id: usize,
    pub pos: Pos,
    pub context_expr: Expr,
    pub optional_vars: Option<Expr>,
}

/// One import alias.
#[derive(Debug, Clone)]
pub struct Alias {
    pub id: usize,
    pub pos: Pos,
    pub name: String,
    pub asname: Option<String>,
}

/// One exception handler of a try statement.
#[derive(Debug, Clone)]
pub struct ExceptHandler {
    pub id: usize,
    pub pos: Pos,
    pub type_: Option<Expr>,
    pub name: Option<String>,
    pub body: Vec<Stmt>,
}

/// One case of a match statement.
#[derive(Debug, Clone)]
pub struct MatchCase {
    pub id: usize,
    pub pos: Pos,
    pub pattern: Pattern,
    pub guard: Option<Expr>,
    pub body: Vec<Stmt>,
}

/// One match pattern.
#[derive(Debug, Clone)]
pub struct Pattern {
    pub id: usize,
    pub pos: Pos,
    pub kind: PatternKind,
}

#[derive(Debug, Clone)]
pub enum PatternKind {
    MatchValue(Expr),
    MatchSingleton(ConstVal),
    MatchSequence(Vec<Pattern>),
    MatchStar(Option<String>),
    MatchMapping {
        keys: Vec<Expr>,
        patterns: Vec<Pattern>,
        rest: Option<String>,
    },
    MatchClass {
        cls: Box<Expr>,
        patterns: Vec<Pattern>,
        kwd_attrs: Vec<String>,
        kwd_patterns: Vec<Pattern>,
    },
    MatchAs {
        pattern: Option<Box<Pattern>>,
        name: Option<String>,
    },
    MatchOr(Vec<Pattern>),
}

/// A statement node.
#[derive(Debug, Clone)]
pub struct Stmt {
    pub id: usize,
    pub pos: Pos,
    pub kind: StmtKind,
}

#[derive(Debug, Clone)]
pub enum StmtKind {
    FuncDef {
        name: String,
        args: Box<Arguments>,
        body: Vec<Stmt>,
        decorators: Vec<Expr>,
        returns: Option<Box<Expr>>,
        type_params: Vec<TypeParam>,
        is_async: bool,
    },
    ClassDef {
        name: String,
        bases: Vec<Expr>,
        keywords: Vec<Keyword>,
        body: Vec<Stmt>,
        decorators: Vec<Expr>,
        type_params: Vec<TypeParam>,
    },
    Return {
        value: Option<Expr>,
    },
    Delete {
        targets: Vec<Expr>,
    },
    Assign {
        targets: Vec<Expr>,
        value: Expr,
    },
    AugAssign {
        target: Expr,
        op: BinOpKind,
        value: Expr,
    },
    AnnAssign {
        target: Expr,
        annotation: Expr,
        value: Option<Expr>,
        simple: bool,
    },
    For {
        target: Expr,
        iter: Expr,
        body: Vec<Stmt>,
        orelse: Vec<Stmt>,
        is_async: bool,
    },
    While {
        test: Expr,
        body: Vec<Stmt>,
        orelse: Vec<Stmt>,
    },
    If {
        test: Expr,
        body: Vec<Stmt>,
        orelse: Vec<Stmt>,
    },
    With {
        items: Vec<WithItem>,
        body: Vec<Stmt>,
        is_async: bool,
    },
    Match {
        subject: Expr,
        cases: Vec<MatchCase>,
    },
    Raise {
        exc: Option<Expr>,
        cause: Option<Expr>,
    },
    Try {
        body: Vec<Stmt>,
        handlers: Vec<ExceptHandler>,
        orelse: Vec<Stmt>,
        finalbody: Vec<Stmt>,
        star: bool,
    },
    Assert {
        test: Expr,
        msg: Option<Expr>,
    },
    Import {
        names: Vec<Alias>,
    },
    ImportFrom {
        module: Option<String>,
        names: Vec<Alias>,
        level: u32,
    },
    Global {
        names: Vec<String>,
    },
    Nonlocal {
        names: Vec<String>,
    },
    ExprStmt {
        value: Expr,
    },
    Pass,
    Break,
    Continue,
    TypeAlias {
        name: Expr,
        type_params: Vec<TypeParam>,
        value: Expr,
    },
}

/// A parsed module, the root of one tree.
#[derive(Debug, Clone)]
pub struct Module {
    pub id: usize,
    pub body: Vec<Stmt>,
}
