# Runtime Transition

Maximus is moving from a Node.js runtime implementation to a Rust runtime implementation.

This document defines the transition boundary so roadmap work, contributor expectations, and user-facing docs all point at the same direction.

## Current State

- The published CLI behavior is currently implemented by the Node.js runtime in `bin/maximus.js` and `src/**/*.js`.
- Local development and regression commands still validate that Node.js implementation today.
- The user-facing command surface must remain stable during the transition:
  - `npx maximus audit`
  - `npx maximus doctor`
  - `npx maximus fix`

## Canonical Runtime Direction

- The canonical runtime direction for Maximus is Rust.
- The current JS runtime is kept as a reference implementation until the cutover is complete.
- The cutover does not change the primary command surface; it changes the implementation behind that surface.

## Scope Freeze

The local planning docs are split into two groups during the rewrite:

- `docs/plan/001` through `docs/plan/012`
  - These remain active inputs for Rust v1.
  - Their authoritative sections are:
    - `Objective`
    - `Target Outcome`
    - `Public Interface Changes`
    - `Tests And Acceptance`
    - `Done Criteria`
  - Their JS file lists, PR units, and Node verification commands are legacy reference only.
- `docs/plan/013+`
  - These are post-cutover backlog items.
  - They stay deferred until the Rust cutover is complete.

This means contributors should not directly resume the older JS backlog as the default execution path.

## Transition Families

The rewrite is organized into five roadmap families:

1. `062` Rust rewrite master roadmap
   - Declares the runtime direction and freezes the older JS backlog as the default implementation lane.
2. `063` Rust bootstrap and core
   - Adds the toolchain, Cargo workspace, JS golden reference harness, and Rust pure-library foundations.
3. `064` Rust current MVP parity
   - Rebuilds the currently shipped `audit`, `doctor`, `fix`, `--json`, and env fix behavior in Rust.
4. `065` Rust v1 backlog `001~012`
   - Ports the accepted v1 feature contracts that go beyond today's shipped MVP.
5. `066` Rust npm wrapper, cutover, and distribution
   - Connects the Rust binary to the npm wrapper, GitHub Action, release flow, and final runtime cutover.

## Distribution Surface

The long-term distribution contract stays familiar for users:

- `npx maximus ...` remains the main invocation path.
- The root npm package name and `bin.maximus` entry stay stable.
- After cutover, the npm package becomes a thin launcher for platform-specific Rust binaries.

## Contributor Rules

- Do not treat `001~012` as approval to expand the JS codebase first.
- Do not start `013+` roadmap work before the Rust cutover phases are done.
- Keep README, contributing docs, CI, wrapper plans, and roadmap terminology aligned when transition wording changes.
- When in doubt, prefer preserving the user-facing CLI contract while moving implementation responsibility into Rust.

## Exit Condition

The transition is complete when all of the following are true:

- Rust reproduces the currently shipped MVP behavior.
- Rust implements the accepted `001~012` feature contracts.
- The npm wrapper and GitHub Action run the Rust runtime.
- Maximus documentation treats Rust as the canonical runtime and the JS runtime as frozen reference code.
