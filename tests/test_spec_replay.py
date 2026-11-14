"""The fidelity pins of the Round 18 groundwork, GW-3 and GW-6.

The two recorded batteries the port proved against are
replayed here against the restored Python modules the
groundwork brought back, so the restoration stands or falls
as the specification those batteries were recorded from.
A recorded answer carrying a refused key stands as a
refusal the restored entry must raise. A case carrying the
port bound flag names a standing of the port itself, its
nesting bound, and its recorded answers stand as the
Python answers the generator recorded, so those cases
replay like any other here. The scan replay runs on a deep
stack thread under a raised recursion limit, the condition
the battery was recorded under, because the depth family
recurses as far as its sources nest, and the mutation
replay runs on a deep stack thread under a raised
recursion limit, because its deepest cases that carry
mutant answers recurse as far as their sources nest, and
the recursion refusals of its battery stand outside the
strict replay under the port standing rule above, so no
case's standing turns on the limit the replay runs at.
Each replay restores the limit it found when it finishes.
"""

from __future__ import annotations

import json
import re
import sys
import threading
from pathlib import Path

from ostensible.mutations import generate_mutants
from ostensible.settings import CollectionSettings
from ostensible.static_scan import (
    has_unselected_test_shapes,
    scan_file,
    scan_source,
    selectable_test_items,
)
from ostensible.static_scan import (
    test_function_names as scan_test_function_names,
)
from ostensible.static_scan import (
    top_level_test_functions as scan_top_level_test_functions,
)

ROOT = Path(__file__).resolve().parent.parent
CONFORMANCE = ROOT / "tests" / "conformance"

PORT_STANDING_REFUSAL = "the source nests past the recursion standing of the port"
# Six cases of the battery carry that refusal text. Five of
# them refuse in the Python as well, at its own recursion
# ceiling, and one returns the mutants the Python returns,
# the port refusing by rule where the Python answers. All
# six stand outside the strict replay, because the answer
# recorded for them is the port's standing and not a
# Python answer.


def load(name: str) -> dict:
    return json.loads((CONFORMANCE / name).read_text(encoding="utf-8"))


def normalize_surrogates(text: str) -> str:
    """Spell a lone surrogate codepoint the way the recording does.

    The one battery case carrying a lone surrogate in a
    mutant source records that source with the surrogate
    standing as doubled escape text, where the live reading
    carries the codepoint itself. The comparison is made on
    the decoded values, so the live side is spelled the
    same way before it is compared.
    """
    return re.sub(
        r"[\ud800-\udfff]",
        lambda match: "\\\\u%04x" % ord(match.group(0)),
        text,
    )


def build_settings(record):
    if record is None:
        return None

    def words(key):
        value = record.get(key)
        if value is None:
            return None
        return tuple(value)

    return CollectionSettings(
        python_functions=words("python_functions"),
        python_classes=words("python_classes"),
        python_files=words("python_files"),
    )


def supply_of(case):
    supply = case.get("supply")
    if supply is None:
        return None
    return list(supply)


def findings_tuples(findings):
    return [(f.kind, f.path, f.line, f.detail) for f in findings]


def recorded_findings(record):
    return [
        (f["kind"], f["path"], f["line"], f["detail"]) for f in record["findings"]
    ]


def is_refused(record) -> bool:
    return isinstance(record, dict) and "refused" in record


def test_restored_mutation_generation_replays_the_recorded_battery():
    previous_limit = sys.getrecursionlimit()
    sys.setrecursionlimit(10000)
    threading.stack_size(512 * 1024 * 1024)
    outcome: list[BaseException] = []

    def run() -> None:
        try:
            replay_mutation_battery()
        except BaseException as error:  # noqa: BLE001, the thread carries it back
            outcome.append(error)

    worker = threading.Thread(target=run)
    worker.start()
    worker.join()
    sys.setrecursionlimit(previous_limit)
    if outcome:
        raise outcome[0]


def replay_mutation_battery() -> None:
    recording = load("mutation_cases.json")
    cases = recording["cases"]
    checked = 0
    for case in cases:
        if "refusal" in case:
            if case["refusal"] == PORT_STANDING_REFUSAL:
                continue
            try:
                generate_mutants(case["source"])
            except Exception:
                checked += 1
                continue
            raise AssertionError(f"case {case['name']} must refuse and did not")
        mutants = generate_mutants(case["source"])
        mine = [
            {
                "kind": m.kind,
                "line": m.line,
                "detail": normalize_surrogates(m.detail),
                "source": normalize_surrogates(m.source),
            }
            for m in mutants
        ]
        want = [
            {
                "kind": w["kind"],
                "line": w["line"],
                "detail": w["detail"],
                "source": w["source"],
            }
            for w in case["mutants"]
        ]
        assert mine == want, f"case {case['name']} diverges"
        checked += 1
    assert checked == 99, f"the battery replays 99 cases, replayed {checked}"


def replay_scan_battery() -> None:
    recording = load("scan_cases.json")
    sources = recording["sources"]
    cases = recording["cases"]
    checked = 0
    for index, case in enumerate(cases):
        context = f"case {index} of family {case['family']}"
        source = sources[case["source"]]["text"]
        settings = build_settings(case.get("settings"))
        if "scan" in case:
            expected = case["scan"]
            if is_refused(expected):
                try:
                    scan_source(source, case["path"], settings, supply_of(case))
                except Exception:
                    pass
                else:
                    raise AssertionError(f"{context}: scan must refuse")
            else:
                findings = scan_source(
                    source, case["path"], settings, supply_of(case)
                )
                assert findings_tuples(findings) == recorded_findings(expected), (
                    f"{context}: findings diverge"
                )
        if "items" in case:
            expected = case["items"]
            if is_refused(expected):
                try:
                    selectable_test_items(source, settings)
                except Exception:
                    pass
                else:
                    raise AssertionError(f"{context}: selector must refuse")
            else:
                assert selectable_test_items(source, settings) == expected[
                    "items"
                ], f"{context}: items diverge"
        if "shapes" in case:
            expected = case["shapes"]
            if is_refused(expected):
                try:
                    has_unselected_test_shapes(
                        source, list(expected.get("items", [])), settings
                    )
                except Exception:
                    pass
                else:
                    raise AssertionError(f"{context}: shapes must refuse")
            else:
                assert (
                    has_unselected_test_shapes(
                        source, list(expected["items"]), settings
                    )
                    == expected["answer"]
                ), f"{context}: shapes answer diverges"
        if "names" in case:
            expected = case["names"]
            if is_refused(expected):
                try:
                    scan_test_function_names(source)
                except Exception:
                    pass
                else:
                    raise AssertionError(f"{context}: names must refuse")
            else:
                assert scan_test_function_names(source) == expected["names"], (
                    f"{context}: names diverge"
                )
        if "top" in case:
            expected = case["top"]
            if is_refused(expected):
                try:
                    scan_top_level_test_functions(source)
                except Exception:
                    pass
                else:
                    raise AssertionError(f"{context}: top level must refuse")
            else:
                assert scan_top_level_test_functions(source) == expected[
                    "names"
                ], f"{context}: top level diverges"
        checked += 1
    assert checked == 533, f"the battery carries 533 cases, replayed {checked}"
    file_cases = recording["file_cases"]
    import tempfile

    scratch = Path(tempfile.mkdtemp(prefix="ostensible-spec-replay-"))
    file_checked = 0
    for index, case in enumerate(file_cases):
        context = f"file case {index} of family {case['family']}"
        path = scratch / f"case_{index}.py"
        path.write_bytes(bytes(case["bytes"]))
        settings = build_settings(case.get("settings"))
        expected = case["scan_file"]
        if is_refused(expected):
            try:
                scan_file(path, case["display"], settings, supply_of(case))
            except Exception:
                pass
            else:
                raise AssertionError(f"{context}: scan_file must refuse")
        else:
            findings = scan_file(path, case["display"], settings, supply_of(case))
            assert findings_tuples(findings) == recorded_findings(expected), (
                f"{context}: findings diverge"
            )
        file_checked += 1
    assert file_checked == 45, (
        f"the battery carries 45 file cases, replayed {file_checked}"
    )


def test_restored_scan_replays_the_recorded_battery():
    previous_limit = sys.getrecursionlimit()
    sys.setrecursionlimit(100000)
    threading.stack_size(512 * 1024 * 1024)
    outcome: list[BaseException] = []

    def run() -> None:
        try:
            replay_scan_battery()
        except BaseException as error:  # noqa: BLE001, the thread carries it back
            outcome.append(error)

    worker = threading.Thread(target=run)
    worker.start()
    worker.join()
    sys.setrecursionlimit(previous_limit)
    if outcome:
        raise outcome[0]
