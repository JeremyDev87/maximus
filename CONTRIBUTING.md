# Contributing to Maximus

Thanks for helping improve Maximus.

This project aims to make config-heavy repositories easier to understand, safer to change, and faster to onboard into. Good contributions usually improve one of those three outcomes.

## Canonical Runtime

Maximus now treats Rust as the canonical runtime for the published CLI, the npm wrapper, and the GitHub Action.

- `bin/maximus.js` is a thin launcher for the published `@jeremyfellaz/maximus` wrapper and prefers repository Rust builds plus installed platform-specific Rust binaries.
- `src/**/*.js` stays in the repository as frozen reference code for parity checks, golden output generation, and roadmap context. It also remains available as a compatibility fallback for legacy-compatible CLI invocations, but config auto-loading and Rust-only CLI flags must still run on the canonical Rust runtime.
- New user-facing runtime or distribution behavior should land in the Rust crates, the thin launcher, and the docs. Do not treat `src/**/*.js` as the default implementation surface for new product behavior.
- Historical rewrite planning notes may still exist in maintainer workflows, but tracked repository docs are the public source of truth for contributors. Do not treat older JS backlog notes as the default implementation lane.

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

- Treat tracked contributor docs such as `docs/roadmap.md`, `docs/runtime-transition.md`, and `docs/architecture/checker-authoring.md` as the public source of truth first.
- Re-check current `master`, merge history, and the active board before assuming a local planning slice is still pending.
- Do not reuse stale JS-oriented file lists inside older planning notes as ownership guidance for new implementation work.
- Keep new runtime behavior in the Rust crates, the launcher, and the docs instead of reviving the frozen JS reference tree as a default implementation lane.

## Development Setup

```bash
git clone https://github.com/JeremyDev87/maximus.git
cd maximus
npm test
cargo test --workspace
node ./bin/maximus.js audit ./test/fixtures/clean-project
```

## Vite Alias Reproduction

Run this from the repository checkout containing the **unpublished fix candidate**. It builds and directly runs `target/debug/maximus`; the public npm `0.1.4` install is a separate baseline and does not contain this fix. Prerequisites: Rust/Cargo, Node.js 24, npm, and a POSIX shell on macOS or Linux. Node.js 24 is for this Vite demo; the Maximus wrapper requirement remains Node.js >=20.

Copy the entire block. It creates only dummy inputs in a fresh temporary directory, pins Vite 8.3.5 there, and removes that directory and its caches on exit. Expected nonzero exits are handled explicitly so the first failure does not stop the demo. Alias correction is manual, not `maximus fix`.

```bash
(
set -eu
MAXIMUS_REPO="$(pwd -P)"
cargo build -p maximus-cli
MAXIMUS_BIN="$MAXIMUS_REPO/target/debug/maximus"
DEMO_TMP="$(mktemp -d "${TMPDIR:-/tmp}/maximus-alias-demo.XXXXXX")"
DEMO_TMP="$(cd "$DEMO_TMP" && pwd -P)"
trap 'rm -rf -- "$DEMO_TMP"' EXIT
export MAXIMUS_DEMO_ROOT="$DEMO_TMP/demo"
export TMPDIR="$DEMO_TMP/tmp"
export npm_config_cache="$DEMO_TMP/npm-cache"
export NODE_COMPILE_CACHE="$DEMO_TMP/node-cache"
export BUN_RUNTIME_TRANSPILER_CACHE_PATH="$DEMO_TMP/bun-cache"
mkdir -p "$MAXIMUS_DEMO_ROOT/src" "$TMPDIR" "$NODE_COMPILE_CACHE" "$BUN_RUNTIME_TRANSPILER_CACHE_PATH"
cat > "$MAXIMUS_DEMO_ROOT/package.json" <<'JSON'
{"name":"maximus-alias-demo","private":true,"type":"module","scripts":{"build":"vite build"}}
JSON
cat > "$MAXIMUS_DEMO_ROOT/index.html" <<'HTML'
<!doctype html><html><body><script type="module" src="/src/main.ts"></script></body></html>
HTML
cat > "$MAXIMUS_DEMO_ROOT/src/main.ts" <<'TS'
import { message } from '@/message';
document.body.textContent = message;
TS
cat > "$MAXIMUS_DEMO_ROOT/src/message.ts" <<'TS'
export const message = 'Maximus alias demo';
TS
cat > "$MAXIMUS_DEMO_ROOT/tsconfig.json" <<'JSON'
{"compilerOptions":{"baseUrl":".","paths":{"@/*":["src/*"]}},"include":["src"]}
JSON
npm install --ignore-scripts --no-audit --no-fund --prefix "$MAXIMUS_DEMO_ROOT" vite@8.3.5

# 1. A nonexistent absolute Vite alias conflicts with TypeScript's src path.
node --input-type=module -e 'import fs from "node:fs"; const root=process.env.MAXIMUS_DEMO_ROOT; fs.writeFileSync(root+"/vite.config.mjs", `export default { resolve: { alias: { "@": ${JSON.stringify(root+"/missing")} } } };\n`);'
if "$MAXIMUS_BIN" audit "$MAXIMUS_DEMO_ROOT" --only vite-tsconfig-alias --json --fail-on info; then audit_exit=0; else audit_exit=$?; fi
if (cd "$MAXIMUS_DEMO_ROOT" && npm run build); then build_exit=0; else build_exit=$?; fi
printf 'conflict: audit=%s build=%s\n' "$audit_exit" "$build_exit"
test "$audit_exit" -eq 1
test "$build_exit" -eq 1

# 2. Manually point Vite at the real src directory, then rerun both commands.
node --input-type=module -e 'import fs from "node:fs"; const root=process.env.MAXIMUS_DEMO_ROOT; fs.writeFileSync(root+"/vite.config.mjs", `export default { resolve: { alias: { "@": ${JSON.stringify(root+"/src")} } } };\n`);'
"$MAXIMUS_BIN" audit "$MAXIMUS_DEMO_ROOT" --only vite-tsconfig-alias --json --fail-on info
(cd "$MAXIMUS_DEMO_ROOT" && npm run build)
printf 'manual correction: audit=0 build=0\n'

# 3. Remove the explicit alias and declare native tsconfig paths.
cat > "$MAXIMUS_DEMO_ROOT/vite.config.mjs" <<'JS'
export default { resolve: { tsconfigPaths: true } };
JS
"$MAXIMUS_BIN" audit "$MAXIMUS_DEMO_ROOT" --only vite-tsconfig-alias --json --fail-on info
(cd "$MAXIMUS_DEMO_ROOT" && npm run build)
printf 'native true: audit=0 build=0\n'

# Negative diagnostic control: false still builds with the observed Vite/Rolldown pair.
cat > "$MAXIMUS_DEMO_ROOT/vite.config.mjs" <<'JS'
export default { resolve: { tsconfigPaths: false } };
JS
if "$MAXIMUS_BIN" audit "$MAXIMUS_DEMO_ROOT" --only vite-tsconfig-alias --json --fail-on info; then audit_exit=0; else audit_exit=$?; fi
(cd "$MAXIMUS_DEMO_ROOT" && npm run build)
printf 'native false: audit=%s build=0\n' "$audit_exit"
test "$audit_exit" -eq 1
)
```

The conflict yields one `warn` (`Vite alias "@" differs from tsconfig paths`) and a build import-resolution failure. Manual correction and native `true` yield no filtered findings and build exit 0. The negative control yields one `info` ending in `:tsconfig-only` and audit exit 1 with `--fail-on info`, while Vite 8.3.5/Rolldown 1.2.16 still builds successfully. These results check declared configuration; they do not establish that `true` caused build success or that `false` must fail a build.

The candidate confirms native paths only for literal `true` in `export default { ... }` or `export default defineConfig({ ... })`. Dynamic values, callbacks, plugins, duplicate/escaped/computed keys and spreads remain unconfirmed. Maximus does not execute the Vite config during audit, certify the installed Vite version, or resolve every import. The build commands do execute this dummy Vite config. The demo's `--only` and `--fail-on` require native Rust; the limited JS fallback cannot run them.

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

## Contract-Sensitive Changes

Treat the user-facing CLI contract as fail-closed.

- Text output, JSON shape, exit semantics, and wrapper invocation behavior are not soft conventions.
- If a pull request changes any of those surfaces intentionally, update the matching regression evidence in the same pull request. The usual Rust-direct contract files are `test/reference-parity.test.js`, `test/golden-rust/*`, `crates/maximus-cli/tests/mvp_parity.rs`, and the affected CLI integration tests.
- If the change touches launcher resolution, wrapper invocation, packed-install behavior, or frozen-JS fallback boundaries, also run and cite `test/wrapper-runtime.test.js` and, when the packed-install path matters, `test/packed-wrapper-fallback.test.js`.
- Do not defer golden or parity updates to a follow-up cleanup pull request.
- Reviewers should block contract-sensitive changes when the pull request does not include matching test evidence and doc alignment.

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
If your change is contract-sensitive, also run the smallest matching direct gate instead of relying on the wrapper baseline alone. For Rust CLI behavior this usually means `cargo test -p maximus-cli --test mvp_parity`; for launcher or packed-install behavior, use `node --test test/wrapper-runtime.test.js` and `node --test test/packed-wrapper-fallback.test.js` as needed.

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
- Whether the user-facing CLI contract stayed stable or changed intentionally
- Which golden/parity checks back that claim when text, JSON, exit, or wrapper behavior moved
- Any known limitations or follow-up work

Small, well-tested pull requests are much easier to review and merge quickly.

## Security

If you discover a security issue, please do not open a public issue. Follow the private reporting instructions in [SECURITY.md](https://github.com/JeremyDev87/maximus/blob/master/SECURITY.md).
