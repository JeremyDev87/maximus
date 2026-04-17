# Contributing to Maximus

Thanks for helping improve Maximus.

This project aims to make config-heavy repositories easier to understand, safer to change, and faster to onboard into. Good contributions usually improve one of those three outcomes.

## Ways to Contribute

- Report bugs or false positives in existing checks
- Propose new analyzers or safe automatic fixes
- Improve docs, examples, and onboarding
- Add tests for tricky config edge cases

## Before You Start

For larger features or behavior changes, open an issue first so we can agree on scope and expected behavior before code is written.

For small fixes, feel free to open a pull request directly.

## Development Setup

```bash
git clone https://github.com/JeremyDev87/maximus.git
cd maximus
npm test
node ./bin/maximus.js audit
node ./bin/maximus.js fix --dry-run
```

## Project Shape

```text
bin/              CLI entrypoint
src/checks/       config analyzers
src/core/         discovery, aggregation, reporting, fix orchestration
src/lib/          parsing and filesystem helpers
test/             regression tests
```

## Contribution Guidelines

- Keep changes focused. One behavior change per pull request is ideal.
- Prefer safe heuristics over surprising automation.
- Add or update tests when changing detection or fix behavior.
- Update `README.md` if the user-facing behavior or supported checks change.
- Avoid destructive fixes unless the user can clearly preview and understand them.

## Testing

Before opening a pull request, run:

```bash
npm test
node ./bin/maximus.js audit
node ./bin/maximus.js fix --dry-run
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

If you discover a security issue, please do not open a public issue. Follow the private reporting instructions in [SECURITY.md](SECURITY.md).
