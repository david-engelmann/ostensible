#!/usr/bin/env python3
"""Generate the PORT10 runner recordings.

Runs the restored Python execution machinery over synthetic
projects this script builds in its own scratch and records each
project's files with the analysis dictionary the machinery
returns, at tests/conformance/runner_cases.json. The recordings
are never edited by hand. The dictionary carries no wall clock
value at any point, so the comparison is whole.

The battery does not carry a project whose governing source
stands above the project folder, because the analysis records
that folder as an absolute path, which is a fact of the
recording machine and not of the project.
"""

import json
import shutil
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT))

from ostensible.runner import analyse_project  # noqa: E402

CALC = '''def add(a, b):
    return a + b


def is_positive(x):
    if x > 0:
        return True
    return False
'''

TEST_CALC = '''from calc import add, is_positive


def test_add():
    assert add(2, 3) == 5
    assert add(-1, 1) == 0


def test_is_positive():
    assert is_positive(3) is True
    assert is_positive(-2) is False
    assert is_positive(0) is False
'''

STATS = '''def mean(values):
    return sum(values) / len(values)
'''

TEST_STATS = '''from stats import mean


def test_mean_runs():
    mean([1, 2, 3])
    assert True
'''

BROKEN = '''def double(x):
    return x * 2
'''

TEST_BROKEN = '''from broken import double


def test_double():
    assert double(3) == 7
'''

ALPHA = '''def shout(word):
    return word.upper()
'''

BETA = '''def whisper(word):
    return word.lower()
'''

TEST_MISC = '''from alpha import shout


def test_shout():
    assert shout("hey") == "HEY"
'''

UTIL = '''def tidy(text):
    return text.strip()
'''

TEST_UTIL = '''from util import tidy


def helper_clean():
    return tidy("  x  ")
'''

MOD = '''def ping():
    return "pong"
'''

TEST_MOD = '''from mod import ping

assert ping() == "pang"
'''

SLOW = '''def drift():
    return 1
'''

TEST_SLOW = '''import time

from slow import drift


def test_drift():
    time.sleep(5)
    assert drift() == 1
'''

CHECKS = '''def total(prices):
    return sum(prices)
'''

TEST_CHECKS = '''from checks import total


def check_total():
    assert total([1, 2, 3]) == 6
    assert total([]) == 0
'''

PYPROJECT = '''[tool.pytest.ini_options]
python_functions = ["check_*"]
'''

BUMP = '''def bump(x):
    return x + 1
'''

CONFTEST = '''import pytest


@pytest.fixture
def supply():
    return 41
'''

TEST_BUMP = '''from bump import bump


def test_bump(supply):
    assert bump(supply) == 42
'''

SHAPES = '''def area(width, height):
    return width * height
'''

TEST_SHAPES = '''from shapes import area


def test_area():
    assert area(3, 4) == 12
    assert area(0, 5) == 0
'''

CASES = [
    ("well_tested", 20.0, {"calc.py": CALC, "test_calc.py": TEST_CALC}),
    ("decorative", 20.0, {"stats.py": STATS, "test_stats.py": TEST_STATS}),
    ("baseline_failure", 20.0, {"broken.py": BROKEN, "test_broken.py": TEST_BROKEN}),
    (
        "unmatched_stem",
        20.0,
        {"alpha.py": ALPHA, "beta.py": BETA, "test_misc.py": TEST_MISC},
    ),
    ("whole_file_no_tests", 20.0, {"util.py": UTIL, "test_util.py": TEST_UTIL}),
    ("whole_file_baseline_failure", 20.0, {"mod.py": MOD, "test_mod.py": TEST_MOD}),
    ("timeout", 1.0, {"slow.py": SLOW, "test_slow.py": TEST_SLOW}),
    (
        "settings",
        20.0,
        {
            "pyproject.toml": PYPROJECT,
            "checks.py": CHECKS,
            "test_checks.py": TEST_CHECKS,
        },
    ),
    (
        "conftest_supply",
        20.0,
        {"conftest.py": CONFTEST, "bump.py": BUMP, "test_bump.py": TEST_BUMP},
    ),
    (
        "subfolder",
        20.0,
        {"shapes.py": SHAPES, "sub/test_shapes.py": TEST_SHAPES},
    ),
]


def main() -> None:
    scratch = Path(tempfile.mkdtemp(prefix="ostensible-port10-gen-"))
    cases = []
    try:
        for name, timeout, files in CASES:
            project = scratch / name
            project.mkdir(parents=True)
            for relative, text in files.items():
                target = project / relative
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_text(text, encoding="utf-8")
            analysis = analyse_project(project, timeout=timeout)
            cases.append(
                {
                    "name": name,
                    "timeout": timeout,
                    "files": files,
                    "expected": analysis.to_dict(),
                }
            )
            print(f"recorded {name}")
    finally:
        shutil.rmtree(scratch, ignore_errors=True)
    document = {"format": "ostensible-port10-cases/1", "cases": cases}
    out = ROOT / "tests" / "conformance" / "runner_cases.json"
    out.write_text(json.dumps(document, indent=1) + "\n", encoding="utf-8")
    print(f"wrote {out} with {len(cases)} cases")


if __name__ == "__main__":
    main()
