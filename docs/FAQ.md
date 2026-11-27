# Frequently Asked Questions

## Does a `can_fail` verdict prove a test is thorough?

No, and every text report states that explicitly at the footer. Killing one mutant from the fixed four-operator panel proves the test is *not purely decorative* against that panel, whereas `canaryfield` evaluates broad defect-class catch rates across 200 seeded defect patterns.

## Why does the report schema say `cantfail-report/3`?

`cantfail` was the initial working name of the repository prior to its rename to `ostensible`. Keeping `"schema": "cantfail-report/3"` preserves zero-breakage compatibility for downstream parsers and Gate Truth consumers.

## Why did every mutant survive on my `src/`-layout Python repository?

Check whether your virtual environment installed the target package with `pip install -e .`. Editable installs write a `.pth` hook pointing back to your original checkout's `src/` folder, so `pytest` inside Ostensible's temporary scratch copy imports the unmutated checkout instead of the mutated scratch copy. Install third-party test dependencies without `-e .` and set `pythonpath = ["src"]` in `pyproject.toml` (or `pythonpath = src` in `pytest.ini`) so `pytest` resolves `src/` relative to each scratch copy's `rootdir`.

## Does Ostensible modify my working tree or leave `.pytest_cache` behind?

Never. Static scanning reads source files read-only, and dynamic mutation copies the project into temporary scratch directories and passes `-p no:cacheprovider` to `pytest`. Conformance tests verify byte-for-byte working-tree invariance before and after every run.

## Can I run Ostensible on untrusted code safely?

Static scanning parses Python syntax in pure Rust without executing any Python code. Dynamic evaluation runs `python3 -m pytest` on the target repository, which executes the target's test suite and imports as your OS user, so run dynamic evaluation on untrusted code inside an isolated container or VM.
