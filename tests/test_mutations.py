"""Tests for obligation C2, the fixed mutation panel."""

import ast

from ostensible.mutations import generate_mutants

PANEL_SOURCE = (
    "def check(x):\n"
    "    if x > 3:\n"
    "        return True\n"
    "    return x + 2\n"
)


def test_panel_applies_every_mutation_kind():
    mutants = generate_mutants(PANEL_SOURCE)
    kinds = {item.kind for item in mutants}
    assert kinds == {
        "return_none",
        "compare_invert",
        "constant_shift",
        "bool_negate",
    }


def test_panel_shifts_numeric_constants_in_both_directions():
    mutants = generate_mutants(PANEL_SOURCE)
    shift_details = [
        item.detail for item in mutants if item.kind == "constant_shift"
    ]
    assert any("plus one" in detail for detail in shift_details)
    assert any("minus one" in detail for detail in shift_details)


def test_every_mutant_parses_and_differs_from_the_original():
    original = ast.dump(ast.parse(PANEL_SOURCE))
    mutants = generate_mutants(PANEL_SOURCE)
    assert mutants
    for mutant in mutants:
        assert ast.dump(ast.parse(mutant.source)) != original
        assert mutant.line >= 1


def test_return_none_mutant_really_returns_none():
    mutants = generate_mutants("def f():\n    return 9\n")
    kinds = [item.kind for item in mutants]
    assert "return_none" in kinds
    mutant = mutants[kinds.index("return_none")]
    namespace: dict = {}
    exec(mutant.source, namespace)
    assert namespace["f"]() is None


def test_compare_invert_mutant_really_inverts():
    mutants = generate_mutants("def f(x):\n    return x > 3\n")
    mutant = next(item for item in mutants if item.kind == "compare_invert")
    namespace: dict = {}
    exec(mutant.source, namespace)
    assert namespace["f"](9) is False
    assert namespace["f"](1) is True


def test_bool_negate_mutant_really_negates():
    mutants = generate_mutants("def f():\n    return True\n")
    mutant = next(item for item in mutants if item.kind == "bool_negate")
    namespace: dict = {}
    exec(mutant.source, namespace)
    assert namespace["f"]() is False


def test_panel_is_deterministic():
    first = [(m.kind, m.line, m.source) for m in generate_mutants(PANEL_SOURCE)]
    second = [(m.kind, m.line, m.source) for m in generate_mutants(PANEL_SOURCE)]
    assert first == second
