# Contributor Roadmap

This document helps contributors choose work that matches the repository shape today.

Maximus now treats Rust as the canonical runtime. That means the main implementation surface lives in the Rust workspace, while `src/**/*.js` stays as frozen reference code for parity and compatibility checks.

## Current Project Shape

The repository currently has four practical contribution lanes:

| Lane | What lives there today | Best starting files | Typical validation |
| --- | --- | --- | --- |
| Rust core and discovery | project scanning, config loading, shared data models | `crates/maximus-core/src/` | `cargo test --workspace` |
| Rust checks and CLI | registered checks, report rendering, fix selection | `crates/maximus-checks/src/`, `crates/maximus-cli/src/` | `cargo test --workspace`, `npm test`, `node ./bin/maximus.js audit ./test/fixtures/clean-project` |
| Wrapper and release wiring | npm launcher, packed-install checks, GitHub Actions | `bin/maximus.js`, `scripts/`, `.github/workflows/` | `npm test`, release wiring checks, packed wrapper smoke |
| Docs and contributor UX | onboarding, runbooks, architecture notes | `README.md`, `CONTRIBUTING.md`, `docs/` | static link review, `git diff --check` |

## What Is Already Implemented

The Rust runtime already ships these registered checks:

- `duplicates`
- `env`
- `eslint-prettier`
- `tsconfig`
- `lockfiles`
- `package-entrypoints`

The repository also already has:

- a thin npm launcher in `bin/maximus.js`
- platform package manifests under `npm/`
- packed-install smoke checks
- GitHub Action and release workflow wiring
- frozen JS reference code under `src/`

These are current repository facts, not backlog items.

## What Still Makes Good Contribution Work

The next useful work tends to fall into three buckets:

### 1. Harden existing checks

Good work in this lane usually improves one of these:

- false positive reduction
- missing edge-case coverage
- better file targeting or clearer findings
- regression tests for already-supported behavior

This is the best lane if you want to work in `crates/maximus-checks` without changing the whole product surface.

### 2. Expand coverage with one check at a time

There is still room to add narrow, well-tested checks around:

- TypeScript project configuration drift
- package metadata and publish safety
- environment file safety
- workspace-level consistency rules

In this repository, the healthiest change shape is still one checker or one regression family per pull request.

### 3. Improve contributor and operator experience

Not every useful contribution is a new rule. We also need:

- clearer onboarding docs
- better fixture coverage for real project shapes
- sharper release and maintenance runbooks
- examples that help new contributors reproduce findings locally

## Recommended Starting Path

Choose the path that matches your comfort level:

| If you want to... | Start here |
| --- | --- |
| understand the current runtime boundary | [Runtime Transition](./runtime-transition.md) |
| add or extend a checker | [Checker Authoring](./architecture/checker-authoring.md) |
| pick a small starter task | [Good First Issues](./good-first-issues.md) |
| browse possible future checks | [Checker Ideas](./checker-ideas.md) |
| work on release or packaging behavior | [Release Operator Runbook](./release-operator-runbook.md) |

## Rules That Keep Contributions Mergeable

- Keep one behavior change per pull request when possible.
- Add or update regression tests for every checker change.
- Keep README, CONTRIBUTING, and docs aligned when user-facing behavior changes.
- Treat `src/**/*.js` as frozen reference code, not the default lane for new runtime behavior.
- Prefer repository evidence over plan assumptions when the code and an older note disagree.

## A Good First Week In This Repo

If you are new to Maximus, this sequence tends to work well:

1. Read [CONTRIBUTING.md](../CONTRIBUTING.md).
2. Read [Checker Authoring](./architecture/checker-authoring.md) if you want to touch runtime logic.
3. Pick one item from [Good First Issues](./good-first-issues.md).
4. Run the baseline commands before and after your change:

```bash
npm test
cargo test --workspace
node ./bin/maximus.js audit ./test/fixtures/clean-project
```

## What This Document Is Not

This roadmap is a contributor-facing orientation guide. It does not try to mirror every local planning note or every unmerged idea. Use it to find the right lane, then validate your understanding against the repository as it exists on `master`.
