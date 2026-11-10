//! Pins for the deep entry points of the Round 15 fix pass.
//! The public parse and unparse readings run on a spawned
//! thread with a large stack, so a deep source inside the
//! port bound of 4,000 answers on a plain thread and a
//! source past the bound refuses in the port's wording.
//! These tests call the entries from the test thread
//! itself, never from a deep stack of their own: before the
//! fix the same calls aborted the process, and the deepest
//! chains here stand past those measured aborts. Every
//! spelling expectation is the byte shape the unparse
//! reading of the landed Python gives for the same source.

use ostensible::pyast::{
    Arguments, ConstVal, Expr, ExprKind, Module, Pos, Stmt, StmtKind, UnaryKind,
};
use ostensible::pyparse::parse_module;
use ostensible::pyscan::scan_source;
use ostensible::pyunparse::unparse_module;

fn minus_source(n: usize) -> String {
    format!("def test_a():\n    x = {}1\n", "-".repeat(n))
}

fn minus_spelling(n: usize) -> String {
    format!("def test_a():\n    x = {}1", "-".repeat(n))
}

fn ifexp_source(n: usize) -> String {
    let mut source = String::from("def test_a():\n    x = ");
    for _ in 0..n {
        source.push_str("1 if flag else ");
    }
    source.push_str("2\n");
    source
}

fn ifexp_spelling(n: usize) -> String {
    let mut spelling = String::from("def test_a():\n    x = ");
    for _ in 0..n {
        spelling.push_str("1 if flag else ");
    }
    spelling.push('2');
    spelling
}

fn lambda_source(n: usize) -> String {
    let mut source = String::from("x = ");
    for _ in 0..n {
        source.push_str("lambda: ");
    }
    source.push_str("1\n");
    source
}

/// A module holding one function whose body assigns a
/// unary minus chain of the given depth, built node by
/// node without a parse, so the unparse reading stands
/// alone under test.
fn hand_built_minus_module(n: usize) -> Module {
    let mut next_id = 0usize;
    let mut fresh = move || {
        next_id += 1;
        next_id
    };
    let pos = Pos::new(1, 0);
    let mut expr = Expr {
        id: fresh(),
        pos,
        kind: ExprKind::Constant(ConstVal::Int("1".to_string())),
    };
    for _ in 0..n {
        expr = Expr {
            id: fresh(),
            pos,
            kind: ExprKind::UnaryOp {
                op: UnaryKind::USub,
                operand: Box::new(expr),
            },
        };
    }
    let assign = Stmt {
        id: fresh(),
        pos,
        kind: StmtKind::Assign {
            targets: vec![Expr {
                id: fresh(),
                pos,
                kind: ExprKind::Name("x".to_string()),
            }],
            value: expr,
        },
    };
    let funcdef = Stmt {
        id: fresh(),
        pos,
        kind: StmtKind::FuncDef {
            name: "test_a".to_string(),
            args: Box::new(Arguments {
                id: fresh(),
                posonlyargs: Vec::new(),
                args: Vec::new(),
                vararg: None,
                kwonlyargs: Vec::new(),
                kw_defaults: Vec::new(),
                kwarg: None,
                defaults: Vec::new(),
            }),
            body: vec![assign],
            decorators: Vec::new(),
            returns: None,
            type_params: Vec::new(),
            is_async: false,
        },
    };
    Module {
        id: fresh(),
        body: vec![funcdef],
    }
}

#[test]
fn parse_answers_a_minus_chain_past_the_old_abort() {
    let module = parse_module(&minus_source(2500)).expect("the parse answers at 2500");
    assert_eq!(unparse_module(&module), minus_spelling(2500));
}

#[test]
fn parse_answers_a_minus_chain_at_the_bound() {
    let module = parse_module(&minus_source(4000)).expect("the parse answers at the bound");
    assert_eq!(unparse_module(&module), minus_spelling(4000));
}

#[test]
fn parse_refuses_a_minus_chain_past_the_bound() {
    let error = parse_module(&minus_source(4001)).expect_err("the parse refuses past the bound");
    assert_eq!(error.message, "nesting past the port bound");
}

#[test]
fn unparse_answers_a_hand_built_chain_past_the_old_abort() {
    let module = hand_built_minus_module(2000);
    assert_eq!(unparse_module(&module), minus_spelling(2000));
}

#[test]
fn unparse_answers_a_hand_built_chain_at_the_bound() {
    let module = hand_built_minus_module(4000);
    assert_eq!(unparse_module(&module), minus_spelling(4000));
}

#[test]
fn parse_answers_a_conditional_chain_and_spells_it() {
    let module = parse_module(&ifexp_source(2500)).expect("the parse answers at 2500");
    assert_eq!(unparse_module(&module), ifexp_spelling(2500));
}

#[test]
fn parse_refuses_a_conditional_chain_past_the_bound() {
    let error = parse_module(&ifexp_source(4001)).expect_err("the parse refuses past the bound");
    assert_eq!(error.message, "nesting past the port bound");
}

#[test]
fn parse_refuses_a_lambda_chain_past_the_bound() {
    let error = parse_module(&lambda_source(4001)).expect_err("the parse refuses past the bound");
    assert_eq!(error.message, "nesting past the port bound");
}

#[test]
fn scan_refuses_where_the_interpreters_scan_dies_on_a_plain_chain() {
    // A plain assignment chain inside one test function:
    // the interpreter's scan dies on this shape as well,
    // its readings of the definition reaching the chain
    // however it stands in the function, and the port
    // refuses in the recursion wording. Parse and unparse
    // answer the same source whole, as the pins above
    // show: the refusal is the scan's, not the tree's.
    let source = format!(
        "def test_a():\n    x = {}1\n    assert True\n",
        "-".repeat(3900)
    );
    let error = scan_source(&source, "test_case.py", None, None)
        .expect_err("the scan refuses at the recursion standing");
    assert_eq!(
        error.message,
        "the source nests past the recursion standing of the interpreter"
    );
}

#[test]
fn scan_refuses_where_the_interpreters_own_walk_dies() {
    // The same chain inside an effective helper
    // definition: the interpreter's own walk of the
    // definition dies of its recursion standing near a
    // thousand nodes of path, and the scan refuses in the
    // recursion wording where the Python's reading fails.
    let source = format!(
        "def helper():\n    x = {}1\n    return x\n",
        "-".repeat(3900)
    );
    let error = scan_source(&source, "test_case.py", None, None)
        .expect_err("the scan refuses at the walk standing");
    assert_eq!(
        error.message,
        "the source nests past the recursion standing of the interpreter"
    );
}
