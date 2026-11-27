# How Ostensible Compares

Ostensible is purpose-built for a specific verification failure that proliferated with AI-generated pull requests, tests that execute lines of code and turn coverage green while being structurally incapable of failing.

## Ecosystem Comparison

| Capability | `coverage.py` / `pytest-cov` | `flake8-pytest-style` / `ruff` | Full mutators (`mutmut`, `cosmic-ray`) | `ostensible` |
| --- | --- | --- | --- | --- |
| Catches `mock.return_value = 42` asserted against `42` across `conftest.py` chains | No | No | Only if non-mocked code exists | **Yes (instant static scan)** |
| Detects assertion-free or tautological tests whose body runs code | No (counts lines as covered) | Partial (trivial literals only) | Yes (slow full-repo sweep) | **Yes (targeted 4-family panel)** |
| Honours all 7 `pytest 9.1.1` upward config sources identically | Via `pytest` | No | Partial | **Yes (`FIX-14` arbitrated)** |
| Per-function and per-method `decorative_candidate` conviction | No | No | Per-mutant (not per-test) | **Yes (`cantfail-report/3`)** |
| Runtime dependency footprint for static scanner | Python + `coverage` | Python or Rust | Python + AST libraries | **Zero-dependency Rust binary** |

## Why Not Just Use Line Coverage?

Line and branch coverage measure whether a test *executed* statements in your module, not whether the test *checked* anything meaningful afterward. A test that calls `process_order()` and asserts `True` (or asserts a mocked collaborator returned its own configured literal) achieves 100% line coverage while catching zero defects.

## Why Pair Ostensible with Canaryfield?

- **Ostensible** is **test-centric**. For each test function on a pull request, it asks whether any small source mutation can turn *that test* red, catching decorative tests before they enter the suite.
- **Canaryfield** is **harness-centric**. Across the entire CI pipeline (tests, typecheckers, linters, custom review gates), it plants 200 realistic defect classes to measure which defects your combined gates catch and which sail past.
