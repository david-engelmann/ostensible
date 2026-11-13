# Groundwork for ostensible Round 16, port slice 3, the mutant generation

Prepared by the groundwork lane for the owner chat, Gate Truth line.
The subject is the mutant generation of ostensible, which asks
whether a test can fail. Round 15 stands closed at 15 of 100. This
file states what the landed Python does, on recorded evidence, so
the owner can write the Round 16 contract from it.

## 0. State verified, method, and counting

Remote state was verified through the GitHub API before any local
copy was trusted. Remote main of david-engelmann/ostensible stands
at 5b4c936d2baeb0210111b33d81f1a02e1415253d, the Round 15 merge
commit (Merge pull request #21, Port the static scan to Rust
whole), tree d804790d949ec2bcab9d1782a12204e84a5c2d3d. The source
read for this groundwork is the tarball of that head, so every file
and line reference below is a reference into that tree.

Method is the line's standing one. The landed Python is the
behavioural specification. Every behavioural claim in this file is
tied to a probe program that was run against the landed Python at
that head, or to a file and line of that tree. The probe programs
and their recorded outputs are banked beside this file under
probes/ (probe_panel.py, probe_limits.py, probe_refine.py and the
three .output.txt records). The probes assert as they record, so a
wrong claim would have failed the run; one probe did fail on its
first run, on the null byte exception type, and the record below
states the type the interpreter actually raised.

Counting is by the repo's calibrated method, stated at PORT7-11 of
CONTRACT.md: lines of code, with blank lines and comment lines
standing outside the count, and recordings counted as data and
standing outside both sides. Counted by that method at this head,
ostensible/mutations.py is 128 code lines (148 raw). A recount of
the whole tree at this merged head gives 1,872 Python code lines
in the package and its test suite together and 12,635 Rust code
lines in the crate and its tests, against the contract's stated
1,899 and 12,490, which were counted at the closure head before
the three post-closure CodeRabbit fix commits and the CI repair
landed on the same branch. The method is the contract's; the
figures are owed again at this round's closure, counted at its
final head, on the PORT7-11 pattern.

## 1. Census of the mutant generation

The mutant generation lives in exactly one module,
ostensible/mutations.py, 128 code lines (148 raw). Nothing else in
the package generates mutants, and nothing outside the package
imports it except the test suite (section 5).

Public surface, complete:

- generate_mutants(source: str) -> list[Mutant], at
  mutations.py:112. The one generation entry.
- Mutant, a frozen dataclass at mutations.py:18, with fields
  kind: str, line: int, detail: str, source: str, in that order.
  Its two methods are describe() at mutations.py:27, returning
  f"{kind} at line {line}, {detail}", and to_dict() at
  mutations.py:30, returning a dict of kind, line, detail in that
  key order. The source field is not in the dict.
- The package front door does not re-export either name.
  ostensible/__init__.py carries only a docstring and
  __version__.

Private machinery, complete: the inversion table _INVERSIONS at
mutations.py:37, _parents at mutations.py:50, _sites at
mutations.py:57, and _apply at mutations.py:93.

Dead machinery the port must not invent a need for, each item
verified by reading and by grep over the tree:

- The module imports copy (mutations.py:12) and never uses it.
- _apply computes parents = _parents(tree) at mutations.py:94
  and never reads it. No parent map is needed to apply any
  mutation in the panel.
- _apply raises ValueError(f"not an in place mutation kind,
  {kind}") for any kind it does not know, which is the
  constant_shift path, but generate_mutants never routes
  constant_shift through _apply, so the raise is unreachable
  from the public entry.

## 2. The panel, stated exactly

The panel is fixed, in the module's own words, "so that two runs
over the same project apply the same mutants in the same order"
(mutations.py:1-8). It has four kinds. Sites are collected by
_sites over ast.walk(tree), which is breadth first order, and
the mutant list stands in site order, with one adjacency rule
below. The site rules, per node, first match wins in the order
Return, Compare, Constant (mutations.py:57-90):

- return_none. A Return node whose value is not None and is not
  the constant None. The mutant sets the value to the constant
  None. Detail: f"return value {ast.unparse(node.value)}",
  spelled from the original tree. A bare return and an explicit
  return None earn no site (probe C4).
- compare_invert. A Compare node earns exactly one site, at its
  first operator whose type stands in _INVERSIONS, and the
  detail is f"comparison {ast.unparse(node)} operator {index}".
  The mutant replaces that one operator with its inverse and
  leaves any later operators of a chained comparison untouched
  (probe A2: a < b < c becomes a >= b < c). The table holds ten
  directed entries, five pairs: Eq with NotEq, Lt with GtE, Gt
  with LtE, In with NotIn, Is with IsNot. Those are all ten
  comparison operator types the grammar has, so every Compare
  node earns a site and the recorded index is 0 in every probe
  (family A1, all ten operators). The contract should state the
  panel covers every comparison operator type, so the port is
  not tempted to treat the index as a variable.
- bool_negate. A Constant whose value is a bool. The mutant
  negates it. Detail: f"boolean constant {node.value}", so
  "boolean constant True" or "boolean constant False". The
  bool test stands before the numeric test in _sites, which is
  what keeps True and False out of constant_shift even though
  bool is a subclass of int.
- constant_shift. A Constant whose value is an int or a float
  and not a bool. Each site earns TWO mutants, adjacent in the
  list, plus one first and minus one second, each applied to a
  fresh parse of the source. Detail: f"numeric constant
  {node.value}" for the site, then f"{detail} moved plus one"
  or f"{detail} moved minus one" per mutant. The shift is the
  value plus or minus the integer 1 in Python arithmetic, so
  an int shifts exactly at any size (probe C1: the 30-digit
  literal shifts digit-exact) and a float shifts in IEEE double
  arithmetic (probe C2).

What earns no site, each probed (family C): a complex constant
(1j earns nothing; in 1 + 2j only the int 1 is a site), a str or
bytes constant, None, Ellipsis, and any node kind other than
Return, Compare, Constant. A yield earns no site of its own,
though constants inside its value do (probe D1c). There are no
walk boundaries: the walk crosses function, class, and lambda
boundaries and reads constants in default arguments, subscript
indices, comprehensions, f-string values, and match literal
patterns (family C5; a match literal pattern constant is a
site, and its mutant rewrites the pattern). This is the exact
opposite boundary from the scan's walks of PORT7-6, and the
contract should say so, because a builder coming from slice 2
will expect boundaries that are not here.

Ordering, recorded not inferred (families B1 and D1b). The
order is breadth first walk order, not source order. In
"def f(): def g(): return 1 / return g" the outer return at
line 4 earns its mutant before the inner return at line 3,
because the outer Return stands shallower in the tree. In the
two-returns probe the line 4 return, standing in the function
body, precedes the line 3 return nested inside an if. Within
one statement the container precedes its contents: for
"return x > 3" the order is return_none, compare_invert, then
the two shifts of the constant 3 (probe A3).

Relocation. generate_mutants parses the source once for the
site list, then re-parses the source fresh for every mutant and
finds the same node again by the site's position in the fresh
site list (mutations.py:118-148). Behaviourally the port needs
only this: mutants are built one at a time from an unmutated
tree, and the node a mutant acts on is the node standing at the
same walk position. The crate's tree already carries stable
node ids and pywalk::node_registry (src/pywalk.rs:527) maps
them, so a clone-and-apply by id is the same behaviour with no
re-parse; the choice is the builder's, the behaviour is the
contract's.

The record a mutant carries:

- kind, one of the four words above.
- line, the line number of the target node in the freshly
  parsed tree: the Return's own line for return_none, the
  Compare's line, which is its left operand's first line
  (probe: a comparison broken across lines 2-3 records line
  2), and the constant's own line for the other two kinds
  (probe: constants on lines 1 and 2 of one parenthesised
  expression record their own lines).
- detail, spelled as above, always from the ORIGINAL tree's
  unparse or the constant's own value spelling, never from
  the mutated tree.
- source, ast.unparse of the WHOLE mutated module, not a
  fragment. Two consequences are probed and worth stating.
  The spelling is the interpreter's normalised spelling, so
  digit separators and base prefixes are already resolved
  (x = 0x10 shifts to x = 17 and x = 15, detail "numeric
  constant 16"). And a mutant can be a value-level no-op:
  both shifts of 1e308 parse back to the original tree,
  because 1e308 plus or minus 1 is 1e308 in double arithmetic
  (probe E2). The suite's own invariant that every mutant
  differs from the original holds for its panel source only;
  generate emits the no-op mutants anyway, and the recordings
  must carry them, not an invariant.

Identity. A mutant has no identifier. Within one call its
identity is its position in the returned list. Two mutants can
share (kind, line, detail) exactly: for "x = 1 + 1" the two
plus-one mutants carry the same kind, line, and detail and
differ only in source (probe E1), and generate never
deduplicates. This matters one slice later: the runner's file
level fold keys outcomes on (source_path, kind, line, detail)
at runner.py:174, so those two mutants fold into one file
level outcome. Round 16 must preserve both mutants and their
order whole; the fold is the execution slice's behaviour to
reproduce, not a reason to dedupe here.

Determinism. There is no randomness and no seed anywhere in
the module (grep: no random, no seed; imports are ast, copy,
dataclasses). Two calls in one process return identical lists
(probe B2), and the sha256 of the full recorded output over a
five-source corpus is identical under PYTHONHASHSEED 1, 42,
and the default (probe_limits digest, 7e217cc98fb979adf2aab
73cc5f7a3f1405aec24d7b60d9b592350fc3ddfcf96 in all three
runs), so no hash order reaches the output.
