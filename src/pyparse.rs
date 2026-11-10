//! The parser for the Python 3.12 reading of PORT7-2. It
//! reads the token stream into the tree of pyast, with the
//! shapes and positions CPython's ast.parse assigns. A source
//! the grammar refuses raises the parse refusal the scan
//! turns into its own wording under PORT7-7. Nesting past
//! the port bound of 4,000 refuses here, the bound standing
//! over the tokenizer's shallower bracket limit.

use crate::pyast::*;
use crate::pytoken::{tokenize, FPart, StrTok, Tok, Token, TokenError};

/// A failure to parse, carried up to the scan refusal.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
}

fn err(message: &str) -> ParseError {
    ParseError {
        message: message.to_string(),
    }
}

impl From<TokenError> for ParseError {
    fn from(error: TokenError) -> ParseError {
        err(&error.message)
    }
}

/// Parse a whole source into a module. The parse runs on
/// a spawned thread with a large stack, on the pattern the
/// scan path states, so a deep source inside the port
/// bound of 4,000 cannot exhaust the stack of the thread
/// that called the parse.
pub fn parse_module(source: &str) -> Result<Module, ParseError> {
    let outcome = std::thread::scope(|scope| {
        let handle = std::thread::Builder::new()
            .stack_size(512 * 1024 * 1024)
            .spawn_scoped(scope, || parse_module_inner(source));
        match handle {
            Ok(joined) => joined.join(),
            Err(_) => Err(Box::new(()) as Box<dyn std::any::Any + Send>),
        }
    });
    match outcome {
        Ok(result) => result,
        Err(payload) => std::panic::resume_unwind(payload),
    }
}

fn parse_module_inner(source: &str) -> Result<Module, ParseError> {
    let tokens = tokenize(source, 1, 0)?;
    let mut parser = Parser::new(tokens);
    parser.parse_module()
}

/// Parse the expression of one f-string field from its
/// fragment tokens.
fn parse_fragment(field: &crate::pytoken::FieldData) -> Result<Expr, ParseError> {
    let trimmed = field.expr.trim_start();
    if trimmed.starts_with("lambda")
        && (trimmed.len() == 6 || !is_ident_char(trimmed.chars().nth(6).unwrap_or(' ')))
    {
        return Err(err(
            "f-string: lambda expressions are not allowed without parentheses",
        ));
    }
    // Leading blanks of the first line stand outside the
    // fragment's own reading, the field's expression reading
    // them as separation, and the base column moves past
    // them so positions stay the source's.
    let mut lead = 0usize;
    for ch in field.expr.chars() {
        if ch == ' ' || ch == '\t' {
            lead += 1;
        } else {
            break;
        }
    }
    let tokens = tokenize(
        &field.expr[lead..],
        field.expr_line,
        field.expr_col + lead as u32,
    )?;
    let mut parser = Parser::new(tokens);
    parser.skip_newlines();
    let expr = parser.parse_expr()?;
    parser.skip_newlines();
    if !matches!(parser.peek_kind(), Tok::End) {
        return Err(err("f-string field does not hold one expression"));
    }
    Ok(expr)
}

fn is_ident_char(ch: char) -> bool {
    ch == '_' || ch.is_alphanumeric()
}

struct Parser {
    tokens: Vec<Token>,
    index: usize,
    next_id: usize,
    depth: usize,
}

impl Parser {
    fn new(tokens: Vec<Token>) -> Parser {
        Parser {
            tokens,
            index: 0,
            next_id: 1,
            depth: 0,
        }
    }

    fn fresh_id(&mut self) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn peek_kind(&self) -> &Tok {
        &self.tokens[self.index].kind
    }

    fn peek_at_kind(&self, ahead: usize) -> &Tok {
        let at = (self.index + ahead).min(self.tokens.len() - 1);
        &self.tokens[at].kind
    }

    fn peek_pos(&self) -> Pos {
        self.tokens[self.index].pos
    }

    fn bump(&mut self) -> Token {
        let token = self.tokens[self.index].clone();
        if self.index < self.tokens.len() - 1 {
            self.index += 1;
        }
        token
    }

    fn at_op(&self, op: &str) -> bool {
        matches!(self.peek_kind(), Tok::Op(text) if text == op)
    }

    fn at_op_at(&self, ahead: usize, op: &str) -> bool {
        matches!(self.peek_at_kind(ahead), Tok::Op(text) if text == op)
    }

    fn eat_op(&mut self, op: &str) -> bool {
        if self.at_op(op) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect_op(&mut self, op: &str) -> Result<Pos, ParseError> {
        if self.at_op(op) {
            Ok(self.bump().pos)
        } else {
            Err(err(&format!("expected {}", op)))
        }
    }

    fn at_name(&self, word: &str) -> bool {
        matches!(self.peek_kind(), Tok::Name(text) if text == word)
    }

    fn eat_name(&mut self, word: &str) -> bool {
        if self.at_name(word) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn at_newline(&self) -> bool {
        matches!(self.peek_kind(), Tok::Newline)
    }

    fn skip_newlines(&mut self) {
        while self.at_newline() {
            self.bump();
        }
    }

    fn expect_newline(&mut self) -> Result<(), ParseError> {
        if self.at_newline() {
            self.bump();
            Ok(())
        } else {
            Err(err("expected the end of the line"))
        }
    }

    fn deepen<T>(
        &mut self,
        f: impl FnOnce(&mut Parser) -> Result<T, ParseError>,
    ) -> Result<T, ParseError> {
        self.depth += 1;
        if self.depth > 4000 {
            return Err(err("nesting past the port bound"));
        }
        let out = f(self);
        self.depth -= 1;
        out
    }

    fn expr(&mut self, pos: Pos, kind: ExprKind) -> Expr {
        Expr {
            id: self.fresh_id(),
            pos,
            kind,
        }
    }

    fn stmt(&mut self, pos: Pos, kind: StmtKind) -> Stmt {
        Stmt {
            id: self.fresh_id(),
            pos,
            kind,
        }
    }

    fn parse_module(&mut self) -> Result<Module, ParseError> {
        let mut body = Vec::new();
        self.skip_newlines();
        while !matches!(self.peek_kind(), Tok::End) {
            let mut statements = self.parse_stmt()?;
            body.append(&mut statements);
            self.skip_newlines();
        }
        Ok(Module {
            id: self.fresh_id(),
            body,
        })
    }

    /// Parse one statement, which may be a line of simple
    /// statements, and return the statements it holds.
    fn parse_stmt(&mut self) -> Result<Vec<Stmt>, ParseError> {
        match self.peek_kind().clone() {
            Tok::Op(text) if text == "@" => self.parse_decorated(),
            Tok::Name(word) => match word.as_str() {
                "if" => Ok(vec![self.parse_if()?]),
                "while" => Ok(vec![self.parse_while()?]),
                "for" => Ok(vec![self.parse_for(false)?]),
                "async" => {
                    if matches!(self.peek_at_kind(1), Tok::Name(text) if text == "def") {
                        Ok(vec![self.parse_funcdef(true, Vec::new())?])
                    } else if matches!(self.peek_at_kind(1), Tok::Name(text) if text == "for") {
                        Ok(vec![self.parse_for(true)?])
                    } else if matches!(self.peek_at_kind(1), Tok::Name(text) if text == "with") {
                        Ok(vec![self.parse_with(true)?])
                    } else {
                        self.parse_simple_line()
                    }
                }
                "with" => Ok(vec![self.parse_with(false)?]),
                "try" => Ok(vec![self.parse_try()?]),
                "def" => Ok(vec![self.parse_funcdef(false, Vec::new())?]),
                "class" => Ok(vec![self.parse_classdef(Vec::new())?]),
                "match" if self.looks_like_match() => Ok(vec![self.parse_match()?]),
                "type" if self.looks_like_type_alias() => Ok(vec![self.parse_type_alias()?]),
                _ => self.parse_simple_line(),
            },
            _ => self.parse_simple_line(),
        }
    }

    fn looks_like_match(&self) -> bool {
        // A match statement's logical line ends with a colon
        // at bracket depth zero, and a case line follows the
        // indent. A name read as an expression never does.
        let mut depth = 0i32;
        let mut scan = self.index;
        let mut colon_end = false;
        while scan < self.tokens.len() {
            match &self.tokens[scan].kind {
                Tok::Op(text) if text == "(" || text == "[" || text == "{" => depth += 1,
                Tok::Op(text) if text == ")" || text == "]" || text == "}" => depth -= 1,
                Tok::Op(text) if text == ":" && depth == 0 => colon_end = true,
                Tok::Newline => break,
                Tok::End => break,
                _ => {}
            }
            scan += 1;
        }
        if !colon_end {
            return false;
        }
        // The colon must be the last token of the line.
        let mut probe = self.index + 1;
        let mut last_significant = self.index;
        let mut depth = 0i32;
        while probe < self.tokens.len() {
            match &self.tokens[probe].kind {
                Tok::Op(text) if text == "(" || text == "[" || text == "{" => {
                    depth += 1;
                    last_significant = probe;
                }
                Tok::Op(text) if text == ")" || text == "]" || text == "}" => {
                    depth -= 1;
                    last_significant = probe;
                }
                Tok::Newline | Tok::End => break,
                _ => {
                    let _ = depth;
                    last_significant = probe;
                }
            }
            probe += 1;
        }
        matches!(&self.tokens[last_significant].kind, Tok::Op(text) if text == ":")
    }

    fn looks_like_type_alias(&self) -> bool {
        // type NAME = or type NAME[ opens an alias statement.
        if !matches!(self.peek_at_kind(1), Tok::Name(_)) {
            return false;
        }
        if let Tok::Name(word) = self.peek_at_kind(1) {
            if is_keyword(word) {
                return false;
            }
        }
        self.at_op_at(2, "=") || self.at_op_at(2, "[")
    }

    /// Parse a suite, the indented block or the simple
    /// statement standing on the same line after the colon.
    fn parse_suite(&mut self) -> Result<Vec<Stmt>, ParseError> {
        if self.at_newline() {
            self.bump();
            if !matches!(self.peek_kind(), Tok::Indent) {
                return Err(err("expected an indented block"));
            }
            self.bump();
            let mut body = Vec::new();
            self.skip_newlines();
            while !matches!(self.peek_kind(), Tok::Dedent | Tok::End) {
                let mut statements = self.parse_stmt()?;
                body.append(&mut statements);
                self.skip_newlines();
            }
            if matches!(self.peek_kind(), Tok::Dedent) {
                self.bump();
            }
            Ok(body)
        } else {
            self.parse_simple_line()
        }
    }

    fn parse_simple_line(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut statements = Vec::new();
        loop {
            let statement = self.parse_small_stmt()?;
            statements.push(statement);
            if self.eat_op(";") {
                if self.at_newline() || matches!(self.peek_kind(), Tok::End) {
                    break;
                }
                continue;
            }
            break;
        }
        if self.at_newline() {
            self.bump();
        } else if !matches!(self.peek_kind(), Tok::End) {
            return Err(err("expected the end of the line"));
        }
        Ok(statements)
    }

    fn parse_small_stmt(&mut self) -> Result<Stmt, ParseError> {
        let pos = self.peek_pos();
        if let Tok::Name(word) = self.peek_kind().clone() {
            match word.as_str() {
                "return" => {
                    self.bump();
                    let value = if self.at_newline()
                        || self.at_op(";")
                        || matches!(self.peek_kind(), Tok::End)
                    {
                        None
                    } else {
                        Some(self.parse_value_list()?)
                    };
                    return Ok(self.stmt(pos, StmtKind::Return { value }));
                }
                "del" => {
                    self.bump();
                    let targets = self.parse_target_list()?;
                    return Ok(self.stmt(pos, StmtKind::Delete { targets }));
                }
                "pass" => {
                    self.bump();
                    return Ok(self.stmt(pos, StmtKind::Pass));
                }
                "break" => {
                    self.bump();
                    return Ok(self.stmt(pos, StmtKind::Break));
                }
                "continue" => {
                    self.bump();
                    return Ok(self.stmt(pos, StmtKind::Continue));
                }
                "raise" => {
                    self.bump();
                    if self.at_newline() || self.at_op(";") {
                        return Ok(self.stmt(
                            pos,
                            StmtKind::Raise {
                                exc: None,
                                cause: None,
                            },
                        ));
                    }
                    let exc = self.parse_expr()?;
                    let cause = if self.eat_name("from") {
                        Some(self.parse_expr()?)
                    } else {
                        None
                    };
                    return Ok(self.stmt(
                        pos,
                        StmtKind::Raise {
                            exc: Some(exc),
                            cause,
                        },
                    ));
                }
                "import" => {
                    self.bump();
                    let names = self.parse_import_aliases(false)?;
                    return Ok(self.stmt(pos, StmtKind::Import { names }));
                }
                "from" => {
                    self.bump();
                    let mut level = 0u32;
                    let mut module: Option<String> = None;
                    while self.at_op(".") || self.at_op("...") {
                        if self.at_op("...") {
                            level += 3;
                            self.bump();
                        } else {
                            level += 1;
                            self.bump();
                        }
                    }
                    if !self.at_name("import") {
                        let mut parts = vec![self.expect_name_text()?];
                        while self.eat_op(".") {
                            parts.push(self.expect_name_text()?);
                        }
                        module = Some(parts.join("."));
                    }
                    if !self.eat_name("import") {
                        return Err(err("expected import"));
                    }
                    let names = if self.eat_op("*") {
                        vec![Alias {
                            id: self.fresh_id(),
                            pos,
                            name: "*".to_string(),
                            asname: None,
                        }]
                    } else if self.eat_op("(") {
                        let names = self.parse_import_aliases(true)?;
                        self.expect_op(")")?;
                        names
                    } else {
                        self.parse_import_aliases(true)?
                    };
                    return Ok(self.stmt(
                        pos,
                        StmtKind::ImportFrom {
                            module,
                            names,
                            level,
                        },
                    ));
                }
                "global" => {
                    self.bump();
                    let names = self.parse_name_list()?;
                    return Ok(self.stmt(pos, StmtKind::Global { names }));
                }
                "nonlocal" => {
                    self.bump();
                    let names = self.parse_name_list()?;
                    return Ok(self.stmt(pos, StmtKind::Nonlocal { names }));
                }
                "assert" => {
                    self.bump();
                    let test = self.parse_expr()?;
                    let msg = if self.eat_op(",") {
                        Some(self.parse_expr()?)
                    } else {
                        None
                    };
                    return Ok(self.stmt(pos, StmtKind::Assert { test, msg }));
                }
                _ => {}
            }
        }
        self.parse_expr_stmt(pos)
    }

    fn parse_name_list(&mut self) -> Result<Vec<String>, ParseError> {
        let mut names = vec![self.expect_name_text()?];
        while self.eat_op(",") {
            names.push(self.expect_name_text()?);
        }
        Ok(names)
    }

    fn expect_name_text(&mut self) -> Result<String, ParseError> {
        match self.peek_kind().clone() {
            Tok::Name(word) => {
                self.bump();
                Ok(word)
            }
            _ => Err(err("expected a name")),
        }
    }

    fn parse_import_aliases(&mut self, from_form: bool) -> Result<Vec<Alias>, ParseError> {
        let mut aliases = Vec::new();
        loop {
            let pos = self.peek_pos();
            let mut parts = vec![self.expect_name_text()?];
            if !from_form {
                while self.eat_op(".") {
                    parts.push(self.expect_name_text()?);
                }
            }
            let asname = if self.eat_name("as") {
                Some(self.expect_name_text()?)
            } else {
                None
            };
            aliases.push(Alias {
                id: self.fresh_id(),
                pos,
                name: parts.join("."),
                asname,
            });
            if !self.eat_op(",") {
                break;
            }
            if matches!(self.peek_kind(), Tok::Newline) || self.at_op(")") {
                break;
            }
        }
        Ok(aliases)
    }

    /// The expression statement family, assignments and
    /// annotated assignments among them.
    fn parse_expr_stmt(&mut self, pos: Pos) -> Result<Stmt, ParseError> {
        let start_index = self.index;
        if self.at_name("yield") {
            // A yield expression standing alone as a
            // statement, the one expression statement form
            // the value readings do not reach.
            let yielded = self.parse_yield_expr()?;
            return Ok(self.stmt(pos, StmtKind::ExprStmt { value: yielded }));
        }
        let first = self.parse_star_or_expr()?;
        let mut segment = vec![first];
        let mut trailing_comma = false;
        while self.at_op(",") {
            // A comma opens an annotation or an assignment
            // boundary only through the cases below, so read
            // the list out and decide at its end.
            if self.at_op_at(1, "=") || self.peek_is_line_end(1) {
                self.bump();
                trailing_comma = true;
                break;
            }
            self.bump();
            let next = self.parse_star_or_expr()?;
            segment.push(next);
        }
        if self.at_op("=") {
            // An assignment, possibly chained.
            let mut target_lists = vec![segment];
            loop {
                self.bump(); // the equals sign
                let (value, trailing) = self.parse_assign_rhs_segment()?;
                if !self.at_op("=") {
                    let folded = target_lists
                        .into_iter()
                        .map(|list| fold_target_list(self, list))
                        .collect::<Result<Vec<Folded>, ParseError>>()?;
                    let targets = folded
                        .into_iter()
                        .map(|target| match target {
                            Folded::One(expr) => expr,
                            Folded::Many(exprs) => {
                                let at = exprs.first().map(|expr| expr.pos).unwrap_or(pos);
                                self.expr(at, ExprKind::Tuple { elts: exprs })
                            }
                        })
                        .collect();
                    let value = fold_expr_list(self, value, trailing);
                    return Ok(self.stmt(pos, StmtKind::Assign { targets, value }));
                }
                target_lists.push(value);
            }
        }
        if let Some(op) = self.at_augassign_op() {
            if segment.len() != 1 {
                return Err(err("an augmented assignment takes one target"));
            }
            self.bump();
            let target = segment.into_iter().next().unwrap();
            let value = self.parse_value_expr()?;
            return Ok(self.stmt(pos, StmtKind::AugAssign { target, op, value }));
        }
        if self.at_op(":") {
            if segment.len() != 1 {
                return Err(err("an annotation takes one target"));
            }
            self.bump();
            let target = segment.into_iter().next().unwrap();
            // The simple reading stands where the target
            // stands unwrapped, a target the parentheses
            // carried whole reading not simple, which the
            // spelling of an annotated name assignment
            // shows again.
            let simple = !(matches!(
                &self.tokens[start_index].kind,
                Tok::Op(text) if text == "("
            ) && matches!(
                &self.tokens[self.index - 2].kind,
                Tok::Op(text) if text == ")"
            ));
            let annotation = self.parse_expr()?;
            let value = if self.eat_op("=") {
                Some(self.parse_value_expr()?)
            } else {
                None
            };
            return Ok(self.stmt(
                pos,
                StmtKind::AnnAssign {
                    target,
                    annotation,
                    value,
                    simple,
                },
            ));
        }
        let value = fold_expr_list(self, segment, trailing_comma);
        Ok(self.stmt(pos, StmtKind::ExprStmt { value }))
    }

    fn peek_is_line_end(&self, ahead: usize) -> bool {
        matches!(self.peek_at_kind(ahead), Tok::Newline | Tok::End) || self.at_op_at(ahead, ";")
    }

    /// Read one right hand segment of an assignment, the list
    /// of expressions or a yield expression, returning its
    /// expressions unfolded for the chain reading and whether
    /// a trailing comma closed the list.
    fn parse_assign_rhs_segment(&mut self) -> Result<(Vec<Expr>, bool), ParseError> {
        if self.at_name("yield") {
            let yielded = self.parse_yield_expr()?;
            return Ok((vec![yielded], false));
        }
        let first = self.parse_star_or_expr()?;
        let mut segment = vec![first];
        let mut trailing = false;
        while self.at_op(",") {
            if self.peek_is_line_end(1) || self.at_op_at(1, "=") {
                self.bump();
                trailing = true;
                break;
            }
            self.bump();
            segment.push(self.parse_star_or_expr()?);
        }
        Ok((segment, trailing))
    }

    fn at_augassign_op(&self) -> Option<BinOpKind> {
        let op = match self.peek_kind() {
            Tok::Op(text) => match text.as_str() {
                "+=" => BinOpKind::Add,
                "-=" => BinOpKind::Sub,
                "*=" => BinOpKind::Mult,
                "@=" => BinOpKind::MatMult,
                "/=" => BinOpKind::Div,
                "%=" => BinOpKind::Mod,
                "**=" => BinOpKind::Pow,
                "<<=" => BinOpKind::LShift,
                ">>=" => BinOpKind::RShift,
                "|=" => BinOpKind::BitOr,
                "^=" => BinOpKind::BitXor,
                "&=" => BinOpKind::BitAnd,
                "//=" => BinOpKind::FloorDiv,
                _ => return None,
            },
            _ => return None,
        };
        Some(op)
    }

    /// A value in an assignment right hand or a return, an
    /// expression list folded the way the grammar folds it.
    fn parse_value_list(&mut self) -> Result<Expr, ParseError> {
        if self.at_name("yield") {
            return self.parse_yield_expr();
        }
        let first = self.parse_star_or_expr()?;
        let mut segment = vec![first];
        let mut trailing = false;
        while self.at_op(",") {
            if self.peek_is_line_end(1) {
                self.bump();
                trailing = true;
                break;
            }
            self.bump();
            segment.push(self.parse_star_or_expr()?);
        }
        Ok(fold_expr_list(self, segment, trailing))
    }

    fn parse_value_expr(&mut self) -> Result<Expr, ParseError> {
        if self.at_name("yield") {
            return self.parse_yield_expr();
        }
        self.parse_expr()
    }

    fn parse_yield_expr(&mut self) -> Result<Expr, ParseError> {
        let pos = self.bump().pos; // the yield keyword
        if self.eat_name("from") {
            let value = self.parse_expr()?;
            return Ok(self.expr(
                pos,
                ExprKind::YieldFrom {
                    value: Box::new(value),
                },
            ));
        }
        // A bare yield carries no value when what follows
        // cannot open an expression.
        if self.at_newline()
            || self.at_op(";")
            || self.at_op(",")
            || self.at_op(")")
            || self.at_op("]")
            || matches!(self.peek_kind(), Tok::End)
        {
            return Ok(self.expr(pos, ExprKind::Yield { value: None }));
        }
        let value = self.parse_expr()?;
        Ok(self.expr(
            pos,
            ExprKind::Yield {
                value: Some(Box::new(value)),
            },
        ))
    }

    fn parse_target_list(&mut self) -> Result<Vec<Expr>, ParseError> {
        let mut targets = vec![self.parse_target()?];
        while self.eat_op(",") {
            if self.peek_is_line_end(0) {
                break;
            }
            targets.push(self.parse_target()?);
        }
        Ok(targets)
    }

    /// A target, the expression shapes an assignment can
    /// bind.
    fn parse_target(&mut self) -> Result<Expr, ParseError> {
        if self.at_op("*") {
            let pos = self.bump().pos;
            let value = self.parse_target()?;
            return Ok(self.expr(
                pos,
                ExprKind::Starred {
                    value: Box::new(value),
                },
            ));
        }
        if self.at_op("(") {
            let pos = self.bump().pos;
            let mut targets = Vec::new();
            if !self.at_op(")") {
                targets = self.parse_target_list()?;
            }
            self.expect_op(")")?;
            return Ok(self.expr(pos, ExprKind::Tuple { elts: targets }));
        }
        if self.at_op("[") {
            let pos = self.bump().pos;
            let mut targets = Vec::new();
            if !self.at_op("]") {
                targets = self.parse_target_list()?;
            }
            self.expect_op("]")?;
            return Ok(self.expr(pos, ExprKind::List { elts: targets }));
        }
        self.parse_primary_with_trailers()
    }
}

enum Folded {
    One(Expr),
    Many(Vec<Expr>),
}

fn fold_target_list(_parser: &mut Parser, list: Vec<Expr>) -> Result<Folded, ParseError> {
    if list.len() == 1 {
        Ok(Folded::One(list.into_iter().next().unwrap()))
    } else {
        Ok(Folded::Many(list))
    }
}

/// Fold an expression list the way the grammar folds a bare
/// tuple, one expression standing alone unless a comma made
/// a tuple of it.
fn fold_expr_list(parser: &mut Parser, list: Vec<Expr>, trailing_comma: bool) -> Expr {
    if list.len() == 1 && !trailing_comma {
        list.into_iter().next().unwrap()
    } else {
        let pos = list.first().map(|expr| expr.pos).unwrap_or(Pos::new(0, 0));
        parser.expr(pos, ExprKind::Tuple { elts: list })
    }
}

fn is_keyword(word: &str) -> bool {
    matches!(
        word,
        "False"
            | "None"
            | "True"
            | "and"
            | "as"
            | "assert"
            | "async"
            | "await"
            | "break"
            | "class"
            | "continue"
            | "def"
            | "del"
            | "elif"
            | "else"
            | "except"
            | "finally"
            | "for"
            | "from"
            | "global"
            | "if"
            | "import"
            | "in"
            | "is"
            | "lambda"
            | "nonlocal"
            | "not"
            | "or"
            | "pass"
            | "raise"
            | "return"
            | "try"
            | "while"
            | "with"
            | "yield"
    )
}

impl Parser {
    fn parse_if(&mut self) -> Result<Stmt, ParseError> {
        let pos = self.bump().pos;
        let test = self.parse_expr()?;
        self.expect_op(":")?;
        let body = self.parse_suite()?;
        let mut orelse = Vec::new();
        if self.at_name("elif") {
            let elif = self.parse_if()?;
            orelse = vec![elif];
        } else if self.eat_name("else") {
            self.expect_op(":")?;
            orelse = self.parse_suite()?;
        }
        Ok(self.stmt(pos, StmtKind::If { test, body, orelse }))
    }

    fn parse_while(&mut self) -> Result<Stmt, ParseError> {
        let pos = self.bump().pos;
        let test = self.parse_expr()?;
        self.expect_op(":")?;
        let body = self.parse_suite()?;
        let orelse = if self.eat_name("else") {
            self.expect_op(":")?;
            self.parse_suite()?
        } else {
            Vec::new()
        };
        Ok(self.stmt(pos, StmtKind::While { test, body, orelse }))
    }

    fn parse_for(&mut self, is_async: bool) -> Result<Stmt, ParseError> {
        let pos = self.peek_pos();
        if is_async {
            self.bump(); // async
        }
        self.bump(); // for
        let first = self.parse_target()?;
        let mut targets = vec![first];
        while self.eat_op(",") {
            if self.at_name("in") {
                break;
            }
            targets.push(self.parse_target()?);
        }
        let target = if targets.len() == 1 {
            targets.into_iter().next().unwrap()
        } else {
            let at = targets[0].pos;
            self.expr(at, ExprKind::Tuple { elts: targets })
        };
        if !self.eat_name("in") {
            return Err(err("expected in"));
        }
        let iter = self.parse_value_list()?;
        self.expect_op(":")?;
        let body = self.parse_suite()?;
        let orelse = if self.eat_name("else") {
            self.expect_op(":")?;
            self.parse_suite()?
        } else {
            Vec::new()
        };
        Ok(self.stmt(
            pos,
            StmtKind::For {
                target,
                iter,
                body,
                orelse,
                is_async,
            },
        ))
    }

    fn parse_with(&mut self, is_async: bool) -> Result<Stmt, ParseError> {
        let pos = self.peek_pos();
        if is_async {
            self.bump(); // async
        }
        self.bump(); // with
        let mut items = Vec::new();
        if self.at_op("(") && self.with_parens_are_items() {
            self.bump();
            loop {
                let item = self.parse_with_item()?;
                items.push(item);
                if !self.eat_op(",") {
                    break;
                }
                if self.at_op(")") {
                    break;
                }
            }
            self.expect_op(")")?;
        } else {
            loop {
                let item = self.parse_with_item()?;
                items.push(item);
                if !self.eat_op(",") {
                    break;
                }
            }
        }
        self.expect_op(":")?;
        let body = self.parse_suite()?;
        Ok(self.stmt(
            pos,
            StmtKind::With {
                items,
                body,
                is_async,
            },
        ))
    }

    /// The parenthesized with form reads as an item list
    /// when the group carries an as clause or a trailing
    /// comma at its own level, and as one parenthesized
    /// context otherwise.
    fn with_parens_are_items(&self) -> bool {
        let mut depth = 0i32;
        let mut scan = self.index;
        let mut saw_as = false;
        let mut trailing_comma = false;
        let mut previous_was_comma = false;
        while scan < self.tokens.len() {
            match &self.tokens[scan].kind {
                Tok::Op(text) if text == "(" || text == "[" || text == "{" => {
                    depth += 1;
                    previous_was_comma = false;
                }
                Tok::Op(text) if text == ")" || text == "]" || text == "}" => {
                    depth -= 1;
                    if depth == 0 {
                        trailing_comma = previous_was_comma;
                        break;
                    }
                    previous_was_comma = false;
                }
                Tok::Op(text) if text == "," && depth == 1 => {
                    previous_was_comma = true;
                }
                Tok::Name(text) if text == "as" && depth == 1 => {
                    saw_as = true;
                    previous_was_comma = false;
                }
                Tok::Newline | Tok::End => break,
                _ => {
                    previous_was_comma = false;
                }
            }
            scan += 1;
        }
        saw_as || trailing_comma
    }

    fn parse_with_item(&mut self) -> Result<WithItem, ParseError> {
        let pos = self.peek_pos();
        let context_expr = self.parse_expr()?;
        let optional_vars = if self.eat_name("as") {
            Some(self.parse_target()?)
        } else {
            None
        };
        Ok(WithItem {
            id: self.fresh_id(),
            pos,
            context_expr,
            optional_vars,
        })
    }

    fn parse_try(&mut self) -> Result<Stmt, ParseError> {
        let pos = self.bump().pos;
        self.expect_op(":")?;
        let body = self.parse_suite()?;
        let mut handlers = Vec::new();
        let mut star = false;
        while self.at_name("except") {
            let hpos = self.bump().pos;
            if self.eat_op("*") {
                star = true;
            }
            let (htype, name) = if self.at_op(":") {
                (None, None)
            } else {
                let htype = self.parse_expr()?;
                let name = if self.eat_name("as") {
                    Some(self.expect_name_text()?)
                } else {
                    None
                };
                (Some(htype), name)
            };
            self.expect_op(":")?;
            let hbody = self.parse_suite()?;
            handlers.push(ExceptHandler {
                id: self.fresh_id(),
                pos: hpos,
                type_: htype,
                name,
                body: hbody,
            });
        }
        let orelse = if self.eat_name("else") {
            self.expect_op(":")?;
            self.parse_suite()?
        } else {
            Vec::new()
        };
        let finalbody = if self.eat_name("finally") {
            self.expect_op(":")?;
            self.parse_suite()?
        } else {
            Vec::new()
        };
        Ok(self.stmt(
            pos,
            StmtKind::Try {
                body,
                handlers,
                orelse,
                finalbody,
                star,
            },
        ))
    }

    fn parse_decorated(&mut self) -> Result<Vec<Stmt>, ParseError> {
        let mut decorators = Vec::new();
        while self.at_op("@") {
            self.bump();
            let decorator = self.parse_expr()?;
            decorators.push(decorator);
            self.expect_newline()?;
        }
        if self.at_name("def") {
            Ok(vec![self.parse_funcdef(false, decorators)?])
        } else if self.at_name("class") {
            Ok(vec![self.parse_classdef(decorators)?])
        } else if self.at_name("async")
            && matches!(self.peek_at_kind(1), Tok::Name(text) if text == "def")
        {
            Ok(vec![self.parse_funcdef(true, decorators)?])
        } else {
            Err(err("a decorator stands over a definition"))
        }
    }

    fn parse_type_params(&mut self) -> Result<Vec<TypeParam>, ParseError> {
        let mut params = Vec::new();
        if !self.eat_op("[") {
            return Ok(params);
        }
        loop {
            let pos = self.peek_pos();
            if self.eat_op("*") {
                let name = self.expect_name_text()?;
                params.push(TypeParam {
                    id: self.fresh_id(),
                    pos,
                    kind: TypeParamKind::TypeVarTuple { name },
                });
            } else if self.eat_op("**") {
                let name = self.expect_name_text()?;
                params.push(TypeParam {
                    id: self.fresh_id(),
                    pos,
                    kind: TypeParamKind::ParamSpec { name },
                });
            } else {
                let name = self.expect_name_text()?;
                let bound = if self.eat_op(":") {
                    Some(Box::new(self.parse_expr()?))
                } else {
                    None
                };
                params.push(TypeParam {
                    id: self.fresh_id(),
                    pos,
                    kind: TypeParamKind::TypeVar { name, bound },
                });
            }
            if !self.eat_op(",") {
                break;
            }
            if self.at_op("]") {
                break;
            }
        }
        self.expect_op("]")?;
        Ok(params)
    }

    fn parse_funcdef(
        &mut self,
        is_async: bool,
        decorator_list: Vec<Expr>,
    ) -> Result<Stmt, ParseError> {
        // The interpreter places a plain definition at its
        // def keyword and an async definition at its async
        // keyword, and the entry token is that keyword in
        // both call paths.
        let pos = self.peek_pos();
        if is_async {
            self.bump(); // async
        }
        self.bump(); // def
        let name = self.expect_name_text()?;
        let type_params = self.parse_type_params()?;
        self.expect_op("(")?;
        let args = self.parse_arguments(true)?;
        self.expect_op(")")?;
        let returns = if self.eat_op("->") {
            Some(self.parse_expr()?)
        } else {
            None
        };
        self.expect_op(":")?;
        let body = self.parse_suite()?;
        Ok(self.stmt(
            pos,
            StmtKind::FuncDef {
                name,
                args: Box::new(args),
                body,
                decorators: decorator_list,
                returns: returns.map(Box::new),
                type_params,
                is_async,
            },
        ))
    }

    /// The arguments of a definition, in the interpreter's
    /// field split. A lambda's parameters carry no
    /// annotations, its colon ending the list where a
    /// definition's colon would open an annotation, so the
    /// annotation reading stands only where the caller
    /// allows it.
    fn parse_arguments(&mut self, allow_annotations: bool) -> Result<Arguments, ParseError> {
        let mut args = Arguments {
            id: self.fresh_id(),
            posonlyargs: Vec::new(),
            args: Vec::new(),
            vararg: None,
            kwonlyargs: Vec::new(),
            kw_defaults: Vec::new(),
            kwarg: None,
            defaults: Vec::new(),
        };
        #[derive(PartialEq)]
        enum Zone {
            Positional,
            KwOnly,
            Done,
        }
        let mut zone = Zone::Positional;
        // Positional parameters in declaration order, each
        // with its default where one stands. The slash moves
        // the split between posonlyargs and args without
        // disturbing the order, and the defaults ride the
        // tail of the combined run, which is the
        // interpreter's arrangement.
        let mut posonly: Vec<(Arg, Option<Expr>)> = Vec::new();
        let mut positional: Vec<(Arg, Option<Expr>)> = Vec::new();
        if self.at_op(")") {
            return Ok(args);
        }
        loop {
            if zone == Zone::Positional && self.at_op("/") {
                self.bump();
                posonly.append(&mut positional);
                if !self.eat_op(",") {
                    break;
                }
                if self.at_op(")") {
                    break;
                }
                continue;
            }
            if zone == Zone::Positional && self.at_op("*") {
                self.bump();
                zone = Zone::KwOnly;
                if matches!(self.peek_kind(), Tok::Name(_)) {
                    let arg = self.parse_one_arg(true, allow_annotations)?;
                    args.vararg = Some(arg);
                }
                if !self.eat_op(",") {
                    break;
                }
                if self.at_op(")") {
                    break;
                }
                continue;
            }
            if self.at_op("**") {
                self.bump();
                let arg = self.parse_one_arg(false, allow_annotations)?;
                args.kwarg = Some(arg);
                self.eat_op(",");
                break;
            }
            if zone == Zone::Done {
                return Err(err("arguments after the keyword argument"));
            }
            let arg = self.parse_one_arg(false, allow_annotations)?;
            let default = if self.eat_op("=") {
                Some(self.parse_expr()?)
            } else {
                None
            };
            match zone {
                Zone::Positional => positional.push((arg, default)),
                Zone::KwOnly => {
                    args.kwonlyargs.push(arg);
                    args.kw_defaults.push(default);
                }
                Zone::Done => unreachable!(),
            }
            if !self.eat_op(",") {
                break;
            }
            if self.at_op(")") {
                break;
            }
        }
        let mut combined = posonly;
        let split = combined.len();
        combined.append(&mut positional);
        for (index, (arg, default)) in combined.into_iter().enumerate() {
            if index < split {
                args.posonlyargs.push(arg);
            } else {
                args.args.push(arg);
            }
            if let Some(default) = default {
                args.defaults.push(default);
            }
        }
        Ok(args)
    }

    fn parse_one_arg(
        &mut self,
        starred_annotation: bool,
        allow_annotation: bool,
    ) -> Result<Arg, ParseError> {
        let pos = self.peek_pos();
        let name = self.expect_name_text()?;
        let annotation = if allow_annotation && self.eat_op(":") {
            // The annotation of a star argument may itself
            // be starred, the variadic spelling.
            Some(if starred_annotation {
                self.parse_star_or_expr()?
            } else {
                self.parse_expr()?
            })
        } else {
            None
        };
        Ok(Arg {
            id: self.fresh_id(),
            pos,
            name,
            annotation: annotation.map(Box::new),
        })
    }

    fn parse_classdef(&mut self, decorator_list: Vec<Expr>) -> Result<Stmt, ParseError> {
        let pos = self.bump().pos;
        let name = self.expect_name_text()?;
        let type_params = self.parse_type_params()?;
        let mut bases = Vec::new();
        let mut keywords = Vec::new();
        if self.eat_op("(") {
            if !self.at_op(")") {
                let (call_args, call_keywords) = self.parse_call_arguments()?;
                bases = call_args;
                keywords = call_keywords;
            }
            self.expect_op(")")?;
        }
        self.expect_op(":")?;
        let body = self.parse_suite()?;
        Ok(self.stmt(
            pos,
            StmtKind::ClassDef {
                name,
                bases,
                keywords,
                body,
                decorators: decorator_list,
                type_params,
            },
        ))
    }

    fn parse_type_alias(&mut self) -> Result<Stmt, ParseError> {
        let pos = self.bump().pos;
        let name_token = self.bump();
        let name_pos = name_token.pos;
        let name_text = match name_token.kind {
            Tok::Name(word) => word,
            _ => return Err(err("expected a name")),
        };
        let name = self.expr(name_pos, ExprKind::Name(name_text));
        let type_params = self.parse_type_params()?;
        self.expect_op("=")?;
        let value = self.parse_expr()?;
        Ok(self.stmt(
            pos,
            StmtKind::TypeAlias {
                name,
                type_params,
                value,
            },
        ))
    }
}

impl Parser {
    /// A full expression, the lambda, the walrus, and the
    /// conditional forms standing at its head.
    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        if self.at_name("lambda") {
            return self.parse_lambda();
        }
        if matches!(self.peek_kind(), Tok::Name(_)) && self.at_op_at(1, ":=") {
            let token = self.bump();
            let pos = token.pos;
            let name = match token.kind {
                Tok::Name(word) => word,
                _ => unreachable!(),
            };
            self.bump(); // the walrus
            let target = self.expr(pos, ExprKind::Name(name));
            let value = self.parse_expr()?;
            return Ok(self.expr(
                pos,
                ExprKind::NamedExpr {
                    target: Box::new(target),
                    value: Box::new(value),
                },
            ));
        }
        let body = self.parse_or()?;
        if self.eat_name("if") {
            let pos = body.pos;
            let (test, orelse) = self.deepen(|parser| {
                let test = parser.parse_or()?;
                if !parser.eat_name("else") {
                    return Err(err("expected else in a conditional expression"));
                }
                let orelse = parser.parse_expr()?;
                Ok((test, orelse))
            })?;
            return Ok(self.expr(
                pos,
                ExprKind::IfExp {
                    test: Box::new(test),
                    body: Box::new(body),
                    orelse: Box::new(orelse),
                },
            ));
        }
        Ok(body)
    }

    fn parse_lambda(&mut self) -> Result<Expr, ParseError> {
        let pos = self.bump().pos;
        let args = if self.at_op(":") {
            Arguments {
                id: self.fresh_id(),
                posonlyargs: Vec::new(),
                args: Vec::new(),
                vararg: None,
                kwonlyargs: Vec::new(),
                kw_defaults: Vec::new(),
                kwarg: None,
                defaults: Vec::new(),
            }
        } else {
            self.parse_arguments(false)?
        };
        self.expect_op(":")?;
        let body = self.deepen(|parser| parser.parse_expr())?;
        Ok(self.expr(
            pos,
            ExprKind::Lambda {
                args: Box::new(args),
                body: Box::new(body),
            },
        ))
    }

    fn parse_or(&mut self) -> Result<Expr, ParseError> {
        let first = self.parse_and()?;
        if !self.at_name("or") {
            return Ok(first);
        }
        let pos = first.pos;
        let mut values = vec![first];
        while self.eat_name("or") {
            values.push(self.parse_and()?);
        }
        Ok(self.expr(
            pos,
            ExprKind::BoolOp {
                op: BoolKind::Or,
                values,
            },
        ))
    }

    fn parse_and(&mut self) -> Result<Expr, ParseError> {
        let first = self.parse_not()?;
        if !self.at_name("and") {
            return Ok(first);
        }
        let pos = first.pos;
        let mut values = vec![first];
        while self.eat_name("and") {
            values.push(self.parse_not()?);
        }
        Ok(self.expr(
            pos,
            ExprKind::BoolOp {
                op: BoolKind::And,
                values,
            },
        ))
    }

    fn parse_not(&mut self) -> Result<Expr, ParseError> {
        if self.at_name("not") {
            let pos = self.bump().pos;
            let operand = self.deepen(|parser| parser.parse_not())?;
            return Ok(self.expr(
                pos,
                ExprKind::UnaryOp {
                    op: UnaryKind::Not,
                    operand: Box::new(operand),
                },
            ));
        }
        self.parse_comparison()
    }

    fn parse_comparison(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_bitor()?;
        let mut ops = Vec::new();
        let mut comparators = Vec::new();
        loop {
            let op = if self.at_op("<") {
                CmpKind::Lt
            } else if self.at_op("<=") {
                CmpKind::LtE
            } else if self.at_op(">") {
                CmpKind::Gt
            } else if self.at_op(">=") {
                CmpKind::GtE
            } else if self.at_op("==") {
                CmpKind::Eq
            } else if self.at_op("!=") {
                CmpKind::NotEq
            } else if self.at_name("in") {
                CmpKind::In
            } else if self.at_name("is") {
                if matches!(self.peek_at_kind(1), Tok::Name(text) if text == "not") {
                    self.bump();
                    self.bump();
                    ops.push(CmpKind::IsNot);
                    comparators.push(self.parse_bitor()?);
                    continue;
                }
                CmpKind::Is
            } else if self.at_name("not") {
                if matches!(self.peek_at_kind(1), Tok::Name(text) if text == "in") {
                    self.bump();
                    self.bump();
                    ops.push(CmpKind::NotIn);
                    comparators.push(self.parse_bitor()?);
                    continue;
                }
                break;
            } else {
                break;
            };
            self.bump();
            ops.push(op);
            comparators.push(self.parse_bitor()?);
        }
        if ops.is_empty() {
            return Ok(left);
        }
        let pos = left.pos;
        Ok(self.expr(
            pos,
            ExprKind::Compare {
                left: Box::new(left),
                ops,
                comparators,
            },
        ))
    }

    fn parse_binary_level(
        &mut self,
        next: fn(&mut Parser) -> Result<Expr, ParseError>,
        table: &[(&str, BinOpKind)],
    ) -> Result<Expr, ParseError> {
        let mut left = next(self)?;
        loop {
            let found = match self.peek_kind() {
                Tok::Op(text) => table
                    .iter()
                    .find(|(spelling, _)| spelling == &text.as_str())
                    .map(|(_, kind)| *kind),
                _ => None,
            };
            let Some(op) = found else { break };
            self.bump();
            let right = next(self)?;
            let pos = left.pos;
            left = self.expr(
                pos,
                ExprKind::BinOp {
                    left: Box::new(left),
                    op,
                    right: Box::new(right),
                },
            );
        }
        Ok(left)
    }

    fn parse_bitor(&mut self) -> Result<Expr, ParseError> {
        self.parse_binary_level(Parser::parse_bitxor, &[("|", BinOpKind::BitOr)])
    }

    fn parse_bitxor(&mut self) -> Result<Expr, ParseError> {
        self.parse_binary_level(Parser::parse_bitand, &[("^", BinOpKind::BitXor)])
    }

    fn parse_bitand(&mut self) -> Result<Expr, ParseError> {
        self.parse_binary_level(Parser::parse_shift, &[("&", BinOpKind::BitAnd)])
    }

    fn parse_shift(&mut self) -> Result<Expr, ParseError> {
        self.parse_binary_level(
            Parser::parse_arith,
            &[("<<", BinOpKind::LShift), (">>", BinOpKind::RShift)],
        )
    }

    fn parse_arith(&mut self) -> Result<Expr, ParseError> {
        self.parse_binary_level(
            Parser::parse_term,
            &[("+", BinOpKind::Add), ("-", BinOpKind::Sub)],
        )
    }

    fn parse_term(&mut self) -> Result<Expr, ParseError> {
        self.parse_binary_level(
            Parser::parse_factor,
            &[
                ("*", BinOpKind::Mult),
                ("@", BinOpKind::MatMult),
                ("/", BinOpKind::Div),
                ("%", BinOpKind::Mod),
                ("//", BinOpKind::FloorDiv),
            ],
        )
    }

    fn parse_factor(&mut self) -> Result<Expr, ParseError> {
        let kind = match self.peek_kind() {
            Tok::Op(text) if text == "+" => Some(UnaryKind::UAdd),
            Tok::Op(text) if text == "-" => Some(UnaryKind::USub),
            Tok::Op(text) if text == "~" => Some(UnaryKind::Invert),
            _ => None,
        };
        if let Some(op) = kind {
            let pos = self.bump().pos;
            let operand = self.deepen(|parser| parser.parse_factor())?;
            return Ok(self.expr(
                pos,
                ExprKind::UnaryOp {
                    op,
                    operand: Box::new(operand),
                },
            ));
        }
        self.parse_power()
    }

    fn parse_power(&mut self) -> Result<Expr, ParseError> {
        let base = self.parse_primary_with_trailers()?;
        if self.at_op("**") {
            self.bump();
            let pos = base.pos;
            let right = self.deepen(|parser| parser.parse_factor())?;
            return Ok(self.expr(
                pos,
                ExprKind::BinOp {
                    left: Box::new(base),
                    op: BinOpKind::Pow,
                    right: Box::new(right),
                },
            ));
        }
        Ok(base)
    }

    fn parse_primary_with_trailers(&mut self) -> Result<Expr, ParseError> {
        if self.at_name("await") {
            let pos = self.bump().pos;
            let inner = self.deepen(|parser| parser.parse_primary_with_trailers())?;
            return Ok(self.expr(
                pos,
                ExprKind::Await {
                    value: Box::new(inner),
                },
            ));
        }
        let mut base = self.parse_atom()?;
        loop {
            if self.at_op(".") {
                self.bump();
                let pos = base.pos;
                let attr = self.expect_name_text()?;
                base = self.expr(
                    pos,
                    ExprKind::Attribute {
                        value: Box::new(base),
                        attr,
                    },
                );
            } else if self.at_op("(") {
                self.bump();
                let pos = base.pos;
                let (args, keywords) = self.parse_call_arguments()?;
                self.expect_op(")")?;
                base = self.expr(
                    pos,
                    ExprKind::Call {
                        func: Box::new(base),
                        args,
                        keywords,
                    },
                );
            } else if self.at_op("[") {
                self.bump();
                let pos = base.pos;
                let slice = self.parse_subscript()?;
                self.expect_op("]")?;
                base = self.expr(
                    pos,
                    ExprKind::Subscript {
                        value: Box::new(base),
                        slice: Box::new(slice),
                    },
                );
            } else {
                break;
            }
        }
        Ok(base)
    }

    /// The arguments of a call, shared with the class
    /// definition's base list.
    fn parse_call_arguments(&mut self) -> Result<(Vec<Expr>, Vec<Keyword>), ParseError> {
        let mut args = Vec::new();
        let mut keywords = Vec::new();
        if self.at_op(")") {
            return Ok((args, keywords));
        }
        loop {
            if self.at_op("**") {
                let pos = self.bump().pos;
                let value = self.parse_expr()?;
                keywords.push(Keyword {
                    id: self.fresh_id(),
                    pos,
                    arg: None,
                    value,
                });
            } else if self.at_op("*") {
                let pos = self.bump().pos;
                let value = self.parse_bitor()?;
                args.push(self.expr(
                    pos,
                    ExprKind::Starred {
                        value: Box::new(value),
                    },
                ));
            } else if matches!(self.peek_kind(), Tok::Name(_))
                && self.at_op_at(1, "=")
                && !matches!(self.peek_kind(), Tok::Name(text) if is_keyword(text))
            {
                let token = self.bump();
                let pos = token.pos;
                let name = match token.kind {
                    Tok::Name(word) => word,
                    _ => unreachable!(),
                };
                self.bump(); // the equals sign
                let value = self.parse_expr()?;
                keywords.push(Keyword {
                    id: self.fresh_id(),
                    pos,
                    arg: Some(name),
                    value,
                });
            } else {
                let value = self.parse_expr()?;
                if self.at_name("for")
                    || (self.at_name("async")
                        && matches!(self.peek_at_kind(1), Tok::Name(text) if text == "for"))
                {
                    let pos = value.pos;
                    let generators = self.parse_comp_clauses()?;
                    args.push(self.expr(
                        pos,
                        ExprKind::GeneratorExp {
                            elt: Box::new(value),
                            generators,
                        },
                    ));
                } else {
                    args.push(value);
                }
            }
            if !self.eat_op(",") {
                break;
            }
            if self.at_op(")") {
                break;
            }
        }
        Ok((args, keywords))
    }

    fn parse_subscript(&mut self) -> Result<Expr, ParseError> {
        let mut items = Vec::new();
        loop {
            let item = self.parse_subscript_item()?;
            items.push(item);
            if !self.eat_op(",") {
                break;
            }
            if self.at_op("]") {
                // A trailing comma still folds a one item
                // subscript into a tuple.
                if items.len() == 1 {
                    let pos = items[0].pos;
                    let only = items.into_iter().next().unwrap();
                    return Ok(self.expr(pos, ExprKind::Tuple { elts: vec![only] }));
                }
                break;
            }
        }
        if items.len() == 1 {
            Ok(items.into_iter().next().unwrap())
        } else {
            let pos = items[0].pos;
            Ok(self.expr(pos, ExprKind::Tuple { elts: items }))
        }
    }

    fn parse_subscript_item(&mut self) -> Result<Expr, ParseError> {
        if self.at_op(":") {
            return self.parse_slice(None);
        }
        if self.at_op("*") {
            let pos = self.bump().pos;
            let value = self.parse_bitor()?;
            let starred = self.expr(
                pos,
                ExprKind::Starred {
                    value: Box::new(value),
                },
            );
            if self.at_op(":") {
                return self.parse_slice(Some(starred));
            }
            return Ok(starred);
        }
        let first = self.parse_expr()?;
        if self.at_op(":") {
            return self.parse_slice(Some(first));
        }
        Ok(first)
    }

    fn parse_slice(&mut self, lower: Option<Expr>) -> Result<Expr, ParseError> {
        let pos = lower
            .as_ref()
            .map(|expr| expr.pos)
            .unwrap_or_else(|| self.peek_pos());
        self.bump(); // the colon
        let upper = if self.at_op(":") || self.at_op(",") || self.at_op("]") {
            None
        } else {
            Some(self.parse_expr()?)
        };
        let step = if self.eat_op(":") {
            if self.at_op(",") || self.at_op("]") {
                None
            } else {
                Some(self.parse_expr()?)
            }
        } else {
            None
        };
        Ok(self.expr(
            pos,
            ExprKind::Slice {
                lower: lower.map(Box::new),
                upper: upper.map(Box::new),
                step: step.map(Box::new),
            },
        ))
    }

    /// An expression or a starred expression, the element
    /// shape of displays and bare tuples.
    fn parse_star_or_expr(&mut self) -> Result<Expr, ParseError> {
        if self.at_op("*") {
            let pos = self.bump().pos;
            let value = self.parse_bitor()?;
            return Ok(self.expr(
                pos,
                ExprKind::Starred {
                    value: Box::new(value),
                },
            ));
        }
        self.parse_expr()
    }

    /// The comprehension clauses after a display's first
    /// element.
    fn parse_comp_clauses(&mut self) -> Result<Vec<Comprehension>, ParseError> {
        let mut clauses = Vec::new();
        loop {
            let is_async = if self.at_name("async")
                && matches!(self.peek_at_kind(1), Tok::Name(text) if text == "for")
            {
                self.bump();
                true
            } else {
                false
            };
            if !self.at_name("for") {
                break;
            }
            let pos = self.bump().pos;
            let first = self.parse_target()?;
            let mut targets = vec![first];
            while self.eat_op(",") {
                targets.push(self.parse_target()?);
            }
            let target = if targets.len() == 1 {
                targets.into_iter().next().unwrap()
            } else {
                let at = targets[0].pos;
                self.expr(at, ExprKind::Tuple { elts: targets })
            };
            if !self.eat_name("in") {
                return Err(err("expected in in a comprehension"));
            }
            let iter = self.parse_comp_expr()?;
            let mut ifs = Vec::new();
            loop {
                if self.at_name("if") {
                    self.bump();
                    ifs.push(self.parse_comp_expr()?);
                } else {
                    break;
                }
            }
            clauses.push(Comprehension {
                id: self.fresh_id(),
                pos,
                target,
                iter,
                ifs,
                is_async,
            });
            if !(self.at_name("for")
                || (self.at_name("async")
                    && matches!(self.peek_at_kind(1), Tok::Name(text) if text == "for")))
            {
                break;
            }
        }
        if clauses.is_empty() {
            return Err(err("expected a comprehension clause"));
        }
        Ok(clauses)
    }

    /// A comprehension's iteration or condition expression,
    /// the walrus standing open and the conditional closed.
    fn parse_comp_expr(&mut self) -> Result<Expr, ParseError> {
        if matches!(self.peek_kind(), Tok::Name(_)) && self.at_op_at(1, ":=") {
            let token = self.bump();
            let pos = token.pos;
            let name = match token.kind {
                Tok::Name(word) => word,
                _ => unreachable!(),
            };
            self.bump();
            let target = self.expr(pos, ExprKind::Name(name));
            let value = self.parse_comp_expr()?;
            return Ok(self.expr(
                pos,
                ExprKind::NamedExpr {
                    target: Box::new(target),
                    value: Box::new(value),
                },
            ));
        }
        self.parse_or()
    }

    fn parse_atom(&mut self) -> Result<Expr, ParseError> {
        let token = self.tokens[self.index].clone();
        match token.kind {
            Tok::Name(word) => {
                self.bump();
                match word.as_str() {
                    "True" => Ok(self.expr(token.pos, ExprKind::Constant(ConstVal::Bool(true)))),
                    "False" => Ok(self.expr(token.pos, ExprKind::Constant(ConstVal::Bool(false)))),
                    "None" => Ok(self.expr(token.pos, ExprKind::Constant(ConstVal::NoneVal))),
                    _ if is_keyword(&word) => Err(err("a keyword stands where a value belongs")),
                    _ => Ok(self.expr(token.pos, ExprKind::Name(word))),
                }
            }
            Tok::Number(text) => {
                self.bump();
                let value = number_value(&text)?;
                Ok(self.expr(token.pos, ExprKind::Constant(value)))
            }
            Tok::Str(_) => self.parse_string_expr(),
            Tok::Op(text) => match text.as_str() {
                "(" => self.parse_paren_display(),
                "[" => self.parse_list_display(),
                "{" => self.parse_dict_set_display(),
                "..." => {
                    self.bump();
                    Ok(self.expr(token.pos, ExprKind::Constant(ConstVal::Ellipsis)))
                }
                _ => Err(err("an operator stands where a value belongs")),
            },
            _ => Err(err("expected a value")),
        }
    }

    fn parse_paren_display(&mut self) -> Result<Expr, ParseError> {
        let pos = self.bump().pos;
        if self.eat_op(")") {
            return Ok(self.expr(pos, ExprKind::Tuple { elts: Vec::new() }));
        }
        if self.at_name("yield") {
            let yielded = self.parse_yield_expr()?;
            self.expect_op(")")?;
            return Ok(yielded);
        }
        let first = self.deepen(|parser| parser.parse_star_or_expr())?;
        if self.at_name("for")
            || (self.at_name("async")
                && matches!(self.peek_at_kind(1), Tok::Name(text) if text == "for"))
        {
            let generators = self.parse_comp_clauses()?;
            self.expect_op(")")?;
            return Ok(self.expr(
                pos,
                ExprKind::GeneratorExp {
                    elt: Box::new(first),
                    generators,
                },
            ));
        }
        let mut elts = vec![first];
        let mut trailing = false;
        while self.eat_op(",") {
            if self.at_op(")") {
                trailing = true;
                break;
            }
            elts.push(self.deepen(|parser| parser.parse_star_or_expr())?);
        }
        self.expect_op(")")?;
        if elts.len() == 1 && !trailing {
            Ok(elts.into_iter().next().unwrap())
        } else {
            Ok(self.expr(pos, ExprKind::Tuple { elts }))
        }
    }

    fn parse_list_display(&mut self) -> Result<Expr, ParseError> {
        let pos = self.bump().pos;
        if self.eat_op("]") {
            return Ok(self.expr(pos, ExprKind::List { elts: Vec::new() }));
        }
        let first = self.deepen(|parser| parser.parse_star_or_expr())?;
        if self.at_name("for")
            || (self.at_name("async")
                && matches!(self.peek_at_kind(1), Tok::Name(text) if text == "for"))
        {
            let generators = self.parse_comp_clauses()?;
            self.expect_op("]")?;
            return Ok(self.expr(
                pos,
                ExprKind::ListComp {
                    elt: Box::new(first),
                    generators,
                },
            ));
        }
        let mut elts = vec![first];
        while self.eat_op(",") {
            if self.at_op("]") {
                break;
            }
            elts.push(self.deepen(|parser| parser.parse_star_or_expr())?);
        }
        self.expect_op("]")?;
        Ok(self.expr(pos, ExprKind::List { elts }))
    }

    fn parse_dict_set_display(&mut self) -> Result<Expr, ParseError> {
        let pos = self.bump().pos;
        if self.eat_op("}") {
            return Ok(self.expr(
                pos,
                ExprKind::Dict {
                    keys: Vec::new(),
                    values: Vec::new(),
                },
            ));
        }
        // The first element decides the display family.
        if self.at_op("**") {
            self.bump();
            let first_value = self.parse_bitor()?;
            let mut keys: Vec<Option<Expr>> = vec![None];
            let mut values = vec![first_value];
            while self.eat_op(",") {
                if self.at_op("}") {
                    break;
                }
                if self.eat_op("**") {
                    keys.push(None);
                    values.push(self.parse_bitor()?);
                } else {
                    keys.push(Some(self.parse_expr()?));
                    self.expect_op(":")?;
                    values.push(self.parse_expr()?);
                }
            }
            self.expect_op("}")?;
            return Ok(self.expr(pos, ExprKind::Dict { keys, values }));
        }
        let first = self.deepen(|parser| parser.parse_star_or_expr())?;
        if self.at_op(":") {
            self.bump();
            let first_value = self.parse_expr()?;
            if self.at_name("for")
                || (self.at_name("async")
                    && matches!(self.peek_at_kind(1), Tok::Name(text) if text == "for"))
            {
                let generators = self.parse_comp_clauses()?;
                self.expect_op("}")?;
                return Ok(self.expr(
                    pos,
                    ExprKind::DictComp {
                        key: Box::new(first),
                        value: Box::new(first_value),
                        generators,
                    },
                ));
            }
            let mut keys: Vec<Option<Expr>> = vec![Some(first)];
            let mut values = vec![first_value];
            while self.eat_op(",") {
                if self.at_op("}") {
                    break;
                }
                if self.eat_op("**") {
                    keys.push(None);
                    values.push(self.parse_bitor()?);
                } else {
                    keys.push(Some(self.parse_expr()?));
                    self.expect_op(":")?;
                    values.push(self.parse_expr()?);
                }
            }
            self.expect_op("}")?;
            return Ok(self.expr(pos, ExprKind::Dict { keys, values }));
        }
        if self.at_name("for")
            || (self.at_name("async")
                && matches!(self.peek_at_kind(1), Tok::Name(text) if text == "for"))
        {
            let generators = self.parse_comp_clauses()?;
            self.expect_op("}")?;
            return Ok(self.expr(
                pos,
                ExprKind::SetComp {
                    elt: Box::new(first),
                    generators,
                },
            ));
        }
        let mut elts = vec![first];
        while self.eat_op(",") {
            if self.at_op("}") {
                break;
            }
            elts.push(self.deepen(|parser| parser.parse_star_or_expr())?);
        }
        self.expect_op("}")?;
        Ok(self.expr(pos, ExprKind::Set { elts }))
    }

    /// Strings standing together, the concatenation the
    /// grammar folds into one constant or one joined string.
    fn parse_string_expr(&mut self) -> Result<Expr, ParseError> {
        let mut tokens: Vec<(Pos, StrTok)> = Vec::new();
        while let Tok::Str(part) = self.peek_kind().clone() {
            let pos = self.peek_pos();
            self.bump();
            tokens.push((pos, part));
        }
        let pos = tokens[0].0;
        let any_fstring = tokens
            .iter()
            .any(|(_, part)| matches!(part, StrTok::FString(_)));
        let any_bytes = tokens
            .iter()
            .any(|(_, part)| matches!(part, StrTok::Value(ConstVal::Bytes(_))));
        let any_text = tokens.iter().any(|(_, part)| {
            matches!(part, StrTok::Value(ConstVal::Str(_))) || matches!(part, StrTok::FString(_))
        });
        if any_bytes && any_text {
            return Err(err("cannot mix bytes and text strings"));
        }
        if !any_fstring {
            // Plain concatenation of decoded values.
            let mut merged: Option<ConstVal> = None;
            for (_, part) in tokens {
                let StrTok::Value(value) = part else {
                    unreachable!()
                };
                merged = Some(match (merged, value) {
                    (None, value) => value,
                    (Some(ConstVal::Str(mut text)), ConstVal::Str(more)) => {
                        text.push_text(&more);
                        ConstVal::Str(text)
                    }
                    (Some(ConstVal::Bytes(mut bytes)), ConstVal::Bytes(more)) => {
                        bytes.extend(more);
                        ConstVal::Bytes(bytes)
                    }
                    _ => return Err(err("cannot mix bytes and text strings")),
                });
            }
            return Ok(self.expr(pos, ExprKind::Constant(merged.unwrap())));
        }
        let mut values: Vec<Expr> = Vec::new();
        for (part_pos, part) in tokens {
            match part {
                StrTok::Value(ConstVal::Str(text)) => {
                    push_literal(self, &mut values, part_pos, &text);
                }
                StrTok::Value(_) => return Err(err("cannot mix bytes and text strings")),
                StrTok::FString(parts) => {
                    for fpart in parts {
                        match fpart {
                            FPart::Lit(codepoints) => {
                                let text = PyText::from_codepoints(codepoints.clone());
                                push_literal(self, &mut values, part_pos, &text);
                            }
                            FPart::Field(field) => {
                                if let Some(debug) = &field.debug {
                                    push_literal(
                                        self,
                                        &mut values,
                                        field.pos,
                                        &PyText::from_text(debug),
                                    );
                                }
                                let value = parse_fragment(&field)?;
                                // The self documenting form
                                // defaults to the repr conversion
                                // only where no format spec
                                // stands, the interpreter's rule.
                                let conversion = field.conversion.map(|ch| ch as i32).unwrap_or(
                                    if field.debug.is_some() && field.spec.is_none() {
                                        114
                                    } else {
                                        -1
                                    },
                                );
                                let format_spec = match &field.spec {
                                    None => None,
                                    Some(spec_parts) => {
                                        let spec_expr = self.build_joined(spec_parts, field.pos)?;
                                        Some(Box::new(spec_expr))
                                    }
                                };
                                values.push(self.expr(
                                    field.pos,
                                    ExprKind::FormattedValue {
                                        value: Box::new(value),
                                        conversion,
                                        format_spec,
                                    },
                                ));
                            }
                        }
                    }
                }
            }
        }
        Ok(self.expr(pos, ExprKind::JoinedStr { values }))
    }

    /// Build a joined string from the parts of a format
    /// spec, whose literal parts carry their decoded
    /// values the way the parts of the string body do.
    fn build_joined(&mut self, parts: &[FPart], pos: Pos) -> Result<Expr, ParseError> {
        let mut values: Vec<Expr> = Vec::new();
        for fpart in parts {
            match fpart {
                FPart::Lit(codepoints) => {
                    let text = PyText::from_codepoints(codepoints.clone());
                    push_literal(self, &mut values, pos, &text);
                }
                FPart::Field(field) => {
                    if let Some(debug) = &field.debug {
                        push_literal(self, &mut values, field.pos, &PyText::from_text(debug));
                    }
                    let value = parse_fragment(field)?;
                    let conversion = field.conversion.map(|ch| ch as i32).unwrap_or(
                        if field.debug.is_some() && field.spec.is_none() {
                            114
                        } else {
                            -1
                        },
                    );
                    let format_spec = match &field.spec {
                        None => None,
                        Some(spec_parts) => {
                            Some(Box::new(self.build_joined(spec_parts, field.pos)?))
                        }
                    };
                    values.push(self.expr(
                        field.pos,
                        ExprKind::FormattedValue {
                            value: Box::new(value),
                            conversion,
                            format_spec,
                        },
                    ));
                }
            }
        }
        Ok(self.expr(pos, ExprKind::JoinedStr { values }))
    }
}

/// Append a literal run to the values of a joined string,
/// merging it into the constant before it when both are
/// text, the fold the interpreter makes.
fn push_literal(parser: &mut Parser, values: &mut Vec<Expr>, pos: Pos, text: &PyText) {
    if let Some(last) = values.last_mut() {
        if let ExprKind::Constant(ConstVal::Str(existing)) = &mut last.kind {
            existing.push_text(text);
            return;
        }
    }
    let value = parser.expr(pos, ExprKind::Constant(ConstVal::Str(text.clone())));
    values.push(value);
}

/// Fold a number token's text into the constant the
/// interpreter assigns it.
fn number_value(text: &str) -> Result<ConstVal, ParseError> {
    let (imaginary, body) = if let Some(stripped) = text.strip_suffix(['j', 'J']) {
        (true, stripped)
    } else {
        (false, text)
    };
    validate_underscores(body)?;
    let clean: String = body.chars().filter(|ch| *ch != '_').collect();
    let value = if let Some(rest) = clean
        .strip_prefix("0x")
        .or_else(|| clean.strip_prefix("0X"))
    {
        let decimal =
            int_literal_to_decimal(rest, 16).ok_or_else(|| err("invalid hexadecimal literal"))?;
        ConstVal::Int(decimal)
    } else if let Some(rest) = clean
        .strip_prefix("0o")
        .or_else(|| clean.strip_prefix("0O"))
    {
        let decimal =
            int_literal_to_decimal(rest, 8).ok_or_else(|| err("invalid octal literal"))?;
        ConstVal::Int(decimal)
    } else if let Some(rest) = clean
        .strip_prefix("0b")
        .or_else(|| clean.strip_prefix("0B"))
    {
        let decimal =
            int_literal_to_decimal(rest, 2).ok_or_else(|| err("invalid binary literal"))?;
        ConstVal::Int(decimal)
    } else if clean.contains('.') || clean.contains('e') || clean.contains('E') {
        check_leading_zero(body)?;
        let parsed: f64 = clean.parse().map_err(|_| err("invalid float literal"))?;
        ConstVal::Float(parsed)
    } else {
        check_leading_zero(body)?;
        let decimal =
            int_literal_to_decimal(&clean, 10).ok_or_else(|| err("invalid integer literal"))?;
        ConstVal::Int(decimal)
    };
    if imaginary {
        let magnitude = match &value {
            ConstVal::Int(decimal) => decimal
                .parse::<f64>()
                .map_err(|_| err("invalid imaginary literal"))?,
            ConstVal::Float(value) => *value,
            _ => unreachable!(),
        };
        return Ok(ConstVal::Complex(0.0, magnitude));
    }
    Ok(value)
}

/// An integer written with a leading zero and a further
/// digit refuses, the reading the interpreter makes.
fn check_leading_zero(body: &str) -> Result<(), ParseError> {
    let mut chars = body.chars();
    if chars.next() == Some('0') {
        if let Some(second) = chars.next() {
            if second.is_ascii_digit() {
                return Err(err(
                    "leading zeros in decimal integer literals are not permitted",
                ));
            }
        }
    }
    Ok(())
}

/// Underscores in a numeric literal stand between digits,
/// or directly after a base prefix.
fn validate_underscores(body: &str) -> Result<(), ParseError> {
    let chars: Vec<char> = body.chars().collect();
    let hex = chars.len() >= 2 && chars[0] == '0' && matches!(chars[1], 'x' | 'X');
    for (index, ch) in chars.iter().enumerate() {
        if *ch != '_' {
            continue;
        }
        let before = if index == 0 {
            None
        } else {
            Some(chars[index - 1])
        };
        let after = chars.get(index + 1).copied();
        let digit_like = |c: char| c.is_ascii_digit() || (hex && c.is_ascii_alphabetic());
        let before_ok = match before {
            // Directly after the base prefix, the 0x and its
            // kin.
            Some(prev)
                if index == 2
                    && chars[0] == '0'
                    && matches!(prev, 'x' | 'X' | 'o' | 'O' | 'b' | 'B') =>
            {
                true
            }
            Some(prev) if digit_like(prev) && (hex || (prev != 'e' && prev != 'E')) => true,
            _ => false,
        };
        let after_ok = matches!(after, Some(next) if digit_like(next));
        if !before_ok || !after_ok {
            return Err(err("invalid underscore in a numeric literal"));
        }
    }
    Ok(())
}

impl Parser {
    fn parse_match(&mut self) -> Result<Stmt, ParseError> {
        let pos = self.bump().pos;
        let subject = self.parse_value_list()?;
        self.expect_op(":")?;
        self.expect_newline()?;
        if !matches!(self.peek_kind(), Tok::Indent) {
            return Err(err("expected an indented block"));
        }
        self.bump();
        let mut cases = Vec::new();
        self.skip_newlines();
        while self.at_name("case") {
            let case_pos = self.bump().pos;
            let pattern = self.parse_pattern()?;
            let guard = if self.eat_name("if") {
                Some(self.parse_expr()?)
            } else {
                None
            };
            self.expect_op(":")?;
            let body = self.parse_suite()?;
            cases.push(MatchCase {
                id: self.fresh_id(),
                pos: case_pos,
                pattern,
                guard,
                body,
            });
            self.skip_newlines();
        }
        if cases.is_empty() {
            return Err(err("a match statement holds at least one case"));
        }
        if matches!(self.peek_kind(), Tok::Dedent) {
            self.bump();
        }
        Ok(self.stmt(pos, StmtKind::Match { subject, cases }))
    }

    fn parse_pattern(&mut self) -> Result<Pattern, ParseError> {
        let first = self.parse_pattern_closed()?;
        let mut pattern = if self.at_op("|") {
            let pos = first.pos;
            let mut alts = vec![first];
            while self.eat_op("|") {
                alts.push(self.parse_pattern_closed()?);
            }
            Pattern {
                id: self.fresh_id(),
                pos,
                kind: PatternKind::MatchOr(alts),
            }
        } else {
            first
        };
        if self.eat_name("as") {
            let pos = pattern.pos;
            let name = self.expect_name_text()?;
            pattern = Pattern {
                id: self.fresh_id(),
                pos,
                kind: PatternKind::MatchAs {
                    pattern: Some(Box::new(pattern)),
                    name: Some(name),
                },
            };
        }
        Ok(pattern)
    }

    fn pattern(&mut self, pos: Pos, kind: PatternKind) -> Pattern {
        Pattern {
            id: self.fresh_id(),
            pos,
            kind,
        }
    }

    fn parse_pattern_closed(&mut self) -> Result<Pattern, ParseError> {
        let pos = self.peek_pos();
        match self.peek_kind().clone() {
            Tok::Op(text) if text == "[" => {
                self.bump();
                let patterns = self.parse_pattern_list("]")?;
                self.expect_op("]")?;
                Ok(self.pattern(pos, PatternKind::MatchSequence(patterns)))
            }
            Tok::Op(text) if text == "(" => {
                self.bump();
                if self.eat_op(")") {
                    return Ok(self.pattern(pos, PatternKind::MatchSequence(Vec::new())));
                }
                let first = self.parse_pattern()?;
                if !self.at_op(",") {
                    self.expect_op(")")?;
                    return Ok(first);
                }
                let mut patterns = vec![first];
                while self.eat_op(",") {
                    if self.at_op(")") {
                        break;
                    }
                    patterns.push(self.parse_pattern()?);
                }
                self.expect_op(")")?;
                Ok(self.pattern(pos, PatternKind::MatchSequence(patterns)))
            }
            Tok::Op(text) if text == "{" => self.parse_mapping_pattern(),
            Tok::Op(text) if text == "-" || text == "+" => {
                let value = self.parse_pattern_number()?;
                Ok(self.pattern(pos, PatternKind::MatchValue(value)))
            }
            Tok::Number(_) => {
                let value = self.parse_pattern_number()?;
                Ok(self.pattern(pos, PatternKind::MatchValue(value)))
            }
            Tok::Str(_) => {
                let value = self.parse_string_expr()?;
                Ok(self.pattern(pos, PatternKind::MatchValue(value)))
            }
            Tok::Name(word) => {
                match word.as_str() {
                    "_" => {
                        self.bump();
                        Ok(self.pattern(
                            pos,
                            PatternKind::MatchAs {
                                pattern: None,
                                name: None,
                            },
                        ))
                    }
                    "None" => {
                        self.bump();
                        Ok(self.pattern(pos, PatternKind::MatchSingleton(ConstVal::NoneVal)))
                    }
                    "True" => {
                        self.bump();
                        Ok(self.pattern(pos, PatternKind::MatchSingleton(ConstVal::Bool(true))))
                    }
                    "False" => {
                        self.bump();
                        Ok(self.pattern(pos, PatternKind::MatchSingleton(ConstVal::Bool(false))))
                    }
                    _ => {
                        // A dotted name is a value or a class
                        // pattern head. A bare name followed by
                        // a call opens a class pattern. Any
                        // other bare name captures.
                        let mut dotted = false;
                        let mut probe = self.index + 1;
                        while matches!(&self.tokens[probe].kind, Tok::Op(text) if text == ".")
                            && matches!(&self.tokens[probe + 1].kind, Tok::Name(_))
                        {
                            dotted = true;
                            probe += 2;
                        }
                        let followed_by_call =
                            matches!(&self.tokens[probe].kind, Tok::Op(text) if text == "(");
                        if dotted || followed_by_call {
                            let cls = self.parse_dotted_name_expr()?;
                            if self.at_op("(") {
                                return self.parse_class_pattern(cls);
                            }
                            return Ok(self.pattern(pos, PatternKind::MatchValue(cls)));
                        }
                        self.bump();
                        Ok(self.pattern(
                            pos,
                            PatternKind::MatchAs {
                                pattern: None,
                                name: Some(word),
                            },
                        ))
                    }
                }
            }
            _ => Err(err("expected a pattern")),
        }
    }

    fn parse_pattern_list(&mut self, _closer: &str) -> Result<Vec<Pattern>, ParseError> {
        let mut patterns = Vec::new();
        if self.at_op("]") || self.at_op(")") || self.at_op("}") {
            return Ok(patterns);
        }
        loop {
            if self.at_op("*") {
                let star_pos = self.bump().pos;
                let name = if matches!(self.peek_kind(), Tok::Name(word) if word != "_") {
                    Some(self.expect_name_text()?)
                } else if self.at_name("_") {
                    self.bump();
                    None
                } else {
                    None
                };
                patterns.push(self.pattern(star_pos, PatternKind::MatchStar(name)));
            } else {
                patterns.push(self.parse_pattern()?);
            }
            if !self.eat_op(",") {
                break;
            }
            if self.at_op("]") || self.at_op(")") {
                break;
            }
        }
        Ok(patterns)
    }

    fn parse_mapping_pattern(&mut self) -> Result<Pattern, ParseError> {
        let pos = self.bump().pos;
        let mut keys = Vec::new();
        let mut patterns = Vec::new();
        let mut rest = None;
        if !self.at_op("}") {
            loop {
                if self.eat_op("**") {
                    let name = self.expect_name_text()?;
                    rest = if name == "_" { None } else { Some(name) };
                } else {
                    let key = self.parse_pattern_key()?;
                    self.expect_op(":")?;
                    let pattern = self.parse_pattern()?;
                    keys.push(key);
                    patterns.push(pattern);
                }
                if !self.eat_op(",") {
                    break;
                }
                if self.at_op("}") {
                    break;
                }
            }
        }
        self.expect_op("}")?;
        Ok(self.pattern(
            pos,
            PatternKind::MatchMapping {
                keys,
                patterns,
                rest,
            },
        ))
    }

    fn parse_pattern_key(&mut self) -> Result<Expr, ParseError> {
        let pos = self.peek_pos();
        match self.peek_kind().clone() {
            Tok::Str(_) => self.parse_string_expr(),
            Tok::Number(_) => self.parse_pattern_number(),
            Tok::Op(text) if text == "-" || text == "+" => self.parse_pattern_number(),
            Tok::Name(word) => match word.as_str() {
                "None" => {
                    self.bump();
                    Ok(self.expr(pos, ExprKind::Constant(ConstVal::NoneVal)))
                }
                "True" => {
                    self.bump();
                    Ok(self.expr(pos, ExprKind::Constant(ConstVal::Bool(true))))
                }
                "False" => {
                    self.bump();
                    Ok(self.expr(pos, ExprKind::Constant(ConstVal::Bool(false))))
                }
                _ => self.parse_dotted_name_expr(),
            },
            _ => Err(err("expected a mapping pattern key")),
        }
    }

    fn parse_dotted_name_expr(&mut self) -> Result<Expr, ParseError> {
        let token = self.bump();
        let pos = token.pos;
        let word = match token.kind {
            Tok::Name(word) => word,
            _ => return Err(err("expected a name")),
        };
        let mut expr = self.expr(pos, ExprKind::Name(word));
        while self.eat_op(".") {
            let attr = self.expect_name_text()?;
            expr = self.expr(
                pos,
                ExprKind::Attribute {
                    value: Box::new(expr),
                    attr,
                },
            );
        }
        Ok(expr)
    }

    fn parse_class_pattern(&mut self, cls: Expr) -> Result<Pattern, ParseError> {
        let pos = cls.pos;
        self.bump(); // the opening paren
        let mut patterns = Vec::new();
        let mut kwd_attrs = Vec::new();
        let mut kwd_patterns = Vec::new();
        if !self.at_op(")") {
            loop {
                if matches!(self.peek_kind(), Tok::Name(_)) && self.at_op_at(1, "=") {
                    let attr = self.expect_name_text()?;
                    self.bump();
                    kwd_attrs.push(attr);
                    kwd_patterns.push(self.parse_pattern()?);
                } else {
                    patterns.push(self.parse_pattern()?);
                }
                if !self.eat_op(",") {
                    break;
                }
                if self.at_op(")") {
                    break;
                }
            }
        }
        self.expect_op(")")?;
        Ok(self.pattern(
            pos,
            PatternKind::MatchClass {
                cls: Box::new(cls),
                patterns,
                kwd_attrs,
                kwd_patterns,
            },
        ))
    }

    /// A number standing in a pattern, with its sign and its
    /// complex companion where they stand.
    fn parse_pattern_number(&mut self) -> Result<Expr, ParseError> {
        let pos = self.peek_pos();
        let sign = if self.at_op("-") {
            self.bump();
            Some(UnaryKind::USub)
        } else if self.at_op("+") {
            self.bump();
            Some(UnaryKind::UAdd)
        } else {
            None
        };
        let token = self.bump();
        let text = match token.kind {
            Tok::Number(text) => text,
            _ => return Err(err("expected a number in a pattern")),
        };
        let value = number_value(&text)?;
        let mut expr = self.expr(pos, ExprKind::Constant(value));
        if let Some(op) = sign {
            expr = self.expr(
                pos,
                ExprKind::UnaryOp {
                    op,
                    operand: Box::new(expr),
                },
            );
        }
        // A complex companion, the real part and the
        // imaginary part joined.
        let join = if self.at_op("+") {
            Some(BinOpKind::Add)
        } else if self.at_op("-") {
            Some(BinOpKind::Sub)
        } else {
            None
        };
        if let Some(op) = join {
            // The companion stands past the joining sign, an
            // imaginary number, and the sign is consumed only
            // where the companion truly follows.
            let companion = matches!(
                self.peek_at_kind(1),
                Tok::Number(text) if text.ends_with('j') || text.ends_with('J')
            );
            if companion {
                self.bump(); // the joining sign
                let token = self.bump();
                let text = match token.kind {
                    Tok::Number(text) => text,
                    _ => return Err(err("expected a number in a pattern")),
                };
                let imag = number_value(&text)?;
                let right = self.expr(pos, ExprKind::Constant(imag));
                expr = self.expr(
                    pos,
                    ExprKind::BinOp {
                        left: Box::new(expr),
                        op,
                        right: Box::new(right),
                    },
                );
            }
        }
        Ok(expr)
    }
}
