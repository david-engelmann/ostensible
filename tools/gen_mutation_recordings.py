"""Generate the mutant generation conformance battery of PORT8-8.

Runs the landed Python of this repo (ostensible.mutations) over
the case corpus below and records each case's input and the
answers the Python gave into tests/conformance/mutation_cases.json.
No recording carries an answer written by hand: every mutant list
below is the output of generate_mutants over the case's source,
and every refusal is the classification of the exception the
Python raised, stated in the port's own wording as PORT8-5 names
the family.

Run this program against the landed Python BEFORE the retirement
of PORT8-7, from a checkout of the head this slice branches from,
with the interpreter the groundwork probed:

    python3 tools/gen_mutation_recordings.py

It asserts the interpreter it runs under (CPython 3.12), because
the spellings it records are that interpreter's spellings. After
the retirement the program stands as the provenance record of the
battery; re-running it needs the pre-retirement tree.

The f-string nesting families are measured rather than listed.
The cap, bracket, and spec chain cases stand at the interpreter's
fixed tokenizer bounds, one case on each side of each bound. The
recursion band cases stand at depths this run bisects out of the
interpreter in this process, the deepest depth the interpreter
answers and the first depth it refuses for the return value,
comparison, and module shapes, because the band of PORT8-5 is
the interpreter's stack speaking and its edges move with the
caller. The port prices its generate standings from the same
measured edges, as the closing entry of PORT8-9 states after
this fix.

One case family is transformed, and the transformation is stated
here rather than hidden. For the format spec lone surrogate
shape, the interpreter's unparse leaves the raw surrogate
character standing in the literal text of the format spec, and
the port spells it as two backslashes and its escape text, the
standing spelling the Round 15 amendment records. PORT8-5 as
repaired states the battery carries that case with the port's
spelling recorded as the expected answer, so the generator
rewrites the raw character to the port's spelling in that one
family's answers, and asserts the raw character stood in the
interpreter's answer before the rewrite. For every other family
the generator asserts no raw surrogate stands in any recorded
answer, so a spelling of that shape anywhere else fails the
generation instead of passing silently.
"""

from __future__ import annotations

import ast
import json
import platform
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))

from ostensible.mutations import generate_mutants  # noqa: E402

# The port's own wordings for the refusal family, as src/mutations.rs
# states them. The generator classifies the interpreter's exception
# into the family and records the port's wording, never the
# interpreter's error text.
PARSE_REFUSAL = "the source does not parse under the port's reading"
RECURSION_REFUSAL = "the source nests past the recursion standing of the port"

SURROGATES = [chr(point) for point in range(0xD800, 0xE000)]


def names_chain(terms: int) -> str:
    return " + ".join(["x"] * terms)


def parens(depth: int, inner: str) -> str:
    return "(" * depth + inner + ")" * depth


def fnest(depth: int, core: str) -> str:
    # A chain of f-strings, each standing in the replacement
    # field of the one before, quotes alternating the way a
    # written chain must.
    text = core
    for level in range(depth):
        quote = '"' if level % 2 == 0 else "'"
        text = "f" + quote + "{" + text + "}" + quote
    return text


def spec_fnest(depth: int, core: str) -> str:
    # A chain of f-strings, each standing in a field of the
    # format spec of the one before, so every level opens
    # two braces on the interpreter's one bracket account.
    text = core
    for level in range(depth):
        quote = '"' if level % 2 == 0 else "'"
        text = "f" + quote + "{x:{" + text + "}}" + quote
    return text


def measure_refusal_depth(make) -> int:
    # The first depth at which the interpreter's generation
    # refuses with RecursionError for one shape, bisected in
    # this run, at the same call depth the case recording
    # itself runs at.
    lo, hi = 120, 149
    while lo < hi:
        mid = (lo + hi) // 2
        try:
            generate_mutants(make(mid))
            lo = mid + 1
        except RecursionError:
            hi = mid
    return lo


def mixed_nesting(total: int, inner: str) -> str:
    # Alternate parentheses and brackets to the given total depth.
    opens = []
    closes = []
    pairs = [("(", ")"), ("[", "]")]
    for index in range(total):
        open_ch, close_ch = pairs[index % 2]
        opens.append(open_ch)
        closes.append(close_ch)
    return "".join(opens) + inner + "".join(reversed(closes))


CASES: list[tuple[str, str, str]] = [
    # The suite's own inputs, tests/test_mutations.py at this head.
    ("suite_panel", "suite",
     "def check(x):\n    if x > 3:\n        return True\n    return x + 2\n"),
    ("suite_return_none_exec", "suite", "def f():\n    return 9\n"),
    ("suite_compare_invert_exec", "suite", "def f(x):\n    return x > 3\n"),
    ("suite_bool_negate_exec", "suite", "def f():\n    return True\n"),
    # The operator panel, one case per directed inversion.
    ("panel_eq", "panel", "def f(a, b):\n    return a == b\n"),
    ("panel_not_eq", "panel", "def f(a, b):\n    return a != b\n"),
    ("panel_lt", "panel", "def f(a, b):\n    return a < b\n"),
    ("panel_gt_e", "panel", "def f(a, b):\n    return a >= b\n"),
    ("panel_gt", "panel", "def f(a, b):\n    return a > b\n"),
    ("panel_lt_e", "panel", "def f(a, b):\n    return a <= b\n"),
    ("panel_in", "panel", "def f(a, b):\n    return a in b\n"),
    ("panel_not_in", "panel", "def f(a, b):\n    return a not in b\n"),
    ("panel_is", "panel", "def f(a, b):\n    return a is b\n"),
    ("panel_is_not", "panel", "def f(a, b):\n    return a is not b\n"),
    ("panel_chained", "panel", "def f(a, b, c):\n    return a < b < c\n"),
    ("panel_chained_mixed", "panel", "def f(a, b, c, d):\n    return a < b == c > d\n"),
    ("panel_compare_multiline", "panel",
     "def f(x):\n    return (x\n            > 3)\n"),
    # Walk order discriminators: breadth first, not source order.
    ("order_outer_return_first", "order",
     "def f():\n    def g():\n        return 1\n    return g\n"),
    ("order_if_nested_return", "order",
     "def f(x):\n    if x:\n        return 1\n    return 2\n"),
    ("order_shallow_before_deep", "order", "x = [[[5]]]\ny = 3\n"),
    ("order_container_before_contents", "order", "def f(x):\n    return x > 3\n"),
    # Constant spellings and the float edges.
    ("const_underscore_int", "constants", "x = 1_000\n"),
    ("const_hex_int", "constants", "x = 0x10\n"),
    ("const_octal_binary", "constants", "x = 0o17\ny = 0b101\n"),
    ("const_big_int", "constants", "x = 123456789012345678901234567890\n"),
    ("const_big_int_negative", "constants", "x = -123456789012345678901234567890\n"),
    ("const_float_point_zero", "constants", "x = 1.0\ny = 1\n"),
    ("const_zero_shift_to_negative", "constants", "x = 0\n"),
    ("const_negative_int", "constants", "x = -5\n"),
    ("const_negative_float", "constants", "x = -1.5\n"),
    ("const_negative_zero_float", "constants", "x = -0.0\n"),
    ("const_float_1e308_noop", "constants", "x = 1e308\n"),
    ("const_float_1e999_inf", "constants", "x = 1e999\n"),
    ("const_float_neg_1e999", "constants", "x = -1e999\n"),
    ("const_float_5e_324", "constants", "x = 5e-324\n"),
    ("const_float_2_pow_53", "constants", "x = 9007199254740992.0\n"),
    ("const_complex_no_site", "constants", "x = 1j\ny = 1 + 2j\n"),
    ("const_str_bytes_none_ellipsis", "constants",
     "x = 'text'\ny = b'bytes'\nz = None\nw = ...\n"),
    ("const_bool_pair", "constants", "x = True\ny = False\n"),
    ("const_return_underscore_detail", "constants", "def f():\n    return 1_000\n"),
    # Positions the walk reaches.
    ("pos_default_argument", "positions", "def f(x=10):\n    return x\n"),
    ("pos_subscript_index", "positions", "x = items[2]\n"),
    ("pos_comprehension", "positions", "x = [n + 1 for n in items if n > 0]\n"),
    ("pos_fstring_value", "positions", "x = f'value {42} and {3.5}'\n"),
    ("pos_match_literal", "positions",
     "def f(cmd):\n    match cmd:\n        case 1:\n            return 'one'\n        case 'go':\n            return 2\n"),
    ("pos_match_singleton", "positions",
     "def f(cmd):\n    match cmd:\n        case True:\n            return 'yes'\n        case None:\n            return 'nothing'\n"),
    ("pos_class_body", "positions", "class C:\n    limit = 5\n"),
    ("pos_lambda_body", "positions", "f = lambda: 7\n"),
    ("pos_decorator_argument", "positions", "@register(3)\ndef f():\n    return 1\n"),
    ("pos_dict_and_set", "positions", "x = {1: 'a', 2: 'b'}\ny = {3, 4}\n"),
    ("pos_assert_message", "positions", "def f(x):\n    assert x, 'code 8'\n    return x\n"),
    ("pos_walrus", "positions", "def f():\n    if (n := 5) > 1:\n        return n\n    return 0\n"),
    ("pos_yield_value", "positions", "def f():\n    yield 5\n    return 6\n"),
    ("pos_annotation_default", "positions", "def f(x: int = 4) -> int:\n    return x\n"),
    ("pos_nested_function", "positions",
     "def f():\n    def g():\n        return 11\n    return g()\n"),
    # Duplicate identity shapes: same kind, line, and detail, kept whole.
    ("dup_one_plus_one", "duplicates", "x = 1 + 1\n"),
    ("dup_same_constant_twice", "duplicates", "x = 2\ny = 2\n"),
    ("dup_two_returns", "duplicates",
     "def f(x):\n    if x:\n        return 1\n    return 1\n"),
    # Detail spellings: quote choices, escapes, non ASCII standing as itself.
    ("detail_string_quotes", "details",
     "def f():\n    return 'it\\'s'\ndef g():\n    return 'say \"hi\"'\n"),
    ("detail_string_escapes", "details", "def f():\n    return 'a\\nb\\tc\\\\d\\x41'\n"),
    ("detail_non_ascii", "details", "def f():\n    return 'café ☕'\n"),
    ("detail_return_tuple", "details", "def f():\n    return (1, 'two', 3.0)\n"),
    ("detail_return_call", "details", "def f():\n    return len('abcd')\n"),
    ("detail_compare_strings", "details", "def f():\n    return 'a' == 'b'\n"),
    ("detail_surrogate_escape", "details", "def f():\n    return '\\ud800'\n"),
    # Non refusals stated in PORT8-5.
    ("shape_crlf", "shapes", "def f():\r\n    return 9\r\n"),
    ("shape_coding_cookie", "shapes", "# coding: utf-8\ndef f():\n    return 9\n"),
    ("shape_empty", "shapes", ""),
    ("shape_comment_only", "shapes", "# nothing here\n# at all\n"),
    ("shape_site_free", "shapes", "x = 'text'\nimport os\n"),
    ("shape_deep_names_standing", "shapes",
     "def f(x):\n    return " + names_chain(300) + "\n"),
    ("shape_parens_200_standing", "shapes", "x = " + parens(200, "1") + "\n"),
    # The refusal family of PORT8-5, recorded as refusals.
    ("refusal_syntax_error", "refusals", "def f(:\n"),
    ("refusal_null_byte", "refusals", "x = 1\x00\n"),
    ("refusal_leading_bom", "refusals", "\ufeffx = 1\n"),
    ("refusal_parens_201", "refusals", "x = " + parens(201, "1") + "\n"),
    ("refusal_mixed_nesting_201", "refusals", "x = " + mixed_nesting(201, "1") + "\n"),
    ("refusal_recursion_return_chain", "refusals",
     "def f(x):\n    return " + names_chain(400) + "\n"),
    ("refusal_recursion_assign_chain", "refusals",
     "y = " + " + ".join(["1"] * 400) + "\n"),
    ("refusal_recursion_deep_module_with_site", "refusals",
     "y = " + names_chain(400) + "\ndef f():\n    return 9\n"),
    # A deep chain that earns no site is never unparsed by the
    # Python at all: generation returns the empty list for it,
    # which is an answer and not a refusal.
    ("shape_deep_assign_names_empty", "shapes",
     "y = " + names_chain(400) + "\n"),
    # Line ends inside string literals, normalized by the
    # interpreter to one line feed in the value, on every
    # reading path.
    ("crlf_docstring", "shapes",
     "\"\"\"doc\r\nmore\"\"\"\ndef f():\n    return 9\n"),
    ("crlf_return_detail", "shapes",
     "def f():\n    return \"\"\"a\r\nb\"\"\"\n"),
    ("crlf_compare_detail", "shapes",
     "def f(x):\n    return \"\"\"a\r\nb\"\"\" == x\n"),
    ("crlf_raw_literal", "shapes",
     "def f():\n    return r\"\"\"a\r\nb\"\"\"\n"),
    ("crlf_bytes_literal", "shapes",
     "def f():\n    return b\"\"\"a\r\nb\"\"\"\n"),
    ("crlf_fstring_literal", "shapes",
     "def f():\n    return f\"\"\"a\r\nb\"\"\"\n"),
    ("crlf_two_pairs", "shapes",
     "def f():\n    return \"\"\"a\r\nb\r\nc\"\"\"\n"),
    # The fixed tokenizer bounds on f-string shapes, one
    # case standing on each side of each bound. The nested
    # f-string cap at one hundred and fifty, site free and
    # with a site.
    ("shape_fstring_cap_standing", "shapes",
     "x = " + fnest(149, "y") + "\n"),
    ("refusal_fstring_cap", "refusals",
     "x = " + fnest(150, "y") + "\n"),
    ("refusal_fstring_cap_with_site", "refusals",
     "x = " + fnest(150, "1") + "\n"),
    # The bracket ceiling over format spec braces: a chain
    # opening two hundred braces stands, two hundred and
    # two refuses at the tokenizer.
    ("shape_fstring_spec_braces_standing", "shapes",
     "x = " + spec_fnest(100, "y") + "\n"),
    ("refusal_fstring_spec_braces", "refusals",
     "x = " + spec_fnest(101, "y") + "\n"),
    # Fields chained through format specs inside one
    # string: three deep stands, four refuses.
    ("shape_spec_nest_standing", "shapes",
     "x = f\"{x:{y:{1}}}\"\n"),
    ("refusal_spec_nest", "refusals",
     "x = f\"{x:{y:{z:{w}}}}\"\n"),
]

# The format spec lone surrogate shape carries its own generation
# rule, stated in the module docstring above.
FORMAT_SPEC_CASE = (
    "shape_format_spec_surrogate", "shapes",
    "def f(x):\n    return f'{x:\\ud800}' + 2\n",
)


def tree_equal(original_dump: str, mutant_source: str) -> bool:
    return ast.dump(ast.parse(mutant_source)) == original_dump


def record_case(name: str, family: str, source: str, port_spelling: bool) -> dict:
    case: dict = {"family": family, "name": name, "source": source}
    try:
        mutants = generate_mutants(source)
    except SyntaxError:
        case["refusal"] = PARSE_REFUSAL
        return case
    except RecursionError:
        case["refusal"] = RECURSION_REFUSAL
        return case
    original_dump = ast.dump(ast.parse(source))
    recorded = []
    for mutant in mutants:
        entry = {
            "detail": mutant.detail,
            "kind": mutant.kind,
            "line": mutant.line,
            "source": mutant.source,
            # The format spec family's mutant sources carry a raw
            # surrogate the interpreter itself cannot re-parse, so
            # no tree reading stands for them; the case carries
            # tree_check false below and the field is not read.
            "tree_equal": False if port_spelling else tree_equal(original_dump, mutant.source),
        }
        recorded.append(entry)
    if port_spelling:
        raw = [mark for mark in SURROGATES if any(
            mark in entry["source"] or mark in entry["detail"] for entry in recorded)]
        assert raw, "the format spec case must carry a raw surrogate in the answer"
        for entry in recorded:
            for field in ("source", "detail"):
                for mark in SURROGATES:
                    entry[field] = entry[field].replace(mark, "\\\\u" + format(ord(mark), "04x"))
        case["tree_check"] = False
    else:
        for entry in recorded:
            for field in ("source", "detail"):
                assert not any(mark in entry[field] for mark in SURROGATES), (
                    f"raw surrogate in a recorded answer outside the format "
                    f"spec family: {name}"
                )
    case["mutants"] = recorded
    return case


def main() -> None:
    assert platform.python_implementation() == "CPython", "the spec interpreter is CPython"
    version = sys.version_info
    assert version[:2] == (3, 12), f"the groundwork probed CPython 3.12, not {version}"

    cases = [record_case(name, family, source, False) for name, family, source in CASES]
    name, family, source = FORMAT_SPEC_CASE
    cases.append(record_case(name, family, source, True))

    # The recursion band of the f-string shapes, at the
    # edges this run measures: for each shape the deepest
    # depth the interpreter answers and the first depth it
    # refuses, recorded in this process.
    for label, make in (
        ("return", lambda d: "def f():\n    return " + fnest(d, "1") + "\n"),
        ("compare", lambda d: "def f():\n    return " + fnest(d, "1") + " == 1\n"),
        ("module", lambda d: "x = " + fnest(d, "1") + "\ndef f():\n    return 9\n"),
    ):
        first_refusal = measure_refusal_depth(make)
        cases.append(record_case(
            f"shape_fstring_nest_{label}_deepest", "shapes",
            make(first_refusal - 1), False))
        cases.append(record_case(
            f"refusal_fstring_nest_{label}", "refusals",
            make(first_refusal), False))

    # The demo project and the module over itself, read from the
    # tree at generation time.
    for label, path in (
        ("demo_calc", ROOT / "examples" / "demo_project" / "calc.py"),
        ("demo_greeter", ROOT / "examples" / "demo_project" / "greeter.py"),
        ("self_mutations_module", ROOT / "ostensible" / "mutations.py"),
    ):
        text = path.read_text(encoding="utf-8")
        cases.append(record_case(label, "corpus", text, False))

    battery = {"cases": cases, "version": 1}
    text = json.dumps(battery, ensure_ascii=False, indent=1, sort_keys=True) + "\n"
    assert not any(mark in text for mark in SURROGATES), "no raw surrogate in the battery file"
    out = ROOT / "tests" / "conformance" / "mutation_cases.json"
    out.write_text(text, encoding="utf-8")
    print(f"recorded {len(cases)} cases to {out}")


if __name__ == "__main__":
    main()
