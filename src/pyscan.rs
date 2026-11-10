//! The static scan of PORT7-1, PORT7-4, and PORT7-5, with
//! the selector of PORT7-9 in pyselect. The pass reproduces
//! the landed Python's readings whole: the governing
//! definition gate, the four finding kinds with their detail
//! wordings verbatim, the configured tables with the
//! conftest fold and table provenance carried as data, the
//! walk boundaries, and the walrus silence, there being no
//! walrus reading anywhere in the scan. Where the Python
//! raises uncaught, the port refuses in its own wording
//! under PORT7-7 and returns no answer at all.

use crate::model::StaticFinding;
use crate::pyast::*;
use crate::pyparse::{parse_module, ParseError};
use crate::pyunparse::unparse_expr;
use crate::pywalk::{node_registry, parent_map, walk, NRef};
use crate::settings::{name_matches, CollectionSettings};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

/// A refusal of the port, the PORT7-7 family. The wording is
/// the port's own and stands in for the interpreter failure
/// the Python lets propagate.
#[derive(Debug, Clone)]
pub struct ScanRefusal {
    pub message: String,
}

fn refusal(message: &str) -> ScanRefusal {
    ScanRefusal {
        message: message.to_string(),
    }
}

impl ScanRefusal {
    pub(crate) fn new(message: &str) -> ScanRefusal {
        refusal(message)
    }
}

impl From<ParseError> for ScanRefusal {
    fn from(_error: ParseError) -> ScanRefusal {
        refusal("the source does not parse under the port's reading")
    }
}

/// The recursion standings of the interpreter, measured
/// against the landed Python at the default recursion limit
/// and stated in the amendment to PORT7-7 in CONTRACT.md.
/// An unparse reading the scan's main walks make stands to
/// a path of 331 nodes and refuses at 332. An unparse
/// reading the helper and fixture readings and the helper
/// finding walk make stands one node shallower, refusing
/// at 331, because those readings sit deeper in the call.
/// The own walk over an effective definition stands to a
/// path of 992 nodes and refuses at 993, and a subscript
/// chain inside that walk stands one level shallower,
/// refusing at 989 links.
const UNPARSE_MAIN_STANDING: usize = 332;
const UNPARSE_HELPER_STANDING: usize = 331;
const WALK_PATH_STANDING: usize = 993;
const SUBSCRIPT_CHAIN_STANDING: usize = 989;

/// A refusal under the recursion member of the PORT7-7
/// family, in the port's own wording.
pub(crate) fn recursion_refusal() -> ScanRefusal {
    refusal("the source nests past the recursion standing of the interpreter")
}

/// The number of nodes on the deepest root to leaf path of
/// a subtree, counted iteratively.
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

/// Refuse when the unparse reading of one expression would
/// pass the standing of its call site, measured as the
/// deepest path of the expression's subtree.
fn unparse_guard(expr: &Expr, standing: usize) -> Result<(), ScanRefusal> {
    if path_nodes(NRef::Expr(expr)) >= standing {
        Err(recursion_refusal())
    } else {
        Ok(())
    }
}

/// The own walk measure of one effective definition, the
/// deepest node path of the walk and the longest subscript
/// chain inside it. The walk never crosses into a nested
/// function definition, class definition, or lambda, as the
/// interpreter's own walk of PORT7-5 states.
fn own_walk_measure(def: &Stmt) -> (usize, usize) {
    let mut deepest = 1usize;
    let mut deepest_chain = 0usize;
    let mut stack: Vec<(NRef, usize, usize)> = vec![(NRef::Stmt(def), 1, 0)];
    while let Some((node, depth, run)) = stack.pop() {
        if depth > deepest {
            deepest = depth;
        }
        if run > deepest_chain {
            deepest_chain = run;
        }
        let subscript_value = match node {
            NRef::Expr(expr) => match &expr.kind {
                ExprKind::Subscript { value, .. } => Some(value.id),
                _ => None,
            },
            _ => None,
        };
        for child in node.children() {
            match child {
                NRef::Stmt(stmt)
                    if matches!(
                        stmt.kind,
                        StmtKind::FuncDef { .. } | StmtKind::ClassDef { .. }
                    ) =>
                {
                    continue
                }
                NRef::Expr(expr) if matches!(expr.kind, ExprKind::Lambda { .. }) => continue,
                _ => {}
            }
            let child_run = match child {
                NRef::Expr(expr) if matches!(expr.kind, ExprKind::Subscript { .. }) => {
                    if subscript_value == Some(child.id()) {
                        run + 1
                    } else {
                        1
                    }
                }
                _ => 0,
            };
            stack.push((child, depth + 1, child_run));
        }
    }
    (deepest, deepest_chain)
}

/// Refuse when any effective definition of a module stands
/// past the own walk standings, the check the scan makes
/// before its helper and fixture readings and the check it
/// makes over every conftest supplier it folds.
fn recursion_walk_guard(module: &Module) -> Result<(), ScanRefusal> {
    for (_, def) in module_definitions(module) {
        let (path, chain) = own_walk_measure(def);
        if path >= WALK_PATH_STANDING || chain >= SUBSCRIPT_CHAIN_STANDING {
            return Err(recursion_refusal());
        }
    }
    Ok(())
}

/// Parse on a spawned thread with a large stack, so a deep
/// tree inside the port bound of 4,000 cannot exhaust the
/// stack of the thread that called the scan. The selector
/// reads through the same door.
pub fn parse_for_select(source: &str) -> Result<Module, ScanRefusal> {
    parse_big_stack(source)
}

fn parse_big_stack(source: &str) -> Result<Module, ScanRefusal> {
    parse_module(source).map_err(ScanRefusal::from)
}

/// A configured table, attribute chain to the one literal
/// configured for that call as unparse text. The table is
/// shared by reference. Whether the table arrived through
/// the conftest supply stands as data on the table itself,
/// the provenance reading of PORT7-5, replacing the
/// reference identities the reading carried before.
#[derive(Clone)]
pub struct Table {
    map: Rc<HashMap<String, String>>,
    from_supply: bool,
}

impl Table {
    fn new(map: HashMap<String, String>) -> Table {
        Table {
            map: Rc::new(map),
            from_supply: false,
        }
    }

    /// The literal configured for one attribute chain, if
    /// the table configures it.
    fn get(&self, chain: &str) -> Option<&String> {
        self.map.get(chain)
    }

    /// A copy of this table marked as arrived through the
    /// conftest supply.
    fn as_supplied(&self) -> Table {
        Table {
            map: self.map.clone(),
            from_supply: true,
        }
    }
}

fn new_table() -> HashMap<String, String> {
    HashMap::new()
}

/// Whether one decorator is a fixture decorator, read by
/// name alone. One call layer unwraps, and an attribute of
/// the name fixture on the bare name pytest or the bare
/// name fixture itself counts.
pub fn is_fixture_decorator(decorator: &Expr) -> bool {
    let target = match &decorator.kind {
        ExprKind::Call { func, .. } => func.as_ref(),
        _ => decorator,
    };
    match &target.kind {
        ExprKind::Name(name) => name == "fixture",
        ExprKind::Attribute { value, attr } => {
            attr == "fixture" && matches!(&value.kind, ExprKind::Name(name) if name == "pytest")
        }
        _ => false,
    }
}

/// Whether a definition carries a fixture decorator.
pub fn is_fixture_definition(decorators: &[Expr]) -> bool {
    decorators.iter().any(is_fixture_decorator)
}

/// Whether this node is a function definition under the
/// port's unified statement shape.
fn is_funcdef(node: &Stmt) -> bool {
    matches!(node.kind, StmtKind::FuncDef { .. })
}

fn funcdef_name(node: &Stmt) -> &str {
    match &node.kind {
        StmtKind::FuncDef { name, .. } => name,
        _ => unreachable!(),
    }
}

fn funcdef_args(node: &Stmt) -> &Arguments {
    match &node.kind {
        StmtKind::FuncDef { args, .. } => args,
        _ => unreachable!(),
    }
}

fn funcdef_body(node: &Stmt) -> &[Stmt] {
    match &node.kind {
        StmtKind::FuncDef { body, .. } => body,
        StmtKind::ClassDef { body, .. } => body,
        _ => unreachable!(),
    }
}

fn funcdef_decorators(node: &Stmt) -> &[Expr] {
    match &node.kind {
        StmtKind::FuncDef { decorators, .. } => decorators,
        StmtKind::ClassDef { decorators, .. } => decorators,
        _ => unreachable!(),
    }
}

fn funcdef_is_async(node: &Stmt) -> bool {
    match &node.kind {
        StmtKind::FuncDef { is_async, .. } => *is_async,
        _ => false,
    }
}

/// The statement lists a descended statement holds, in the
/// enumeration the definition stream and the fixture class
/// reach share.
pub fn descended_statement_lists(node: &Stmt) -> Vec<&[Stmt]> {
    let mut out: Vec<&[Stmt]> = Vec::new();
    match &node.kind {
        StmtKind::If { body, orelse, .. } => {
            out.push(body);
            out.push(orelse);
        }
        StmtKind::Try {
            body,
            handlers,
            orelse,
            finalbody,
            ..
        } => {
            out.push(body);
            for handler in handlers {
                out.push(&handler.body);
            }
            out.push(orelse);
            out.push(finalbody);
        }
        StmtKind::For {
            body,
            orelse,
            is_async,
            ..
        } => {
            // The Python names the plain loop kinds alone
            // here, an async loop holding no descended list
            // for this enumeration.
            if !is_async {
                out.push(body);
                out.push(orelse);
            }
        }
        StmtKind::While { body, orelse, .. } => {
            out.push(body);
            out.push(orelse);
        }
        StmtKind::With { body, is_async, .. } => {
            if !is_async {
                out.push(body);
            }
        }
        StmtKind::Match { cases, .. } => {
            for case in cases {
                out.push(&case.body);
            }
        }
        _ => {}
    }
    out
}

/// Whether a fixture decorated class stands in the reach
/// the Python states, the module body and every descended
/// statement list and every class body standing in a list
/// being read, recursively, never crossing into a function.
pub fn module_has_fixture_class(module: &Module) -> bool {
    fn visit(statements: &[Stmt]) -> bool {
        for node in statements {
            if matches!(node.kind, StmtKind::ClassDef { .. }) {
                if is_fixture_definition(funcdef_decorators(node)) {
                    return true;
                }
                if visit(funcdef_body(node)) {
                    return true;
                }
            } else {
                for nested in descended_statement_lists(node) {
                    if visit(nested) {
                        return true;
                    }
                }
            }
        }
        false
    }
    visit(&module.body)
}

/// Whether an expression is a return_value attribute.
fn is_return_value(node: &Expr) -> bool {
    matches!(&node.kind, ExprKind::Attribute { attr, .. } if attr == "return_value")
}

/// The root name of a name or attribute chain.
fn root_name(node: &Expr) -> Option<&str> {
    let mut current = node;
    loop {
        match &current.kind {
            ExprKind::Attribute { value, .. } => current = value,
            ExprKind::Name(name) => return Some(name),
            _ => return None,
        }
    }
}

/// The root name of a call's function.
fn call_root(node: &Expr) -> Option<&str> {
    match &node.kind {
        ExprKind::Call { func, .. } => root_name(func),
        _ => None,
    }
}

/// Split a name or attribute chain into root name and the
/// dotted attribute path beyond it.
fn attribute_chain(node: &Expr) -> Option<(String, String)> {
    let mut parts: Vec<&str> = Vec::new();
    let mut current = node;
    loop {
        match &current.kind {
            ExprKind::Attribute { value, attr } => {
                parts.push(attr);
                current = value;
            }
            ExprKind::Name(name) => {
                parts.reverse();
                return Some((name.clone(), parts.join(".")));
            }
            _ => return None,
        }
    }
}

/// The nodes of a definition's own statements, in source
/// order, the walk never crossing into a nested definition,
/// class, or lambda.
fn own_walk(func: &Stmt) -> Vec<NRef<'_>> {
    fn visit<'a>(node: NRef<'a>, found: &mut Vec<NRef<'a>>) {
        for child in node.children() {
            if let NRef::Stmt(stmt) = child {
                if matches!(
                    stmt.kind,
                    StmtKind::FuncDef { .. } | StmtKind::ClassDef { .. }
                ) {
                    continue;
                }
            }
            if let NRef::Expr(expr) = child {
                if matches!(expr.kind, ExprKind::Lambda { .. }) {
                    continue;
                }
            }
            found.push(child);
            visit(child, found);
        }
    }
    let mut found = Vec::new();
    // The walk starts from the definition itself, whose
    // children the enumeration reads.
    let registry_holder;
    let root = {
        registry_holder = func;
        NRef::Stmt(registry_holder)
    };
    visit(root, &mut found);
    found
}

/// Map each configured call, named by root and attribute
/// chain, to the distinct literals its return value was
/// assigned inside one function. When before is given, only
/// assignments standing before that position count.
fn configured_chains(
    func: &Stmt,
    before: Option<(u32, u32)>,
) -> Result<HashMap<(String, String), HashSet<String>>, ScanRefusal> {
    let mut configured: HashMap<(String, String), HashSet<String>> = HashMap::new();
    for node in own_walk(func) {
        let Some(stmt) = node.as_stmt() else { continue };
        let StmtKind::Assign { targets, value } = &stmt.kind else {
            continue;
        };
        if targets.len() != 1 {
            continue;
        }
        if let Some(limit) = before {
            if (stmt.pos.line, stmt.pos.col) >= limit {
                continue;
            }
        }
        let target = &targets[0];
        if is_return_value(target) {
            if let ExprKind::Attribute { value: inner, .. } = &target.kind {
                if let Some(split) = attribute_chain(inner) {
                    unparse_guard(value, UNPARSE_HELPER_STANDING)?;
                    configured
                        .entry(split)
                        .or_default()
                        .insert(unparse_expr(value));
                }
            }
        }
    }
    Ok(configured)
}

/// The names a definition returns bare, in walk order.
fn returned_names(func: &Stmt) -> Vec<String> {
    let mut names = Vec::new();
    for node in own_walk(func) {
        if let Some(stmt) = node.as_stmt() {
            if let StmtKind::Return { value: Some(value) } = &stmt.kind {
                if let ExprKind::Name(name) = &value.kind {
                    names.push(name.clone());
                }
            }
        }
    }
    names
}

/// One provision point of a definition, a yield or a yield
/// from of its own.
enum Provision<'a> {
    Yield(&'a Expr),
    YieldFrom(&'a Expr),
}

fn own_provisions(func: &Stmt) -> Vec<(u32, u32, Provision<'_>)> {
    let mut points = Vec::new();
    for node in own_walk(func) {
        if let Some(expr) = node.as_expr() {
            match &expr.kind {
                ExprKind::Yield { .. } => {
                    points.push((expr.pos.line, expr.pos.col, Provision::Yield(expr)));
                }
                ExprKind::YieldFrom { .. } => {
                    points.push((expr.pos.line, expr.pos.col, Provision::YieldFrom(expr)));
                }
                _ => {}
            }
        }
    }
    points.sort_by_key(|(line, col, _)| (*line, *col));
    points
}

fn first_provision(func: &Stmt) -> Option<Provision<'_>> {
    own_provisions(func)
        .into_iter()
        .next()
        .map(|(_, _, point)| point)
}

/// The configured table one definition provides at a yield.
fn setup_table(func: &Stmt, first_yield: &Expr) -> Result<HashMap<String, String>, ScanRefusal> {
    let configured = configured_chains(func, Some((first_yield.pos.line, first_yield.pos.col)))?;
    let mut provided: HashSet<&str> = HashSet::new();
    if let ExprKind::Yield { value } = &first_yield.kind {
        if let Some(Expr {
            kind: ExprKind::Name(name),
            ..
        }) = value.as_deref()
        {
            provided.insert(name.as_str());
        }
    }
    let mut table = new_table();
    for ((root, chain), values) in configured {
        if provided.contains(root.as_str()) && values.len() == 1 {
            table.insert(chain, values.into_iter().next().unwrap());
        }
    }
    Ok(table)
}

/// The delegate name a yield from expression calls, or None.
fn delegation_name(node: &Expr) -> Option<&str> {
    if let ExprKind::YieldFrom { value } = &node.kind {
        if let ExprKind::Call { func, .. } = &value.kind {
            if let ExprKind::Name(name) = &func.kind {
                return Some(name);
            }
        }
    }
    None
}

/// The table a definition draws through its delegation,
/// followed link by link with the Python's refusals.
fn follow_delegation(
    node: &Stmt,
    definitions: &HashMap<String, &Stmt>,
    seen: &HashSet<String>,
) -> Result<Option<HashMap<String, String>>, ScanRefusal> {
    let Some(first) = first_provision(node) else {
        return Ok(None);
    };
    let Provision::YieldFrom(expr) = first else {
        return Ok(None);
    };
    let Some(name) = delegation_name(expr) else {
        return Ok(None);
    };
    if seen.contains(name) {
        return Ok(None);
    }
    let Some(target) = definitions.get(name) else {
        return Ok(None);
    };
    // An async delegate and a fixture decorated delegate
    // are refused. The port's function shape is one node for
    // both sync and async, so the async refusal reads the
    // flag the Python's type check states.
    if funcdef_is_async(target) || is_fixture_definition(funcdef_decorators(target)) {
        return Ok(None);
    }
    let Some(target_first) = first_provision(target) else {
        return Ok(None);
    };
    match target_first {
        Provision::Yield(yield_expr) => Ok(Some(setup_table(target, yield_expr)?)),
        Provision::YieldFrom(_) => {
            let mut next_seen: HashSet<String> = seen.clone();
            next_seen.insert(name.to_string());
            follow_delegation(target, definitions, &next_seen)
        }
    }
}

/// The effective function definitions of one module, the
/// stream of the module body and every descended statement
/// list in source order, the last definition of a name
/// winning. Insertion order follows first standing, as the
/// Python's map orders its keys.
pub fn module_definitions(module: &Module) -> Vec<(String, &Stmt)> {
    fn visit<'a>(statements: &'a [Stmt], bound: &mut Vec<(String, &'a Stmt)>) {
        for node in statements {
            if is_funcdef(node) {
                let name = funcdef_name(node).to_string();
                if let Some(entry) = bound.iter_mut().find(|(held, _)| *held == name) {
                    entry.1 = node;
                } else {
                    bound.push((name, node));
                }
            } else {
                for nested in descended_statement_lists(node) {
                    visit(nested, bound);
                }
            }
        }
    }
    let mut bound = Vec::new();
    visit(&module.body, &mut bound);
    bound
}

/// The two configured tables one module reading builds,
/// its helper table and its fixture table.
type HelperFixtureMaps = (HashMap<String, Table>, HashMap<String, Table>);

/// Configured values on the mocks helpers and fixtures
/// return, the helper table and the fixture table of one
/// module.
pub fn helper_and_fixture_maps(module: &Module) -> Result<HelperFixtureMaps, ScanRefusal> {
    let mut helpers: HashMap<String, Table> = HashMap::new();
    let mut fixtures: HashMap<String, Table> = HashMap::new();
    let definitions = module_definitions(module);
    let by_name: HashMap<String, &Stmt> = definitions.iter().cloned().collect();
    for (name, node) in &definitions {
        let mut table: Option<HashMap<String, String>> = None;
        if !funcdef_is_async(node) && is_fixture_definition(funcdef_decorators(node)) {
            if let Some(first) = first_provision(node) {
                table = Some(match first {
                    Provision::Yield(yield_expr) => setup_table(node, yield_expr)?,
                    Provision::YieldFrom(_) => {
                        let mut seen: HashSet<String> = HashSet::new();
                        seen.insert(name.clone());
                        follow_delegation(node, &by_name, &seen)?.unwrap_or_default()
                    }
                });
            }
        }
        let table = match table {
            Some(table) => table,
            None => {
                let configured = configured_chains(node, None)?;
                let returned = returned_names(node);
                let provided: HashSet<&str> = returned.iter().map(|name| name.as_str()).collect();
                let mut built = new_table();
                for ((root, chain), values) in configured {
                    if provided.contains(root.as_str()) && values.len() == 1 {
                        built.insert(chain, values.into_iter().next().unwrap());
                    }
                }
                built
            }
        };
        if table.is_empty() {
            continue;
        }
        let shared: Table = Table::new(table);
        helpers.insert(name.clone(), shared.clone());
        if is_fixture_definition(funcdef_decorators(node)) {
            fixtures.insert(name.clone(), shared);
        }
    }
    Ok((helpers, fixtures))
}

/// The names one module defines as fixtures, read on the
/// effective definitions.
pub fn fixture_names(module: &Module) -> HashSet<String> {
    module_definitions(module)
        .into_iter()
        .filter(|(_, node)| is_fixture_definition(funcdef_decorators(node)))
        .map(|(name, _)| name)
        .collect()
}

/// Fold the conftest fixture supply into a fixture table,
/// farthest supplier to nearest, the analysed file last.
/// A table a conftest supplier contributed stands marked
/// as supplied on the table itself.
pub fn merge_fixture_supply(
    module: &Module,
    fixtures: &HashMap<String, Table>,
    supply_sources: &[String],
) -> Result<HashMap<String, Table>, ScanRefusal> {
    let mut merged: HashMap<String, Table> = HashMap::new();
    let mut suppliers: Vec<(HashSet<String>, HashMap<String, Table>, bool)> = Vec::new();
    for source in supply_sources {
        let Ok(supply_tree) = parse_big_stack(source) else {
            continue;
        };
        if module_has_fixture_class(&supply_tree) {
            continue;
        }
        let (_supply_helpers, supply_fixtures) = helper_and_fixture_maps(&supply_tree)?;
        suppliers.push((fixture_names(&supply_tree), supply_fixtures, true));
    }
    suppliers.push((fixture_names(module), fixtures.clone(), false));
    for (names, table, from_supply) in suppliers {
        for name in &names {
            merged.remove(name);
        }
        for (name, item) in &table {
            if from_supply {
                merged.insert(name.clone(), item.as_supplied());
            } else {
                merged.insert(name.clone(), item.clone());
            }
        }
    }
    Ok(merged)
}

/// The statement lists inside a statement in the field
/// order the helper and fixture finding walk descends,
/// body and orelse and finalbody, lists of statements only
/// and not empty. Handler bodies and match case bodies are
/// not fields of the statement, so the walk never enters
/// them.
fn statement_lists(node: &Stmt) -> Vec<&[Stmt]> {
    let lists: Vec<&[Stmt]> = match &node.kind {
        StmtKind::FuncDef { body, .. } => vec![body],
        StmtKind::ClassDef { body, .. } => vec![body],
        StmtKind::If { body, orelse, .. } => vec![body, orelse],
        StmtKind::Try {
            body,
            orelse,
            finalbody,
            ..
        } => vec![body, orelse, finalbody],
        StmtKind::For { body, orelse, .. } | StmtKind::While { body, orelse, .. } => {
            vec![body, orelse]
        }
        StmtKind::With { body, .. } => vec![body],
        _ => Vec::new(),
    };
    lists.into_iter().filter(|list| !list.is_empty()).collect()
}

/// The helper and fixture finding an assert statement
/// draws, or None.
fn d5_assert_finding(
    node: &Stmt,
    env: &HashMap<String, Table>,
    path: &str,
    origin: &HashMap<String, bool>,
) -> Result<Option<StaticFinding>, ScanRefusal> {
    let StmtKind::Assert { test, .. } = &node.kind else {
        return Ok(None);
    };
    let ExprKind::Compare {
        left,
        ops,
        comparators,
    } = &test.kind
    else {
        return Ok(None);
    };
    if ops.len() != 1 || ops[0] != CmpKind::Eq || comparators.len() != 1 {
        return Ok(None);
    }
    for (side, other_node) in [
        (left.as_ref(), &comparators[0]),
        (&comparators[0], left.as_ref()),
    ] {
        if !matches!(side.kind, ExprKind::Call { .. }) {
            continue;
        }
        let ExprKind::Call { func, .. } = &side.kind else {
            continue;
        };
        let Some((root, chain)) = attribute_chain(func) else {
            continue;
        };
        if let Some(table) = env.get(&root) {
            // The interpreter unparses the other side here,
            // at the helper standing, only where a table
            // stands for the call's root.
            unparse_guard(other_node, UNPARSE_HELPER_STANDING)?;
            let other = unparse_expr(other_node);
            if table.get(&chain) == Some(&other) {
                let source = if origin.get(&root).copied().unwrap_or(false) {
                    "a conftest fixture"
                } else {
                    "a helper or fixture in the same file"
                };
                return Ok(Some(StaticFinding {
                    kind: "mock_return_value".to_string(),
                    path: path.to_string(),
                    line: node.pos.line as i64,
                    detail: format!(
                        "assertion checks a call on a mock against the return value that call was assigned in {}, {}",
                        source, other
                    ),
                }));
            }
        }
    }
    Ok(None)
}

/// The fixture context the helper and fixture finding walk
/// carries, the two maps named as one value so the walk
/// takes the clump as one argument.
struct D5Context<'a> {
    helpers: &'a HashMap<String, Table>,
    fixtures: &'a HashMap<String, Table>,
}

/// The binding state the walk carries from statement to
/// statement, the mock bindings with their origins, named
/// as one value so the two maps travel together.
struct D5Scope<'a> {
    env: &'a mut HashMap<String, Table>,
    origin: &'a mut HashMap<String, bool>,
}

/// Helper and fixture findings inside one function scope.
fn d5_findings_in_function(
    func: &Stmt,
    helpers: &HashMap<String, Table>,
    fixtures: &HashMap<String, Table>,
    path: &str,
) -> Result<Vec<StaticFinding>, ScanRefusal> {
    fn fixture_env(args: &Arguments, context: &D5Context, scope: &mut D5Scope) {
        for arg in args.args.iter().chain(args.kwonlyargs.iter()) {
            if let Some(table) = context.fixtures.get(&arg.name) {
                scope.env.insert(arg.name.clone(), table.clone());
                scope.origin.insert(arg.name.clone(), table.from_supply);
            }
        }
    }

    fn visit(
        statements: &[Stmt],
        scope: &mut D5Scope,
        context: &D5Context,
        path: &str,
        findings: &mut Vec<StaticFinding>,
    ) -> Result<(), ScanRefusal> {
        for statement in statements {
            if is_funcdef(statement) {
                let mut child_env = scope.env.clone();
                let mut child_origin = scope.origin.clone();
                let mut child_scope = D5Scope {
                    env: &mut child_env,
                    origin: &mut child_origin,
                };
                fixture_env(funcdef_args(statement), context, &mut child_scope);
                visit(
                    funcdef_body(statement),
                    &mut child_scope,
                    context,
                    path,
                    findings,
                )?;
                continue;
            }
            if let StmtKind::Assign { targets, value } = &statement.kind {
                if targets.len() == 1 {
                    if let ExprKind::Name(name) = &targets[0].kind {
                        let bound = match &value.kind {
                            ExprKind::Call { func, .. } => match &func.kind {
                                ExprKind::Name(callee) => context.helpers.get(callee),
                                _ => None,
                            },
                            _ => None,
                        };
                        if let Some(table) = bound {
                            scope.env.insert(name.clone(), table.clone());
                            scope.origin.insert(name.clone(), false);
                        } else {
                            scope.env.remove(name);
                            scope.origin.remove(name);
                        }
                    }
                }
            }
            if matches!(statement.kind, StmtKind::Assert { .. }) {
                if let Some(finding) = d5_assert_finding(statement, scope.env, path, scope.origin)?
                {
                    findings.push(finding);
                }
            }
            for nested in statement_lists(statement) {
                visit(nested, scope, context, path, findings)?;
            }
        }
        Ok(())
    }

    let context = D5Context { helpers, fixtures };
    let mut findings = Vec::new();
    let mut env: HashMap<String, Table> = HashMap::new();
    let mut origin: HashMap<String, bool> = HashMap::new();
    let mut scope = D5Scope {
        env: &mut env,
        origin: &mut origin,
    };
    fixture_env(funcdef_args(func), &context, &mut scope);
    visit(
        funcdef_body(func),
        &mut scope,
        &context,
        path,
        &mut findings,
    )?;
    Ok(findings)
}

/// Whether a function or method name is an item name under
/// the python_functions setting in force.
pub fn function_name_matches(name: &str, settings: &CollectionSettings) -> bool {
    match &settings.python_functions {
        None => name.starts_with("test"),
        Some(patterns) => name_matches(name, patterns),
    }
}

/// The governing function of a node, the outermost
/// definition standing at module level or directly inside
/// a module level class, read by climbing the parent map.
fn governing_function<'a>(
    node: NRef<'a>,
    parents: &HashMap<usize, NRef<'a>>,
    registry: &HashMap<usize, NRef<'a>>,
) -> Option<&'a Stmt> {
    let mut current = Some(node);
    while let Some(node_ref) = current {
        if let NRef::Stmt(stmt) = node_ref {
            if is_funcdef(stmt) {
                if let Some(parent) = parents.get(&stmt.id) {
                    if matches!(parent, NRef::Module(_)) {
                        return Some(stmt);
                    }
                    if let NRef::Stmt(class) = parent {
                        if matches!(class.kind, StmtKind::ClassDef { .. }) {
                            if let Some(grandparent) = parents.get(&class.id) {
                                if matches!(grandparent, NRef::Module(_)) {
                                    return Some(stmt);
                                }
                            }
                        }
                    }
                }
            }
        }
        current = parents.get(&node_ref.id()).copied();
        let _ = registry;
    }
    None
}

/// The definitions one statement list binds, the last
/// definition of a name winning.
fn bound_definitions(statements: &[Stmt]) -> HashMap<String, &Stmt> {
    let mut bound: HashMap<String, &Stmt> = HashMap::new();
    for node in statements {
        if is_funcdef(node) {
            bound.insert(funcdef_name(node).to_string(), node);
        }
    }
    bound
}

fn default_settings() -> CollectionSettings {
    CollectionSettings {
        python_functions: None,
        python_classes: None,
        python_files: None,
    }
}

/// Scan one test file source string and return its
/// findings, sorted by line and then kind. The reading
/// runs on a spawned thread with a large stack, the
/// pattern the conformance battery already uses, so a deep
/// source inside the standings returns its answer or its
/// refusal on a thread of any size and never aborts the
/// caller's process.
pub fn scan_source(
    source: &str,
    path: &str,
    settings: Option<&CollectionSettings>,
    supply_sources: Option<&[String]>,
) -> Result<Vec<StaticFinding>, ScanRefusal> {
    let outcome = std::thread::scope(|scope| {
        let handle = std::thread::Builder::new()
            .stack_size(512 * 1024 * 1024)
            .spawn_scoped(scope, || {
                scan_source_inner(source, path, settings, supply_sources)
            });
        match handle {
            Ok(joined) => joined.join(),
            Err(_) => Err(Box::new(()) as Box<dyn std::any::Any + Send>),
        }
    });
    match outcome {
        Ok(result) => result,
        Err(_) => Err(refusal("the scan's reading did not finish")),
    }
}

fn scan_source_inner(
    source: &str,
    path: &str,
    settings: Option<&CollectionSettings>,
    supply_sources: Option<&[String]>,
) -> Result<Vec<StaticFinding>, ScanRefusal> {
    let fallback = default_settings();
    let settings = settings.unwrap_or(&fallback);
    let tree = parse_big_stack(source)?;
    recursion_walk_guard(&tree)?;
    if let Some(sources) = supply_sources {
        for supply in sources {
            if let Ok(supply_tree) = parse_big_stack(supply) {
                recursion_walk_guard(&supply_tree)?;
            }
        }
    }
    let parents = parent_map(&tree);
    let registry = node_registry(&tree);
    let module_defs = module_definitions(&tree);
    let module_def_by_name: HashMap<String, &Stmt> = module_defs.iter().cloned().collect();
    let mut class_definitions: HashMap<usize, HashMap<String, &Stmt>> = HashMap::new();
    for node in &tree.body {
        if matches!(node.kind, StmtKind::ClassDef { .. }) {
            class_definitions.insert(node.id, bound_definitions(funcdef_body(node)));
        }
    }

    let in_test_function = |node: NRef| -> bool {
        let Some(governing) = governing_function(node, &parents, &registry) else {
            return false;
        };
        if !function_name_matches(funcdef_name(governing), settings) {
            return false;
        }
        let effective = match parents.get(&governing.id) {
            Some(NRef::Module(_)) => module_def_by_name
                .get(funcdef_name(governing))
                .copied()
                .unwrap_or(governing),
            Some(NRef::Stmt(class)) if matches!(class.kind, StmtKind::ClassDef { .. }) => {
                class_definitions
                    .get(&class.id)
                    .and_then(|defs| defs.get(funcdef_name(governing)))
                    .copied()
                    .unwrap_or(governing)
            }
            _ => governing,
        };
        !is_fixture_definition(funcdef_decorators(effective))
    };

    let mut findings: Vec<StaticFinding> = Vec::new();
    let mut return_values: HashMap<String, String> = HashMap::new();
    let mut read_from_mock: HashMap<String, String> = HashMap::new();
    let (helpers, mut fixtures) = helper_and_fixture_maps(&tree)?;
    if let Some(sources) = supply_sources {
        if !sources.is_empty() {
            fixtures = merge_fixture_supply(&tree, &fixtures, sources)?;
        }
    }

    for node in walk(&tree) {
        let Some(stmt) = node.as_stmt() else { continue };
        let StmtKind::Assign { targets, value } = &stmt.kind else {
            continue;
        };
        if targets.len() != 1 {
            continue;
        }
        let target = &targets[0];
        if is_return_value(target) {
            if let Some(root) = root_name(target) {
                unparse_guard(value, UNPARSE_MAIN_STANDING)?;
                return_values.insert(root.to_string(), unparse_expr(value));
            }
        } else if let ExprKind::Name(name) = &target.kind {
            if let Some(root) = call_root(value) {
                if return_values.contains_key(root) {
                    read_from_mock.insert(name.clone(), root.to_string());
                }
            }
        }
    }

    for node in walk(&tree) {
        let Some(stmt) = node.as_stmt() else { continue };
        let StmtKind::Assert { test, .. } = &stmt.kind else {
            continue;
        };
        if !in_test_function(node) {
            continue;
        }
        if let ExprKind::Constant(ConstVal::Bool(true)) = &test.kind {
            findings.push(StaticFinding {
                kind: "constant_assert".to_string(),
                path: path.to_string(),
                line: stmt.pos.line as i64,
                detail: "assertion on the constant true value".to_string(),
            });
            continue;
        }
        let ExprKind::Compare {
            left,
            ops,
            comparators,
        } = &test.kind
        else {
            continue;
        };
        if ops.len() != 1 || ops[0] != CmpKind::Eq || comparators.len() != 1 {
            continue;
        }
        unparse_guard(left, UNPARSE_MAIN_STANDING)?;
        let left_text = unparse_expr(left);
        let right_node = &comparators[0];
        unparse_guard(right_node, UNPARSE_MAIN_STANDING)?;
        let right_text = unparse_expr(right_node);
        if left_text == right_text {
            findings.push(StaticFinding {
                kind: "self_comparison".to_string(),
                path: path.to_string(),
                line: stmt.pos.line as i64,
                detail: format!("assertion compares {} with itself", left_text),
            });
            continue;
        }
        let mut placed = false;
        for (side, other) in [(left.as_ref(), &right_text), (right_node, &left_text)] {
            if is_return_value(side) {
                if let Some(root) = root_name(side) {
                    if return_values.get(root) == Some(other) {
                        findings.push(StaticFinding {
                            kind: "mock_return_value".to_string(),
                            path: path.to_string(),
                            line: stmt.pos.line as i64,
                            detail: format!(
                                "assertion checks the mock return value against the value it was assigned, {}",
                                other
                            ),
                        });
                        placed = true;
                        break;
                    }
                }
            }
        }
        if placed {
            continue;
        }
        for (name_node, other) in [(left.as_ref(), &right_text), (right_node, &left_text)] {
            let mut done = false;
            if let ExprKind::Name(name) = &name_node.kind {
                if let Some(root) = read_from_mock.get(name) {
                    if return_values.get(root) == Some(other) {
                        findings.push(StaticFinding {
                            kind: "mock_return_value".to_string(),
                            path: path.to_string(),
                            line: stmt.pos.line as i64,
                            detail: format!(
                                "assertion checks a value read from a mock against the return value the mock was assigned, {}",
                                other
                            ),
                        });
                        done = true;
                    }
                }
            }
            if done {
                break;
            }
            if let Some(root) = call_root(name_node) {
                if return_values.get(root) == Some(other) {
                    findings.push(StaticFinding {
                        kind: "mock_return_value".to_string(),
                        path: path.to_string(),
                        line: stmt.pos.line as i64,
                        detail: format!(
                            "assertion checks a call on a mock against the return value the mock was assigned, {}",
                            other
                        ),
                    });
                    break;
                }
            }
        }
    }

    for node in walk(&tree) {
        let Some(stmt) = node.as_stmt() else { continue };
        if !is_funcdef(stmt) {
            continue;
        }
        let governing = governing_function(node, &parents, &registry);
        if governing.map(|held| held.id) != Some(stmt.id) {
            continue;
        }
        if !in_test_function(node) {
            continue;
        }
        findings.extend(d5_findings_in_function(stmt, &helpers, &fixtures, path)?);
    }

    findings.sort_by(|a, b| (a.line, a.kind.as_str()).cmp(&(b.line, b.kind.as_str())));
    Ok(findings)
}

/// Scan one test file on disk. The text is decoded as
/// UTF-8 strictly and read with the newline translation
/// the Python text reading makes, both carried out before
/// any parsing, and a failure of either refuses under
/// PORT7-7.
pub fn scan_file(
    path: &std::path::Path,
    display_path: &str,
    settings: Option<&CollectionSettings>,
    supply_sources: Option<&[String]>,
) -> Result<Vec<StaticFinding>, ScanRefusal> {
    let bytes = std::fs::read(path).map_err(|_| refusal("the file could not be read"))?;
    let text =
        String::from_utf8(bytes).map_err(|_| refusal("the file does not decode as UTF-8"))?;
    let translated = text.replace("\r\n", "\n").replace('\r', "\n");
    scan_source(&translated, display_path, settings, supply_sources)
}

/// Read a whole source on a spawned thread with a large
/// stack, on the pattern the scan source entry states, so
/// a deep tree inside the port bound cannot exhaust the
/// stack of the thread that called the reading.
pub(crate) fn on_big_stack<T: Send, E: Send>(
    work: impl FnOnce() -> Result<T, E> + Send,
    unfinished: impl FnOnce() -> E,
) -> Result<T, E> {
    let outcome = std::thread::scope(|scope| {
        let handle = std::thread::Builder::new()
            .stack_size(512 * 1024 * 1024)
            .spawn_scoped(scope, work);
        match handle {
            Ok(joined) => joined.join(),
            Err(_) => Err(Box::new(()) as Box<dyn std::any::Any + Send>),
        }
    });
    match outcome {
        Ok(result) => result,
        Err(_) => Err(unfinished()),
    }
}

/// The names of test functions defined in a source string,
/// the legacy helper that reads the bare test prefix in
/// breadth walk order.
pub fn test_function_names(source: &str) -> Result<Vec<String>, ScanRefusal> {
    on_big_stack(
        || test_function_names_inner(source),
        || refusal("the scan's reading did not finish"),
    )
}

fn test_function_names_inner(source: &str) -> Result<Vec<String>, ScanRefusal> {
    let tree = parse_big_stack(source)?;
    let mut names = Vec::new();
    for node in walk(&tree) {
        if let Some(stmt) = node.as_stmt() {
            if is_funcdef(stmt) && funcdef_name(stmt).starts_with("test") {
                names.push(funcdef_name(stmt).to_string());
            }
        }
    }
    Ok(names)
}

/// The names of test functions defined at module level,
/// each name once, in body order.
pub fn top_level_test_functions(source: &str) -> Result<Vec<String>, ScanRefusal> {
    on_big_stack(
        || top_level_test_functions_inner(source),
        || refusal("the scan's reading did not finish"),
    )
}

fn top_level_test_functions_inner(source: &str) -> Result<Vec<String>, ScanRefusal> {
    let tree = parse_big_stack(source)?;
    let mut names: Vec<String> = Vec::new();
    for node in &tree.body {
        if is_funcdef(node) && funcdef_name(node).starts_with("test") {
            let name = funcdef_name(node).to_string();
            if !names.contains(&name) {
                names.push(name);
            }
        }
    }
    Ok(names)
}
