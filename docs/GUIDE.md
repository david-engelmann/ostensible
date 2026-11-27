# Ostensible Operator Guide

Ostensible asks whether a test can fail. It combines a static tautology scan (`mock_return_value`, `self_comparison`, and `constant_assert`) with a fast four-family dynamic mutation panel over Python `pytest` suites, reporting both static findings and test items that stay green across every applicable source mutant.

## Installation

### Building from Source

With a Rust 1.80 or newer toolchain installed, build and install directly from the repository.

```sh
cargo install --git https://github.com/david-engelmann/ostensible --locked
```

### Prebuilt Release Archive

For releases cut from `v0.20.0` onward on Linux (`x86_64`, `aarch64`) and macOS (`aarch64`, `x86_64`), the installer downloads the matching GitHub Release archive, verifies its SHA-256 digest, and atomically installs `ostensible` into `$HOME/.local/bin` (or `$OSTENSIBLE_INSTALL_DIR`).

```sh
curl -fsSL https://raw.githubusercontent.com/david-engelmann/ostensible/main/install.sh | sh
```

Pin a specific release tag that carries release archives by setting `OSTENSIBLE_VERSION` (for example `OSTENSIBLE_VERSION=v0.20.0`). Because `install.sh` does not edit shell profile files, ensure `$HOME/.local/bin` (or your custom `$OSTENSIBLE_INSTALL_DIR`) stands on `PATH` in your shell (`export PATH="$HOME/.local/bin:$PATH"`). On Windows (`x86_64-pc-windows-msvc`), download the `.zip` archive and its `.sha256` file from the GitHub Releases page.

## Preparing Your Python Environment

Ostensible invokes `python3 -m pytest -q -p no:cacheprovider` inside temporary scratch copies of the target project, resolving `python3` from `PATH`. Before running `ostensible` against a repository that manages dependencies in a virtual environment (`uv`, `venv`, `poetry`, or `conda`), activate that environment or place its `bin` directory first on `PATH`.

```sh
PATH="/path/to/project/.venv/bin:$PATH" ostensible /path/to/project
```

Make sure `pytest` imports the code under test from each temporary scratch copy rather than from an editable install (`pip install -e .`) pointing back at your original checkout directory, or every mutant in the scratch copy will go unexecuted and appear to survive. Install third-party test dependencies without editable-installing the target package itself, and for `src/`-layout repositories configure `pythonpath = ["src"]` inside `pyproject.toml` or `pytest.ini` (which `pytest 9.1.1` resolves relative to each scratch copy's `rootdir`, unlike an absolute `PYTHONPATH`).

On Windows, Python virtual environments under `.venv\Scripts` frequently create `python.exe` without a `python3.exe` alias. Because `ostensible` invokes `python3`, copy or link `python.exe` to `python3.exe` inside `.venv\Scripts` and prepend `.venv\Scripts` to `PATH` so Windows does not fall through to the Microsoft Store `python3.exe` stub.

If the `python3` on `PATH` lacks `pytest`, every baseline run exits non-zero without timing out and test functions report `baseline_failed` rather than being evaluated against mutants. Keep virtual environments inside dotted directories (`.venv` rather than `venv`) and clone target projects outside hidden dot-prefixed parent folders so source discovery walks your project cleanly while ignoring site-packages.

## Command Line Interface

```
ostensible [-h | --help] [--json] [--timeout SECONDS] <project>
```

Options

- `-h`, `--help` Print the help summary to standard output and exit `0`.
- `--json` Print the `cantfail-report/3` JSON document to standard output instead of the human-readable text report.
- `--timeout SECONDS` Per-invocation timeout in seconds for each baseline or mutant `pytest` run (default `30`). Accepts integer or floating-point values.
- `<project>` Path to the Python project directory to analyse.

Exit codes

- `0` Analysis completed (or `--help` printed). Inspect `static_findings` and `verdicts` in the output to enforce policy in CI.
- `2` Usage error, missing project directory, unreadable file, or unparseable Python source.

## How Ostensible Evaluates a Suite

### 1. Collection Settings Discovery

Before scanning or running tests, Ostensible walks upward from `<project>` to the filesystem root to locate the governing `pytest` configuration source across all seven `pytest 9.1.1` sources in precedence order (`pytest.toml`, `.pytest.toml`, `pytest.ini`, `.pytest.ini`, `pyproject.toml` carrying `[tool.pytest]` or `[tool.pytest.ini_options]`, `tox.ini`, and `setup.cfg`). Custom `python_files` patterns govern test file discovery, `python_functions` and `python_classes` prefix-or-glob patterns govern dynamic test item selection, and `python_functions` governs which functions and methods inside discovered test files are inspected by the static scan (`python_classes` does not filter static scanning). When non-default settings are active, the report records them alongside their source location.

### 2. Static Tautology Scan

Ostensible parses every discovered test file and its `conftest.py` chain without executing untrusted code and reports three static finding kinds inside functions and methods matching `python_functions` (excluding fixture-decorated definitions)

- `mock_return_value` Direct `.return_value` assignment in a test body where a call on the configured object (or a local variable assigned from that call) is asserted equal to the configured expression, plus literal `.return_value` configurations returned or yielded from helper functions in the same module and module-level or `conftest.py` fixtures along the directory chain (respecting shadowing and overrides, `yield` setup statements prior to the first `yield`, `yield from` delegating fixtures, and top-level `if`, `try`, `for`, `while`, `with`, and `match` blocks in source order).
- `self_comparison` Comparing an expression directly against itself in an assertion (such as `assert x == x`).
- `constant_assert` Asserting the literal constant expected by the assertion (`assert True`, or `unittest`-style `self.assertTrue(True)`, `self.assertFalse(False)`, and `self.assertIsNone(None)`).

### 3. Dynamic Mutation Panel

For each discovered test file, Ostensible pairs matching source modules (`test_foo.py` or `foo_test.py` pairs with `foo.py`, falling back to all discovered project source modules when no stem match exists) and generates mutants across four deterministic operators

- `return_none` Replaces a non-`None` `return <expr>` value with `return None`.
- `compare_invert` Inverts the first comparison operator of a comparison (`==` to `!=`, `<` to `>=`, `>` to `<=`, `in` to `not in`, `is` to `is not`, and their converses).
- `bool_negate` Flips `True` to `False` and `False` to `True`.
- `constant_shift` Produces two mutants per integer or floating-point numeric constant site, shifting the constant by `+1` and `-1`.

Selectable test functions and `ClassName::method_name` items run unmutated first in a shared baseline scratch copy for that test file. Passing items then run against each mutant (one scratch copy per mutant shared across the passing items of that file), recording each mutant's `"result"` in JSON as `"killed"` (test turned red), `"survived"` (test stayed green), or `"timeout"` (run hit `--timeout`).

## Reading Verdicts and Rollups

Each function or method receives one of five status values

- `inconclusive` Either the unmutated baseline run timed out, or the baseline passed, zero mutants were killed, and at least one mutant timed out.
- `baseline_failed` The unmutated baseline run failed without timing out, so the item is not judged decorative.
- `no_applicable_mutants` The baseline passed and zero mutants in the four-operator panel applied to the paired source files.
- `decorative_candidate` The baseline passed, at least one mutant applied, and **every** applicable mutant survived without timing out. Every surviving mutant is listed beneath the function.
- `can_fail` The baseline passed and at least one mutant turned the test red.

File-level verdicts roll up their member function verdicts in the exact order evaluated by the runner

- `decorative_candidate` Zero functions have `baseline_failed`, at least one function is judged (`baseline_ok` with at least one applicable mutant), and every judged function has status `decorative_candidate`.
- `can_fail` Otherwise, at least one function in the file has status `can_fail`.
- `baseline_failed` Zero functions are `can_fail` and **at least one** function has status `baseline_failed` (preventing a broken sibling test from allowing a file to be labelled `decorative_candidate`).
- `inconclusive` Neither `decorative_candidate`, `can_fail`, nor `baseline_failed` fired, and at least one function has status `inconclusive` (such as a mutant timeout or a baseline timeout without judged `decorative_candidate` siblings).
- `no_applicable_mutants` Every function in the file had `no_applicable_mutants`. When static item selection finds zero selectable functions or methods in a file (`whole_file` fallback), Ostensible executes the whole file (`can_fail`, `decorative_candidate`, `baseline_failed`, or `inconclusive`) and reports `no_applicable_mutants` when `pytest` collects zero tests (`no_tests`) or zero mutants apply.

## Report Schema (`cantfail-report/3`)

Under `--json`, Ostensible prints a sorted-key JSON object carrying `"schema"` set to `"cantfail-report/3"` (the stable format name retained from before the project's rename from `cantfail` to `ostensible`). Top-level keys include

- `project` Project folder name.
- `schema` Always `"cantfail-report/3"`.
- `collection_settings` and `collection_settings_source` Present when non-default `pytest` naming patterns govern the run.
- `static_findings` List of `{ "path", "line", "kind", "detail" }` objects.
- `verdicts` List of per-test-file objects containing `path`, `status`, `whole_file`, `no_tests`, `partial_selection`, `test_functions`, `functions` (per-item `name`, `status`, `mutants`, `surviving_mutants` where each mutant entry carries `"result"` as `"killed"`, `"survived"`, or `"timeout"`), `mutants`, and `surviving_mutants`.

## GitHub Actions Pull Request Check

The workflow below builds `ostensible`, installs third-party test dependencies into `.venv` without an editable `.pth` back to the checkout (for `src/` layouts, set `pythonpath = ["src"]` in `pyproject.toml` or `pytest.ini`), writes the `cantfail-report/3` JSON report, and fails the job if any static finding, whole-file `decorative_candidate`, or individual `decorative_candidate` function inside a mixed test file is detected.

```yaml
name: decorative-tests

on:
  pull_request:

jobs:
  ostensible:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-python@v5
        with:
          python-version: "3.12"
      - name: Install third-party test dependencies
        run: |
          python3 -m venv .venv
          .venv/bin/pip install pytest -r requirements-dev.txt
      - name: Build ostensible
        run: cargo install --git https://github.com/david-engelmann/ostensible --locked
      - name: Audit suite for decorative tests
        run: |
          PATH="$PWD/.venv/bin:$PATH" ostensible --json . > ostensible-report.json
          python3 - <<'PY'
          import json, pathlib, sys
          report = json.loads(pathlib.Path("ostensible-report.json").read_text())
          findings = report.get("static_findings", [])
          decorative_files = [
              v["path"] for v in report.get("verdicts", [])
              if v.get("status") == "decorative_candidate"
          ]
          decorative_funcs = [
              f"{v['path']}::{fn['name']}"
              for v in report.get("verdicts", [])
              for fn in v.get("functions", [])
              if fn.get("status") == "decorative_candidate"
          ]
          if findings or decorative_files or decorative_funcs:
              print("Static findings", len(findings))
              print("Decorative files", decorative_files)
              print("Decorative functions", decorative_funcs)
              sys.exit(1)
          PY
```
