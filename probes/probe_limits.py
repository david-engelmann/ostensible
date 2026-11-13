"""Probe families B, E, F, M against the landed ostensible.mutations."""
import sys, ast, hashlib, json
sys.path.insert(0, "/tmp/ostensible-r16/src")

from ostensible.mutations import Mutant, generate_mutants

def triples(ms):
    return [(m.kind, m.line, m.detail, m.source) for m in ms]

print("== B1: walk order is breadth first, not source order ==")
src = "def f():\n    def g():\n        return 1\n    return g\n"
print("   ", [(k, l) for k, l, _, _ in triples(generate_mutants(src))])
src2 = "x = 2\ndef f():\n    return 1\n"
print("    module constant after a function:", [(k, l) for k, l, _, _ in triples(generate_mutants(src2))])

print("== B2: determinism in process ==")
panel = "def check(x):\n    if x > 3:\n        return True\n    return x + 2\n"
assert triples(generate_mutants(panel)) == triples(generate_mutants(panel))
print("    two calls identical: True")

print("== E1: duplicate (kind, line, detail) pairs are both emitted ==")
ms = generate_mutants("x = 1 + 1\n")
for t in triples(ms):
    print("   ", t)
plus = [m for m in ms if m.detail.endswith("plus one")]
assert len(plus) == 2 and plus[0].detail == plus[1].detail and plus[0].line == plus[1].line
assert plus[0].source != plus[1].source
print("    two plus-one mutants share (kind, line, detail); sources differ; no dedup")

print("== E2: a mutant can be a value level no-op ==")
orig = ast.dump(ast.parse("x = 1e308\n"))
ms = generate_mutants("x = 1e308\n")
same = [ast.dump(ast.parse(m.source)) == orig for m in ms]
print("    1e308 mutants parse back to the original dump:", same)
orig2 = ast.dump(ast.parse("x = 7\n"))
ms2 = generate_mutants("x = 7\n")
print("    7 mutants parse back to the original dump:", [ast.dump(ast.parse(m.source)) == orig2 for m in ms2])

print("== F1: syntax error propagates ==")
try:
    generate_mutants("def f(:\n")
    print("    NO RAISE")
except SyntaxError as e:
    print(f"    SyntaxError: {e.msg} (line {e.lineno})")

print("== F2: null byte ==")
try:
    generate_mutants("x = 1\x00\n")
    print("    NO RAISE")
except SyntaxError as e:
    print(f"    SyntaxError: {e.msg}")
except ValueError as e:
    print(f"    ValueError: {e}")

print("== F3: empty and inert sources ==")
for s in ["", "\n\n", "# only a comment\n", "pass\n", "x = 'str'\n"]:
    print(f"    {s!r}: {len(generate_mutants(s))} mutants")

print("== F4: bytes input ==")
try:
    ms = generate_mutants(b"x = 1\n")
    print("    bytes accepted:", [(m.kind, m.detail) for m in ms])
except Exception as e:
    print(f"    {type(e).__name__}: {e}")

print("== F5: nested parentheses depth ==")
for depth in [50, 100, 200, 500, 1000, 5000]:
    s = "x = " + "(" * depth + "1" + ")" * depth + "\n"
    try:
        ms = generate_mutants(s)
        print(f"    depth {depth}: {len(ms)} mutants")
    except Exception as e:
        print(f"    depth {depth}: {type(e).__name__}: {str(e)[:80]}")

print("== F6: unparse standing over a long name chain with one constant ==")
for terms in [100, 300, 330, 400, 900, 2000]:
    s = "x = " + " + ".join(["a"] * terms + ["1"]) + "\n"
    try:
        ms = generate_mutants(s)
        print(f"    terms {terms}: {len(ms)} mutants")
    except Exception as e:
        print(f"    terms {terms}: {type(e).__name__}: {str(e)[:80]}")

print("== F7: lone surrogate through an escape in a string constant ==")
ms = generate_mutants("def f():\n    return '\\ud800'\n")
for m in ms:
    print(f"    {m.kind} | {m.detail!r} | {m.source!r}")
print("    raw surrogate character in source text:")
try:
    ms = generate_mutants("x = '\ud800'\n")
    print("    accepted:", [(m.kind, m.detail) for m in ms])
except Exception as e:
    print(f"    {type(e).__name__}: {str(e)[:100]}")

print("== F8: BOM and CRLF ==")
try:
    generate_mutants("\ufeffx = 1\n")
    print("    BOM: NO RAISE")
except Exception as e:
    print(f"    BOM: {type(e).__name__}: {str(e)[:80]}")
ms = generate_mutants("x = 1\r\ny = 2\r\n")
print("    CRLF mutants:", [(m.kind, m.line, m.source) for m in ms])

print("== F9: source with a coding cookie ==")
ms = generate_mutants("# -*- coding: latin-1 -*-\nx = 1\n")
print("    cookie:", [(m.kind, m.line, m.detail) for m in ms])

print("== M: the Mutant record ==")
m = Mutant(kind="return_none", line=1, detail="return value 1", source="x")
print("    describe():", repr(m.describe()))
d = m.to_dict()
print("    to_dict():", d, "keys in order:", list(d.keys()))
import dataclasses
try:
    m.kind = "other"
    print("    NOT FROZEN")
except dataclasses.FrozenInstanceError:
    print("    frozen: attribute assignment raises FrozenInstanceError")
print("    equality by value:", Mutant("a", 1, "d", "s") == Mutant("a", 1, "d", "s"),
      "| hash equal:", hash(Mutant("a", 1, "d", "s")) == hash(Mutant("a", 1, "d", "s")))
print("    describe of a generated mutant:", repr(generate_mutants("def f():\n    return 9\n")[0].describe()))

print("== digest for the hash seed probe ==")
corpus = [panel, "x = 1 + 1\n", "def f(x):\n    if x > 3:\n        return True\n    return x + 2\n",
          "match cmd:\n    case 1:\n        pass\n", "x = 'café'\ny = 0x10\n"]
payload = json.dumps([triples(generate_mutants(s)) for s in corpus], ensure_ascii=True, sort_keys=True)
print("    sha256:", hashlib.sha256(payload.encode()).hexdigest())
