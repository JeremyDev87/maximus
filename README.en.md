# Maximus

[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](https://github.com/JeremyDev87/maximus/blob/master/LICENSE)

<p align="center">
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.md">한국어</a> |
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.en.md">English</a> |
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.zh-CN.md">中文</a> |
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.es.md">Español</a> |
  <a href="https://github.com/JeremyDev87/maximus/blob/master/README.ja.md">日本語</a>
</p>

Bring order to chaotic configs.

Maximus is a CLI that audits scattered project configuration files, untangles conflicts and duplication, and helps teams keep their development environment organized.

Modern projects stand on top of countless config layers like `tsconfig`, `eslint`, `prettier`, `vite`, `jest`, `next.config`, and `.env`. Maximus restores order when that setup starts to drift.

## Canonical Runtime

Maximus now uses the Rust runtime as its canonical implementation.

- The root `@jeremyfellaz/maximus` npm package is a thin launcher, and the actual execution path is delegated to platform-specific prebuilt Rust binaries.
- The published npm wrapper and GitHub Action also target that Rust runtime path by default.
- The published npm entrypoint is now `npx @jeremyfellaz/maximus audit`, `npx @jeremyfellaz/maximus doctor`, and `npx @jeremyfellaz/maximus fix`, while the installed binary name stays `maximus`.
- `src/**/*.js` stays in the repository as frozen reference code for parity work and comparisons. When no native Rust runtime is available, it only serves as a limited compatibility fallback; it is no longer the primary implementation surface for new runtime or distribution behavior. Rust remains the canonical runtime for Maximus config files and Rust-only flags such as `--only`.
- Historical rewrite planning notes may still exist in maintainer workflows, but public contributor guidance should follow tracked repo docs such as `CONTRIBUTING.md`, `docs/roadmap.md`, `docs/runtime-transition.md`, and `docs/architecture/checker-authoring.md`.

See the [runtime transition document](https://github.com/JeremyDev87/maximus/blob/master/docs/runtime-transition.md) for the transition boundary and contributor rules.

## What It Does

- Detects config conflicts
- Detects duplicate config sources
- Warns on outdated TypeScript options
- Checks broken path alias wiring
- Analyzes ESLint / Prettier conflicts
- Checks missing or mismatched environment variables
- Generates a recommended project-structure report

## Commands

```bash
npx @jeremyfellaz/maximus audit
npx @jeremyfellaz/maximus doctor
npx @jeremyfellaz/maximus fix
```

### `audit`

Inspects the current config state of a project and summarizes the highest-risk issues.

### `doctor`

A more explanatory diagnostic mode than `audit`, with prioritization and structural guidance.

### `fix`

Applies only safe automatic fixes.

Current MVP auto-fixes:

- Create `.env.example` from concrete `.env` files
- Append missing keys to `.env.example`

## Audit Signal Policy

### Env contract

Maximus treats contract files such as `.env.example` as shareable interfaces. The `env-example-secret` warning now considers both the key name and the value shape.

- Keys such as `*_TOKEN`, `*_SECRET`, `*_PASSWORD`, `*_SERVICE_KEY`, `PRIVATE_KEY`, `*_API_KEY`, and `*_ACCESS_KEY` still warn when they contain non-placeholder values.
- High-confidence secret value shapes still warn regardless of the key name, including `sk_live_`, `sk_test_`, `ghp_`, `github_pat_`, `xoxb-`, `xoxp-`, `xoxa-`, AWS `AKIA...`, Google `AIza...`, and private key blocks.
- Public or config-like identifiers such as `NEXT_PUBLIC_*_CLIENT_ID`, URLs, repository names, labels, dates, percentages, and hours do not warn only because the value is long.
- Blank values, `change-me`, `example`, `placeholder`, `your-*`, `localhost`, `127.0.0.1`, `true`, `false`, `0`, and `1` are treated as placeholders.

Env keys that are missing locally but injected by CI or hosting can be declared in `maximus.config.json` or `.maximusrc.json`.

```json
{
  "env": {
    "ciInjectedKeys": ["GH_COLLECTOR_TOKEN"],
    "optionalLocalKeys": ["NEXT_PUBLIC_OKTA_DOMAIN"]
  }
}
```

Both lists are exact keys excluded from `env-missing-concrete`. There is no glob or prefix matching contract, so write concrete key names instead of patterns such as `VERCEL_*`. Unknown config fields fail parsing instead of being ignored silently.

### TypeScript config

No-op excludes that only repeat TypeScript's default excluded directories, such as `exclude: ["node_modules"]`, are hidden from the default audit output. Non-default excludes that carry project intent, such as `generated/**/*.ts` or `dist/**/*.d.ts`, still produce the existing `Info` finding when they do not remove any included files. Warnings for `include` patterns that match no files are unchanged.

## Example Output

```text
Maximus audit
Target: /workspace/my-app

Status: attention needed
Findings: 1 error, 2 warnings, 1 info
Fixes available: 1

Findings
- [error] Path alias target does not exist
  file: packages/web/tsconfig.json
  detail: @ui/* points to src/missing/*
  hint: Update or remove the stale alias target.

- [warn] Missing .env.example contract
  file: .env
  detail: Runtime env files exist, but .env.example is missing.
  hint: Run `maximus fix` to create a blank contract file.
```

## GitHub Action

<!-- release-docs:start -->
After release tags are published, GitHub Actions use the same npm-wrapper entrypoint as well.

```yaml
- uses: JeremyDev87/maximus@<release-tag>
  with:
    command: audit
    path: .
```

Default inputs:

- `command`: `audit`, `doctor`, `fix`
- `path`: project path to inspect, default `.`
- `registry-url`: optional npm registry override for pre-release smoke or private registry validation
- `release-tag`: replace this with a published immutable release tag, for example `v1.0.0`. After the stable major tag passes smoke, `v1` is also valid.

Maintainers should use the [release operator runbook](https://github.com/JeremyDev87/maximus/blob/master/docs/release-operator-runbook.md) for alpha or stable releases and same-tag reruns. Release Drafter only refreshes draft notes on `master`; actual publication and major tag promotion stay gated by the tag-driven release workflow and action smoke results.
<!-- release-docs:end -->

## Local Development

```bash
npm test
cargo test --workspace
node ./bin/maximus.js audit ./test/fixtures/clean-project
```

`node ./bin/maximus.js` prefers the Rust CLI built inside the repository (`target/debug/maximus`, `target/release/maximus`) and otherwise looks for an installed platform-specific Rust binary. If you do not have a local binary yet, build one with `cargo build -p maximus-cli`. When no Rust runtime is available, `src/**/*.js` still provides a compatibility fallback for the basic `audit` / `doctor` / `fix --dry-run` flow, but config auto-loading and Rust-only flags such as `--only`, `--skip`, and `--fail-on` still require the native Rust runtime.

That fallback path exists for compatibility verification and reference preservation. It is not the default development surface for new canonical runtime behavior.

## Recommended For

- Teams running monorepos or multi-package repositories
- Teams struggling to manage many config files
- Teams where new hires frequently get stuck during setup

## Contributing

Contributions are welcome. If you want to add a new check, improve fix safety, or reduce false positives, start with [CONTRIBUTING.md](https://github.com/JeremyDev87/maximus/blob/master/CONTRIBUTING.md) and the [runtime transition document](https://github.com/JeremyDev87/maximus/blob/master/docs/runtime-transition.md) first, because the canonical runtime and distribution surface are now Rust-first and `src/**/*.js` is kept as frozen reference code.

If you want a quick map of where to start in the current repository, read the [contributor roadmap](https://github.com/JeremyDev87/maximus/blob/master/docs/roadmap.md), [good first issues](https://github.com/JeremyDev87/maximus/blob/master/docs/good-first-issues.md), and [checker ideas](https://github.com/JeremyDev87/maximus/blob/master/docs/checker-ideas.md). These docs separate the implemented surface from backlog-style ideas.

For release preparation, promotion, or rerun policy, use the [release operator runbook](https://github.com/JeremyDev87/maximus/blob/master/docs/release-operator-runbook.md).

## Security

If you believe you found a security issue, please do not open a public issue first. Use [SECURITY.md](https://github.com/JeremyDev87/maximus/blob/master/SECURITY.md) for the private reporting path.

## Sponsor

If Maximus helps your team keep config chaos under control, you can support ongoing maintenance via [GitHub Sponsors](https://github.com/sponsors/JeremyDev87).

## License

Maximus is released under the [MIT License](https://github.com/JeremyDev87/maximus/blob/master/LICENSE).
