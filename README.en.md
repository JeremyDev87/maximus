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

## Runtime Transition

Maximus is now prioritizing a Rust rewrite as its canonical runtime direction instead of continuing the JS backlog as the primary implementation path.

- The currently shipped CLI and the executable code in this repository still run on Node.js today.
- The user-facing command surface stays the same: `npx maximus audit`, `npx maximus doctor`, `npx maximus fix`
- Until the cutover lands, the current JS runtime stays as the reference implementation.
- `docs/plan/001` through `012` are no longer treated as direct JS implementation tickets; they are Rust v1 spec inputs.
- `docs/plan/013+` and the older JS backlog stay deferred until the Rust cutover is complete.

See the [runtime transition document](https://github.com/JeremyDev87/maximus/blob/master/docs/runtime-transition.md) for the transition boundary, phase map, and contributor rules.

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
npx maximus audit
npx maximus doctor
npx maximus fix
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

After the release tags are published, you can use the same npm-wrapper entrypoint from GitHub Actions as well.

```yaml
- uses: JeremyDev87/maximus@v0
  with:
    command: audit
    path: .
```

Default inputs:

- `command`: `audit`, `doctor`, `fix`
- `path`: project path to inspect, default `.`

## Local Development

```bash
npm test
node ./bin/maximus.js audit
node ./bin/maximus.js fix --dry-run
```

These local commands continue to validate the current Node.js reference implementation. Even after the Rust bootstrap starts, user-facing command examples stay in the `npx maximus ...` form.

## Recommended For

- Teams running monorepos or multi-package repositories
- Teams struggling to manage many config files
- Teams where new hires frequently get stuck during setup

## Contributing

Contributions are welcome. If you want to add a new check, improve fix safety, or reduce false positives, start with [CONTRIBUTING.md](https://github.com/JeremyDev87/maximus/blob/master/CONTRIBUTING.md) and the [runtime transition document](https://github.com/JeremyDev87/maximus/blob/master/docs/runtime-transition.md) first, because the active priority is the Rust rewrite family rather than direct JS backlog expansion.

## Security

If you believe you found a security issue, please do not open a public issue first. Use [SECURITY.md](https://github.com/JeremyDev87/maximus/blob/master/SECURITY.md) for the private reporting path.

## Sponsor

If Maximus helps your team keep config chaos under control, you can support ongoing maintenance via [GitHub Sponsors](https://github.com/sponsors/JeremyDev87).

## License

Maximus is released under the [MIT License](https://github.com/JeremyDev87/maximus/blob/master/LICENSE).
