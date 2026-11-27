# Changelog

All notable changes to ostensible are recorded in this file.

## Unreleased

- Complete `Cargo.toml` package metadata (`description`, `license`, `repository`, `documentation`, `keywords`, `categories`, `rust-version`) and `.crate` archive `exclude` rules keeping the `23 MB` conformance recordings outside published packages.
- Add operator guide `docs/GUIDE.md`, storefront installation and quickstart documentation in `README.md`, checksum-verifying `install.sh`, `SECURITY.md`, `CONTRIBUTING.md`, `deny.toml`, and multi-platform GitHub Release workflow `.github/workflows/release.yml`.

## 0.19.0 on 2026-11-21

- Complete the Rust port through `PORT24` across 33 closed verification rounds, retiring the legacy Python implementation while preserving byte-identical `cantfail-report/3` text and JSON reports.
- Carry upward-searched `pytest` collection settings across all seven `pytest 9.1.1` sources (`pytest.toml`, `.pytest.toml`, `pytest.ini`, `.pytest.ini`, `pyproject.toml` `[tool.pytest]` and `[tool.pytest.ini_options]`, `tox.ini`, and `setup.cfg`), `conftest.py` static tautology chain analysis (`mock_return_value`, `self_comparison`, `constant_assert`), `yield` and `yield from` fixture delegation, and the four-family mutation panel (`return_none`, `compare_invert`, `bool_negate`, `constant_shift`).
