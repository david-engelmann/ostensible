# Contributing to Ostensible

Ostensible is developed under the contract-first discipline documented in `AGENTS.md` and `CONTRACT.md`. Read both files in full before proposing a change.

## Core Rules

1. **Contract first** Behavioural changes state their falsifiable obligations in `CONTRACT.md` before implementation code is written. Where behaviour depends on `pytest`, probe against `pytest 9.1.1` first and record the distilled findings.
2. **Discriminating tests** Every load-bearing rule requires a content-level test pin that fails when the behaviour is negated.
3. **Sanitization** Never include real paths from a working machine, real project data, session transcripts, names, or private operational details anywhere in code, tests, fixtures, examples, or prose. All fixtures must be synthetic.
4. **Copy and typography** Public prose and contract text carry zero em dashes (`U+2014`), zero en dashes (`U+2013`), and zero colons or semicolons in prose sentences. Verify every prose edit with a Python codepoint scan.
5. **Pull requests and sign-off** Open a pull request from a topic branch (`main` takes no direct pushes) and include a `Signed-off-by` trailer on every commit.

## Local Verification

Run the full local gate on your branch and verify zero formatting or lint warnings before opening a pull request.

```sh
cargo test --locked
cargo fmt --check
cargo clippy --all-targets
cargo package --locked --allow-dirty
```
