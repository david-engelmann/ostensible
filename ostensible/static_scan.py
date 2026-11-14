"""Static fast path that flags test assertions which cannot fail.

The scan reads test files with the ast module and looks for four
shapes. A mock whose return value is assigned a value and later
asserted equal to that same value. The same shape where the mock
was configured inside a helper or a fixture in the same file and
bound to the test under another name. An assertion that compares
an expression with itself. An assertion on the constant true
value.

Slice 5 gates every shape on the assertion sitting inside a test
function, where what counts as a test function follows the
python_functions setting of the analysed project under G2. The
maps the scan builds stay file wide, because a helper is not a
test function and is never required to be one for the test that
binds its mock to be flagged.

Slice 6 retires the fixture parting on both paths. A definition
that carries a fixture decorator is not a test function for the
scan and never a selectable item for the selector, under the one
reading of the fixture spellings in this module, with the carve
outs H3 states for TestCase classes and fixture decorated
classes on the verdict path.

Slice 8 retires the duplicate name limit FIX-24 recorded. The
fixture condition of the scan gate is read on the effective
definition of a governing definition name, the last definition
of that name in the body that holds it, and the helper and
fixture maps clear both of their entries on a redefinition, so
the last definition alone decides what a name supplies.

Slice 9 widens the fixture form of D5 beyond the analysed file.
The fixture definitions of the project conftest files on the
chain for a test file join the fixture table the form reads,
under the precedence K2 states, so a test consuming a conftest
supplied fixture is flagged the way a test consuming a fixture
defined in its own file is flagged. A conftest contributes
fixtures and never helpers, and the supply reaches the scan
only, because the runs have always resolved it themselves.
"""

from __future__ import annotations

import ast
from dataclasses import dataclass
from pathlib import Path

from .settings import (
    DEFAULT_SETTINGS,
    CollectionSettings,
    name_matches,
)


@dataclass(frozen=True)
class StaticFinding:
    """One static finding in one test file."""

    kind: str
    path: str
    line: int
    detail: str

    def to_dict(self) -> dict:
        return {
            "kind": self.kind,
            "path": self.path,
            "line": self.line,
            "detail": self.detail,
        }


def _is_fixture_decorator(decorator: ast.expr) -> bool:
    """Return True when one decorator is a fixture decorator
    under H2. The spellings are the attribute fixture of the name
    pytest and the bare name fixture, each with or without a call
    and its arguments. The reading is by name and cannot see what
    an import binds, so an attribute of that name on another
    object is not one and a longer name containing the word is
    not one either."""
    target = decorator
    if isinstance(target, ast.Call):
        target = target.func
    if isinstance(target, ast.Name):
        return target.id == "fixture"
    if isinstance(target, ast.Attribute):
        return (
            target.attr == "fixture"
            and isinstance(target.value, ast.Name)
            and target.value.id == "pytest"
        )
    return False


def _is_fixture_definition(
    node: ast.FunctionDef | ast.AsyncFunctionDef | ast.ClassDef,
) -> bool:
    """Return True when a definition carries a fixture decorator.
    Any one fixture decorator in the list is enough."""
    return any(
        _is_fixture_decorator(item) for item in node.decorator_list
    )


def _descended_statement_lists(node: ast.AST):
    """Yield the statement lists a descended statement holds, in
    source order.

    This is the one enumeration of the descent both module walks
    consume, the definition stream of O1 and the fixture class
    reach of P1, so a statement kind one walk learns is a kind
    both learn. A conditional contributes its body and its else
    branch, a try contributes its body and each handler and its
    else and finally lists, a loop contributes its body and its
    else list, a with block contributes its body, and a match
    contributes the body of every case it holds. Any other
    statement, a function or class definition among them, holds
    no descended list here.
    """
    if isinstance(node, ast.If):
        yield node.body
        yield node.orelse
    elif isinstance(node, _TRY_TYPES):
        yield node.body
        for handler in node.handlers:
            yield handler.body
        yield node.orelse
        yield node.finalbody
    elif isinstance(node, (ast.For, ast.While)):
        yield node.body
        yield node.orelse
    elif isinstance(node, ast.With):
        yield node.body
    elif isinstance(node, _MATCH_TYPES):
        for case in node.cases:
            yield case.body


def _module_has_fixture_class(tree: ast.Module) -> bool:
    """Return True when a fixture decorated class stands in the
    reach of P1.

    Applying a fixture decorator to a class raises when the class
    statement runs, so a module holding such a class collects
    nothing under pytest and its fate is the whole file run, under
    FIX-23. The reach follows every statement list the one stream
    of O1 descends through, through the shared enumeration in
    _descended_statement_lists, and crosses into every class body
    standing in a statement list it is reading, the two combining
    recursively, because a descended statement list and a class
    body both execute at import. A class defined inside a function
    runs only when the function is called, so it does not poison
    the module, and the reach never crosses into a function.
    """

    def visit(statements) -> bool:
        for node in statements:
            if isinstance(node, ast.ClassDef):
                if _is_fixture_definition(node):
                    return True
                if visit(node.body):
                    return True
            else:
                for nested in _descended_statement_lists(node):
                    if visit(nested):
                        return True
        return False

    return visit(tree.body)


def _is_return_value(node: ast.AST) -> bool:
    return isinstance(node, ast.Attribute) and node.attr == "return_value"


def _root_name(node: ast.AST) -> str | None:
    while isinstance(node, ast.Attribute):
        node = node.value
    if isinstance(node, ast.Name):
        return node.id
    return None


def _call_root(node: ast.AST) -> str | None:
    if isinstance(node, ast.Call):
        return _root_name(node.func)
    return None


def _attribute_chain(node: ast.AST) -> tuple[str, str] | None:
    """Split a name or attribute chain into root name and the
    dotted attribute path beyond it. svc.fetch becomes the pair
    svc and fetch, and a bare name becomes the name and an empty
    path."""
    parts: list[str] = []
    while isinstance(node, ast.Attribute):
        parts.append(node.attr)
        node = node.value
    if isinstance(node, ast.Name):
        return node.id, ".".join(reversed(parts))
    return None


def _own_walk(
    func: ast.FunctionDef | ast.AsyncFunctionDef,
) -> list[ast.AST]:
    """The nodes of a definition's own statements, in source
    order. The walk never crosses into a nested definition,
    class, or lambda, because what a nested definition
    configures or returns belongs to that definition and not
    to the one holding it, which is the boundary _own_yields
    already states for yields."""

    found: list[ast.AST] = []

    def visit(node: ast.AST) -> None:
        for child in ast.iter_child_nodes(node):
            if isinstance(
                child,
                (
                    ast.FunctionDef,
                    ast.AsyncFunctionDef,
                    ast.ClassDef,
                    ast.Lambda,
                ),
            ):
                continue
            found.append(child)
            visit(child)

    visit(func)
    return found


def _configured_chains(
    func: ast.FunctionDef | ast.AsyncFunctionDef,
    before: tuple[int, int] | None = None,
) -> dict[tuple[str, str], set[str]]:
    """Map each configured call, named by root and attribute
    chain, to the distinct literals its return value was assigned
    inside one function. svc.fetch.return_value = 42 lands under
    the pair svc and fetch. When before is given as a line and
    column pair, only assignments standing before that position
    count, which is how the setup part of a yield fixture is read
    apart from its teardown under L1. The assignments are read
    from the definition's own statements under _own_walk, so a
    configuration inside a definition nested in the function
    belongs to the nested definition and not to this one."""
    configured: dict[tuple[str, str], set[str]] = {}
    for node in _own_walk(func):
        if isinstance(node, ast.Assign) and len(node.targets) == 1:
            if before is not None and (
                node.lineno,
                node.col_offset,
            ) >= before:
                continue
            target = node.targets[0]
            if _is_return_value(target):
                split = _attribute_chain(target.value)
                if split is not None:
                    configured.setdefault(split, set()).add(
                        ast.unparse(node.value)
                    )
    return configured


def _returned_names(
    func: ast.FunctionDef | ast.AsyncFunctionDef,
) -> list[str]:
    names: list[str] = []
    for node in _own_walk(func):
        if isinstance(node, ast.Return) and isinstance(
            node.value, ast.Name
        ):
            names.append(node.value.id)
    return names


def _own_yields(
    func: ast.FunctionDef | ast.AsyncFunctionDef,
) -> list[ast.Yield]:
    """The yield expressions that belong to a definition itself,
    in source order. A yield inside a definition nested in the
    fixture belongs to the nested definition and makes that
    definition a generator rather than the fixture, so the walk
    never crosses into a nested definition, class, or lambda.
    Under L1 the first of these yields is the provision of a
    yield fixture and everything after it is teardown side."""
    found: list[ast.Yield] = []

    def visit(node: ast.AST) -> None:
        for child in ast.iter_child_nodes(node):
            if isinstance(
                child,
                (
                    ast.FunctionDef,
                    ast.AsyncFunctionDef,
                    ast.ClassDef,
                    ast.Lambda,
                ),
            ):
                continue
            if isinstance(child, ast.Yield):
                found.append(child)
            visit(child)

    visit(func)
    found.sort(key=lambda item: (item.lineno, item.col_offset))
    return found


def _own_yield_froms(
    func: ast.FunctionDef | ast.AsyncFunctionDef,
) -> list[ast.YieldFrom]:
    """The yield from expressions that belong to a definition
    itself, in source order. The walk never crosses into a
    nested definition, class, or lambda, the same boundary
    _own_yields draws, because a delegation inside a nested
    definition is a provision of that definition and not of
    the fixture that holds it."""
    found: list[ast.YieldFrom] = []

    def visit(node: ast.AST) -> None:
        for child in ast.iter_child_nodes(node):
            if isinstance(
                child,
                (
                    ast.FunctionDef,
                    ast.AsyncFunctionDef,
                    ast.ClassDef,
                    ast.Lambda,
                ),
            ):
                continue
            if isinstance(child, ast.YieldFrom):
                found.append(child)
            visit(child)

    visit(func)
    found.sort(key=lambda item: (item.lineno, item.col_offset))
    return found


def _first_provision(
    func: ast.FunctionDef | ast.AsyncFunctionDef,
) -> ast.Yield | ast.YieldFrom | None:
    """The first provision point of a definition, or None.

    The provision points of a definition are its own yields
    and its own delegations together in source order, under
    M1. The first of them is the provision a test receives,
    and every later one provides nothing further under the
    rule L2 states for yields and M1 extends to delegations.
    """
    points: list[ast.Yield | ast.YieldFrom] = []
    points.extend(_own_yields(func))
    points.extend(_own_yield_froms(func))
    if not points:
        return None
    return min(
        points, key=lambda item: (item.lineno, item.col_offset)
    )


def _setup_table(
    func: ast.FunctionDef | ast.AsyncFunctionDef,
    first_yield: ast.Yield,
) -> dict[str, str]:
    """The configured table one definition provides at a yield.

    The configured chains are the assignments standing before
    the yield, and the table holds the chains whose root is
    the name the yield yields, under the unambiguity rule the
    fixture form of D5 has always used, so a chain configured
    with two distinct literals in the setup part is left out
    alone. This is the reading L1 gives a fixture that yields
    and M2 gives the delegate of a fixture that delegates.
    """
    configured = _configured_chains(
        func, before=(first_yield.lineno, first_yield.col_offset)
    )
    provided = (
        {first_yield.value.id}
        if isinstance(first_yield.value, ast.Name)
        else set()
    )
    table: dict[str, str] = {}
    for (root, chain), values in configured.items():
        if root in provided and len(values) == 1:
            table[chain] = next(iter(values))
    return table


def _delegation_name(node: ast.YieldFrom) -> str | None:
    """The delegate name a yield from expression calls, or None.

    A delegation is read under M1 when the expression it
    delegates to is a call whose function is a bare name. A
    delegation of any other shape, a bare name with no call or
    a call reached through an attribute, names no delegate
    this reading follows.
    """
    value = node.value
    if isinstance(value, ast.Call) and isinstance(
        value.func, ast.Name
    ):
        return value.func.id
    return None


def _follow_delegation(
    node: ast.FunctionDef | ast.AsyncFunctionDef,
    definitions: dict[
        str, ast.FunctionDef | ast.AsyncFunctionDef
    ],
    seen: frozenset[str],
) -> dict[str, str] | None:
    """The table a definition draws through its delegation.

    The caller guarantees the first provision point of node
    is a delegation. The delegate is the effective definition
    of the delegated name in the same module, and it is read
    by its own first provision point, so a delegate that
    yields is read from its setup part under M2 and a
    delegate that itself delegates is followed to the next
    link. None is returned when the chain cannot be followed
    to a provision this reading covers, under the refusals
    M3 states. A delegate carrying a fixture decorator is
    refused, because a delegating body calling it is refused
    by pytest for calling a fixture directly. An async
    delegate is refused, because the object it hands the
    delegation is not iterable. A delegate name the module
    does not define is refused, and a name already standing
    in the chain is refused, because a delegation cycle
    never finishes setting up and the reading refuses
    rather than following the chain forever.
    """
    first = _first_provision(node)
    if not isinstance(first, ast.YieldFrom):
        return None
    name = _delegation_name(first)
    if name is None or name in seen:
        return None
    target = definitions.get(name)
    if (
        target is None
        or not isinstance(target, ast.FunctionDef)
        or _is_fixture_definition(target)
    ):
        return None
    target_first = _first_provision(target)
    if target_first is None:
        return None
    if isinstance(target_first, ast.Yield):
        return _setup_table(target, target_first)
    return _follow_delegation(
        target, definitions, seen | {name}
    )


_TRY_TYPES: tuple[type, ...] = (ast.Try,) + (
    (ast.TryStar,) if hasattr(ast, "TryStar") else ()
)

_MATCH_TYPES: tuple[type, ...] = (
    (ast.Match,) if hasattr(ast, "Match") else ()
)


def _module_definitions(
    tree: ast.Module,
) -> dict[str, ast.FunctionDef | ast.AsyncFunctionDef]:
    """The effective function definitions of one module, under O1.

    The stream holds the definitions standing in the module
    body and the definitions standing in the statement lists a
    top level conditional or try or loop or with block or match
    statement holds, taken in source order, where a conditional
    contributes its body and its else branch and a try
    contributes its body and each handler and its else and
    finally lists and a loop contributes its body and its else
    list and a with block contributes its body and a match
    contributes the body of every case it holds, and the
    descent is recursive across all six statement kinds. The
    stream never crosses into a function definition or a class
    definition. The last definition of a name in the stream is
    the effective definition of that name, under the discipline
    J2 states for the definitions standing in one body, and no
    condition or iterable or manager or subject or guard is
    ever evaluated, under the limits O4 states.
    """
    bound: dict[str, ast.FunctionDef | ast.AsyncFunctionDef] = {}

    def visit(statements: list[ast.stmt]) -> None:
        for node in statements:
            if isinstance(
                node, (ast.FunctionDef, ast.AsyncFunctionDef)
            ):
                bound[node.name] = node
            else:
                for nested in _descended_statement_lists(node):
                    visit(nested)

    visit(tree.body)
    return bound


def _helper_and_fixture_maps(
    tree: ast.Module,
) -> tuple[dict[str, dict[str, str]], dict[str, dict[str, str]]]:
    """Configured values on the mocks helpers and fixtures return.

    Each helper or fixture maps to a table from attribute chain to
    the one literal configured for that call. A chain configured
    with two distinct literals inside one helper is ambiguous and
    that chain alone is left out. Other chains on the same mock
    still count. The reading walks the effective definitions of
    the module, the last definition of each name alone, so the
    last definition decides what the name supplies to either map,
    under FIX-25 for the fixture entry and J2 for the helper
    entry.

    A fixture definition that yields is read from its setup part
    under L1 instead. The configured chains are the assignments
    standing before the first yield of the definition and the
    provided name is the name that yield yields, so teardown
    assignments configure nothing and a later yield provides
    nothing further. An async fixture definition stands outside
    the yield reading under L4 and keeps the return reading.

    A fixture definition whose first provision point is a
    delegation is read through its delegate under M1 instead.
    The delegate is the effective definition of the delegated
    name in this same module, followed link by link under
    _follow_delegation, and the table is formed from the setup
    part of the delegate that finally yields. A delegation
    this reading does not follow places no table, and the
    return reading does not apply to a definition whose first
    provision point is a delegation, because a return standing
    in a generator body provides nothing a test receives.

    The definitions are read from the one module stream of
    O1, so a definition standing under a top level conditional
    or inside a top level try or loop or with block or match
    statement stands in the maps exactly as one standing
    directly in the module body does, and the delegation
    lookup reads its delegate from that same stream.
    """
    helpers: dict[str, dict[str, str]] = {}
    fixtures: dict[str, dict[str, str]] = {}
    definitions = _module_definitions(tree)
    for node in definitions.values():
        table: dict[str, str] | None = None
        if isinstance(node, ast.FunctionDef) and (
            _is_fixture_definition(node)
        ):
            first = _first_provision(node)
            if first is not None:
                if isinstance(first, ast.Yield):
                    table = _setup_table(node, first)
                else:
                    followed = _follow_delegation(
                        node,
                        definitions,
                        frozenset({node.name}),
                    )
                    table = (
                        followed if followed is not None else {}
                    )
        if table is None:
            configured = _configured_chains(node)
            provided = set(_returned_names(node))
            table = {}
            for (root, chain), values in configured.items():
                if root in provided and len(values) == 1:
                    table[chain] = next(iter(values))
        if not table:
            continue
        helpers[node.name] = table
        if _is_fixture_definition(node):
            fixtures[node.name] = table
    return helpers, fixtures


def _fixture_names(tree: ast.Module) -> set[str]:
    """The names one module defines as fixtures.

    A name counts when its effective definition, the last
    definition of the name in the module, carries a fixture
    decorator under H2. A name whose effective definition is
    plain registers no fixture, however an earlier definition
    of the name was decorated, which is the reading pytest
    gives the rebound name. The effective definition is read
    from the one module stream of O1, so a definition standing
    under a top level conditional or inside a top level try
    or loop or with block or match statement counts exactly
    as one standing directly in the module body does.
    """
    return {
        name
        for name, node in _module_definitions(tree).items()
        if _is_fixture_definition(node)
    }


def _merge_fixture_supply(
    tree: ast.Module,
    fixtures: dict[str, dict[str, str]],
    supply_sources: list[str],
) -> tuple[dict[str, dict[str, str]], set[int]]:
    """Fold the conftest fixture supply into a fixture table.

    The suppliers are read under the precedence of K2. The
    supply sources arrive in chain order from the project root
    down to the folder holding the analysed file, and the
    analysed file itself is the nearest supplier of all, so the
    fold runs from the farthest supplier to the nearest and the
    module table is applied last. A supplier whose effective
    definition of a name is a fixture clears the entry a farther
    supplier recorded before its own table is applied, so a
    nearer fixture that configures no table still shadows. A
    supplier that defines the name without a fixture decorator
    neither supplies nor clears, because pytest registers no
    fixture under that name there and its search continues
    outward. Only the fixture half of a supplier is read, so a
    conftest contributes fixtures and never helpers.

    The second value returned is the set of identities of the
    tables in the merged result that a conftest supplier
    contributed, so a finding drawn from one can name a conftest
    fixture as its source rather than a fixture in the same
    file.

    A supplier whose source does not parse contributes nothing
    and clears nothing, under FIX-26. The run inside the copy
    decides the verdict for such a project, and it refuses the
    conftest the way it refuses any broken file, so the scan
    reading past the broken supplier invents no verdict.

    A supplier holding a fixture decorated class in the reach
    of P1 is inert the same way, under P3, because a run refuses
    such a conftest outright and its fixtures never register,
    so folding its supply in would flag tests against fixtures
    no run could ever supply.
    """
    merged: dict[str, dict[str, str]] = {}
    suppliers = []
    for source in supply_sources:
        try:
            supply_tree = ast.parse(source)
        except SyntaxError:
            continue
        if _module_has_fixture_class(supply_tree):
            continue
        _supply_helpers, supply_fixtures = _helper_and_fixture_maps(
            supply_tree
        )
        suppliers.append(
            (_fixture_names(supply_tree), supply_fixtures, True)
        )
    suppliers.append((_fixture_names(tree), fixtures, False))
    supplied: set[int] = set()
    for names, table, from_supply in suppliers:
        for name in names:
            old = merged.pop(name, None)
            if old is not None:
                supplied.discard(id(old))
        merged.update(table)
        if from_supply:
            supplied.update(id(item) for item in table.values())
    return merged, supplied


def _statement_lists(node: ast.AST):
    """Yield the statement lists inside a node in source order."""
    for field in ("body", "orelse", "finalbody"):
        statements = getattr(node, field, None)
        if isinstance(statements, list) and statements and isinstance(
            statements[0], ast.stmt
        ):
            yield statements


def _d5_findings_in_function(
    func: ast.FunctionDef | ast.AsyncFunctionDef,
    helpers: dict[str, dict[str, str]],
    fixtures: dict[str, dict[str, str]],
    path: str,
    supplied: frozenset[int] = frozenset(),
) -> list[StaticFinding]:
    """Helper and fixture findings inside one function scope.

    Bindings live and die inside the function that creates them. A
    name bound to a helper or fixture mock keeps that binding
    until the name is assigned something else, and only a call
    whose attribute chain matches a configured chain can be
    flagged. A nested function is visited exactly once, with a
    copy of the enclosing bindings plus its own fixture arguments,
    the way a closure reads the enclosing scope, so an assertion
    inside it is reported once and never once per enclosing call.
    The supplied set holds the identities of the fixture tables a
    conftest supplier contributed under K1, and a binding drawn
    from one is tracked beside the binding itself so the finding
    can name a conftest fixture as its source.
    """
    findings: list[StaticFinding] = []
    env: dict[str, dict[str, str]] = {}
    origin: dict[str, bool] = {}
    for arg in func.args.args + func.args.kwonlyargs:
        if arg.arg in fixtures:
            env[arg.arg] = fixtures[arg.arg]
            origin[arg.arg] = id(fixtures[arg.arg]) in supplied

    def visit(
        statements: list[ast.stmt],
        env: dict[str, dict[str, str]],
        origin: dict[str, bool],
    ) -> None:
        for statement in statements:
            if isinstance(
                statement, (ast.FunctionDef, ast.AsyncFunctionDef)
            ):
                child = dict(env)
                child_origin = dict(origin)
                for arg in statement.args.args + statement.args.kwonlyargs:
                    if arg.arg in fixtures:
                        child[arg.arg] = fixtures[arg.arg]
                        child_origin[arg.arg] = (
                            id(fixtures[arg.arg]) in supplied
                        )
                visit(statement.body, child, child_origin)
                continue
            if isinstance(statement, ast.Assign) and len(
                statement.targets
            ) == 1 and isinstance(statement.targets[0], ast.Name):
                name = statement.targets[0].id
                value = statement.value
                if (
                    isinstance(value, ast.Call)
                    and isinstance(value.func, ast.Name)
                    and value.func.id in helpers
                ):
                    env[name] = helpers[value.func.id]
                    origin[name] = False
                else:
                    env.pop(name, None)
                    origin.pop(name, None)
            if isinstance(statement, ast.Assert):
                finding = _d5_assert_finding(
                    statement, env, path, origin
                )
                if finding is not None:
                    findings.append(finding)
            for nested in _statement_lists(statement):
                visit(nested, env, origin)

    visit(func.body, env, origin)
    return findings


def _d5_assert_finding(
    node: ast.Assert,
    env: dict[str, dict[str, str]],
    path: str,
    origin: dict[str, bool] | None = None,
) -> StaticFinding | None:
    test = node.test
    if not (
        isinstance(test, ast.Compare)
        and len(test.ops) == 1
        and isinstance(test.ops[0], ast.Eq)
        and len(test.comparators) == 1
    ):
        return None
    for side, other_node in (
        (test.left, test.comparators[0]),
        (test.comparators[0], test.left),
    ):
        if not isinstance(side, ast.Call):
            continue
        split = _attribute_chain(side.func)
        if split is None:
            continue
        root, chain = split
        table = env.get(root)
        if table is not None and table.get(chain) == ast.unparse(
            other_node
        ):
            source = (
                "a conftest fixture"
                if origin is not None and origin.get(root)
                else "a helper or fixture in the same file"
            )
            return StaticFinding(
                kind="mock_return_value",
                path=path,
                line=node.lineno,
                detail=(
                    "assertion checks a call on a mock against "
                    "the return value that call was assigned in "
                    f"{source}, {ast.unparse(other_node)}"
                ),
            )
    return None


def _parent_map(tree: ast.Module) -> dict[ast.AST, ast.AST]:
    """Map each node in a parsed file to its parent node."""
    parents: dict[ast.AST, ast.AST] = {}
    for node in ast.walk(tree):
        for child in ast.iter_child_nodes(node):
            parents[child] = node
    return parents


def _governing_function(
    node: ast.AST, parents: dict[ast.AST, ast.AST]
) -> ast.FunctionDef | ast.AsyncFunctionDef | None:
    """Return the function definition a node belongs to for the
    scan, under the rule of G2.

    The governing definition is the one on the climb from the node
    to the module that sits at module level or directly inside a
    module level class. A definition nested any deeper belongs to
    that governing definition, and a node outside every such
    definition has none.
    """
    current: ast.AST | None = node
    while current is not None:
        if isinstance(
            current, (ast.FunctionDef, ast.AsyncFunctionDef)
        ):
            parent = parents.get(current)
            if isinstance(parent, ast.Module):
                return current
            if isinstance(parent, ast.ClassDef) and isinstance(
                parents.get(parent), ast.Module
            ):
                return current
        current = parents.get(current)
    return None


def _bound_definitions(
    statements: list[ast.stmt],
) -> dict[str, ast.FunctionDef | ast.AsyncFunctionDef]:
    """Map each function name defined in one statement list to the
    definition the list binds for it. The last definition of a
    name in the list wins, which is the effective reading FIX-22
    gives the shaped count and J1 gives the scan gate."""
    bound: dict[str, ast.FunctionDef | ast.AsyncFunctionDef] = {}
    for node in statements:
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            bound[node.name] = node
    return bound


def scan_source(
    source: str,
    path: str,
    settings: CollectionSettings | None = None,
    supply_sources: list[str] | None = None,
) -> list[StaticFinding]:
    """Scan one test file source string and return its findings.

    The settings argument carries the collection naming settings
    of the analysed project under G1, the way the selector takes
    them, and the defaults are in force when it is not given. A
    finding is reported only for an assertion that sits inside a
    test function under G2, decided by the python_functions
    setting in force through the same matching rule the selector
    applies to that setting. The fixture condition of that gate
    is read on the effective definition of the governing
    definition name under J1, so a name the body rebinds to a
    fixture withholds the findings of every definition of the
    name, and a name whose last definition is plain gives them.
    The effective definition is the one the module stream of
    O1 gives for the name, so a redefinition standing under a
    top level conditional or inside a top level try or loop
    or with block or match statement decides the gate exactly
    as one standing directly in the module body does.

    The supply sources argument carries the source strings of
    the project conftest files on the chain for the file under
    K1, in chain order from the project root down to the folder
    holding the file. Their fixture definitions join the fixture
    table the fixture form of D5 reads, under the precedence of
    K2, and a scan made with no supply argument reads the
    analysed file alone, exactly as the earlier slices did.
    """
    if settings is None:
        settings = DEFAULT_SETTINGS
    tree = ast.parse(source)
    parents = _parent_map(tree)
    module_definitions = _module_definitions(tree)
    class_definitions = {
        id(node): _bound_definitions(node.body)
        for node in tree.body
        if isinstance(node, ast.ClassDef)
    }

    def in_test_function(node: ast.AST) -> bool:
        governing = _governing_function(node, parents)
        if governing is None:
            return False
        if not _function_name_matches(governing.name, settings):
            return False
        parent = parents.get(governing)
        if isinstance(parent, ast.Module):
            effective = module_definitions.get(
                governing.name, governing
            )
        elif isinstance(parent, ast.ClassDef):
            effective = class_definitions.get(id(parent), {}).get(
                governing.name, governing
            )
        else:
            effective = governing
        return not _is_fixture_definition(effective)

    findings: list[StaticFinding] = []

    # Map of root mock name to the source text of its assigned
    # return value, and a map of variable name to the root mock
    # it was read from.
    return_values: dict[str, str] = {}
    read_from_mock: dict[str, str] = {}
    helpers, fixtures = _helper_and_fixture_maps(tree)
    supplied: frozenset[int] = frozenset()
    if supply_sources:
        fixtures, supplied_ids = _merge_fixture_supply(
            tree, fixtures, supply_sources
        )
        supplied = frozenset(supplied_ids)

    for node in ast.walk(tree):
        if isinstance(node, ast.Assign) and len(node.targets) == 1:
            target = node.targets[0]
            if _is_return_value(target):
                root = _root_name(target)
                if root is not None:
                    return_values[root] = ast.unparse(node.value)
            elif isinstance(target, ast.Name):
                root = _call_root(node.value)
                if root is not None and root in return_values:
                    read_from_mock[target.id] = root

    for node in ast.walk(tree):
        if not isinstance(node, ast.Assert):
            continue
        if not in_test_function(node):
            continue
        test = node.test
        if isinstance(test, ast.Constant) and test.value is True:
            findings.append(
                StaticFinding(
                    kind="constant_assert",
                    path=path,
                    line=node.lineno,
                    detail="assertion on the constant true value",
                )
            )
            continue
        if not (
            isinstance(test, ast.Compare)
            and len(test.ops) == 1
            and isinstance(test.ops[0], ast.Eq)
            and len(test.comparators) == 1
        ):
            continue
        left = ast.unparse(test.left)
        right_node = test.comparators[0]
        right = ast.unparse(right_node)
        if left == right:
            findings.append(
                StaticFinding(
                    kind="self_comparison",
                    path=path,
                    line=node.lineno,
                    detail=f"assertion compares {left} with itself",
                )
            )
            continue
        # Direct form, assert mock.return_value == value.
        for side, other in ((test.left, right), (right_node, left)):
            if _is_return_value(side):
                root = _root_name(side)
                if root is not None and return_values.get(root) == other:
                    findings.append(
                        StaticFinding(
                            kind="mock_return_value",
                            path=path,
                            line=node.lineno,
                            detail=(
                                "assertion checks the mock return value "
                                f"against the value it was assigned, {other}"
                            ),
                        )
                    )
                    break
        else:
            # Indirect form, result = mock.call() then
            # assert result == value.
            for name_node, other in ((test.left, right), (right_node, left)):
                if isinstance(name_node, ast.Name):
                    root = read_from_mock.get(name_node.id)
                    if root is not None and return_values.get(root) == other:
                        findings.append(
                            StaticFinding(
                                kind="mock_return_value",
                                path=path,
                                line=node.lineno,
                                detail=(
                                    "assertion checks a value read from "
                                    "a mock against the return value the "
                                    f"mock was assigned, {other}"
                                ),
                            )
                        )
                        break
                root = _call_root(name_node)
                if root is not None and return_values.get(root) == other:
                    findings.append(
                        StaticFinding(
                            kind="mock_return_value",
                            path=path,
                            line=node.lineno,
                            detail=(
                                "assertion checks a call on a mock "
                                "against the return value the mock "
                                f"was assigned, {other}"
                            ),
                        )
                    )
                    break
    for node in ast.walk(tree):
        if (
            isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
            and _governing_function(node, parents) is node
            and in_test_function(node)
        ):
            findings.extend(
                _d5_findings_in_function(
                    node, helpers, fixtures, path, supplied
                )
            )
    findings.sort(key=lambda item: (item.line, item.kind))
    return findings


def scan_file(
    path: Path,
    display_path: str,
    settings: CollectionSettings | None = None,
    supply_sources: list[str] | None = None,
) -> list[StaticFinding]:
    """Scan one test file on disk, under the settings of G1 and
    the conftest fixture supply of K1."""
    return scan_source(
        path.read_text(encoding="utf-8"),
        display_path,
        settings,
        supply_sources,
    )


def test_function_names(source: str) -> list[str]:
    """Return the names of test functions defined in a source string.

    Legacy helper kept for its original callers. It reads the bare
    test prefix and does not follow the collection naming settings
    the scan and the selector have honoured since slice 4, and it
    is not used on the analysis path.
    """
    tree = ast.parse(source)
    names = []
    for node in ast.walk(tree):
        if isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef)):
            if node.name.startswith("test"):
                names.append(node.name)
    return names


def top_level_test_functions(source: str) -> list[str]:
    """Return the names of test functions defined at module level.

    Per function verdicts in slice 2 cover these functions. Methods
    inside classes are not selected on their own by this helper,
    and a name defined twice is returned once, under the single
    name pytest can run.

    Legacy helper kept for its original callers. It reads the bare
    test prefix and does not follow the collection naming settings
    the scan and the selector have honoured since slice 4, and it
    is not used on the analysis path.
    """
    tree = ast.parse(source)
    names: list[str] = []
    for node in tree.body:
        if (
            isinstance(node, (ast.FunctionDef, ast.AsyncFunctionDef))
            and node.name.startswith("test")
            and node.name not in names
        ):
            names.append(node.name)
    return names


def _unittest_bindings(tree):
    """Names in this module that refer to unittest and its TestCase.

    Returns a pair of the local names bound to the TestCase class
    itself and the local names bound to the unittest module.
    """
    testcase_names = set()
    module_names = set()
    for node in tree.body:
        if isinstance(node, ast.Import):
            for alias in node.names:
                if alias.name == "unittest":
                    module_names.add(alias.asname or "unittest")
        elif isinstance(node, ast.ImportFrom):
            if node.module in ("unittest", "unittest.case"):
                for alias in node.names:
                    if alias.name == "TestCase":
                        testcase_names.add(alias.asname or "TestCase")
    return testcase_names, module_names


def _is_testcase_class(node, classes, bindings, seen):
    """Return True when a class resolves to a unittest TestCase.

    Resolution follows the names the module itself binds. A base
    counts when it is a bound TestCase name, the TestCase attribute
    of a bound unittest module, or a local class that itself
    resolves to a TestCase. A name that merely ends in TestCase and
    resolves to nothing here does not count.
    """
    testcase_names, module_names = bindings
    if id(node) in seen:
        return False
    seen = seen | {id(node)}
    for base in node.bases:
        if isinstance(base, ast.Name):
            if base.id in testcase_names:
                return True
            local = classes.get(base.id)
            if local is not None and _is_testcase_class(
                local, classes, bindings, seen
            ):
                return True
        elif isinstance(base, ast.Attribute):
            if (
                base.attr == "TestCase"
                and isinstance(base.value, ast.Name)
                and base.value.id in module_names
            ):
                return True
    return False


def _sets_test_false(node):
    for member in node.body:
        if isinstance(member, ast.Assign):
            for target in member.targets:
                if (
                    isinstance(target, ast.Name)
                    and target.id == "__test__"
                    and isinstance(member.value, ast.Constant)
                    and member.value.value is False
                ):
                    return True
    return False


def _defines_init(node):
    return any(
        isinstance(member, (ast.FunctionDef, ast.AsyncFunctionDef))
        and member.name == "__init__"
        for member in node.body
    )


def _function_name_matches(name, settings):
    """Return True when a function or method name is an item name
    under the python_functions setting in force, or under the
    default test prefix when no such setting is in force."""
    if settings.python_functions is None:
        return name.startswith("test")
    return name_matches(name, settings.python_functions)


def _class_yields_methods(node, classes, bindings, settings):
    """Return True when the selector names methods of this class.

    The rules match what pytest collects, under the settings in
    force. A class that resolves to a unittest TestCase yields its
    methods under any name. A name based class yields its methods
    unless it defines an init of its own, which makes pytest
    refuse it. The name rule is the python_classes setting in
    force when one is set, with its patterns replacing the Test
    prefix entirely, and the Test prefix itself when no setting
    is in force. A class that sets __test__ to False yields
    nothing, whatever its name or bases. A class that carries a
    fixture decorator yields nothing either, under H3, because
    pytest refuses the shape when the decorator is applied.
    """
    if _is_fixture_definition(node):
        return False
    if _sets_test_false(node):
        return False
    if _is_testcase_class(node, classes, bindings, frozenset()):
        return True
    if _defines_init(node):
        return False
    if settings.python_classes is not None:
        return name_matches(node.name, settings.python_classes)
    return node.name.startswith("Test")


def _test_method_nodes(node, classes, seen):
    """The effective method definitions of a class, by name.

    The map reads the definition each name resolves to on this
    class, under H1. The last definition inside the class itself
    wins for a name, and a name the class does not define comes
    from its local base classes in base order. Own names come
    first in the order the class body first gives them, then
    inherited names in base order.
    """
    nodes = {}
    for member in node.body:
        if isinstance(
            member, (ast.FunctionDef, ast.AsyncFunctionDef)
        ):
            nodes[member.name] = member
    if id(node) in seen:
        return nodes
    seen = seen | {id(node)}
    for base in node.bases:
        if isinstance(base, ast.Name):
            local = classes.get(base.id)
            if local is not None:
                for name, member in _test_method_nodes(
                    local, classes, seen
                ).items():
                    if name not in nodes:
                        nodes[name] = member
    return nodes


def _test_method_names(node, classes, seen, settings, exclude_fixtures):
    """Test method names of a class, its own first, then inherited.

    A name counts as a test method name under the matching rule of
    _function_name_matches, read on the effective definition of
    the name. When exclude_fixtures holds, a name whose effective
    definition carries a fixture decorator is not a test method
    name, under H1. The caller clears the flag for a class that
    resolves to a unittest TestCase, under the carve out of H3.
    """
    names = []
    for name, member in _test_method_nodes(
        node, classes, seen
    ).items():
        if not _function_name_matches(name, settings):
            continue
        if exclude_fixtures and _is_fixture_definition(member):
            continue
        names.append(name)
    return names


def selectable_test_items(
    source: str, settings: CollectionSettings | None = None
) -> list[str]:
    """Return the selectable test items of a file in source order.

    An item is the selector pytest uses below the file. A top level
    test function is named by its own name. A test method inside a
    class the collection rules in force would collect is named by
    its class and its method joined the way a pytest node joins
    them, and methods inherited from a local base class are named
    under the class pytest collects them on. Each selector appears
    once, so a function or a method defined twice is judged once,
    under the single node pytest can run, while the same method
    name in two classes names two different items. A class name
    defined twice binds its last definition, exactly as the module
    does, and only that definition yields items. Classes nested
    inside other classes are not selected. The settings argument
    carries the collection naming settings of F1 to F3, and the
    defaults are in force when it is not given.

    Slice 6 adds the fixture reading of H1 to H3. A definition
    that carries a fixture decorator is never an item, read on
    the definition the module or the class binds for the name,
    except that a method of a class resolving to a unittest
    TestCase keeps its item under the carve out of H3. A module
    holding a fixture decorated class names no items at all
    under FIX-23, because pytest collects nothing from it and
    its fate is the whole file run.
    """
    if settings is None:
        settings = DEFAULT_SETTINGS
    tree = ast.parse(source)
    if _module_has_fixture_class(tree):
        return []
    bindings = _unittest_bindings(tree)
    classes = {}
    functions = {}
    for node in tree.body:
        if isinstance(node, ast.ClassDef):
            classes[node.name] = node
        elif isinstance(
            node, (ast.FunctionDef, ast.AsyncFunctionDef)
        ):
            functions[node.name] = node
    items: list[str] = []
    seen_functions: set[str] = set()
    for node in tree.body:
        if isinstance(
            node, (ast.FunctionDef, ast.AsyncFunctionDef)
        ):
            if node.name in seen_functions:
                continue
            seen_functions.add(node.name)
            bound = functions[node.name]
            if (
                _function_name_matches(node.name, settings)
                and not _is_fixture_definition(bound)
                and node.name not in items
            ):
                items.append(node.name)
        elif (
            isinstance(node, ast.ClassDef)
            and classes.get(node.name) is node
            and _class_yields_methods(node, classes, bindings, settings)
        ):
            exclude_fixtures = not _is_testcase_class(
                node, classes, bindings, frozenset()
            )
            for name in _test_method_names(
                node,
                classes,
                frozenset(),
                settings,
                exclude_fixtures,
            ):
                selector = f"{node.name}::{name}"
                if selector not in items:
                    items.append(selector)
    return items


def _shaped_name(name, settings):
    """Return True when a definition name reads as test shaped for
    the disclosure of FIX-12. A name is test shaped under the
    python_functions setting in force when one is set, its
    patterns replacing the test prefix entirely, and under the
    test prefix when no setting is in force, so a name the
    project's own settings have deselected never reads as an
    unjudged test shape."""
    if settings.python_functions is not None:
        return name_matches(name, settings.python_functions)
    return name.startswith("test")


def has_unselected_test_shapes(
    source: str,
    items: list[str],
    settings: CollectionSettings | None = None,
) -> bool:
    """Return True when a file holds test shaped definitions that
    the selector did not name, beside at least one named item.

    Test shaped means a top level function or a method in any class
    in the file, nested classes included, whose name is test shaped
    under _shaped_name. The count compares distinct runnable
    names, so inherited methods named under a subclass do not read
    as unselected. A definition that carries a fixture decorator
    does not count as a test shape, under H6, because it is not a
    test the verdict failed to cover, and the fixture reading
    follows the effective definition under FIX-22, so for a name
    defined more than once the last definition in its body decides
    whether the name counts. A module holding a fixture decorated
    class counts no shapes at all under FIX-23, because pytest
    collects nothing from such a module.
    """
    if settings is None:
        settings = DEFAULT_SETTINGS
    tree = ast.parse(source)
    if _module_has_fixture_class(tree):
        return False
    shaped = 0
    top_level = {}
    for node in tree.body:
        if isinstance(
            node, (ast.FunctionDef, ast.AsyncFunctionDef)
        ):
            top_level[node.name] = (
                not _is_fixture_definition(node)
                and _shaped_name(node.name, settings)
            )
    shaped += sum(1 for counts in top_level.values() if counts)
    for node in ast.walk(tree):
        if isinstance(node, ast.ClassDef):
            methods = {}
            for member in node.body:
                if isinstance(
                    member, (ast.FunctionDef, ast.AsyncFunctionDef)
                ):
                    methods[member.name] = (
                        not _is_fixture_definition(member)
                        and _shaped_name(member.name, settings)
                    )
            shaped += sum(
                1 for counts in methods.values() if counts
            )
    return shaped > len(items)