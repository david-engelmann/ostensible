//! The walks over the tree, reproducing ast.walk and the
//! child enumeration of the interpreter. Children stand in
//! the field order CPython states for each node kind, and the
//! walk is the breadth first order ast.walk yields, because
//! readings in the scan record state in walk order.

use crate::pyast::*;
use std::collections::{HashMap, VecDeque};

/// A reference to any node of a parsed tree, standing in for
/// the node objects the Python readings move over.
#[derive(Clone, Copy)]
pub enum NRef<'a> {
    Module(&'a Module),
    Stmt(&'a Stmt),
    Expr(&'a Expr),
    Args(&'a Arguments),
    Arg(&'a Arg),
    Keyword(&'a Keyword),
    Comp(&'a Comprehension),
    Handler(&'a ExceptHandler),
    Case(&'a MatchCase),
    Pattern(&'a Pattern),
    TypeParam(&'a TypeParam),
    Alias(&'a Alias),
    WithItem(&'a WithItem),
}

impl<'a> NRef<'a> {
    pub fn id(&self) -> usize {
        match self {
            NRef::Module(node) => node.id,
            NRef::Stmt(node) => node.id,
            NRef::Expr(node) => node.id,
            NRef::Args(node) => node.id,
            NRef::Arg(node) => node.id,
            NRef::Keyword(node) => node.id,
            NRef::Comp(node) => node.id,
            NRef::Handler(node) => node.id,
            NRef::Case(node) => node.id,
            NRef::Pattern(node) => node.id,
            NRef::TypeParam(node) => node.id,
            NRef::Alias(node) => node.id,
            NRef::WithItem(node) => node.id,
        }
    }

    pub fn as_stmt(&self) -> Option<&'a Stmt> {
        match self {
            NRef::Stmt(node) => Some(node),
            _ => None,
        }
    }

    pub fn as_expr(&self) -> Option<&'a Expr> {
        match self {
            NRef::Expr(node) => Some(node),
            _ => None,
        }
    }

    /// The child nodes in the field order CPython states.
    pub fn children(&self) -> Vec<NRef<'a>> {
        let mut out: Vec<NRef<'a>> = Vec::new();
        match self {
            NRef::Module(node) => {
                for stmt in &node.body {
                    out.push(NRef::Stmt(stmt));
                }
            }
            NRef::Args(node) => {
                for arg in &node.posonlyargs {
                    out.push(NRef::Arg(arg));
                }
                for arg in &node.args {
                    out.push(NRef::Arg(arg));
                }
                if let Some(arg) = &node.vararg {
                    out.push(NRef::Arg(arg));
                }
                for arg in &node.kwonlyargs {
                    out.push(NRef::Arg(arg));
                }
                for expr in node.kw_defaults.iter().flatten() {
                    out.push(NRef::Expr(expr));
                }
                if let Some(arg) = &node.kwarg {
                    out.push(NRef::Arg(arg));
                }
                for expr in &node.defaults {
                    out.push(NRef::Expr(expr));
                }
            }
            NRef::Arg(node) => {
                if let Some(annotation) = &node.annotation {
                    out.push(NRef::Expr(annotation));
                }
            }
            NRef::Keyword(node) => {
                out.push(NRef::Expr(&node.value));
            }
            NRef::Comp(node) => {
                out.push(NRef::Expr(&node.target));
                out.push(NRef::Expr(&node.iter));
                for expr in &node.ifs {
                    out.push(NRef::Expr(expr));
                }
            }
            NRef::Handler(node) => {
                if let Some(expr) = &node.type_ {
                    out.push(NRef::Expr(expr));
                }
                for stmt in &node.body {
                    out.push(NRef::Stmt(stmt));
                }
            }
            NRef::Case(node) => {
                out.push(NRef::Pattern(&node.pattern));
                if let Some(expr) = &node.guard {
                    out.push(NRef::Expr(expr));
                }
                for stmt in &node.body {
                    out.push(NRef::Stmt(stmt));
                }
            }
            NRef::Pattern(node) => match &node.kind {
                PatternKind::MatchValue(expr) => out.push(NRef::Expr(expr)),
                PatternKind::MatchSingleton(_) => {}
                PatternKind::MatchSequence(patterns) => {
                    for pattern in patterns {
                        out.push(NRef::Pattern(pattern));
                    }
                }
                PatternKind::MatchStar(_) => {}
                PatternKind::MatchMapping { keys, patterns, .. } => {
                    for expr in keys {
                        out.push(NRef::Expr(expr));
                    }
                    for pattern in patterns {
                        out.push(NRef::Pattern(pattern));
                    }
                }
                PatternKind::MatchClass {
                    cls,
                    patterns,
                    kwd_patterns,
                    ..
                } => {
                    out.push(NRef::Expr(cls));
                    for pattern in patterns {
                        out.push(NRef::Pattern(pattern));
                    }
                    for pattern in kwd_patterns {
                        out.push(NRef::Pattern(pattern));
                    }
                }
                PatternKind::MatchAs { pattern, .. } => {
                    if let Some(pattern) = pattern {
                        out.push(NRef::Pattern(pattern));
                    }
                }
                PatternKind::MatchOr(patterns) => {
                    for pattern in patterns {
                        out.push(NRef::Pattern(pattern));
                    }
                }
            },
            NRef::TypeParam(node) => {
                if let TypeParamKind::TypeVar {
                    bound: Some(bound), ..
                } = &node.kind
                {
                    out.push(NRef::Expr(bound));
                }
            }
            NRef::Alias(_) => {}
            NRef::WithItem(node) => {
                out.push(NRef::Expr(&node.context_expr));
                if let Some(expr) = &node.optional_vars {
                    out.push(NRef::Expr(expr));
                }
            }
            NRef::Stmt(node) => stmt_children(node, &mut out),
            NRef::Expr(node) => expr_children(node, &mut out),
        }
        out
    }
}

fn stmt_children<'a>(node: &'a Stmt, out: &mut Vec<NRef<'a>>) {
    match &node.kind {
        StmtKind::FuncDef {
            args,
            body,
            decorators,
            returns,
            type_params,
            ..
        } => {
            out.push(NRef::Args(args));
            for stmt in body {
                out.push(NRef::Stmt(stmt));
            }
            for expr in decorators {
                out.push(NRef::Expr(expr));
            }
            if let Some(expr) = returns {
                out.push(NRef::Expr(expr));
            }
            for param in type_params {
                out.push(NRef::TypeParam(param));
            }
        }
        StmtKind::ClassDef {
            bases,
            keywords,
            body,
            decorators,
            type_params,
            ..
        } => {
            for expr in bases {
                out.push(NRef::Expr(expr));
            }
            for keyword in keywords {
                out.push(NRef::Keyword(keyword));
            }
            for stmt in body {
                out.push(NRef::Stmt(stmt));
            }
            for expr in decorators {
                out.push(NRef::Expr(expr));
            }
            for param in type_params {
                out.push(NRef::TypeParam(param));
            }
        }
        StmtKind::Return { value } => {
            if let Some(expr) = value {
                out.push(NRef::Expr(expr));
            }
        }
        StmtKind::Delete { targets } => {
            for expr in targets {
                out.push(NRef::Expr(expr));
            }
        }
        StmtKind::Assign { targets, value } => {
            for expr in targets {
                out.push(NRef::Expr(expr));
            }
            out.push(NRef::Expr(value));
        }
        StmtKind::AugAssign { target, value, .. } => {
            out.push(NRef::Expr(target));
            out.push(NRef::Expr(value));
        }
        StmtKind::AnnAssign {
            target,
            annotation,
            value,
            ..
        } => {
            out.push(NRef::Expr(target));
            out.push(NRef::Expr(annotation));
            if let Some(expr) = value {
                out.push(NRef::Expr(expr));
            }
        }
        StmtKind::For {
            target,
            iter,
            body,
            orelse,
            ..
        } => {
            out.push(NRef::Expr(target));
            out.push(NRef::Expr(iter));
            for stmt in body {
                out.push(NRef::Stmt(stmt));
            }
            for stmt in orelse {
                out.push(NRef::Stmt(stmt));
            }
        }
        StmtKind::While { test, body, orelse } => {
            out.push(NRef::Expr(test));
            for stmt in body {
                out.push(NRef::Stmt(stmt));
            }
            for stmt in orelse {
                out.push(NRef::Stmt(stmt));
            }
        }
        StmtKind::If { test, body, orelse } => {
            out.push(NRef::Expr(test));
            for stmt in body {
                out.push(NRef::Stmt(stmt));
            }
            for stmt in orelse {
                out.push(NRef::Stmt(stmt));
            }
        }
        StmtKind::With { items, body, .. } => {
            for item in items {
                out.push(NRef::WithItem(item));
            }
            for stmt in body {
                out.push(NRef::Stmt(stmt));
            }
        }
        StmtKind::Match { subject, cases } => {
            out.push(NRef::Expr(subject));
            for case in cases {
                out.push(NRef::Case(case));
            }
        }
        StmtKind::Raise { exc, cause } => {
            if let Some(expr) = exc {
                out.push(NRef::Expr(expr));
            }
            if let Some(expr) = cause {
                out.push(NRef::Expr(expr));
            }
        }
        StmtKind::Try {
            body,
            handlers,
            orelse,
            finalbody,
            ..
        } => {
            for stmt in body {
                out.push(NRef::Stmt(stmt));
            }
            for handler in handlers {
                out.push(NRef::Handler(handler));
            }
            for stmt in orelse {
                out.push(NRef::Stmt(stmt));
            }
            for stmt in finalbody {
                out.push(NRef::Stmt(stmt));
            }
        }
        StmtKind::Assert { test, msg } => {
            out.push(NRef::Expr(test));
            if let Some(expr) = msg {
                out.push(NRef::Expr(expr));
            }
        }
        StmtKind::Import { names } => {
            for alias in names {
                out.push(NRef::Alias(alias));
            }
        }
        StmtKind::ImportFrom { names, .. } => {
            for alias in names {
                out.push(NRef::Alias(alias));
            }
        }
        StmtKind::Global { .. }
        | StmtKind::Nonlocal { .. }
        | StmtKind::Pass
        | StmtKind::Break
        | StmtKind::Continue => {}
        StmtKind::ExprStmt { value } => {
            out.push(NRef::Expr(value));
        }
        StmtKind::TypeAlias {
            name,
            type_params,
            value,
        } => {
            out.push(NRef::Expr(name));
            for param in type_params {
                out.push(NRef::TypeParam(param));
            }
            out.push(NRef::Expr(value));
        }
    }
}

fn expr_children<'a>(node: &'a Expr, out: &mut Vec<NRef<'a>>) {
    match &node.kind {
        ExprKind::Constant(_) | ExprKind::Name(_) => {}
        ExprKind::Attribute { value, .. } => out.push(NRef::Expr(value)),
        ExprKind::Call {
            func,
            args,
            keywords,
        } => {
            out.push(NRef::Expr(func));
            for expr in args {
                out.push(NRef::Expr(expr));
            }
            for keyword in keywords {
                out.push(NRef::Keyword(keyword));
            }
        }
        ExprKind::BinOp { left, right, .. } => {
            out.push(NRef::Expr(left));
            out.push(NRef::Expr(right));
        }
        ExprKind::UnaryOp { operand, .. } => out.push(NRef::Expr(operand)),
        ExprKind::BoolOp { values, .. } => {
            for expr in values {
                out.push(NRef::Expr(expr));
            }
        }
        ExprKind::Compare {
            left, comparators, ..
        } => {
            out.push(NRef::Expr(left));
            for expr in comparators {
                out.push(NRef::Expr(expr));
            }
        }
        ExprKind::IfExp { test, body, orelse } => {
            out.push(NRef::Expr(test));
            out.push(NRef::Expr(body));
            out.push(NRef::Expr(orelse));
        }
        ExprKind::Subscript { value, slice } => {
            out.push(NRef::Expr(value));
            out.push(NRef::Expr(slice));
        }
        ExprKind::Slice { lower, upper, step } => {
            for expr in [lower, upper, step].into_iter().flatten() {
                out.push(NRef::Expr(expr));
            }
        }
        ExprKind::List { elts } | ExprKind::Tuple { elts } | ExprKind::Set { elts } => {
            for expr in elts {
                out.push(NRef::Expr(expr));
            }
        }
        ExprKind::Dict { keys, values } => {
            for expr in keys.iter().flatten() {
                out.push(NRef::Expr(expr));
            }
            for expr in values {
                out.push(NRef::Expr(expr));
            }
        }
        ExprKind::Starred { value } | ExprKind::Await { value } | ExprKind::YieldFrom { value } => {
            out.push(NRef::Expr(value))
        }
        ExprKind::Yield { value } => {
            if let Some(expr) = value {
                out.push(NRef::Expr(expr));
            }
        }
        ExprKind::Lambda { args, body } => {
            out.push(NRef::Args(args));
            out.push(NRef::Expr(body));
        }
        ExprKind::ListComp { elt, generators }
        | ExprKind::SetComp { elt, generators }
        | ExprKind::GeneratorExp { elt, generators } => {
            out.push(NRef::Expr(elt));
            for comp in generators {
                out.push(NRef::Comp(comp));
            }
        }
        ExprKind::DictComp {
            key,
            value,
            generators,
        } => {
            out.push(NRef::Expr(key));
            out.push(NRef::Expr(value));
            for comp in generators {
                out.push(NRef::Comp(comp));
            }
        }
        ExprKind::JoinedStr { values } => {
            for expr in values {
                out.push(NRef::Expr(expr));
            }
        }
        ExprKind::FormattedValue {
            value, format_spec, ..
        } => {
            out.push(NRef::Expr(value));
            if let Some(expr) = format_spec {
                out.push(NRef::Expr(expr));
            }
        }
        ExprKind::NamedExpr { target, value } => {
            out.push(NRef::Expr(target));
            out.push(NRef::Expr(value));
        }
    }
}

/// Every node of the tree in the breadth first order
/// ast.walk yields.
pub fn walk(module: &Module) -> Vec<NRef<'_>> {
    let mut out = Vec::new();
    let mut queue: VecDeque<NRef> = VecDeque::new();
    queue.push_back(NRef::Module(module));
    while let Some(node) = queue.pop_front() {
        out.push(node);
        for child in node.children() {
            queue.push_back(child);
        }
    }
    out
}

/// The parent of each node, keyed by node id, over the same
/// enumeration ast.walk makes when it builds a parent map.
pub fn parent_map(module: &Module) -> HashMap<usize, NRef<'_>> {
    let mut parents: HashMap<usize, NRef> = HashMap::new();
    for node in walk(module) {
        for child in node.children() {
            parents.insert(child.id(), node);
        }
    }
    parents
}

/// A registry from node id to the node itself, for readings
/// that climb the parent map and must read the nodes they
/// reach.
pub fn node_registry(module: &Module) -> HashMap<usize, NRef<'_>> {
    let mut registry = HashMap::new();
    for node in walk(module) {
        registry.insert(node.id(), node);
    }
    registry
}
