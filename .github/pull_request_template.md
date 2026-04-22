## Summary

Briefly explain what changed and why.

## Related Issues

- Fixes #(issue number)
- Related to #(issue number)

## Changes Made

- Change 1
- Change 2

## Testing

- [ ] `npm test`
- [ ] `node ./bin/maximus.js audit`
- [ ] `node ./bin/maximus.js fix --dry-run`
- [ ] If Rust CLI contract changed: `cargo test -p maximus-cli --test mvp_parity`
- [ ] If text output or parity fixtures changed: `node --test test/reference-parity.test.js`
- [ ] If wrapper, launcher, packed-install, or fallback behavior changed: `node --test test/wrapper-runtime.test.js test/packed-wrapper-fallback.test.js`
- [ ] Manual validation performed

## Contract Impact

- CLI contract status: stable | changed intentionally
- Rust-direct evidence:
- Wrapper or fallback evidence, if applicable:

## Checklist

- [ ] The change is scoped and focused
- [ ] Tests were added or updated when behavior changed
- [ ] Documentation was updated if needed
- [ ] No destructive automatic fix was introduced without clear justification

## Notes for Reviewers

Anything reviewers should pay special attention to.
