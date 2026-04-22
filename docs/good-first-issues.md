# Good First Issues

This document collects starter-sized contribution ideas for people who want a clear first pull request.

These are intentionally narrow. They are chosen because they have a small write surface, a straightforward validation path, and low conflict risk with other work.

## Before You Pick One

Read these first:

1. [Contributor Roadmap](./roadmap.md)
2. [Contributing Guide](../CONTRIBUTING.md)
3. [Checker Authoring](./architecture/checker-authoring.md) if the task touches a Rust checker

## Starter Task Candidates

| Topic | Difficulty | Why it is a good first issue | Start here | Validation |
| --- | --- | --- | --- | --- |
| Add one `tsconfig` regression case around false positives or missing diagnostics | Small | One checker, one fixture family, and a tight validation path | `crates/maximus-checks/src/tsconfig.rs`, `crates/maximus-checks/tests/tsconfig_checks.rs` | `cargo test -p maximus-checks --test tsconfig_checks`, `npm test`, `node ./bin/maximus.js audit <fixture>` |
| Add one new `package-entrypoints` regression case | Small | The checker is already isolated and heavily test-driven | `crates/maximus-checks/src/package_entrypoints.rs`, `crates/maximus-checks/tests/package_entrypoints_checks.rs` | targeted Cargo test, `cargo test --workspace`, `npm test` |
| Expand filesystem edge-case fixtures | Small to Medium | Mostly test and fixture work, with low risk to user-facing behavior | `test/fixtures/`, `crates/maximus-checks/tests/`, `test/*.test.js` | targeted tests, `npm test` |
| Add one snapshot-style audit or doctor regression | Small | Good introduction to report output without changing core discovery logic | `test/golden-rust/`, `test/reference-parity.test.js`, related CLI tests | targeted Node tests, `npm test` |
| Tighten contributor-facing docs after a checker lands | Small | Docs-only change, good for learning repository boundaries | `README.md`, `CONTRIBUTING.md`, `docs/` | static link review, `git diff --check` |

## How To Keep A Starter PR Healthy

- Keep the scope to one rule, one edge case, or one doc improvement.
- Prefer one new fixture over a broad refactor.
- Add at least one regression test that fails before the change and passes after it.
- If you touch runtime behavior, run both Rust tests and the wrapper-facing baseline command.

## Good Signals For A First PR

Starter work in this repo usually has most of these properties:

- touches one checker or one docs area
- changes fewer than a handful of files
- can be validated with one targeted test command plus the repo baseline
- does not change the release model or package distribution contract

## Better Left For Later

These are usually not good first issues:

- multi-check refactors
- release workflow redesign
- wrapper/runtime boundary changes
- roadmap-sized work that changes README, workflow files, package metadata, and runtime behavior together

If you want something bigger after a first contribution, continue with [Checker Ideas](./checker-ideas.md).
