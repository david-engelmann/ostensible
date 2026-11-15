"""Generate the recorded battery for PORT11, the command line.

Drives the Python command line, as the module main of the package,
over synthetic projects built in a scratch folder and records each
case's argument list standing relative to the scratch folder, the
project files the case carries, and the exit code and the standard
and error streams the command line gave. The battery stands at
tests/conformance/cli_cases.json in the format
ostensible-port11-cases/1 and is never edited by hand.

Run from the repository root with the package in place. The
generator retires with the package under PORT11-1 and the battery
stands after it as data and as the recorded specification.
"""

from __future__ import annotations

import json
import os
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT))
sys.path.insert(0, str(ROOT / "tests"))

from factories import DECORATIVE, WELL_TESTED  # noqa: E402

SLEEPY = {
    "slow.py": "def drift():\n    return 1\n",
    "test_slow.py": (
        "import time\n"
        "\n"
        "from slow import drift\n"
        "\n"
        "\n"
        "def test_drift():\n"
        "    time.sleep(5)\n"
        "    assert drift() == 1\n"
    ),
}

CASES = [
    ("decorative_text", "decorative", DECORATIVE, ["decorative"]),
    ("decorative_json", "decorative", DECORATIVE, ["decorative", "--json"]),
    ("well_tested_text", "well_tested", WELL_TESTED, ["well_tested"]),
    ("missing_project", None, None, ["missing_project"]),
    ("short_timeout", "sleepy", SLEEPY, ["sleepy", "--timeout", "0.2"]),
]


def main() -> int:
    recorded = []
    with tempfile.TemporaryDirectory() as scratch:
        root = Path(scratch)
        for name, folder, files, argv in CASES:
            if folder is not None:
                target = root / folder
                target.mkdir(parents=True, exist_ok=True)
                for rel, content in files.items():
                    (target / rel).write_text(content, encoding="utf-8")
            env = dict(os.environ)
            env["PYTHONPATH"] = str(ROOT)
            run = subprocess.run(
                [sys.executable, "-m", "ostensible", *argv],
                cwd=root,
                env=env,
                capture_output=True,
                text=True,
            )
            recorded.append(
                {
                    "name": name,
                    "files": files or {},
                    "argv": argv,
                    "exit": run.returncode,
                    "stdout": run.stdout,
                    "stderr": run.stderr,
                }
            )
            print(name, "exit", run.returncode)
    battery = {"format": "ostensible-port11-cases/1", "cases": recorded}
    out = ROOT / "tests" / "conformance" / "cli_cases.json"
    out.write_text(json.dumps(battery, indent=2) + "\n", encoding="utf-8")
    print("wrote", out, out.stat().st_size, "bytes")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
