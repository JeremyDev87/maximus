# Checker Ideas

This document is a contributor-facing backlog of checker ideas that are not all implemented yet.

Use it to spot promising rule candidates without confusing them with the checks that already exist today.

## Already Implemented Today

These checks already exist in the Rust runtime:

- `duplicates`
- `env`
- `eslint-prettier`
- `tsconfig`
- `lockfiles`
- `package-entrypoints`

If you want to extend one of those areas, start with [Checker Authoring](./architecture/checker-authoring.md) and the relevant integration tests under `crates/maximus-checks/tests/`.

## Candidate Ideas

| Idea | Area | Why it matters | Likely home |
| --- | --- | --- | --- |
| Empty `include` or `exclude` pattern detection | TypeScript | Catches configurations that look intentional but match nothing | `crates/maximus-checks/src/tsconfig.rs` |
| Output path overlap detection | TypeScript | Prevents `outDir`, `declarationDir`, or build outputs from stepping on each other | `crates/maximus-checks/src/tsconfig.rs` |
| Module system consistency | TypeScript and package metadata | Flags mismatches between `package.json`, `tsconfig`, and emitted files | `crates/maximus-checks/src/tsconfig.rs`, `crates/maximus-checks/src/package_entrypoints.rs` |
| Path alias shadowing | TypeScript | Finds aliases that hide package imports or each other in surprising ways | `crates/maximus-checks/src/tsconfig.rs` |
| Monorepo tsconfig drift | Workspace | Highlights package-local divergence from shared base configs | `crates/maximus-checks/src/tsconfig.rs` |
| `types` and `typeRoots` guidance | TypeScript | Helps explain resolution bugs that are hard to spot from a broken build alone | `crates/maximus-checks/src/tsconfig.rs` |
| JSX config hints | TypeScript | Warns when React or JSX toolchains are partly configured but incomplete | `crates/maximus-checks/src/tsconfig.rs` |
| Vite and tsconfig alias sync | Toolchain integration | Catches alias maps that differ between bundler and compiler | `crates/maximus-checks/src/tsconfig.rs` plus fixture tests |
| Jest and Vitest dual-config conflict detection | Test tooling | Finds test runners that overlap without clear ownership | new checker module or `structure`-adjacent helper |
| EditorConfig and Prettier conflict detection | Formatting | Surfaces formatting drift that causes noisy diffs and confusing saves | new checker module |
| Ignore file drift | Repo hygiene | Detects generated or packaged artifacts that escape ignore rules | new checker module plus discovery tests |
| `.env` gitignore protection | Env safety | Catches repos that commit runtime env files without explicit intent | `crates/maximus-checks/src/env.rs` or adjacent module |
| Node engines versus CI matrix drift | Release and CI | Warns when supported Node versions and CI reality no longer match | new checker module plus workflow fixtures |
| `files` versus `.npmignore` conflicts | Packaging | Prevents publishing a different file set than maintainers expect | new checker module plus package fixtures |
| Unused config file detection | Cleanup | Helps identify dead config files that silently stop mattering | new checker module, possibly with discovery support |

## What Makes A Good Checker Candidate Here

A good Maximus checker usually does all of the following:

- reads configuration that already exists in the repo
- produces a concrete file-scoped finding
- can be demonstrated with a small fixture
- avoids destructive fixes unless the safe behavior is obvious
- fits into one clear checker id

## Picking The Next One

If you want the easiest place to start:

1. pick one idea from the TypeScript or package metadata rows above
2. add a fixture that reproduces the problem
3. add a single integration test in `crates/maximus-checks/tests/`
4. keep the pull request to one rule family

If you want a smaller entry point than a new checker, go back to [Good First Issues](./good-first-issues.md).
