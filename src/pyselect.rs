//! The selector of PORT7-9, selectable_test_items and
//! has_unselected_test_shapes, reproducing the collection
//! rules the landed Python states: the unittest bindings of
//! the module body, the TestCase resolution through local
//! bases, the fixture readings, and the shaped count.

use crate::pyast::*;
use crate::pyscan::parse_for_select;
use crate::pyscan::{
    function_name_matches, is_fixture_definition, module_has_fixture_class, ScanRefusal,
};
use crate::settings::{name_matches, CollectionSettings};
use std::collections::{HashMap, HashSet};

fn is_funcdef(node: &Stmt) -> bool {
    matches!(node.kind, StmtKind::FuncDef { .. })
}

fn is_classdef(node: &Stmt) -> bool {
    matches!(node.kind, StmtKind::ClassDef { .. })
}

fn def_name(node: &Stmt) -> &str {
    match &node.kind {
        StmtKind::FuncDef { name, .. } => name,
        StmtKind::ClassDef { name, .. } => name,
        _ => unreachable!(),
    }
}

fn def_body(node: &Stmt) -> &[Stmt] {
    match &node.kind {
        StmtKind::FuncDef { body, .. } => body,
        StmtKind::ClassDef { body, .. } => body,
        _ => unreachable!(),
    }
}

fn def_decorators(node: &Stmt) -> &[Expr] {
    match &node.kind {
        StmtKind::FuncDef { decorators, .. } => decorators,
        StmtKind::ClassDef { decorators, .. } => decorators,
        _ => unreachable!(),
    }
}

fn class_bases(node: &Stmt) -> &[Expr] {
    match &node.kind {
        StmtKind::ClassDef { bases, .. } => bases,
        _ => &[],
    }
}

/// The names in this module that refer to unittest and its
/// TestCase, read from the module body's own import
/// statements alone.
fn unittest_bindings(module: &Module) -> (HashSet<String>, HashSet<String>) {
    let mut testcase_names: HashSet<String> = HashSet::new();
    let mut module_names: HashSet<String> = HashSet::new();
    for node in &module.body {
        match &node.kind {
            StmtKind::Import { names } => {
                for alias in names {
                    if alias.name == "unittest" {
                        module_names.insert(
                            alias
                                .asname
                                .clone()
                                .unwrap_or_else(|| "unittest".to_string()),
                        );
                    }
                }
            }
            StmtKind::ImportFrom { module, names, .. } => {
                if matches!(module.as_deref(), Some("unittest") | Some("unittest.case")) {
                    for alias in names {
                        if alias.name == "TestCase" {
                            testcase_names.insert(
                                alias
                                    .asname
                                    .clone()
                                    .unwrap_or_else(|| "TestCase".to_string()),
                            );
                        }
                    }
                }
            }
            _ => {}
        }
    }
    (testcase_names, module_names)
}

/// The selector chain standing of the interpreter,
/// measured against the landed Python at the default
/// recursion limit and stated in the amendment to PORT7-7
/// in CONTRACT.md. The TestCase resolution and the
/// inherited method reading each stand to a chain of 996
/// classes and refuse at 997.
const SELECTOR_CHAIN_STANDING: usize = 997;

/// The depth the interpreter's TestCase resolution reaches
/// from one class, in call frames counting the class's own
/// frame as one, with the resolution's short circuit and
/// cycle guard read as the Python reads them. The second
/// value states whether the resolution found a TestCase.
fn testcase_reach(
    node: &Stmt,
    classes: &HashMap<String, &Stmt>,
    bindings: &(HashSet<String>, HashSet<String>),
    seen: &HashSet<usize>,
) -> (usize, bool) {
    if seen.contains(&node.id) {
        return (1, false);
    }
    let mut seen = seen.clone();
    seen.insert(node.id);
    let (testcase_names, module_names) = bindings;
    let mut deepest = 1usize;
    for base in class_bases(node) {
        match &base.kind {
            ExprKind::Name(name) => {
                if testcase_names.contains(name) {
                    return (deepest, true);
                }
                if let Some(local) = classes.get(name) {
                    let (sub, resolved) = testcase_reach(local, classes, bindings, &seen);
                    deepest = deepest.max(1 + sub);
                    if resolved {
                        return (deepest, true);
                    }
                }
            }
            ExprKind::Attribute { value, attr } => {
                if attr == "TestCase" {
                    if let ExprKind::Name(name) = &value.kind {
                        if module_names.contains(name) {
                            return (deepest, true);
                        }
                    }
                }
            }
            _ => {}
        }
    }
    (deepest, false)
}

/// The depth the interpreter's inherited method reading
/// reaches from one class, in call frames counting the
/// class's own frame as one, following every local base
/// with the cycle guard the Python states.
fn method_path(node: &Stmt, classes: &HashMap<String, &Stmt>, seen: &HashSet<usize>) -> usize {
    if seen.contains(&node.id) {
        return 1;
    }
    let mut seen = seen.clone();
    seen.insert(node.id);
    let mut deepest = 1usize;
    for base in class_bases(node) {
        if let ExprKind::Name(name) = &base.kind {
            if let Some(local) = classes.get(name) {
                deepest = deepest.max(1 + method_path(local, classes, &seen));
            }
        }
    }
    deepest
}

/// Refuse when the selector's own recursions over one
/// module would pass the chain standing, the check the
/// selector makes before it reads any class.
fn selector_chain_guard(
    classes: &HashMap<String, &Stmt>,
    bindings: &(HashSet<String>, HashSet<String>),
    settings: &CollectionSettings,
) -> Result<(), ScanRefusal> {
    for node in classes.values() {
        if is_fixture_definition(def_decorators(node)) || sets_test_false(node) {
            continue;
        }
        let (reach, _) = testcase_reach(node, classes, bindings, &HashSet::new());
        if reach >= SELECTOR_CHAIN_STANDING {
            return Err(crate::pyscan::recursion_refusal());
        }
        if class_yields_methods(node, classes, bindings, settings)
            && method_path(node, classes, &HashSet::new()) >= SELECTOR_CHAIN_STANDING
        {
            return Err(crate::pyscan::recursion_refusal());
        }
    }
    Ok(())
}

/// Whether a class resolves to a unittest TestCase through
/// the names the module itself binds, recursively through
/// local classes with a cycle guard.
fn is_testcase_class(
    node: &Stmt,
    classes: &HashMap<String, &Stmt>,
    bindings: &(HashSet<String>, HashSet<String>),
    seen: &HashSet<usize>,
) -> bool {
    if seen.contains(&node.id) {
        return false;
    }
    let mut seen = seen.clone();
    seen.insert(node.id);
    let (testcase_names, module_names) = bindings;
    for base in class_bases(node) {
        match &base.kind {
            ExprKind::Name(name) => {
                if testcase_names.contains(name) {
                    return true;
                }
                if let Some(local) = classes.get(name) {
                    if is_testcase_class(local, classes, bindings, &seen) {
                        return true;
                    }
                }
            }
            ExprKind::Attribute { value, attr } => {
                if attr == "TestCase" {
                    if let ExprKind::Name(name) = &value.kind {
                        if module_names.contains(name) {
                            return true;
                        }
                    }
                }
            }
            _ => {}
        }
    }
    false
}

/// Whether a class body assigns the constant False to
/// __test__.
fn sets_test_false(node: &Stmt) -> bool {
    for member in def_body(node) {
        if let StmtKind::Assign { targets, value } = &member.kind {
            if let ExprKind::Constant(ConstVal::Bool(false)) = &value.kind {
                for target in targets {
                    if matches!(&target.kind, ExprKind::Name(name) if name == "__test__") {
                        return true;
                    }
                }
            }
        }
    }
    false
}

/// Whether a class defines an __init__ of its own.
fn defines_init(node: &Stmt) -> bool {
    def_body(node)
        .iter()
        .any(|member| is_funcdef(member) && def_name(member) == "__init__")
}

/// Whether the selector names methods of this class.
fn class_yields_methods(
    node: &Stmt,
    classes: &HashMap<String, &Stmt>,
    bindings: &(HashSet<String>, HashSet<String>),
    settings: &CollectionSettings,
) -> bool {
    if is_fixture_definition(def_decorators(node)) {
        return false;
    }
    if sets_test_false(node) {
        return false;
    }
    if is_testcase_class(node, classes, bindings, &HashSet::new()) {
        return true;
    }
    if defines_init(node) {
        return false;
    }
    match &settings.python_classes {
        Some(patterns) => name_matches(def_name(node), patterns),
        None => def_name(node).starts_with("Test"),
    }
}

/// The effective method definitions of a class, by name, in
/// the order the class body first gives them with inherited
/// names in base order after. Insertion follows the first
/// standing of a name and the last definition wins for it,
/// as the Python's map reads.
fn test_method_nodes<'a>(
    node: &'a Stmt,
    classes: &HashMap<String, &'a Stmt>,
    seen: &HashSet<usize>,
) -> Vec<(String, &'a Stmt)> {
    let mut nodes: Vec<(String, &Stmt)> = Vec::new();
    for member in def_body(node) {
        if is_funcdef(member) {
            let name = def_name(member).to_string();
            if let Some(entry) = nodes.iter_mut().find(|(held, _)| *held == name) {
                entry.1 = member;
            } else {
                nodes.push((name, member));
            }
        }
    }
    if seen.contains(&node.id) {
        return nodes;
    }
    let mut seen = seen.clone();
    seen.insert(node.id);
    for base in class_bases(node) {
        if let ExprKind::Name(name) = &base.kind {
            if let Some(local) = classes.get(name) {
                for (name, member) in test_method_nodes(local, classes, &seen) {
                    if !nodes.iter().any(|(held, _)| *held == name) {
                        nodes.push((name, member));
                    }
                }
            }
        }
    }
    nodes
}

/// Test method names of a class, its own first, then
/// inherited, the fixture exclusion read on the effective
/// definition where it holds.
fn test_method_names(
    node: &Stmt,
    classes: &HashMap<String, &Stmt>,
    settings: &CollectionSettings,
    exclude_fixtures: bool,
) -> Vec<String> {
    let mut names = Vec::new();
    for (name, member) in test_method_nodes(node, classes, &HashSet::new()) {
        if !function_name_matches(&name, settings) {
            continue;
        }
        if exclude_fixtures && is_fixture_definition(def_decorators(member)) {
            continue;
        }
        names.push(name);
    }
    names
}

fn default_settings() -> CollectionSettings {
    CollectionSettings {
        python_functions: None,
        python_classes: None,
        python_files: None,
    }
}

/// Return the selectable test items of a file in source
/// order.
pub fn selectable_test_items(
    source: &str,
    settings: Option<&CollectionSettings>,
) -> Result<Vec<String>, ScanRefusal> {
    crate::pyscan::on_big_stack(
        || selectable_test_items_inner(source, settings),
        || crate::pyscan::ScanRefusal::new("the scan's reading did not finish"),
    )
}

fn selectable_test_items_inner(
    source: &str,
    settings: Option<&CollectionSettings>,
) -> Result<Vec<String>, ScanRefusal> {
    let fallback = default_settings();
    let settings = settings.unwrap_or(&fallback);
    let tree = parse_for_select(source)?;
    if module_has_fixture_class(&tree) {
        return Ok(Vec::new());
    }
    let bindings = unittest_bindings(&tree);
    let mut classes: HashMap<String, &Stmt> = HashMap::new();
    let mut functions: HashMap<String, &Stmt> = HashMap::new();
    for node in &tree.body {
        if is_classdef(node) {
            classes.insert(def_name(node).to_string(), node);
        } else if is_funcdef(node) {
            functions.insert(def_name(node).to_string(), node);
        }
    }
    selector_chain_guard(&classes, &bindings, settings)?;
    let mut items: Vec<String> = Vec::new();
    let mut seen_functions: HashSet<String> = HashSet::new();
    for node in &tree.body {
        if is_funcdef(node) {
            let name = def_name(node);
            if seen_functions.contains(name) {
                continue;
            }
            seen_functions.insert(name.to_string());
            let bound = functions[name];
            if function_name_matches(name, settings)
                && !is_fixture_definition(def_decorators(bound))
                && !items.iter().any(|item| item == name)
            {
                items.push(name.to_string());
            }
        } else if is_classdef(node)
            && classes.get(def_name(node)).map(|held| held.id) == Some(node.id)
            && class_yields_methods(node, &classes, &bindings, settings)
        {
            let exclude_fixtures = !is_testcase_class(node, &classes, &bindings, &HashSet::new());
            for name in test_method_names(node, &classes, settings, exclude_fixtures) {
                let selector = format!("{}::{}", def_name(node), name);
                if !items.contains(&selector) {
                    items.push(selector);
                }
            }
        }
    }
    Ok(items)
}

/// Whether a definition name reads as test shaped under
/// the settings in force.
fn shaped_name(name: &str, settings: &CollectionSettings) -> bool {
    match &settings.python_functions {
        Some(patterns) => name_matches(name, patterns),
        None => name.starts_with("test"),
    }
}

/// Return True when a file holds test shaped definitions
/// the selector did not name, beside at least one named
/// item.
pub fn has_unselected_test_shapes(
    source: &str,
    items: &[String],
    settings: Option<&CollectionSettings>,
) -> Result<bool, ScanRefusal> {
    crate::pyscan::on_big_stack(
        || has_unselected_test_shapes_inner(source, items, settings),
        || crate::pyscan::ScanRefusal::new("the scan's reading did not finish"),
    )
}

fn has_unselected_test_shapes_inner(
    source: &str,
    items: &[String],
    settings: Option<&CollectionSettings>,
) -> Result<bool, ScanRefusal> {
    let fallback = default_settings();
    let settings = settings.unwrap_or(&fallback);
    let tree = parse_for_select(source)?;
    if module_has_fixture_class(&tree) {
        return Ok(false);
    }
    let mut shaped: usize = 0;
    let mut top_level: Vec<(String, bool)> = Vec::new();
    for node in &tree.body {
        if is_funcdef(node) {
            let counts = !is_fixture_definition(def_decorators(node))
                && shaped_name(def_name(node), settings);
            let name = def_name(node).to_string();
            if let Some(entry) = top_level.iter_mut().find(|(held, _)| *held == name) {
                entry.1 = counts;
            } else {
                top_level.push((name, counts));
            }
        }
    }
    shaped += top_level.iter().filter(|(_, counts)| *counts).count();
    for node in crate::pywalk::walk(&tree) {
        let Some(stmt) = node.as_stmt() else { continue };
        if !is_classdef(stmt) {
            continue;
        }
        let mut methods: Vec<(String, bool)> = Vec::new();
        for member in def_body(stmt) {
            if is_funcdef(member) {
                let counts = !is_fixture_definition(def_decorators(member))
                    && shaped_name(def_name(member), settings);
                let name = def_name(member).to_string();
                if let Some(entry) = methods.iter_mut().find(|(held, _)| *held == name) {
                    entry.1 = counts;
                } else {
                    methods.push((name, counts));
                }
            }
        }
        shaped += methods.iter().filter(|(_, counts)| *counts).count();
    }
    Ok(shaped > items.len())
}
