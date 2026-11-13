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
  mutations.py:111. The one generation entry.
- Mutant, a frozen dataclass at mutations.py:18, with fields
  kind: str, line: int, detail: str, source: str, in that order.
  Its two methods are describe() at mutations.py:26, returning
  f"{kind} at line {line}, {detail}", and to_dict() at
  mutations.py:29, returning a dict of kind, line, detail in that
  key order. The source field is not in the dict.
- The package front door does not re-export either name.
  ostensible/__init__.py carries only a docstring and
  __version__.

Private machinery, complete: the inversion table _INVERSIONS at
mutations.py:37, _parents at mutations.py:51, _sites at
mutations.py:59, and _apply at mutations.py:92.

Dead machinery the port must not invent a need for, each item
verified by reading and by grep over the tree:

- The module imports copy (mutations.py:13) and never uses it.
- _apply computes parents = _parents(tree) at mutations.py:93
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
Return, Compare, Constant (mutations.py:59-89):

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

## 3. The boundary: who consumes generation, and in what shape

One consumer exists in the package: ostensible/runner.py,
importing Mutant and generate_mutants at runner.py:67. It calls
generate_mutants in exactly two places, and the two places take
the same shape:

- The per item path, analyse_project at runner.py:639-648,
  builds a list of (relative_source_path, mutant) pairs for the
  paired source files of one test file.
- The whole file path, _whole_file_outcomes at runner.py:533,
  iterates the same call for files with no selectable items.

At both call sites the input is source_file.read_text(
encoding="utf-8") (runner.py:534 and the read inside the loop
at runner.py:643): a str decoded as UTF-8 from a .py file of the
analysed project. No caller passes bytes, though the entry
accepts them (probe F4: ast.parse accepts bytes and mutants
come back normally). The port's input contract is therefore a
decoded source text, the same shape scan_source already takes
under PORT7-1.

The output crosses four boundaries, each with its exact shape:

- Into the run: for each mutant the runner writes
  mutant.source + "\n" as the whole replacement text of the
  source file in a scratch copy (runner.py:539 and
  runner.py:650) and runs pytest there. So source is consumed
  as a complete module text, and the trailing newline is the
  runner's, not the generator's: the recorded mutant sources
  carry no trailing newline (ast.unparse never emits one).
- Into the outcome record: MutantOutcome(source_path=
  str(relative_source), mutant=mutant, killed=..., timed_out=
  ...) at runner.py:548 and runner.py:661. The mutant travels
  whole, by reference, into every outcome it earns.
- Into the report, JSON side: Mutant.to_dict() gives
  {kind, line, detail} and MutantOutcome.to_dict() adds the
  keys source (the source PATH, not the mutant's source text)
  and result (runner.py:91). This reading is already
  ported: model.rs MutantOutcome::to_dict reproduces it.
- Into the report, text side: report.py:16-24 renders each
  surviving outcome as "mutant that walked through -
  {source_path} - {kind} at line {line} - {detail}", a
  rendering of the same three fields plus the path. Also
  already ported under the report reading.

describe() has no caller anywhere in the package or the suite
(grep over the tree: no call site). It is public surface of the
record and nothing more; section 7 says what the port should
do with it.

Boundary state at this head, verified by running the suite:
the consumer is dead. import ostensible.runner raises
ModuleNotFoundError: No module named 'ostensible.static_scan'
(runner.py:74), the stated consequence of the Round 15
retirement under PORT7-9, and the whole Python suite except
tests/test_mutations.py errors at collection:
5 errors, tests/test_cli.py, test_functions.py, test_report.py,
test_runner.py, test_slice7.py, while test_mutations.py passes
7 of 7. Round 16 does not change that state and must not be
read as breaking it: after mutations.py retires, the same files
fail one import earlier, at the mutations import of runner.py:67
and tests/test_functions.py:7, for the same stated reason. The
execution machinery slice is the one that has to bridge the
runner to the crate; this slice only has to keep the boundary
shape the runner's two call sites state, which the recordings
pin.

## 4. Recorded probes: what each family established

The probe programs are banked under probes/ with their outputs.
Families and their headline answers, all at head 5b4c936d,
CPython 3.12.3, the interpreter of this box:

- A, the operator panel. All ten comparison operators invert
  exactly as the table at mutations.py:37 states, the detail
  index is 0 for every one, a chained comparison earns one
  site and inverts its first operator only, and a comparison
  inside a return earns both a return_none and a
  compare_invert mutant, container first.
- B, order and determinism. Breadth first walk order, proven
  by two source-order violations (section 2). In-process
  determinism exact, and hash seed independence proven by
  digest across PYTHONHASHSEED values.
- C, constants. Int spellings normalise through the value
  (1_000 records as 1000, 0x10 as 16), big ints shift exactly,
  floats shift in double arithmetic including the no-op and
  infinity shapes of section 6, complex earns no site, str,
  bytes, None, and Ellipsis earn no site, and constants earn
  sites in every position the walk reaches, including match
  literal patterns.
- D, returns. Every valued return earns a site, bare return
  and return None earn none, yield earns none of its own, and
  the return detail is the unparse of the value from the
  original tree (return 1_000 details as "return value 1000").
- E, identity. Duplicate (kind, line, detail) mutants are
  emitted whole with no deduplication, and value-level no-op
  mutants are emitted whole (section 2).
- F, refusals and standings. Section 6 states these in full.
- G, detail spellings. The details are unparse spellings:
  string values arrive with the interpreter's quote choice
  ('it\'s' details with double quotes, 'say "hi"' with
  single), non-ASCII text stands as itself (café, and the
  emoji of probe G1), escapes stand spelled (\n in the source
  details as the two characters backslash n), and numeric
  details are the value's own spelling (1.0 stays "1.0",
  distinct from the int 1, and 1e308 spells "1e+308").
- H, the demo corpus. examples/demo_project/calc.py earns 9
  mutants (3 return_none, 1 compare_invert, 1 bool_negate, 4
  constant_shift) and greeter.py earns 1 (return_none on
  'hello'), in walk order, recorded in full in the output.
  The module over itself: generate_mutants over the text of
  mutations.py earns 18 mutants (5 return_none, 8
  compare_invert, 1 bool_negate, 4 constant_shift), a useful
  scale anchor for the recordings corpus.
- M, the record. Mutant is frozen (attribute assignment
  raises FrozenInstanceError), equality and hash are by
  value, describe() spells "{kind} at line {line},
  {detail}", and to_dict() keys stand in the order kind,
  line, detail with source excluded.

## 5. Test census for the generation surface

Python suite at this head: six test files, 57 test functions by
def test_ count, plus tests/factories.py (123 code lines), the
shared project writer. The files and their standing:

- tests/test_mutations.py, 7 tests, 57 code lines. The only
  file that pins generate_mutants directly, under obligation
  C2, the fixed mutation panel, and the only Python test file
  that runs at this head (7 passed, verified). Its assertions:
  the panel applies every kind; shifts go in both directions;
  every mutant of its panel source parses and differs from the
  original and carries line >= 1; the return_none, compare_
  invert, and bool_negate mutants really do what they say,
  proven by exec of the mutant source; and the panel is
  deterministic across two calls.
- tests/test_functions.py, 6 tests. Constructs a Mutant by
  keyword by hand at line 18 (kind="return_none", line=1,
  detail="return value 1", source="") to build outcomes for
  the verdict readings. It pins the record's field names and
  keyword shape, not generation, and it cannot run at this
  head because it also imports the runner.
- tests/test_runner.py (5), tests/test_report.py (5),
  tests/test_cli.py (3), tests/test_slice7.py (21). These
  name mutants only through the runner and the report. They
  are specification for the execution and command line
  slices, not for this one, and none of them runs at this
  head.

Rust side at this head: 66 #[test] functions in tests/*.rs,
plus the recorded conformance batteries those tests drive
(the scan battery of PORT7 at 533 cases and the report and
settings batteries at 189 cases standing in
tests/conformance/). No recording anywhere in the repo covers
a generate_mutants answer today; the surface is unrecorded.

What the port must re-express, on the PORT7-10 pattern:

- tests/test_mutations.py retires with mutations.py at this
  slice's close, and its specification passes to recordings
  generated from the landed Python by a generator program,
  never written by hand. The recording shape the battery
  wants is one case per source text, carrying the source and
  the full mutant list in order, each mutant as kind, line,
  detail, source, compared field for field and byte for
  byte by the Rust tests offline.
- The corpus must go beyond the suite's one panel source, on
  the PORT7-10 precedent of a generated synthetic corpus
  beside the suite's inputs: the families of section 4 give
  the case list (operator panel, chained comparisons, walk
  order discriminators, constant spellings including the
  float edges and the big int, positions the walk reaches,
  the demo project's two files, and the module's own text).
- Two of the seven suite tests are exec proofs (the mutant
  really returns None, really inverts). A recording cannot
  exec, so the contract should re-express those as Rust tests
  over the generated mutant's tree: the return_none mutant's
  tree holds the constant None at that return, the
  compare_invert mutant's tree holds the inverted operator,
  which the crate can assert on its own parsed tree of the
  recorded mutant source. The parses-and-differs test
  re-expresses as a recorded answer plus the standing caveat
  of probe E2: differs holds for the panel source, and the
  corpus carries the no-op float shapes as recordings in
  their own right, where the recorded answer is that the
  mutant's tree EQUALS the original's.
- tests/test_functions.py's hand-built Mutant is not this
  slice's to retire; it goes when the execution slice takes
  the runner's tests. But its keyword construction is the
  standing proof that the record's four field names are
  load-bearing beyond generation itself.

## 6. Refusals and standings, measured

generate_mutants catches nothing. Every failure of the parse
or of the per mutant unparse propagates uncaught out of the
public entry, and under the standing ruling (PORT7-7 is the
template) each becomes a refusal of the port in its own
wording, at exactly the place the Python fails, never an
answer the Python cannot produce. The family, each member
probed (family F):

- Syntax error in the source: SyntaxError propagates from
  the first ast.parse (probe F1, "def f(:" raises SyntaxError
  "invalid syntax"). This is the common member: the runner
  feeds generate_mutants whatever source files the analysed
  project holds, and a source the grammar refuses is a source
  generation refuses whole.
- A null byte in the source text: SyntaxError, "source code
  string cannot contain null bytes" (probe F2; the probe's
  first run expected ValueError and failed, which is how the
  record comes to state the true type).
- A leading byte order mark: SyntaxError, "invalid
  non-printable character U+FEFF" (probe F8), the same
  standing PORT7-7 records for the scan entries.
- A raw lone surrogate standing in the source TEXT (not
  spelled as an escape inside a string): UnicodeEncodeError,
  "'utf-8' codec can't encode character ... surrogates not
  allowed" (probe F7), raised because the interpreter encodes
  the text to UTF-8 on its way into the parser. At the port's
  boundary this input cannot be presented at all, since the
  crate's source text is a Rust String; the contract should
  state that as the port's form of this refusal rather than
  inventing an error for it. The escape-spelled form is a
  different shape entirely and is NOT a refusal: the constant
  '\ud800' spelled as an escape in the source generates
  normally, and its return detail spells the value with the
  escape, "return value '\\ud800'" (probe F7), through the
  unparse reading the string constant amendment of Round 15
  already pins.
- Nesting past the grammar's own parenthesis limit:
  SyntaxError "too many nested parentheses", standing at 200
  nested parentheses and refusing at 201 (probe F5, boundary
  found by bisection), far inside the crate's 4,000 nesting
  bound of PORT7-2. The port's parser already refuses this
  shape as a grammar refusal of its own family, so the
  generation refusal inherits it unchanged.
- Recursion in the per mutant unparse: the interpreter's
  unparse is recursive, and over a left leaning chain of
  "a + a + ... + 1" the generation stands at 327 terms and
  dies of uncaught RecursionError at 328 terms (probe F6,
  boundary by bisection), measured in a fresh interpreter at
  the default recursion limit of 1,000 with the probe's own
  shallow stack. The standing is stack dependent in the
  Python: the same source generates or dies according to the
  caller's depth, which in the runner is the analysis stack.
  This is the one member the contract cannot copy as a
  number. On the PORT7-7 amendment's own precedent, where the
  scan's recursion standings were measured and named as
  members of its refusal family, the generation contract
  should measure and name the crate's standing for the
  generate path's unparse (the crate's unparse runs on the
  spawned large stack pattern of pyscan.rs:171 wherever the
  scan calls it, and the generate entry should state whether
  it takes the same pattern), refuse past it in the port's
  wording, and never abort the process. What the contract
  must not do is promise the interpreter's 327: that number
  is an accident of this probe's stack, stated here as
  evidence of the shape, not as a boundary to reproduce.

Two input shapes that are NOT refusals, recorded so the
contract does not refuse them by mistake: CRLF sources parse
and generate normally, with lines counted as the grammar
counts them and every mutant source spelled with \n endings
only (probe F8), and a coding cookie line is read as the
comment it is, leaving the following code at its true line
(probe F9). Empty, comment-only, and site-free sources return
the empty list, which is an answer, not a refusal (probe F3).

## 7. Slicing judgement

One round. The generation surface is one module of 128 code
lines with one public function and one public record, and
every piece of machinery the port needs already stands in the
crate, built and proven by Round 15: the parser
(pyparse::parse_module, src/pyparse.rs:35), the tree with its
constant taxonomy (pyast::ConstVal at src/pyast.rs:82, holding
Bool apart from Int(String) with integers as canonical decimal
text computed exactly at any size, which is precisely the
distinction the panel's bool-before-numeric rule and the exact
big int shift stand on), the breadth first walk and the node
registry (pywalk::walk at src/pywalk.rs:499 and
pywalk::node_registry at src/pywalk.rs:527, reproducing
ast.walk and standing in for the re-parse relocation), the ten
operator kinds (pyast::CmpKind, src/pyast.rs:297), the unparse
reading (pyunparse::unparse_module and unparse_expr), and the
float spelling the numeric details need (pyast::float_repr,
src/pyast.rs:237). The genuinely new machinery is small and
enumerable: the site collection over the walk, the four
applications on a cloned tree (the tree types are owned and
Clone, so clone-and-apply stands where Python re-parses), an
exact increment and decrement on canonical decimal text for
the int shift, the widening of the model record below, the
recordings battery and its generator, and the refusal family
of section 6. Nothing in the boundary forces a split: the
sole consumer's shape is already pinned by the ported model,
and the execution machinery and the command line keep their
own later slices under PORT-5 as narrowed. Splitting
generation across two rounds would leave a half panel standing
as a public surface with no consumer able to run, which is
the only outcome worse than the dead runner the repo already
carries honestly.

The exact boundary the slice should cut:

- IN: a generate_mutants entry in the crate taking a source
  text and returning the mutant list or the refusal of
  section 6; the site collection, the four applications, and
  the detail and source spellings of section 2; the Mutant
  record widened as below; the recordings battery and its
  generator under the PORT7-10 pattern; the retirement of
  ostensible/mutations.py and tests/test_mutations.py at the
  slice's close, with PORT-5 narrowed by amendment in the
  same commit, on the PORT7-9 precedent; the PORT7-11
  closing count, restated at the final head.
- The record: model.rs Mutant (src/model.rs:33) today carries
  kind, line, detail only, its own comment stating "Only the
  record is ported in this slice. Mutant generation stays
  Python only." Generation needs the fourth field, so the
  slice widens the Rust Mutant with source: String. The
  widening moves no reading that exists: the Python
  Mutant.to_dict() excludes source, the Rust to_dict at
  model.rs:39 already matches it, and aggregate_outcomes
  (model.rs:164) reads kind, line, detail only. Every Rust
  construction site of Mutant gains the field, which the
  census counts as the slice's model cost. describe() should
  port with the record, as a method spelling "{kind} at line
  {line}, {detail}" (probe M): it has no caller, but it is
  the record's one public reading the Python states, the
  legacy helpers of PORT7-1 ported on exactly that ground,
  and leaving it behind would make the Rust record a
  narrowing of the Python one by silence.
- OUT: runner.py, cli.py, report.py, settings.py,
  __main__.py, and __init__.py are untouched. No binary door
  is added: src/main.rs keeps the settings reading it carries
  today, on the PORT7 precedent that a slice adds library
  entries and the shell gains its behaviours with the command
  line slice. The Python that remains after this slice is the
  execution machinery and the command line with the package
  front doors, each named in PORT-5 as narrowed, and the
  closing entry owes that justification again under
  PORT7-11.

## 8. The three carried debts, dispositioned

B-1, the two repr copies. The copies stand at: (A)
pyvalue::py_str_repr (src/pyvalue.rs:209), taking a &str, with
its private pyvalue::is_printable (src/pyvalue.rs:256) driven
by a NON_PRINTABLE_RANGES table compiled into pyvalue.rs,
serving the PyValue world of the settings and report readings;
and (B) pyunparse::str_repr (src/pyunparse.rs:1455), taking
&[u32] codepoints, with is_printable_cp (src/pyunparse.rs:
1389) delegating to pydata::is_printable (src/pydata.rs:35)
driven by the generated positive table in
src/printable_ranges.txt (711 ranges). Whether the mutations
slice touches repr: yes, one copy of the two, as a consumer.
Every mutant source and every return and comparison detail is
an unparse spelling, and unparse spells its string constants
through copy B (probes G1 and F7 record copy B's behaviour on
the generate path: the quote choice, the escape spellings,
the surrogate spelled as its escape). Copy A never stands on
the generate path; nothing in mutations.py or its details
reads a PyValue. Disposition: the unification does not ride
Round 16 and must not gate it. The contract should pin copy B
as the generate path's one repr, which costs the slice
nothing because pyunparse is already its spelling engine, and
should name the hazard the pin prevents: a builder wiring the
details through pyvalue::py_repr for convenience would fork a
third behaviour, because copy A takes a &str and cannot carry
a lone surrogate codepoint at all, where the generate surface
can meet one in any string constant a return or comparison
unparses. The unification itself, one printable table and one
repr with pyvalue delegating to the codepoint reading,
belongs to a standalone standards fix or to the next slice
that opens pyvalue.rs, because it changes copy A's signature
and every caller in the settings and report world, a second
review surface that has nothing to do with mutant fidelity.

B-2, the D5 walk's arguments. The walk is the nested visit
function inside d5_findings_in_function at src/pyscan.rs:854.
At this head it already takes five parameters, because the
bundling the review asked for stands partly done inside
Round 15 itself: D5Context (src/pyscan.rs:825) carries
helpers and fixtures as one value and D5Scope
(src/pyscan.rs:833) carries env and origin as one value, and
both structs' own doc comments say they exist so the walk
takes the clump as one argument. What still travels unbundled
beside them is statements, path, and findings. Whether the
mutations slice touches it: no. Generation never enters
pyscan.rs; its walk is pywalk::walk, a different walk over a
different surface, and no file this slice opens contains the
D5 walk. Disposition: B-2 does not ride Round 16. Its
remainder is a pyscan-internal style fix with no behavioural
surface, owed to the next slice that opens pyscan.rs or to a
standalone standards commit, and it must not be charged
against a behavioural port's gate.

The workflow shaping debt. Verified at this head:
.github/workflows/ci.yml of ostensible carries one rust job
whose steps are checkout and cargo test alone. The line's
shaped standard, verified the same day against deadgate's
ci.yml at its head (same workflow shape, same push,
pull_request, and merge_group triggers), is three steps after
checkout. The exact delta that closes the debt is two steps
added after the test step:

      - name: Check the formatting
        run: cargo fmt --check
      - name: Run the linter
        run: cargo clippy --offline --all-targets

Disposition: a separate shaping PR, never a rider on the
behavioural branch, on the line's standing precedent
(deadgate Round 3's shaping followed as its own PR while its
branch was mid landing, and Round 15's own CI repair, the
missed Python job removal in 9065cc1, landed as a discrete
commit). Two notes for the owner: the push needs the
Workflows grant on the token, per the connector doc, and
fmt --check should first be run over the crate locally,
because a formatting debt anywhere in the 12,490 lines turns
the new step red on its first run; Round 15's closure record
already notes fmt flagging pre-existing debt files on this
repo, so the shaping PR may carry a fmt pass of its own and
should say so in its body rather than discover it in CI.
