# Runtime Transition

Maximus has already moved its canonical runtime to Rust, while keeping the older Node.js implementation in the repository as frozen reference code.

This document defines the remaining transition boundary so roadmap work, contributor expectations, and user-facing docs all point in the same direction.

## Current State

- The published npm wrapper and GitHub Action are Rust-first. `bin/maximus.js` is a thin launcher that prefers repository Rust builds and published platform-specific Rust binaries.
- `src/**/*.js` stays in the repository as frozen reference code for parity checks, golden output comparison, and limited compatibility fallback.
- Local development uses two complementary validation lanes. Rust-direct checks such as `cargo test -p maximus-cli --test mvp_parity` validate the canonical CLI contract directly.
- `node ./bin/maximus.js ...` is a local wrapper smoke and repo-binary-path baseline. It can still exercise the frozen JS fallback when no native runtime is available, so installed-package resolution, packed-install behavior, and fallback boundaries should be validated with `node --test test/wrapper-runtime.test.js test/packed-wrapper-fallback.test.js` when that surface matters.
- The user-facing command surface stays stable across that implementation shift:
  - `npx @jeremyfellaz/maximus audit`
  - `npx @jeremyfellaz/maximus doctor`
  - `npx @jeremyfellaz/maximus fix`

## Canonical Runtime Direction

- Rust is the canonical runtime for Maximus.
- The JS runtime remains only as frozen reference code and a limited fallback path for legacy-compatible flows.
- New runtime behavior, distribution behavior, and release-facing logic should land in the Rust crates, thin launcher, and docs instead of reviving the JS tree as the default implementation lane.
- The implementation can evolve, but the published command surface and contract-sensitive behavior should remain fail-closed unless a pull request updates the matching regression evidence on purpose.

## Public Planning Boundary

- Contributor-facing guidance should rely on tracked repository documents such as `CONTRIBUTING.md`, `docs/roadmap.md`, `docs/architecture/checker-authoring.md`, and `docs/release-operator-runbook.md`.
- Older rewrite planning families and local execution notes may still exist in maintainer workflows, but they are not the public source of truth for day-to-day contribution work.
- This means contributors should not resume historical JS backlog ideas as the default execution path unless a tracked repo document still points to them explicitly.

## Distribution Surface

The long-term distribution contract stays familiar for users:

- `npx @jeremyfellaz/maximus ...` remains the main published invocation path.
- The scoped npm package name and `bin.maximus` entry stay stable.
- The published npm package is a thin launcher for platform-specific Rust binaries.

## Contributor Rules

- Keep README, contributing docs, CI, wrapper plans, and roadmap terminology aligned when transition wording changes.
- When in doubt, prefer preserving the user-facing CLI contract while moving implementation responsibility into Rust.
- Treat text output, JSON shape, exit semantics, and wrapper invocation behavior as fail-closed contracts. If one of those changes intentionally, update the relevant golden/parity tests in the same pull request.

## Exit Condition

The transition is complete when all of the following are true:

- Rust reproduces the currently shipped MVP behavior.
- The npm wrapper and GitHub Action run the Rust runtime.
- Maximus documentation treats Rust as the canonical runtime and the JS runtime as frozen reference code.
