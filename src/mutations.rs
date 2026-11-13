//! The mutant generation of PORT8, the port of
//! ostensible/mutations.py whole. The entry takes a decoded
//! source text and returns the mutant list the landed Python
//! returns for that text, in the breadth first site order of
//! the panel, or the refusal of PORT8-5 where the Python
//! raises uncaught. The panel has four kinds: return_none,
//! compare_invert, bool_negate, and constant_shift, the last
//! earning two mutants per site, plus one first and minus
//! one second. Every mutant is applied to a clone of the
//! parsed tree, relocated by node id through the registry
//! the walk builds, standing in for the re-parse the Python
//! makes per mutant; the behaviour is the contract's, the
//! mechanism the builder's, as PORT8-3 states.

use crate::model::Mutant;
use crate::pyast::*;
use crate::pyparse::{parse_module, ParseError};
use crate::pyunparse::{unparse_expr, unparse_module};
use crate::pywalk::{walk, NRef};

/// A refusal of the generation port, the PORT8-5 family.
/// The wording is the port's own and stands in for the
/// interpreter failure the Python lets propagate.
#[derive(Debug, Clone)]
pub struct GenerateRefusal {
    pub message: String,
}

fn refusal(message: &str) -> GenerateRefusal {
    GenerateRefusal {
        message: message.to_string(),
    }
}

impl From<ParseError> for GenerateRefusal {
    fn from(_error: ParseError) -> GenerateRefusal {
        refusal("the source does not parse under the port's reading")
    }
}

/// A refusal under the recursion member of the PORT8-5
/// family, in the port's own wording.
fn recursion_refusal() -> GenerateRefusal {
    refusal("the source nests past the recursion standing of the port")
}

/// The crate's own standings for the generate path's
/// unparses, measured against the landed Python at the
/// default recursion limit and stated in the closing entry
/// of PORT8-9. A detail unparse in site collection stands
/// to a path of 326 nodes and refuses at 327, the standing
/// the interpreter's own collection deaths measure for the
/// return and comparison shapes. A whole module unparse in
/// the mutant loop stands to a module path of 329 nodes
/// and refuses at 330. The interpreter's boundary wobbles
/// by a term either way with the caller's stack and the
/// shape's statement nesting; these are the port's figures,
/// stated as PORT8-5 directs, not the interpreter's copied.
const DETAIL_UNPARSE_STANDING: usize = 327;
const MODULE_UNPARSE_STANDING: usize = 330;

/// The number of nodes on the deepest root to leaf path of
/// a subtree, counted iteratively, on the pattern of the
/// scan's own measure.
fn path_nodes(root: NRef) -> usize {
    let mut deepest = 0usize;
    let mut stack: Vec<(NRef, usize)> = vec![(root, 1)];
    while let Some((node, depth)) = stack.pop() {
        if depth > deepest {
            deepest = depth;
        }
        for child in node.children() {
            stack.push((child, depth + 1));
        }
    }
    deepest
}

/// Refuse when the unparse reading of one expression in
/// site collection would pass the detail standing.
fn detail_guard(expr: &Expr) -> Result<(), GenerateRefusal> {
    if path_nodes(NRef::Expr(expr)) >= DETAIL_UNPARSE_STANDING {
        Err(recursion_refusal())
    } else {
        Ok(())
    }
}

/// The ten directed inversions of the panel, as a reading
/// over the operator kinds. Every comparison operator type
/// the grammar has stands in the table, so every Compare
/// node earns a site at its first operator.
fn invert(op: CmpKind) -> CmpKind {
    match op {
        CmpKind::Eq => CmpKind::NotEq,
        CmpKind::NotEq => CmpKind::Eq,
        CmpKind::Lt => CmpKind::GtE,
        CmpKind::GtE => CmpKind::Lt,
        CmpKind::Gt => CmpKind::LtE,
        CmpKind::LtE => CmpKind::Gt,
        CmpKind::In => CmpKind::NotIn,
        CmpKind::NotIn => CmpKind::In,
        CmpKind::Is => CmpKind::IsNot,
        CmpKind::IsNot => CmpKind::Is,
    }
}

/// Increment the magnitude of a canonical decimal spelling.
fn inc_magnitude(digits: &str) -> String {
    let mut bytes = digits.as_bytes().to_vec();
    let mut index = bytes.len();
    loop {
        if index == 0 {
            bytes.insert(0, b'1');
            break;
        }
        index -= 1;
        if bytes[index] == b'9' {
            bytes[index] = b'0';
        } else {
            bytes[index] += 1;
            break;
        }
    }
    String::from_utf8(bytes).expect("digits stay digits")
}

/// Decrement the magnitude of a canonical decimal spelling
/// that stands at one or more.
fn dec_magnitude(digits: &str) -> String {
    let mut bytes = digits.as_bytes().to_vec();
    let mut index = bytes.len();
    loop {
        index -= 1;
        if bytes[index] == b'0' {
            bytes[index] = b'9';
        } else {
            bytes[index] -= 1;
            break;
        }
    }
    let spelled = String::from_utf8(bytes).expect("digits stay digits");
    let trimmed = spelled.trim_start_matches('0');
    if trimmed.is_empty() {
        "0".to_string()
    } else {
        trimmed.to_string()
    }
}

/// Shift a canonical decimal spelling by one in the given
/// direction, exactly, at any size, with no float round
/// trip. The parser's integer spellings carry no sign,
/// because a sign stands in the tree as a unary operator;
/// the signed arms stand for completeness of the reading.
fn shift_decimal_text(text: &str, direction: i32) -> String {
    if let Some(magnitude) = text.strip_prefix('-') {
        if direction > 0 {
            let moved = dec_magnitude(magnitude);
            if moved == "0" {
                moved
            } else {
                format!("-{}", moved)
            }
        } else {
            format!("-{}", inc_magnitude(magnitude))
        }
    } else if direction > 0 {
        inc_magnitude(text)
    } else if text == "0" {
        "-1".to_string()
    } else {
        dec_magnitude(text)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SiteKind {
    ReturnNone,
    CompareInvert,
    BoolNegate,
    ConstantShift,
}

/// One collected site: the panel kind, the id of the node
/// it acts on, the line the mutant records, and the detail
/// spelled from the original tree.
struct Site {
    kind: SiteKind,
    node_id: usize,
    line: i64,
    detail: String,
}

/// Collect the panel's sites over the breadth first walk,
/// per node the first match winning in the order Return,
/// Compare, Constant, as the landed Python collects them.
fn collect_sites(module: &Module) -> Result<Vec<Site>, GenerateRefusal> {
    let mut sites = Vec::new();
    for node in walk(module) {
        match node {
            NRef::Stmt(stmt) => {
                if let StmtKind::Return { value: Some(value) } = &stmt.kind {
                    let is_none_constant =
                        matches!(&value.kind, ExprKind::Constant(ConstVal::NoneVal));
                    if !is_none_constant {
                        detail_guard(value)?;
                        sites.push(Site {
                            kind: SiteKind::ReturnNone,
                            node_id: stmt.id,
                            line: stmt.pos.line as i64,
                            detail: format!("return value {}", unparse_expr(value)),
                        });
                    }
                }
            }
            NRef::Expr(expr) => match &expr.kind {
                ExprKind::Compare { .. } => {
                    detail_guard(expr)?;
                    sites.push(Site {
                        kind: SiteKind::CompareInvert,
                        node_id: expr.id,
                        line: expr.pos.line as i64,
                        detail: format!("comparison {} operator 0", unparse_expr(expr)),
                    });
                }
                ExprKind::Constant(ConstVal::Bool(flag)) => {
                    sites.push(Site {
                        kind: SiteKind::BoolNegate,
                        node_id: expr.id,
                        line: expr.pos.line as i64,
                        detail: format!(
                            "boolean constant {}",
                            if *flag { "True" } else { "False" }
                        ),
                    });
                }
                ExprKind::Constant(ConstVal::Int(text)) => {
                    sites.push(Site {
                        kind: SiteKind::ConstantShift,
                        node_id: expr.id,
                        line: expr.pos.line as i64,
                        detail: format!("numeric constant {}", text),
                    });
                }
                ExprKind::Constant(ConstVal::Float(value)) => {
                    sites.push(Site {
                        kind: SiteKind::ConstantShift,
                        node_id: expr.id,
                        line: expr.pos.line as i64,
                        detail: format!("numeric constant {}", float_repr(*value)),
                    });
                }
                _ => {}
            },
            _ => {}
        }
    }
    Ok(sites)
}

/// A mutable handle to one node of a cloned tree, found by
/// node id, standing in for the fresh site list the Python
/// finds the same node again through.
enum FoundMut<'a> {
    Stmt(&'a mut Stmt),
    Expr(&'a mut Expr),
}

fn find_in_stmts(stmts: &mut [Stmt], id: usize) -> Option<FoundMut<'_>> {
    for stmt in stmts {
        if let found @ Some(_) = find_in_stmt(stmt, id) {
            return found;
        }
    }
    None
}

fn find_in_exprs(exprs: &mut [Expr], id: usize) -> Option<FoundMut<'_>> {
    for expr in exprs {
        if let found @ Some(_) = find_in_expr(expr, id) {
            return found;
        }
    }
    None
}

fn find_in_args(args: &mut Arguments, id: usize) -> Option<FoundMut<'_>> {
    for arg in &mut args.posonlyargs {
        if let found @ Some(_) = find_in_arg(arg, id) {
            return found;
        }
    }
    for arg in &mut args.args {
        if let found @ Some(_) = find_in_arg(arg, id) {
            return found;
        }
    }
    if let Some(arg) = &mut args.vararg {
        if let found @ Some(_) = find_in_arg(arg, id) {
            return found;
        }
    }
    for arg in &mut args.kwonlyargs {
        if let found @ Some(_) = find_in_arg(arg, id) {
            return found;
        }
    }
    for expr in args.kw_defaults.iter_mut().flatten() {
        if let found @ Some(_) = find_in_expr(expr, id) {
            return found;
        }
    }
    if let Some(arg) = &mut args.kwarg {
        if let found @ Some(_) = find_in_arg(arg, id) {
            return found;
        }
    }
    find_in_exprs(&mut args.defaults, id)
}

fn find_in_arg(arg: &mut Arg, id: usize) -> Option<FoundMut<'_>> {
    if let Some(annotation) = &mut arg.annotation {
        return find_in_expr(annotation, id);
    }
    None
}

fn find_in_keyword(keyword: &mut Keyword, id: usize) -> Option<FoundMut<'_>> {
    find_in_expr(&mut keyword.value, id)
}

fn find_in_comp(comp: &mut Comprehension, id: usize) -> Option<FoundMut<'_>> {
    if let found @ Some(_) = find_in_expr(&mut comp.target, id) {
        return found;
    }
    if let found @ Some(_) = find_in_expr(&mut comp.iter, id) {
        return found;
    }
    find_in_exprs(&mut comp.ifs, id)
}

fn find_in_handler(handler: &mut ExceptHandler, id: usize) -> Option<FoundMut<'_>> {
    if let Some(type_) = &mut handler.type_ {
        if let found @ Some(_) = find_in_expr(type_, id) {
            return found;
        }
    }
    find_in_stmts(&mut handler.body, id)
}

fn find_in_case(case: &mut MatchCase, id: usize) -> Option<FoundMut<'_>> {
    if let found @ Some(_) = find_in_pattern(&mut case.pattern, id) {
        return found;
    }
    if let Some(guard) = &mut case.guard {
        if let found @ Some(_) = find_in_expr(guard, id) {
            return found;
        }
    }
    find_in_stmts(&mut case.body, id)
}

fn find_in_pattern(pattern: &mut Pattern, id: usize) -> Option<FoundMut<'_>> {
    match &mut pattern.kind {
        PatternKind::MatchValue(expr) => find_in_expr(expr, id),
        PatternKind::MatchSingleton(_) | PatternKind::MatchStar(_) => None,
        PatternKind::MatchSequence(patterns) => {
            for pattern in patterns {
                if let found @ Some(_) = find_in_pattern(pattern, id) {
                    return found;
                }
            }
            None
        }
        PatternKind::MatchMapping { keys, patterns, .. } => {
            if let found @ Some(_) = find_in_exprs(keys, id) {
                return found;
            }
            for pattern in patterns {
                if let found @ Some(_) = find_in_pattern(pattern, id) {
                    return found;
                }
            }
            None
        }
        PatternKind::MatchClass {
            cls,
            patterns,
            kwd_patterns,
            ..
        } => {
            if let found @ Some(_) = find_in_expr(cls, id) {
                return found;
            }
            for pattern in patterns.iter_mut().chain(kwd_patterns.iter_mut()) {
                if let found @ Some(_) = find_in_pattern(pattern, id) {
                    return found;
                }
            }
            None
        }
        PatternKind::MatchAs { pattern, .. } => match pattern {
            Some(pattern) => find_in_pattern(pattern, id),
            None => None,
        },
        PatternKind::MatchOr(patterns) => {
            for pattern in patterns {
                if let found @ Some(_) = find_in_pattern(pattern, id) {
                    return found;
                }
            }
            None
        }
    }
}

fn find_in_type_param(param: &mut TypeParam, id: usize) -> Option<FoundMut<'_>> {
    if let TypeParamKind::TypeVar {
        bound: Some(bound), ..
    } = &mut param.kind
    {
        return find_in_expr(bound, id);
    }
    None
}

fn find_in_with_item(item: &mut WithItem, id: usize) -> Option<FoundMut<'_>> {
    if let found @ Some(_) = find_in_expr(&mut item.context_expr, id) {
        return found;
    }
    if let Some(vars) = &mut item.optional_vars {
        return find_in_expr(vars, id);
    }
    None
}

fn find_in_stmt(stmt: &mut Stmt, id: usize) -> Option<FoundMut<'_>> {
    if stmt.id == id {
        return Some(FoundMut::Stmt(stmt));
    }
    match &mut stmt.kind {
        StmtKind::FuncDef {
            args,
            body,
            decorators,
            returns,
            type_params,
            ..
        } => {
            if let found @ Some(_) = find_in_args(args, id) {
                return found;
            }
            if let found @ Some(_) = find_in_stmts(body, id) {
                return found;
            }
            if let found @ Some(_) = find_in_exprs(decorators, id) {
                return found;
            }
            if let Some(returns) = returns {
                if let found @ Some(_) = find_in_expr(returns, id) {
                    return found;
                }
            }
            for param in type_params {
                if let found @ Some(_) = find_in_type_param(param, id) {
                    return found;
                }
            }
            None
        }
        StmtKind::ClassDef {
            bases,
            keywords,
            body,
            decorators,
            type_params,
            ..
        } => {
            if let found @ Some(_) = find_in_exprs(bases, id) {
                return found;
            }
            for keyword in keywords {
                if let found @ Some(_) = find_in_keyword(keyword, id) {
                    return found;
                }
            }
            if let found @ Some(_) = find_in_stmts(body, id) {
                return found;
            }
            if let found @ Some(_) = find_in_exprs(decorators, id) {
                return found;
            }
            for param in type_params {
                if let found @ Some(_) = find_in_type_param(param, id) {
                    return found;
                }
            }
            None
        }
        StmtKind::Return { value } => match value {
            Some(value) => find_in_expr(value, id),
            None => None,
        },
        StmtKind::Delete { targets } => find_in_exprs(targets, id),
        StmtKind::Assign { targets, value } => {
            if let found @ Some(_) = find_in_exprs(targets, id) {
                return found;
            }
            find_in_expr(value, id)
        }
        StmtKind::AugAssign { target, value, .. } => {
            if let found @ Some(_) = find_in_expr(target, id) {
                return found;
            }
            find_in_expr(value, id)
        }
        StmtKind::AnnAssign {
            target,
            annotation,
            value,
            ..
        } => {
            if let found @ Some(_) = find_in_expr(target, id) {
                return found;
            }
            if let found @ Some(_) = find_in_expr(annotation, id) {
                return found;
            }
            match value {
                Some(value) => find_in_expr(value, id),
                None => None,
            }
        }
        StmtKind::For {
            target,
            iter,
            body,
            orelse,
            ..
        } => {
            if let found @ Some(_) = find_in_expr(target, id) {
                return found;
            }
            if let found @ Some(_) = find_in_expr(iter, id) {
                return found;
            }
            if let found @ Some(_) = find_in_stmts(body, id) {
                return found;
            }
            find_in_stmts(orelse, id)
        }
        StmtKind::While { test, body, orelse } => {
            if let found @ Some(_) = find_in_expr(test, id) {
                return found;
            }
            if let found @ Some(_) = find_in_stmts(body, id) {
                return found;
            }
            find_in_stmts(orelse, id)
        }
        StmtKind::If { test, body, orelse } => {
            if let found @ Some(_) = find_in_expr(test, id) {
                return found;
            }
            if let found @ Some(_) = find_in_stmts(body, id) {
                return found;
            }
            find_in_stmts(orelse, id)
        }
        StmtKind::With { items, body, .. } => {
            for item in items {
                if let found @ Some(_) = find_in_with_item(item, id) {
                    return found;
                }
            }
            find_in_stmts(body, id)
        }
        StmtKind::Match { subject, cases } => {
            if let found @ Some(_) = find_in_expr(subject, id) {
                return found;
            }
            for case in cases {
                if let found @ Some(_) = find_in_case(case, id) {
                    return found;
                }
            }
            None
        }
        StmtKind::Raise { exc, cause } => {
            if let Some(exc) = exc {
                if let found @ Some(_) = find_in_expr(exc, id) {
                    return found;
                }
            }
            match cause {
                Some(cause) => find_in_expr(cause, id),
                None => None,
            }
        }
        StmtKind::Try {
            body,
            handlers,
            orelse,
            finalbody,
            ..
        } => {
            if let found @ Some(_) = find_in_stmts(body, id) {
                return found;
            }
            for handler in handlers {
                if let found @ Some(_) = find_in_handler(handler, id) {
                    return found;
                }
            }
            if let found @ Some(_) = find_in_stmts(orelse, id) {
                return found;
            }
            find_in_stmts(finalbody, id)
        }
        StmtKind::Assert { test, msg } => {
            if let found @ Some(_) = find_in_expr(test, id) {
                return found;
            }
            match msg {
                Some(msg) => find_in_expr(msg, id),
                None => None,
            }
        }
        StmtKind::Import { .. }
        | StmtKind::ImportFrom { .. }
        | StmtKind::Global { .. }
        | StmtKind::Nonlocal { .. }
        | StmtKind::Pass
        | StmtKind::Break
        | StmtKind::Continue => None,
        StmtKind::ExprStmt { value } => find_in_expr(value, id),
        StmtKind::TypeAlias {
            name,
            type_params,
            value,
        } => {
            if let found @ Some(_) = find_in_expr(name, id) {
                return found;
            }
            for param in type_params {
                if let found @ Some(_) = find_in_type_param(param, id) {
                    return found;
                }
            }
            find_in_expr(value, id)
        }
    }
}

fn find_in_expr(expr: &mut Expr, id: usize) -> Option<FoundMut<'_>> {
    if expr.id == id {
        return Some(FoundMut::Expr(expr));
    }
    match &mut expr.kind {
        ExprKind::Constant(_) | ExprKind::Name(_) => None,
        ExprKind::Attribute { value, .. } => find_in_expr(value, id),
        ExprKind::Call {
            func,
            args,
            keywords,
        } => {
            if let found @ Some(_) = find_in_expr(func, id) {
                return found;
            }
            if let found @ Some(_) = find_in_exprs(args, id) {
                return found;
            }
            for keyword in keywords {
                if let found @ Some(_) = find_in_keyword(keyword, id) {
                    return found;
                }
            }
            None
        }
        ExprKind::BinOp { left, right, .. } => {
            if let found @ Some(_) = find_in_expr(left, id) {
                return found;
            }
            find_in_expr(right, id)
        }
        ExprKind::UnaryOp { operand, .. } => find_in_expr(operand, id),
        ExprKind::BoolOp { values, .. } => find_in_exprs(values, id),
        ExprKind::Compare {
            left, comparators, ..
        } => {
            if let found @ Some(_) = find_in_expr(left, id) {
                return found;
            }
            find_in_exprs(comparators, id)
        }
        ExprKind::IfExp { test, body, orelse } => {
            if let found @ Some(_) = find_in_expr(test, id) {
                return found;
            }
            if let found @ Some(_) = find_in_expr(body, id) {
                return found;
            }
            find_in_expr(orelse, id)
        }
        ExprKind::Subscript { value, slice } => {
            if let found @ Some(_) = find_in_expr(value, id) {
                return found;
            }
            find_in_expr(slice, id)
        }
        ExprKind::Slice { lower, upper, step } => {
            for part in [lower, upper, step].into_iter().flatten() {
                if let found @ Some(_) = find_in_expr(part, id) {
                    return found;
                }
            }
            None
        }
        ExprKind::List { elts } | ExprKind::Tuple { elts } | ExprKind::Set { elts } => {
            find_in_exprs(elts, id)
        }
        ExprKind::Dict { keys, values } => {
            for key in keys.iter_mut().flatten() {
                if let found @ Some(_) = find_in_expr(key, id) {
                    return found;
                }
            }
            find_in_exprs(values, id)
        }
        ExprKind::Starred { value } | ExprKind::Await { value } | ExprKind::YieldFrom { value } => {
            find_in_expr(value, id)
        }
        ExprKind::Yield { value } => match value {
            Some(value) => find_in_expr(value, id),
            None => None,
        },
        ExprKind::Lambda { args, body } => {
            if let found @ Some(_) = find_in_args(args, id) {
                return found;
            }
            find_in_expr(body, id)
        }
        ExprKind::ListComp { elt, generators }
        | ExprKind::SetComp { elt, generators }
        | ExprKind::GeneratorExp { elt, generators } => {
            if let found @ Some(_) = find_in_expr(elt, id) {
                return found;
            }
            for comp in generators {
                if let found @ Some(_) = find_in_comp(comp, id) {
                    return found;
                }
            }
            None
        }
        ExprKind::DictComp {
            key,
            value,
            generators,
        } => {
            if let found @ Some(_) = find_in_expr(key, id) {
                return found;
            }
            if let found @ Some(_) = find_in_expr(value, id) {
                return found;
            }
            for comp in generators {
                if let found @ Some(_) = find_in_comp(comp, id) {
                    return found;
                }
            }
            None
        }
        ExprKind::JoinedStr { values } => find_in_exprs(values, id),
        ExprKind::FormattedValue {
            value, format_spec, ..
        } => {
            if let found @ Some(_) = find_in_expr(value, id) {
                return found;
            }
            match format_spec {
                Some(spec) => find_in_expr(spec, id),
                None => None,
            }
        }
        ExprKind::NamedExpr { target, value } => {
            if let found @ Some(_) = find_in_expr(target, id) {
                return found;
            }
            find_in_expr(value, id)
        }
    }
}

fn find_node_mut(module: &mut Module, id: usize) -> Option<FoundMut<'_>> {
    find_in_stmts(&mut module.body, id)
}

/// Apply one site's mutation to a cloned tree, the four
/// applications of the panel, on the pattern of the landed
/// Python's in place application.
fn apply(tree: &mut Module, site: &Site, direction: i32) {
    match find_node_mut(tree, site.node_id) {
        Some(FoundMut::Stmt(stmt)) => {
            assert_eq!(
                site.kind,
                SiteKind::ReturnNone,
                "a statement site is a return"
            );
            if let StmtKind::Return { value: Some(value) } = &mut stmt.kind {
                value.kind = ExprKind::Constant(ConstVal::NoneVal);
            } else {
                panic!("the return site holds a valued return");
            }
        }
        Some(FoundMut::Expr(expr)) => match (&site.kind, &mut expr.kind) {
            (SiteKind::CompareInvert, ExprKind::Compare { ops, .. }) => {
                ops[0] = invert(ops[0]);
            }
            (SiteKind::BoolNegate, ExprKind::Constant(ConstVal::Bool(flag))) => {
                *flag = !*flag;
            }
            (SiteKind::ConstantShift, ExprKind::Constant(ConstVal::Int(text))) => {
                *text = shift_decimal_text(text, direction);
            }
            (SiteKind::ConstantShift, ExprKind::Constant(ConstVal::Float(value))) => {
                *value += direction as f64;
            }
            _ => panic!("the site kind and the node kind agree"),
        },
        None => panic!("the site node stands in the cloned tree"),
    }
}

/// Generate every mutant in the fixed panel for one module,
/// the port of the landed Python's generate_mutants. Where
/// the Python raises uncaught the port refuses in its own
/// wording under PORT8-5 and returns no list at all.
pub fn generate_mutants(source: &str) -> Result<Vec<Mutant>, GenerateRefusal> {
    let base = parse_module(source)?;
    let sites = collect_sites(&base)?;
    if !sites.is_empty() && path_nodes(NRef::Module(&base)) >= MODULE_UNPARSE_STANDING {
        // The per mutant unparse reads the whole module, so a
        // module past the standing refuses before the first
        // mutant is built, where the Python's first per mutant
        // unparse would die. A site free module is never
        // unparsed by the Python at all and answers the empty
        // list at any depth, so the standing guards only a
        // module the loop below would actually spell.
        return Err(recursion_refusal());
    }
    let mut mutants = Vec::new();
    for site in &sites {
        if site.kind == SiteKind::ConstantShift {
            for (direction, label) in [(1, "plus one"), (-1, "minus one")] {
                let mut tree = base.clone();
                apply(&mut tree, site, direction);
                mutants.push(Mutant {
                    kind: "constant_shift".to_string(),
                    line: site.line,
                    detail: format!("{} moved {}", site.detail, label),
                    source: unparse_module(&tree),
                });
            }
            continue;
        }
        let mut tree = base.clone();
        apply(&mut tree, site, 0);
        let kind = match site.kind {
            SiteKind::ReturnNone => "return_none",
            SiteKind::CompareInvert => "compare_invert",
            SiteKind::BoolNegate => "bool_negate",
            SiteKind::ConstantShift => unreachable!("shifts stand above"),
        };
        mutants.push(Mutant {
            kind: kind.to_string(),
            line: site.line,
            detail: site.detail.clone(),
            source: unparse_module(&tree),
        });
    }
    Ok(mutants)
}
