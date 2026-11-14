"""The fixed mutation panel for slice 1.

The panel has four kinds of mutation. Returning no value in place of
a return value. Inverting a comparison operator. Moving a numeric
constant by one, in both directions. Negating a boolean constant.
The panel is fixed on purpose so that two runs over the same project
apply the same mutants in the same order.
"""

from __future__ import annotations

import ast
import copy
from dataclasses import dataclass


@dataclass(frozen=True)
class Mutant:
    """One mutant of one source module."""

    kind: str
    line: int
    detail: str
    source: str

    def describe(self) -> str:
        return f"{self.kind} at line {self.line}, {self.detail}"

    def to_dict(self) -> dict:
        return {
            "kind": self.kind,
            "line": self.line,
            "detail": self.detail,
        }


_INVERSIONS = {
    ast.Eq: ast.NotEq,
    ast.NotEq: ast.Eq,
    ast.Lt: ast.GtE,
    ast.GtE: ast.Lt,
    ast.Gt: ast.LtE,
    ast.LtE: ast.Gt,
    ast.In: ast.NotIn,
    ast.NotIn: ast.In,
    ast.Is: ast.IsNot,
    ast.IsNot: ast.Is,
}


def _parents(tree: ast.AST) -> dict[int, ast.AST]:
    parents: dict[int, ast.AST] = {}
    for node in ast.walk(tree):
        for child in ast.iter_child_nodes(node):
            parents[id(child)] = node
    return parents


def _sites(tree: ast.AST) -> list[tuple[str, ast.AST, str]]:
    """Collect mutation sites in deterministic walk order."""
    sites: list[tuple[str, ast.AST, str]] = []
    for node in ast.walk(tree):
        if isinstance(node, ast.Return) and node.value is not None:
            if isinstance(node.value, ast.Constant) and node.value.value is None:
                continue
            sites.append(
                ("return_none", node, f"return value {ast.unparse(node.value)}")
            )
        elif isinstance(node, ast.Compare):
            for index, op in enumerate(node.ops):
                if type(op) in _INVERSIONS:
                    sites.append(
                        (
                            "compare_invert",
                            node,
                            f"comparison {ast.unparse(node)} operator {index}",
                        )
                    )
                    break
        elif isinstance(node, ast.Constant):
            if isinstance(node.value, bool):
                sites.append(
                    ("bool_negate", node, f"boolean constant {node.value}")
                )
            elif isinstance(node.value, (int, float)):
                sites.append(
                    ("constant_shift", node, f"numeric constant {node.value}")
                )
    return sites


def _apply(tree: ast.AST, kind: str, target: ast.AST) -> None:
    parents = _parents(tree)
    if kind == "return_none":
        assert isinstance(target, ast.Return)
        target.value = ast.Constant(value=None)
    elif kind == "compare_invert":
        assert isinstance(target, ast.Compare)
        for index, op in enumerate(target.ops):
            inverted = _INVERSIONS.get(type(op))
            if inverted is not None:
                target.ops[index] = inverted()
                break
    elif kind == "bool_negate":
        assert isinstance(target, ast.Constant)
        target.value = not target.value
    else:
        raise ValueError(f"not an in place mutation kind, {kind}")


def generate_mutants(source: str) -> list[Mutant]:
    """Generate every mutant in the fixed panel for one module."""
    base = ast.parse(source)
    mutants: list[Mutant] = []
    sites = _sites(base)
    # Index sites per kind and description so a fresh parse can find
    # the same node again in walk order.
    for position, (kind, node, detail) in enumerate(sites):
        if kind == "constant_shift":
            assert isinstance(node, ast.Constant)
            for direction, label in ((1, "plus one"), (-1, "minus one")):
                tree = ast.parse(source)
                fresh_sites = _sites(tree)
                _, fresh_node, _ = fresh_sites[position]
                assert isinstance(fresh_node, ast.Constant)
                fresh_node.value = fresh_node.value + direction
                mutants.append(
                    Mutant(
                        kind="constant_shift",
                        line=fresh_node.lineno,
                        detail=f"{detail} moved {label}",
                        source=ast.unparse(tree),
                    )
                )
            continue
        tree = ast.parse(source)
        fresh_sites = _sites(tree)
        _, fresh_node, _ = fresh_sites[position]
        _apply(tree, kind, fresh_node)
        mutants.append(
            Mutant(
                kind=kind,
                line=fresh_node.lineno,
                detail=detail,
                source=ast.unparse(tree),
            )
        )
    return mutants
