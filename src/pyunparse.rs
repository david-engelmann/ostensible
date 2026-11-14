//! The unparse spelling layer of PORT7-3. It reproduces
//! ast.unparse of CPython 3.12 byte for byte over the trees
//! the parser builds, following the interpreter's own
//! unparser, its precedence table, its parenthesization, and
//! its string literal spelling, the quote choice and the
//! escape rules among them.

use crate::pyast::*;
use crate::pydata::{is_printable_cp, unicode_escape_cp};

/// The one string repr implementation, standing in
/// pydata under PORT9-1; this entry delegates to it.
pub use crate::pydata::str_repr;

// The precedence table of the interpreter's unparser, in
// its stated order. BOR shares the value of EXPR.
const NAMED_EXPR: u8 = 1;
const TUPLE: u8 = 2;
const YIELD: u8 = 3;
const TEST: u8 = 4;
const OR: u8 = 5;
const AND: u8 = 6;
const NOT: u8 = 7;
const CMP: u8 = 8;
const EXPR: u8 = 9;
const BOR: u8 = 9;
const BXOR: u8 = 10;
const BAND: u8 = 11;
const SHIFT: u8 = 12;
const ARITH: u8 = 13;
const TERM: u8 = 14;
const FACTOR: u8 = 15;
const POWER: u8 = 16;
const AWAIT: u8 = 17;
const ATOM: u8 = 18;

fn next(precedence: u8) -> u8 {
    if precedence >= ATOM {
        ATOM
    } else {
        precedence + 1
    }
}

const MULTI_QUOTES: [&str; 2] = ["\"\"\"", "'''"];
const ALL_QUOTES: [&str; 4] = ["'", "\"", "\"\"\"", "'''"];
const INFSTR: &str = "1e309";

/// Unparse a whole module, the module reading of
/// ast.unparse.
pub fn unparse_module(module: &Module) -> String {
    run_big_stack(|| {
        let mut unparser = Unparser::new();
        unparser.module(module);
        unparser.out
    })
}

/// Unparse one expression, the expression reading of
/// ast.unparse.
pub fn unparse_expr(expr: &Expr) -> String {
    run_big_stack(|| {
        let mut unparser = Unparser::new();
        unparser.expr(expr, TEST);
        unparser.out
    })
}

/// Run a reading on a spawned thread carrying a stack of
/// 512 MiB, the standing of the scan path, so a tree
/// nested to the port bound spells whole on a caller's
/// plain thread instead of overflowing its stack. A panic
/// on the spawned thread resumes on the caller's thread.
fn run_big_stack<T: Send>(work: impl FnOnce() -> T + Send) -> T {
    std::thread::scope(|scope| {
        let handle = std::thread::Builder::new()
            .stack_size(512 * 1024 * 1024)
            .spawn_scoped(scope, work)
            .expect("the unparse thread spawns");
        match handle.join() {
            Ok(value) => value,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    })
}

struct Unparser {
    out: String,
    indent: usize,
    in_try_star: bool,
}

impl Unparser {
    fn new() -> Unparser {
        Unparser {
            out: String::new(),
            indent: 0,
            in_try_star: false,
        }
    }

    fn write(&mut self, text: &str) {
        self.out.push_str(text);
    }

    fn maybe_newline(&mut self) {
        if !self.out.is_empty() {
            self.out.push('\n');
        }
    }

    fn fill(&mut self, text: &str) {
        self.maybe_newline();
        for _ in 0..self.indent {
            self.out.push_str("    ");
        }
        self.out.push_str(text);
    }

    fn block(&mut self, body: &[Stmt]) {
        self.write(":");
        self.indent += 1;
        for statement in body {
            self.stmt(statement);
        }
        self.indent -= 1;
    }

    fn module(&mut self, module: &Module) {
        self.write_body_with_docstring(&module.body);
    }

    fn raw_docstring(body: &[Stmt]) -> Option<&PyText> {
        let first = body.first()?;
        if let StmtKind::ExprStmt { value } = &first.kind {
            if let ExprKind::Constant(ConstVal::Str(text)) = &value.kind {
                return Some(text);
            }
        }
        None
    }

    fn write_body_with_docstring(&mut self, body: &[Stmt]) {
        if let Some(docstring) = Self::raw_docstring(body) {
            let docstring = docstring.clone();
            self.write_docstring(&docstring);
            for statement in &body[1..] {
                self.stmt(statement);
            }
        } else {
            for statement in body {
                self.stmt(statement);
            }
        }
    }

    fn write_docstring(&mut self, value: &PyText) {
        self.fill("");
        let (escaped, quotes) = str_literal_helper(value.codepoints(), &MULTI_QUOTES, false);
        self.write(quotes[0].as_str());
        self.write(&escaped);
        self.write(quotes[0].as_str());
    }

    fn stmt(&mut self, node: &Stmt) {
        match &node.kind {
            StmtKind::FuncDef {
                name,
                args,
                body,
                decorators,
                returns,
                type_params,
                is_async,
            } => {
                self.maybe_newline();
                for decorator in decorators {
                    self.fill("@");
                    self.expr(decorator, TEST);
                }
                let head = if *is_async {
                    format!("async def {}", name)
                } else {
                    format!("def {}", name)
                };
                self.fill(&head);
                self.type_params(type_params);
                self.write("(");
                self.arguments(args);
                self.write(")");
                if let Some(returns) = returns {
                    self.write(" -> ");
                    self.expr(returns, TEST);
                }
                self.write(":");
                self.indent += 1;
                self.write_body_with_docstring(body);
                self.indent -= 1;
            }
            StmtKind::ClassDef {
                name,
                bases,
                keywords,
                body,
                decorators,
                type_params,
            } => {
                self.maybe_newline();
                for decorator in decorators {
                    self.fill("@");
                    self.expr(decorator, TEST);
                }
                self.fill(&format!("class {}", name));
                self.type_params(type_params);
                if !bases.is_empty() || !keywords.is_empty() {
                    self.write("(");
                    let mut comma = false;
                    for base in bases {
                        if comma {
                            self.write(", ");
                        }
                        comma = true;
                        self.expr(base, TEST);
                    }
                    for keyword in keywords {
                        if comma {
                            self.write(", ");
                        }
                        comma = true;
                        self.keyword(keyword);
                    }
                    self.write(")");
                }
                self.write(":");
                self.indent += 1;
                self.write_body_with_docstring(body);
                self.indent -= 1;
            }
            StmtKind::Return { value } => {
                self.fill("return");
                if let Some(value) = value {
                    self.write(" ");
                    self.expr(value, TEST);
                }
            }
            StmtKind::Delete { targets } => {
                self.fill("del ");
                self.interleave_exprs(", ", targets);
            }
            StmtKind::Assign { targets, value } => {
                self.fill("");
                for target in targets {
                    self.expr(target, TUPLE);
                    self.write(" = ");
                }
                self.expr(value, TEST);
            }
            StmtKind::AugAssign { target, op, value } => {
                self.fill("");
                self.expr(target, TEST);
                self.write(" ");
                self.write(binop_spelling(*op));
                self.write("= ");
                self.expr(value, TEST);
            }
            StmtKind::AnnAssign {
                target,
                annotation,
                value,
                simple,
            } => {
                self.fill("");
                if !*simple && matches!(target.kind, ExprKind::Name(_)) {
                    self.write("(");
                    self.expr(target, TEST);
                    self.write(")");
                } else {
                    self.expr(target, TEST);
                }
                self.write(": ");
                self.expr(annotation, TEST);
                if let Some(value) = value {
                    self.write(" = ");
                    self.expr(value, TEST);
                }
            }
            StmtKind::For {
                target,
                iter,
                body,
                orelse,
                is_async,
            } => {
                self.fill(if *is_async { "async for " } else { "for " });
                self.expr(target, TUPLE);
                self.write(" in ");
                self.expr(iter, TEST);
                self.block(body);
                if !orelse.is_empty() {
                    self.fill("else");
                    self.block(orelse);
                }
            }
            StmtKind::While { test, body, orelse } => {
                self.fill("while ");
                self.expr(test, TEST);
                self.block(body);
                if !orelse.is_empty() {
                    self.fill("else");
                    self.block(orelse);
                }
            }
            StmtKind::If { test, body, orelse } => {
                self.fill("if ");
                self.expr(test, TEST);
                self.block(body);
                let mut rest = orelse;
                loop {
                    if rest.len() == 1 {
                        if let StmtKind::If {
                            test: inner_test,
                            body: inner_body,
                            orelse: inner_orelse,
                        } = &rest[0].kind
                        {
                            self.fill("elif ");
                            self.expr(inner_test, TEST);
                            self.block(inner_body);
                            rest = inner_orelse;
                            continue;
                        }
                    }
                    break;
                }
                if !rest.is_empty() {
                    self.fill("else");
                    self.block(rest);
                }
            }
            StmtKind::With {
                items,
                body,
                is_async,
            } => {
                self.fill(if *is_async { "async with " } else { "with " });
                let mut first = true;
                for item in items {
                    if !first {
                        self.write(", ");
                    }
                    first = false;
                    self.with_item(item);
                }
                self.block(body);
            }
            StmtKind::Match { subject, cases } => {
                self.fill("match ");
                self.expr(subject, TEST);
                self.write(":");
                self.indent += 1;
                for case in cases {
                    self.match_case(case);
                }
                self.indent -= 1;
            }
            StmtKind::Raise { exc, cause } => {
                self.fill("raise");
                if let Some(exc) = exc {
                    self.write(" ");
                    self.expr(exc, TEST);
                    if let Some(cause) = cause {
                        self.write(" from ");
                        self.expr(cause, TEST);
                    }
                }
            }
            StmtKind::Try {
                body,
                handlers,
                orelse,
                finalbody,
                star,
            } => {
                let previous = self.in_try_star;
                self.in_try_star = *star;
                self.fill("try");
                self.block(body);
                for handler in handlers {
                    self.except_handler(handler);
                }
                if !orelse.is_empty() {
                    self.fill("else");
                    self.block(orelse);
                }
                if !finalbody.is_empty() {
                    self.fill("finally");
                    self.block(finalbody);
                }
                self.in_try_star = previous;
            }
            StmtKind::Assert { test, msg } => {
                self.fill("assert ");
                self.expr(test, TEST);
                if let Some(msg) = msg {
                    self.write(", ");
                    self.expr(msg, TEST);
                }
            }
            StmtKind::Import { names } => {
                self.fill("import ");
                self.interleave_aliases(names);
            }
            StmtKind::ImportFrom {
                module,
                names,
                level,
            } => {
                self.fill("from ");
                for _ in 0..*level {
                    self.write(".");
                }
                if let Some(module) = module {
                    self.write(module);
                }
                self.write(" import ");
                self.interleave_aliases(names);
            }
            StmtKind::Global { names } => {
                self.fill("global ");
                self.write(&names.join(", "));
            }
            StmtKind::Nonlocal { names } => {
                self.fill("nonlocal ");
                self.write(&names.join(", "));
            }
            StmtKind::ExprStmt { value } => {
                self.fill("");
                self.expr(value, YIELD);
            }
            StmtKind::Pass => self.fill("pass"),
            StmtKind::Break => self.fill("break"),
            StmtKind::Continue => self.fill("continue"),
            StmtKind::TypeAlias {
                name,
                type_params,
                value,
            } => {
                self.fill("type ");
                self.expr(name, TEST);
                self.type_params(type_params);
                self.write(" = ");
                self.expr(value, TEST);
            }
        }
    }

    fn type_params(&mut self, params: &[TypeParam]) {
        if params.is_empty() {
            return;
        }
        self.write("[");
        let mut first = true;
        for param in params {
            if !first {
                self.write(", ");
            }
            first = false;
            match &param.kind {
                TypeParamKind::TypeVar { name, bound } => {
                    self.write(name);
                    if let Some(bound) = bound {
                        self.write(": ");
                        self.expr(bound, TEST);
                    }
                }
                TypeParamKind::TypeVarTuple { name } => {
                    self.write("*");
                    self.write(name);
                }
                TypeParamKind::ParamSpec { name } => {
                    self.write("**");
                    self.write(name);
                }
            }
        }
        self.write("]");
    }

    fn except_handler(&mut self, handler: &ExceptHandler) {
        self.fill(if self.in_try_star {
            "except*"
        } else {
            "except"
        });
        if let Some(type_) = &handler.type_ {
            self.write(" ");
            self.expr(type_, TEST);
        }
        if let Some(name) = &handler.name {
            self.write(" as ");
            self.write(name);
        }
        self.block(&handler.body);
    }

    fn with_item(&mut self, item: &WithItem) {
        self.expr(&item.context_expr, TEST);
        if let Some(vars) = &item.optional_vars {
            self.write(" as ");
            self.expr(vars, TEST);
        }
    }

    fn match_case(&mut self, case: &MatchCase) {
        self.fill("case ");
        self.pattern(&case.pattern, TEST);
        if let Some(guard) = &case.guard {
            self.write(" if ");
            self.expr(guard, TEST);
        }
        self.block(&case.body);
    }

    fn interleave_exprs(&mut self, separator: &str, exprs: &[Expr]) {
        let mut first = true;
        for expr in exprs {
            if !first {
                self.write(separator);
            }
            first = false;
            self.expr(expr, TEST);
        }
    }

    fn interleave_aliases(&mut self, aliases: &[Alias]) {
        let mut first = true;
        for alias in aliases {
            if !first {
                self.write(", ");
            }
            first = false;
            self.write(&alias.name);
            if let Some(asname) = &alias.asname {
                self.write(" as ");
                self.write(asname);
            }
        }
    }

    fn arguments(&mut self, args: &Arguments) {
        let mut first = true;
        let all: Vec<&Arg> = args.posonlyargs.iter().chain(args.args.iter()).collect();
        let padding = all.len() - args.defaults.len();
        for (index, arg) in all.iter().enumerate() {
            if !first {
                self.write(", ");
            }
            first = false;
            self.arg(arg);
            if index >= padding {
                let default = &args.defaults[index - padding];
                self.write("=");
                self.expr(default, TEST);
            }
            if index + 1 == args.posonlyargs.len() {
                self.write(", /");
            }
        }
        if args.vararg.is_some() || !args.kwonlyargs.is_empty() {
            if !first {
                self.write(", ");
            }
            first = false;
            self.write("*");
            if let Some(vararg) = &args.vararg {
                self.write(&vararg.name);
                if let Some(annotation) = &vararg.annotation {
                    self.write(": ");
                    self.expr(annotation, TEST);
                }
            }
        }
        for (index, arg) in args.kwonlyargs.iter().enumerate() {
            self.write(", ");
            self.arg(arg);
            if let Some(Some(default)) = args.kw_defaults.get(index) {
                self.write("=");
                self.expr(default, TEST);
            }
        }
        if let Some(kwarg) = &args.kwarg {
            if !first {
                self.write(", ");
            }
            self.write("**");
            self.write(&kwarg.name);
            if let Some(annotation) = &kwarg.annotation {
                self.write(": ");
                self.expr(annotation, TEST);
            }
        }
    }

    fn arg(&mut self, arg: &Arg) {
        self.write(&arg.name);
        if let Some(annotation) = &arg.annotation {
            self.write(": ");
            self.expr(annotation, TEST);
        }
    }

    fn keyword(&mut self, keyword: &Keyword) {
        match &keyword.arg {
            None => self.write("**"),
            Some(name) => {
                self.write(name);
                self.write("=");
            }
        }
        self.expr(&keyword.value, TEST);
    }

    fn expr(&mut self, node: &Expr, ctx: u8) {
        match &node.kind {
            ExprKind::Constant(value) => self.constant(value),
            ExprKind::Name(name) => self.write(name),
            ExprKind::Attribute { value, attr } => {
                self.expr(value, ATOM);
                if matches!(
                    value.kind,
                    ExprKind::Constant(ConstVal::Int(_)) | ExprKind::Constant(ConstVal::Bool(_))
                ) {
                    self.write(" ");
                }
                self.write(".");
                self.write(attr);
            }
            ExprKind::Call {
                func,
                args,
                keywords,
            } => {
                self.expr(func, ATOM);
                self.write("(");
                let mut comma = false;
                for arg in args {
                    if comma {
                        self.write(", ");
                    }
                    comma = true;
                    self.expr(arg, TEST);
                }
                for keyword in keywords {
                    if comma {
                        self.write(", ");
                    }
                    comma = true;
                    self.keyword(keyword);
                }
                self.write(")");
            }
            ExprKind::BinOp { left, op, right } => {
                let precedence = binop_precedence(*op);
                let (left_ctx, right_ctx) = if *op == BinOpKind::Pow {
                    (next(precedence), precedence)
                } else {
                    (precedence, next(precedence))
                };
                self.delimit_if_parens(ctx > precedence, |me| {
                    me.expr(left, left_ctx);
                    me.write(" ");
                    me.write(binop_spelling(*op));
                    me.write(" ");
                    me.expr(right, right_ctx);
                });
            }
            ExprKind::UnaryOp { op, operand } => {
                let precedence = match op {
                    UnaryKind::Not => NOT,
                    _ => FACTOR,
                };
                let spelling = match op {
                    UnaryKind::Not => "not",
                    UnaryKind::Invert => "~",
                    UnaryKind::UAdd => "+",
                    UnaryKind::USub => "-",
                };
                self.delimit_if_parens(ctx > precedence, |me| {
                    me.write(spelling);
                    if precedence != FACTOR {
                        me.write(" ");
                    }
                    me.expr(operand, precedence);
                });
            }
            ExprKind::BoolOp { op, values } => {
                let precedence = match op {
                    BoolKind::And => AND,
                    BoolKind::Or => OR,
                };
                let spelling = match op {
                    BoolKind::And => " and ",
                    BoolKind::Or => " or ",
                };
                self.delimit_if_parens(ctx > precedence, |me| {
                    let mut level = precedence;
                    let mut first = true;
                    for value in values {
                        if !first {
                            me.write(spelling);
                        }
                        first = false;
                        level = next(level);
                        me.expr(value, level);
                    }
                });
            }
            ExprKind::Compare {
                left,
                ops,
                comparators,
            } => {
                self.delimit_if_parens(ctx > CMP, |me| {
                    me.expr(left, next(CMP));
                    for (op, comparator) in ops.iter().zip(comparators.iter()) {
                        me.write(" ");
                        me.write(cmp_spelling(*op));
                        me.write(" ");
                        me.expr(comparator, next(CMP));
                    }
                });
            }
            ExprKind::IfExp { test, body, orelse } => {
                self.delimit_if_parens(ctx > TEST, |me| {
                    me.expr(body, next(TEST));
                    me.write(" if ");
                    me.expr(test, next(TEST));
                    me.write(" else ");
                    me.expr(orelse, TEST);
                });
            }
            ExprKind::Subscript { value, slice } => {
                self.expr(value, ATOM);
                self.write("[");
                if let ExprKind::Tuple { elts } = &slice.kind {
                    if !elts.is_empty() {
                        self.items_view(elts);
                    } else {
                        self.expr(slice, TEST);
                    }
                } else {
                    self.expr(slice, TEST);
                }
                self.write("]");
            }
            ExprKind::Slice { lower, upper, step } => {
                if let Some(lower) = lower {
                    self.expr(lower, TEST);
                }
                self.write(":");
                if let Some(upper) = upper {
                    self.expr(upper, TEST);
                }
                if let Some(step) = step {
                    self.write(":");
                    self.expr(step, TEST);
                }
            }
            ExprKind::List { elts } => {
                self.write("[");
                self.interleave_exprs(", ", elts);
                self.write("]");
            }
            ExprKind::Tuple { elts } => {
                let parens = elts.is_empty() || ctx > TUPLE;
                self.delimit_if_parens(parens, |me| {
                    me.items_view(elts);
                });
            }
            ExprKind::Set { elts } => {
                if elts.is_empty() {
                    self.write("{*()}");
                } else {
                    self.write("{");
                    self.interleave_exprs(", ", elts);
                    self.write("}");
                }
            }
            ExprKind::Dict { keys, values } => {
                self.write("{");
                let mut first = true;
                for (key, value) in keys.iter().zip(values.iter()) {
                    if !first {
                        self.write(", ");
                    }
                    first = false;
                    match key {
                        None => {
                            self.write("**");
                            self.expr(value, EXPR);
                        }
                        Some(key) => {
                            self.expr(key, TEST);
                            self.write(": ");
                            self.expr(value, TEST);
                        }
                    }
                }
                self.write("}");
            }
            ExprKind::Starred { value } => {
                self.write("*");
                self.expr(value, EXPR);
            }
            ExprKind::Await { value } => {
                self.delimit_if_parens(ctx > AWAIT, |me| {
                    me.write("await ");
                    me.expr(value, ATOM);
                });
            }
            ExprKind::Yield { value } => {
                self.delimit_if_parens(ctx > YIELD, |me| {
                    me.write("yield");
                    if let Some(value) = value {
                        me.write(" ");
                        me.expr(value, ATOM);
                    }
                });
            }
            ExprKind::YieldFrom { value } => {
                self.delimit_if_parens(ctx > YIELD, |me| {
                    me.write("yield from ");
                    me.expr(value, ATOM);
                });
            }
            ExprKind::Lambda { args, body } => {
                self.delimit_if_parens(ctx > TEST, |me| {
                    me.write("lambda");
                    let rendered = {
                        let mut inner = Unparser::new();
                        inner.arguments(args);
                        inner.out
                    };
                    if !rendered.is_empty() {
                        me.write(" ");
                        me.write(&rendered);
                    }
                    me.write(": ");
                    me.expr(body, TEST);
                });
            }
            ExprKind::ListComp { elt, generators } => {
                self.write("[");
                self.expr(elt, TEST);
                for generator in generators {
                    self.comprehension(generator);
                }
                self.write("]");
            }
            ExprKind::SetComp { elt, generators } => {
                self.write("{");
                self.expr(elt, TEST);
                for generator in generators {
                    self.comprehension(generator);
                }
                self.write("}");
            }
            ExprKind::GeneratorExp { elt, generators } => {
                self.write("(");
                self.expr(elt, TEST);
                for generator in generators {
                    self.comprehension(generator);
                }
                self.write(")");
            }
            ExprKind::DictComp {
                key,
                value,
                generators,
            } => {
                self.write("{");
                self.expr(key, TEST);
                self.write(": ");
                self.expr(value, TEST);
                for generator in generators {
                    self.comprehension(generator);
                }
                self.write("}");
            }
            ExprKind::JoinedStr { values } => self.joined_str(values),
            ExprKind::FormattedValue {
                value,
                conversion,
                format_spec,
            } => {
                self.formatted_value(value, *conversion, format_spec.as_deref());
            }
            ExprKind::NamedExpr { target, value } => {
                self.delimit_if_parens(ctx > NAMED_EXPR, |me| {
                    me.expr(target, ATOM);
                    me.write(" := ");
                    me.expr(value, ATOM);
                });
            }
        }
    }

    fn comprehension(&mut self, node: &Comprehension) {
        if node.is_async {
            self.write(" async for ");
        } else {
            self.write(" for ");
        }
        self.expr(&node.target, TUPLE);
        self.write(" in ");
        self.expr(&node.iter, next(TEST));
        for condition in &node.ifs {
            self.write(" if ");
            self.expr(condition, next(TEST));
        }
    }

    /// The items view of the interpreter, one item carrying
    /// its trailing comma.
    fn items_view(&mut self, elts: &[Expr]) {
        if elts.len() == 1 {
            self.expr(&elts[0], TEST);
            self.write(",");
        } else {
            self.interleave_exprs(", ", elts);
        }
    }

    fn delimit_if_parens(&mut self, parens: bool, f: impl FnOnce(&mut Unparser)) {
        if parens {
            self.write("(");
        }
        f(self);
        if parens {
            self.write(")");
        }
    }

    fn pattern(&mut self, node: &Pattern, ctx: u8) {
        match &node.kind {
            PatternKind::MatchValue(value) => self.expr(value, TEST),
            PatternKind::MatchSingleton(value) => self.constant(value),
            PatternKind::MatchSequence(patterns) => {
                self.write("[");
                let mut first = true;
                for pattern in patterns {
                    if !first {
                        self.write(", ");
                    }
                    first = false;
                    self.pattern(pattern, TEST);
                }
                self.write("]");
            }
            PatternKind::MatchStar(name) => {
                self.write("*");
                self.write(name.as_deref().unwrap_or("_"));
            }
            PatternKind::MatchMapping {
                keys,
                patterns,
                rest,
            } => {
                self.write("{");
                let mut first = true;
                for (key, pattern) in keys.iter().zip(patterns.iter()) {
                    if !first {
                        self.write(", ");
                    }
                    first = false;
                    self.expr(key, TEST);
                    self.write(": ");
                    self.pattern(pattern, TEST);
                }
                if let Some(rest) = rest {
                    if !keys.is_empty() {
                        self.write(", ");
                    }
                    self.write("**");
                    self.write(rest);
                }
                self.write("}");
            }
            PatternKind::MatchClass {
                cls,
                patterns,
                kwd_attrs,
                kwd_patterns,
            } => {
                self.expr(cls, ATOM);
                self.write("(");
                let mut comma = false;
                for pattern in patterns {
                    if comma {
                        self.write(", ");
                    }
                    comma = true;
                    self.pattern(pattern, TEST);
                }
                for (attr, pattern) in kwd_attrs.iter().zip(kwd_patterns.iter()) {
                    if comma {
                        self.write(", ");
                    }
                    comma = true;
                    self.write(attr);
                    self.write("=");
                    self.pattern(pattern, TEST);
                }
                self.write(")");
            }
            PatternKind::MatchAs { pattern, name } => match (pattern, name) {
                (_, None) => self.write("_"),
                (None, Some(name)) => self.write(name),
                (Some(inner), Some(name)) => {
                    self.delimit_if_parens(ctx > TEST, |me| {
                        me.pattern(inner, BOR);
                        me.write(" as ");
                        me.write(name);
                    });
                }
            },
            PatternKind::MatchOr(alts) => {
                self.delimit_if_parens(ctx > BOR, |me| {
                    let mut first = true;
                    for alt in alts {
                        if !first {
                            me.write(" | ");
                        }
                        first = false;
                        me.pattern(alt, next(BOR));
                    }
                });
            }
        }
    }

    fn constant(&mut self, value: &ConstVal) {
        match value {
            ConstVal::NoneVal => self.write("None"),
            ConstVal::Bool(flag) => self.write(if *flag { "True" } else { "False" }),
            ConstVal::Int(text) => self.write(text),
            ConstVal::Float(value) => {
                let spelled = float_repr(*value)
                    .replace("inf", INFSTR)
                    .replace("nan", &format!("({}-{})", INFSTR, INFSTR));
                self.write(&spelled);
            }
            ConstVal::Complex(real, imag) => {
                let spelled = complex_repr(*real, *imag)
                    .replace("inf", INFSTR)
                    .replace("nan", &format!("({}-{})", INFSTR, INFSTR));
                self.write(&spelled);
            }
            ConstVal::Str(text) => {
                let spelled = str_repr(text.codepoints());
                self.write(&spelled);
            }
            ConstVal::Bytes(bytes) => {
                let spelled = bytes_repr(bytes);
                self.write(&spelled);
            }
            ConstVal::Ellipsis => self.write("..."),
        }
    }

    fn joined_str(&mut self, values: &[Expr]) {
        self.write("f");
        let mut parts: Vec<(Vec<u32>, bool)> = Vec::new();
        for value in values {
            let cps = Self::fstring_inner_cps(value);
            let is_constant = matches!(value.kind, ExprKind::Constant(ConstVal::Str(_)));
            parts.push((cps, is_constant));
        }
        let mut quote_types: Vec<String> = ALL_QUOTES.iter().map(|q| q.to_string()).collect();
        let mut fallback = false;
        let mut new_parts: Vec<String> = Vec::new();
        for (text, is_constant) in &parts {
            if *is_constant {
                let (escaped, new_quotes) =
                    str_literal_helper(text, &quote_refs(&quote_types), true);
                if new_quotes.iter().all(|quote| !quote_types.contains(quote)) {
                    fallback = true;
                    break;
                }
                quote_types = new_quotes;
                new_parts.push(escaped);
            } else {
                if text.contains(&0x0a) {
                    quote_types.retain(|quote| MULTI_QUOTES.contains(&quote.as_str()));
                }
                new_parts.push(cps_to_string(text));
            }
        }
        if fallback {
            quote_types = vec!["'''".to_string()];
            new_parts.clear();
            for (text, is_constant) in &parts {
                if *is_constant {
                    let mut prefixed: Vec<u32> = vec![0x22];
                    prefixed.extend_from_slice(text);
                    let spelled = str_repr(&prefixed);
                    let inner = &spelled[2..spelled.len() - 1];
                    new_parts.push(inner.to_string());
                } else {
                    new_parts.push(cps_to_string(text));
                }
            }
        }
        let quote = quote_types[0].clone();
        self.write(&quote);
        for part in &new_parts {
            self.write(part);
        }
        self.write(&quote);
    }

    /// The codepoints one part of a joined string carries
    /// before the quote reading spells it, the inner reading
    /// of the interpreter's unparser. A constant part stands
    /// raw here, lone surrogates among its codepoints, and
    /// the spelling layers above this one escape them.
    fn fstring_inner_cps(node: &Expr) -> Vec<u32> {
        match &node.kind {
            ExprKind::JoinedStr { values } => {
                let mut out: Vec<u32> = Vec::new();
                for value in values {
                    out.extend(Self::fstring_inner_cps(value));
                }
                out
            }
            ExprKind::Constant(ConstVal::Str(text)) => literal_part_cps(text),
            ExprKind::FormattedValue {
                value,
                conversion,
                format_spec,
            } => {
                let mut inner = Unparser::new();
                inner.formatted_value(value, *conversion, format_spec.as_deref());
                inner.out.chars().map(|ch| ch as u32).collect()
            }
            _ => {
                // The parser builds only the shapes above
                // inside a joined string.
                let mut inner = Unparser::new();
                inner.expr(node, TEST);
                inner.out.chars().map(|ch| ch as u32).collect()
            }
        }
    }

    fn formatted_value(&mut self, value: &Expr, conversion: i32, format_spec: Option<&Expr>) {
        self.write("{");
        let rendered = {
            let mut inner = Unparser::new();
            inner.expr(value, next(TEST));
            inner.out
        };
        if rendered.starts_with('{') {
            self.write(" ");
        }
        self.write(&rendered);
        if conversion != -1 {
            self.write("!");
            if let Some(ch) = char::from_u32(conversion as u32) {
                let mut buffer = [0u8; 4];
                self.write(ch.encode_utf8(&mut buffer));
            }
        }
        if let Some(spec) = format_spec {
            self.write(":");
            self.write_spec(spec);
        }
        self.write("}");
    }

    /// Write one format spec, part by part, the reading the
    /// interpreter's unparser makes. A constant part takes
    /// the spec spelling of its carried value. A nested
    /// field stands as its own rendering, written whole and
    /// never spelled again by the surface around it.
    fn write_spec(&mut self, spec: &Expr) {
        match &spec.kind {
            ExprKind::JoinedStr { values } => {
                for value in values {
                    self.write_spec_part(value);
                }
            }
            _ => {
                let cps = Self::fstring_inner_cps(spec);
                self.write(&spec_spelling(&cps));
            }
        }
    }

    fn write_spec_part(&mut self, node: &Expr) {
        match &node.kind {
            ExprKind::Constant(ConstVal::Str(text)) => {
                let cps = literal_part_cps(text);
                self.write(&spec_spelling(&cps));
            }
            ExprKind::FormattedValue {
                value,
                conversion,
                format_spec,
            } => {
                let mut inner = Unparser::new();
                inner.formatted_value(value, *conversion, format_spec.as_deref());
                self.write(&inner.out);
            }
            ExprKind::JoinedStr { values } => {
                for value in values {
                    self.write_spec_part(value);
                }
            }
            _ => {
                let mut inner = Unparser::new();
                inner.expr(node, TEST);
                self.write(&inner.out);
            }
        }
    }
}

/// The codepoints one literal run of a joined string
/// carries before the spelling layer above it writes
/// them. Only the braces change here, doubling wherever
/// they stand. Every other codepoint stands as its
/// carried value, a lone surrogate and a backslash among
/// them, and the surface doing the writing escapes what
/// its own surface requires.
fn literal_part_cps(text: &PyText) -> Vec<u32> {
    let mut out: Vec<u32> = Vec::new();
    for point in text.codepoints() {
        match point {
            0x7b => {
                out.push(0x7b);
                out.push(0x7b);
            }
            0x7d => {
                out.push(0x7d);
                out.push(0x7d);
            }
            _ => out.push(*point),
        }
    }
    out
}

/// Write the codepoints of one format spec constant into
/// its spelling, the reading the contract amendment on
/// the carrying of string constants states for this
/// surface. A backslash in the carried text stands
/// quadrupled, the interpreter's doubling of a literal
/// backslash and one doubling more. A lone surrogate
/// stands as two backslashes and its escape text, because
/// the interpreter leaves the raw character standing here
/// and a Rust string cannot hold it. The two quotes and
/// the newline take the escaped forms the interpreter's
/// unparser writes for them, a single backslash before
/// each. Every other codepoint stands as itself, and a
/// codepoint no Rust char holds takes its unicode escape.
fn spec_spelling(cps: &[u32]) -> String {
    let mut out = String::new();
    for point in cps {
        match *point {
            0x5c => {
                out.push(0x5c as char);
                out.push(0x5c as char);
                out.push(0x5c as char);
                out.push(0x5c as char);
            }
            0xd800..=0xdfff => {
                out.push(0x5c as char);
                out.push(0x5c as char);
                out.push('u');
                out.push_str(&format!("{:04x}", point));
            }
            0x27 => {
                out.push(0x5c as char);
                out.push(0x27 as char);
            }
            0x22 => {
                out.push(0x5c as char);
                out.push(0x22 as char);
            }
            0x0a => {
                out.push(0x5c as char);
                out.push(0x6e as char);
            }
            _ => match char::from_u32(*point) {
                Some(ch) => out.push(ch),
                None => out.push_str(&unicode_escape_cp(*point)),
            },
        }
    }
    out
}

fn cps_to_string(cps: &[u32]) -> String {
    let mut out = String::new();
    for point in cps {
        match char::from_u32(*point) {
            Some(ch) => out.push(ch),
            None => out.push_str(&unicode_escape_cp(*point)),
        }
    }
    out
}

fn quote_refs(quotes: &[String]) -> Vec<&str> {
    quotes.iter().map(|quote| quote.as_str()).collect()
}

fn binop_spelling(op: BinOpKind) -> &'static str {
    match op {
        BinOpKind::Add => "+",
        BinOpKind::Sub => "-",
        BinOpKind::Mult => "*",
        BinOpKind::MatMult => "@",
        BinOpKind::Div => "/",
        BinOpKind::Mod => "%",
        BinOpKind::Pow => "**",
        BinOpKind::LShift => "<<",
        BinOpKind::RShift => ">>",
        BinOpKind::BitOr => "|",
        BinOpKind::BitXor => "^",
        BinOpKind::BitAnd => "&",
        BinOpKind::FloorDiv => "//",
    }
}

fn binop_precedence(op: BinOpKind) -> u8 {
    match op {
        BinOpKind::Add | BinOpKind::Sub => ARITH,
        BinOpKind::Mult
        | BinOpKind::MatMult
        | BinOpKind::Div
        | BinOpKind::Mod
        | BinOpKind::FloorDiv => TERM,
        BinOpKind::Pow => POWER,
        BinOpKind::LShift | BinOpKind::RShift => SHIFT,
        BinOpKind::BitOr => BOR,
        BinOpKind::BitXor => BXOR,
        BinOpKind::BitAnd => BAND,
    }
}

fn cmp_spelling(op: CmpKind) -> &'static str {
    match op {
        CmpKind::Eq => "==",
        CmpKind::NotEq => "!=",
        CmpKind::Lt => "<",
        CmpKind::LtE => "<=",
        CmpKind::Gt => ">",
        CmpKind::GtE => ">=",
        CmpKind::Is => "is",
        CmpKind::IsNot => "is not",
        CmpKind::In => "in",
        CmpKind::NotIn => "not in",
    }
}

/// The escape a codepoint takes under the interpreter's
/// unicode escape spelling. A lone surrogate spells here,
/// in the four digit form of its own range.
/// The string literal helper of the interpreter's unparser,
/// choosing quotes and escapes so backslashes stay few.
/// Returns the escaped body and the quote types still open.
fn str_literal_helper(
    string: &[u32],
    quote_types: &[&str],
    escape_special_whitespace: bool,
) -> (String, Vec<String>) {
    let escaped: String = string
        .iter()
        .map(|point| {
            if !escape_special_whitespace && (*point == 0x0a || *point == 0x09) {
                char::from_u32(*point).unwrap().to_string()
            } else if *point == 0x5c || !is_printable_cp(*point) {
                unicode_escape_cp(*point)
            } else {
                char::from_u32(*point).unwrap().to_string()
            }
        })
        .collect();
    let mut possible: Vec<String> = quote_types.iter().map(|q| q.to_string()).collect();
    if escaped.contains('\n') {
        possible.retain(|quote| MULTI_QUOTES.contains(&quote.as_str()));
    }
    possible.retain(|quote| !escaped.contains(quote.as_str()));
    if possible.is_empty() {
        let spelled = str_repr(string);
        let first_char = spelled.chars().next().unwrap_or('\'');
        let quote = quote_types
            .iter()
            .find(|quote| quote.contains(first_char))
            .map(|quote| quote.to_string())
            .unwrap_or_else(|| first_char.to_string());
        let inner = &spelled[1..spelled.len() - 1];
        return (inner.to_string(), vec![quote]);
    }
    if !escaped.is_empty() {
        let last = escaped.chars().last().unwrap();
        // A stable sort carrying the quotes whose first
        // character is not the string's last character first.
        possible.sort_by_key(|quote| quote.starts_with(last));
        if possible[0].starts_with(last) {
            debug_assert!(possible[0].len() == 3);
            let mut chars: Vec<char> = escaped.chars().collect();
            let tail = chars.pop().unwrap();
            let mut rebuilt: String = chars.into_iter().collect();
            rebuilt.push('\\');
            rebuilt.push(tail);
            return (rebuilt, possible);
        }
    }
    (escaped, possible)
}

/// Spell a bytes constant the way the interpreter's repr
/// spells it.
pub fn bytes_repr(value: &[u8]) -> String {
    let quote = if value.contains(&b'\'') && !value.contains(&b'"') {
        '"'
    } else {
        '\''
    };
    let mut out = String::new();
    out.push('b');
    out.push(quote);
    for byte in value {
        let ch = *byte as char;
        if ch == quote || ch == '\\' {
            out.push('\\');
            out.push(ch);
        } else if *byte == b'\n' {
            out.push_str("\\n");
        } else if *byte == b'\r' {
            out.push_str("\\r");
        } else if *byte == b'\t' {
            out.push_str("\\t");
        } else if (0x20..0x7f).contains(byte) {
            out.push(ch);
        } else {
            out.push_str(&format!("\\x{:02x}", byte));
        }
    }
    out.push(quote);
    out
}
