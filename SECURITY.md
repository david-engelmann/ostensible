# Security Policy

## Supported Versions

Security fixes land on the latest `0.19.x` and subsequent `0.x` release line on `main`. Earlier tags do not receive backported patches.

## Security Model

Ostensible parses Python source files and `pytest` configuration files with its own pure-Rust readers without executing code during static scanning or configuration discovery. Dynamic mutation evaluation invokes `python3 -m pytest` inside temporary scratch copies of the target project, meaning **running Ostensible executes the target repository's test suite and imported code under the current user account**. Only run dynamic analysis on untrusted repositories inside an isolated container or sandbox, and never commit real credentials or private paths into synthetic test fixtures.

## Reporting a Vulnerability

Do not open a public issue for a suspected security vulnerability. Report vulnerabilities privately through [GitHub Security Advisories](https://github.com/david-engelmann/ostensible/security/advisories/new) using synthetic reproduction inputs only, naming the affected version and the relevant section of `CONTRACT.md`. Every report receives an initial review within seven days, and confirmed fixes ship with a regression pin in the test suite before public disclosure.
