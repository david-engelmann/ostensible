# Ostensible Wire Formats (`cantfail-report/3`)

Ostensible emits two deterministic output representations from the same analysis model, human-readable text (default) and sorted-key JSON (`--json`). Both carry the stable schema identifier `"cantfail-report/3"` preserved across the project's rename from `cantfail` to `ostensible`.

## JSON Schema (`--json`)

Top-level object keys (sorted lexicographically, 2-space indented)

- `collection_settings` Optional object present only when non-default `python_files`, `python_classes`, or `python_functions` patterns are active.
- `collection_settings_source` Optional string present when non-default settings apply, set to `"project root"` when governed at `<project>` or the governing ancestor directory path when inherited from above `<project>`.
- `project` String directory name of `<project>`.
- `schema` Always `"cantfail-report/3"`.
- `static_findings` Array of static tautology objects carrying
  - `detail` Human-readable explanation naming the literal or expression.
  - `kind` One of `"mock_return_value"`, `"self_comparison"`, or `"constant_assert"`.
  - `line` 1-based source line number in the test file.
  - `path` Relative path to the test file inside `<project>`.
- `verdicts` Array of per-test-file verdict objects carrying
  - `functions` Array of per-item verdict objects (`name`, `status`, `mutants`, `surviving_mutants`). Class methods spell `name` as `"ClassName::method_name"`.
  - `mutants` File-level aggregated mutant outcomes (`source`, `line`, `kind`, `detail`, and `result` set to `"killed"`, `"survived"`, or `"timeout"`).
  - `no_tests` Boolean (`true` when `whole_file` fallback encountered `pytest` exit code `5`).
  - `partial_selection` Boolean indicating partial item selection.
  - `path` Relative path to the test file.
  - `status` File rollup verdict (`"can_fail"`, `"decorative_candidate"`, `"baseline_failed"`, `"inconclusive"`, or `"no_applicable_mutants"`).
  - `surviving_mutants` Subset of `mutants` whose `result` is `"survived"`.
  - `test_functions` Ordered list of selectable item names discovered in the file.
  - `whole_file` Boolean (`true` when zero selectable functions/methods stood in the file and whole-file fallback ran).

## Example JSON Excerpt

```json
{
  "project": "demo_project",
  "schema": "cantfail-report/3",
  "static_findings": [
    {
      "detail": "assertion checks a value read from a mock against the return value the mock was assigned, 42",
      "kind": "mock_return_value",
      "line": 8,
      "path": "test_greeter.py"
    }
  ]
}
```
