"""Probe families A, C, D, G against the landed ostensible.mutations.

Run from the extracted tree root so `ostensible` is the landed package.
Every printed line is a recorded answer of the landed Python.
"""
import sys
sys.path.insert(0, "/tmp/ostensible-r16/src")

from ostensible.mutations import generate_mutants

def show(label, mutants):
    print(f"{label}: {len(mutants)} mutants")
    for m in mutants:
        print(f"    {m.kind} | line {m.line} | {m.detail} | {m.source!r}")

def triples(mutants):
    return [(m.kind, m.line, m.detail) for m in mutants]

print("== A1: every comparison operator, in a return ==")
OPS = ["==", "!=", "<", "<=", ">", ">=", "in", "not in", "is", "is not"]
for sym in OPS:
    src = f"def f(a, b):\n    return a {sym} b\n"
    ms = generate_mutants(src)
    comp = [m for m in ms if m.kind == "compare_invert"]
    assert len(comp) == 1, (sym, triples(ms))
    print(f"    {sym!r}: detail={comp[0].detail!r} mutant={comp[0].source.splitlines()[1].strip()!r}")

print("== A2: chained comparison ==")
show("A2 'a < b < c'", generate_mutants("def f(a, b, c):\n    return a < b < c\n"))

print("== A3: compare inside a return value ==")
show("A3", generate_mutants("def f(x):\n    return x > 3\n"))

print("== A4: not / unary / boolop contexts ==")
show("A4a 'return not True'", generate_mutants("def f():\n    return not True\n"))
show("A4b 'return -5'", generate_mutants("def f():\n    return -5\n"))
show("A4c 'x = True and False'", generate_mutants("x = True and False\n"))

print("== C1: int spellings ==")
for lit in ["0", "7", "1_000", "0x10", "0o17", "0b101", "123456789012345678901234567890"]:
    ms = generate_mutants(f"x = {lit}\n")
    print(f"    {lit}: {triples(ms)} sources={[m.source for m in ms]}")

print("== C2: float shapes ==")
for lit in ["1.5", "1.0", "0.1", "1e308", "1e999", "5e-324"]:
    ms = generate_mutants(f"x = {lit}\n")
    print(f"    {lit}: {triples(ms)} sources={[m.source for m in ms]}")

print("== C3: complex constants ==")
show("C3a 'x = 1j'", generate_mutants("x = 1j\n"))
show("C3b 'x = 1 + 2j'", generate_mutants("x = 1 + 2j\n"))

print("== C4: constants that earn no site ==")
for src in ["x = 'abc'\n", "x = b'ab'\n", "x = None\n", "x = ...\n",
            "def f():\n    return None\n", "def f():\n    return\n",
            "def f():\n    pass\n"]:
    ms = generate_mutants(src)
    print(f"    {src!r}: {triples(ms)}")

print("== C5: constants in less obvious positions ==")
show("C5a default arg", generate_mutants("def f(x=5):\n    pass\n"))
show("C5b subscript index", generate_mutants("x = items[0]\n"))
show("C5c nested function", generate_mutants("def f():\n    def g():\n        return 1\n    return g\n"))
show("C5d lambda body", generate_mutants("f = lambda: 42\n"))
show("C5e class body", generate_mutants("class C:\n    limit = 10\n"))
show("C5f f-string value", generate_mutants("x = f'{1}'\n"))
show("C5g match literal pattern", generate_mutants("match cmd:\n    case 1:\n        pass\n    case 'go':\n        pass\n"))
show("C5h comprehension", generate_mutants("x = [i for i in range(3)]\n"))

print("== D1: return shapes ==")
show("D1a tuple", generate_mutants("def f():\n    return (1, 2)\n"))
show("D1b two returns", generate_mutants("def f(x):\n    if x:\n        return 1\n    return 2\n"))
show("D1c yield is not a return", generate_mutants("def f():\n    yield 1\n"))

print("== D2/G4: return detail normalization ==")
for expr in ["1_000", "0x10", "'abc'", "(1, 2)"]:
    ms = generate_mutants(f"def f():\n    return {expr}\n")
    rn = [m for m in ms if m.kind == "return_none"]
    print(f"    return {expr}: detail={rn[0].detail!r}")

print("== G1: string detail spellings through unparse ==")
for expr in ["'it\\'s'", '"say \\"hi\\""', "'café'", "'a\\nb'", "'\\U0001F600'"]:
    ms = generate_mutants(f"def f():\n    return {expr}\n")
    rn = [m for m in ms if m.kind == "return_none"]
    print(f"    return {expr}: detail={rn[0].detail!r} source={rn[0].source!r}")

print("== G2: numeric detail spellings ==")
for lit in ["1.0", "0.25", "100000000000000000000"]:
    ms = generate_mutants(f"x = {lit}\n")
    print(f"    {lit}: details={[m.detail for m in ms]}")

print("== H1: demo project corpus ==")
for path in ["examples/demo_project/calc.py", "examples/demo_project/greeter.py"]:
    text = open("/tmp/ostensible-r16/src/" + path).read()
    ms = generate_mutants(text)
    from collections import Counter
    print(f"    {path}: {len(ms)} mutants, kinds={dict(Counter(m.kind for m in ms))}")
    for t in triples(ms):
        print(f"        {t}")
