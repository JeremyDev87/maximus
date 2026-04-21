# Contributing to Maximus

Thanks for helping improve Maximus.

This project aims to make config-heavy repositories easier to understand, safer to change, and faster to onboard into. Good contributions usually improve one of those three outcomes.

## Canonical Runtime

Maximus now treats Rust as the canonical runtime for the published CLI, the npm wrapper, and the GitHub Action.

- `bin/maximus.js` is a thin launcher for the published `@jeremyfellaz/maximus` wrapper and prefers repository Rust builds plus installed platform-specific Rust binaries.
- `src/**/*.js` stays in the repository as frozen reference code for parity checks, golden output generation, and roadmap context. It also remains available as a compatibility fallback for legacy-compatible CLI invocations, but config auto-loading and Rust-only CLI flags must still run on the canonical Rust runtime.
- New user-facing runtime or distribution behavior should land in the Rust crates, the thin launcher, and the docs. Do not treat `src/**/*.js` as the default implementation surface for new product behavior.
- `docs/plan/001` through `012` should be read as Rust v1 feature specs, not as instructions to expand the JS codebase directly.
- `docs/plan/013+` and the older JS backlog are not the default implementation lane while the rewrite family is still being closed out.

Read [docs/runtime-transition.md](https://github.com/JeremyDev87/maximus/blob/master/docs/runtime-transition.md) before starting any roadmap-sized work.

## Ways to Contribute

- Report bugs or false positives in existing checks
- Propose new analyzers or safe automatic fixes
- Improve docs, examples, and onboarding
- Add tests for tricky config edge cases

## Before You Start

For larger features or behavior changes, open an issue first so we can agree on scope and expected behavior before code is written.

For small fixes, feel free to open a pull request directly.

If you are adding or changing a checker, read [docs/architecture/checker-authoring.md](https://github.com/JeremyDev87/maximus/blob/master/docs/architecture/checker-authoring.md) first. It describes the current Rust crate layout, registry, and test locations that this repository uses today.

If you are working from the local planning docs, use the following rule set:

- Treat `docs/plan/001` through `012` as the source of truth for Rust v1 objectives, target outcomes, public interface changes, tests and acceptance, and done criteria.
- Do not reuse the JS file lists inside those plan docs as ownership guidance for new implementation work.
- Do not start `docs/plan/013+` implementation work before the Rust cutover phases are complete.
- Follow the transition families in order: `062` for direction, `063` for bootstrap/core, `064` for current MVP parity, `065` for backlog `001~012`, `066` for wrapper/cutover/distribution.

## Development Setup

```bash
git clone https://github.com/JeremyDev87/maximus.git
cd maximus
npm test
cargo test --workspace
node ./bin/maximus.js audit ./test/fixtures/clean-project
```

## Project Shape

```text
bin/              thin npm launcher to the Rust runtime
crates/           Rust workspace and CLI/library implementation
src/              frozen JS reference implementation
test/             regression tests and wrapper/runtime checks
```

Current repository layout keeps the JS source tree for reference, but new runtime or distribution behavior should treat Rust as the source of truth.

## Contribution Guidelines

- Keep changes focused. One behavior change per pull request is ideal.
- Prefer safe heuristics over surprising automation.
- Add or update tests when changing detection or fix behavior.
- Update `README.md` if the user-facing behavior or supported checks change.
- Avoid destructive fixes unless the user can clearly preview and understand them.
- If the change belongs to the rewrite roadmap, keep README, `README.en.md`, `CONTRIBUTING.md`, package metadata, and transition docs aligned.
- Do not land canonical CLI behavior only in the frozen JS reference tree unless the change is explicitly about parity/reference maintenance.

## Testing

Before opening a pull request, run:

```bash
npm test
cargo test --workspace
node ./bin/maximus.js audit ./test/fixtures/clean-project
```

If your change affects a specific detector, add a regression test covering the edge case you fixed.

## Commit Style

Conventional Commits are encouraged:

```text
feat: add prettier duplicate-source detection
fix: ignore template-only env contracts
docs: explain safe fixes in readme
test: cover wildcard alias suffix mismatches
```

## Pull Requests

Please include:

- What changed
- Why it changed
- How you tested it
- Any known limitations or follow-up work

Small, well-tested pull requests are much easier to review and merge quickly.

## Security

If you discover a security issue, please do not open a public issue. Follow the private reporting instructions in [SECURITY.md](https://github.com/JeremyDev87/maximus/blob/master/SECURITY.md).
