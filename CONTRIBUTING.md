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

If you are new to the repository, these contributor docs are the quickest way to orient yourself:

- [docs/roadmap.md](https://github.com/JeremyDev87/maximus/blob/master/docs/roadmap.md) for the current contribution lanes and repository shape
- [docs/good-first-issues.md](https://github.com/JeremyDev87/maximus/blob/master/docs/good-first-issues.md) for narrow starter tasks
- [docs/checker-ideas.md](https://github.com/JeremyDev87/maximus/blob/master/docs/checker-ideas.md) for backlog-style checker candidates that are not implemented yet

If you are working from the local planning docs, use the following rule set:

- Treat `docs/plan/001` through `012` as the source of truth for Rust v1 objectives, target outcomes, public interface changes, tests and acceptance, and done criteria.
- Do not reuse the JS file lists inside those plan docs as ownership guidance for new implementation work.
- Treat `docs/plan/013+` as post-cutover backlog work. Re-check current `master`, merge history, and the active board before assuming a slice is still pending.
- Keep new runtime behavior in the Rust crates, the launcher, and the docs instead of reviving the frozen JS reference tree as a default implementation lane.

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

## Release-Related Changes

If your change touches release wiring, packaging, release notes automation, or packed-install behavior, keep the maintainer runbook and release-drafter contract aligned with the code.

- Read [docs/release-operator-runbook.md](https://github.com/JeremyDev87/maximus/blob/master/docs/release-operator-runbook.md) before changing the release model.
- Treat Release Drafter as draft-notes automation for `master`, not as the publish workflow.
- If you touch `.github/workflows/release.yml`, `.github/workflows/action-smoke.yml`, `.github/workflows/release-drafter.yml`, `.github/release-drafter.yml`, package publish metadata, or packed-wrapper smoke logic, run the release-specific checks before opening the PR.

Recommended release-specific verification:

```bash
node ./scripts/validate-rust-release-wiring.mjs
node --test test/github-action-wiring.test.js test/release-workflow-context.test.js test/release-plan.test.js
npm test
```

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
