import sys
sys.path.insert(0, "/tmp/ostensible-r16/src")
from ostensible.mutations import generate_mutants

def ok_parens(d):
    try:
        generate_mutants("x = " + "(" * d + "1" + ")" * d + "\n"); return True
    except SyntaxError:
        return False
lo, hi = 200, 500
while lo + 1 < hi:
    mid = (lo + hi) // 2
    if ok_parens(mid): lo = mid
    else: hi = mid
print(f"parens: stands at {lo}, refuses at {hi}")

def ok_terms(n):
    try:
        generate_mutants("x = " + " + ".join(["a"] * n + ["1"]) + "\n"); return True
    except RecursionError:
        return False
lo, hi = 300, 330
while lo + 1 < hi:
    mid = (lo + hi) // 2
    if ok_terms(mid): lo = mid
    else: hi = mid
print(f"unparse chain: stands at {lo} terms, RecursionError at {hi} terms")

ms = generate_mutants("def f(a, b):\n    return (a\n        < b)\n")
print("multiline compare:", [(m.kind, m.line, m.detail) for m in ms])
ms = generate_mutants("x = (1 +\n     2)\n")
print("multiline constants:", [(m.kind, m.line, m.detail) for m in ms])
src = open("/tmp/ostensible-r16/src/ostensible/mutations.py").read()
from collections import Counter
ms = generate_mutants(src)
print("mutations.py over itself:", len(ms), dict(Counter(m.kind for m in ms)))
