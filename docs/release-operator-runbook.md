# Release Operator Runbook

This runbook is for maintainers preparing and promoting Maximus releases.

It documents the preflight checks, the alpha-to-stable promotion path, the v1 major tag policy, and the rerun rules that match the checked-in GitHub workflows. It does not publish anything by itself. The tag-driven workflow in `.github/workflows/release.yml` remains the only release path.

## Release Model

- The release source of truth is the verified package release tag.
- `package.json` version and the tag must match exactly. For example, `1.0.0-alpha.1` must be released from `v1.0.0-alpha.1`.
- `.github/workflows/release.yml` listens only to package release tags such as `v1.0.0` and `v1.0.0-alpha.1`; moving major tags such as `v1` must not trigger npm publication.
- Prerelease versions publish with the npm dist-tag `next`.
- Stable versions publish with the npm dist-tag `latest`.
- v1.0.0 ships the root package `@jeremyfellaz/maximus` plus four native runtime packages:
  - `@jeremyfellaz/maximus-darwin-arm64`
  - `@jeremyfellaz/maximus-darwin-x64`
  - `@jeremyfellaz/maximus-linux-arm64-gnu`
  - `@jeremyfellaz/maximus-linux-x64-gnu`
- Windows and Linux musl do not have prebuilt native runtime packages in v1.0.0. They remain limited compatibility targets only while the frozen JS fallback is still present.
- The moving major action tag `v1` is updated only after the immutable stable tag, such as `v1.0.0`, has completed npm publication plus root action and marketplace wrapper smoke. Never move `v1` for prerelease tags.
- `.github/workflows/release-drafter.yml` only maintains draft notes on `master`. It does not publish npm packages, move `v1`, or run release smoke jobs.
- `workflow_dispatch` reruns are only valid for an existing tag ref. Do not run the release workflow from `master` or any other branch.

## Preflight Before Creating A New Tag

Run this checklist on a clean `master` checkout before creating a new release tag.

1. Pull the target commit from `master`.
2. Confirm the release notes draft looks correct on GitHub. Treat Release Drafter output as notes only.
3. Confirm the package namespace state with npm.
4. Run the local final gate from the exact release candidate SHA.

Suggested commands:

```bash
git switch master
git pull --ff-only

export RELEASE_VERSION=1.0.0
export NPM_CONFIG_CACHE=/tmp/maximus-npm-cache
export PACK_ROOT=/tmp/maximus-release-pack

cargo test --workspace
npm test
node ./scripts/validate-rust-release-wiring.mjs
node --test test/github-action-wiring.test.js
node --test test/release-workflow-context.test.js
node --test test/wrapper-runtime.test.js test/packed-wrapper-fallback.test.js

env npm_config_cache="$NPM_CONFIG_CACHE" npm view "@jeremyfellaz/maximus@$RELEASE_VERSION" version
for package in \
  @jeremyfellaz/maximus-darwin-arm64 \
  @jeremyfellaz/maximus-darwin-x64 \
  @jeremyfellaz/maximus-linux-arm64-gnu \
  @jeremyfellaz/maximus-linux-x64-gnu
do
  env npm_config_cache="$NPM_CONFIG_CACHE" npm view "${package}@${RELEASE_VERSION}" version
done

rm -rf "$PACK_ROOT"
mkdir -p "$PACK_ROOT"
env npm_config_cache="$PACK_ROOT/.npm-cache" npm pack --json --pack-destination "$PACK_ROOT" > "$PACK_ROOT/pack.json"
node ./scripts/run-packed-wrapper-smoke.mjs "$PACK_ROOT/pack.json" ./test/fixtures/clean-project
```

How to read the npm checks:

- Before the first public release, `npm view "<pkg>@$RELEASE_VERSION" version` returning `E404` is acceptable.
- After a release already exists, that exact version should resolve.
- If npm returns an auth or permission failure instead of `E404`, stop and confirm the publishing account has access to the `@jeremyfellaz` scope before tagging.
- Keep `npm_config_cache` pointed at a disposable path for all local npm preflight and `npm pack` commands. Some maintainer machines may have a default npm cache that is not writable by the current user.

## Preflight Before A Same-Tag Rerun

Use this checklist only when a release tag already exists and you need to rerun the release workflow for that exact tag.

The local verification target must match the tag commit, not the current tip of `master`.

Suggested commands:

```bash
export RELEASE_TAG=v1.0.0
export RELEASE_VERSION=1.0.0
export NPM_CONFIG_CACHE=/tmp/maximus-npm-cache
export PACK_ROOT=/tmp/maximus-release-pack

git fetch --tags origin
git switch --detach "$RELEASE_TAG"

cargo test --workspace
npm test
node ./scripts/validate-rust-release-wiring.mjs
node --test test/github-action-wiring.test.js
node --test test/release-workflow-context.test.js
node --test test/wrapper-runtime.test.js test/packed-wrapper-fallback.test.js

env npm_config_cache="$NPM_CONFIG_CACHE" npm view "@jeremyfellaz/maximus@$RELEASE_VERSION" version
for package in \
  @jeremyfellaz/maximus-darwin-arm64 \
  @jeremyfellaz/maximus-darwin-x64 \
  @jeremyfellaz/maximus-linux-arm64-gnu \
  @jeremyfellaz/maximus-linux-x64-gnu
do
  env npm_config_cache="$NPM_CONFIG_CACHE" npm view "${package}@${RELEASE_VERSION}" version
done

rm -rf "$PACK_ROOT"
mkdir -p "$PACK_ROOT"
env npm_config_cache="$PACK_ROOT/.npm-cache" npm pack --json --pack-destination "$PACK_ROOT" > "$PACK_ROOT/pack.json"
node ./scripts/run-packed-wrapper-smoke.mjs "$PACK_ROOT/pack.json" ./test/fixtures/clean-project
```

Rules:

- Do not validate a same-tag rerun from a newer `master` checkout.
- Do not change `package.json` or rebuild a new release candidate for the rerun. The rerun must stay on the exact tagged snapshot.
- After local confirmation, dispatch the workflow with the same tag as both the selected ref and the `release_tag` input.

## Alpha Candidate Flow

Use this path when the package version includes a prerelease suffix such as `-alpha.1`.

1. Open and merge a version-only PR that sets the next prerelease version.
2. Re-run the new-tag preflight checklist on the merged `master` commit.
3. Create and push the annotated release tag that matches `package.json`.
4. Watch the `release` workflow until all publish and smoke jobs finish.
5. Verify the exact prerelease version is available on npm.

Example:

```bash
git switch master
git pull --ff-only
git tag -a v1.0.0-alpha.1 -m "release: v1.0.0-alpha.1"
git push origin v1.0.0-alpha.1
```

Expected behavior:

- The release workflow publishes with dist-tag `next`.
- Platform packages publish before the root wrapper.
- Published-wrapper smoke and GitHub Action smoke both run against the same tagged snapshot.
- The moving major tag `v1` is not moved for prerelease candidates.

## Stable Promotion Flow

Use this path after a prerelease has been validated and you are ready to promote the same feature set to a stable version.

1. Open a version-only PR that removes the prerelease suffix across the package manifests.
2. Merge that PR to `master`.
3. Re-run the new-tag preflight checklist on the new stable commit.
4. Create and push the immutable stable tag that matches the stable package version.
5. Watch the release workflow and verify the stable version resolves on npm.
6. After the immutable tag publish, published-wrapper smoke, and action smoke are all green, move the `v1` major tag to that same commit.
7. Re-run `action-smoke.yml` through `v1` to prove both the root action and marketplace wrapper work from the moving major tag.

Example:

```bash
git switch master
git pull --ff-only
git tag -a v1.0.0 -m "release: v1.0.0"
git push origin v1.0.0
```

Major tag promotion after the release workflow is green:

```bash
export RELEASE_TAG=v1.0.0
export RELEASE_SHA="$(git rev-list -n 1 "$RELEASE_TAG")"
git tag -f v1 "$RELEASE_TAG"
git push origin refs/tags/v1 --force-with-lease
gh workflow run action-smoke.yml --ref v1 -f release_tag=v1 -f release_sha="$RELEASE_SHA"
```

Expected behavior:

- The release workflow publishes with dist-tag `latest`.
- The release tag and `package.json` version match exactly.
- Moving `v1` does not trigger the tag-driven release workflow.
- Root action smoke and marketplace wrapper smoke both pass before and after `v1` moves.
- Release Drafter continues to prepare the next notes draft on `master`, but it does not publish, promote, or move tags by itself.

## Safe Reruns

The release workflow is rerun-safe for an existing tag.

Use `workflow_dispatch` only with the same tag as both the selected ref and the `release_tag` input.

Example:

```bash
gh workflow run release.yml --ref v1.0.0 -f release_tag=v1.0.0
```

Rules:

- Do not dispatch from `master`.
- Do not dispatch from an unrelated branch.
- If you want a local confirmation before dispatching, run it from `git switch --detach <tag>` so the local snapshot matches the tag that the workflow will use.
- If a package version is already published, the workflow should skip that publish step instead of failing the entire release.
- Trusted publishing is attempted first. If it fails and `NPM_TOKEN` is configured, the workflow retries with `NPM_TOKEN`.

## Failure Handling

### Tag and version do not match

- Symptom: release validation fails because the tag and `package.json` version differ.
- Response: fix the version mismatch in a PR and create the correct new tag after merge.
- Do not retarget an existing release tag in place.

### npm reports auth or permission failure

- Symptom: `npm view` or `npm publish` fails with registry or auth errors instead of `E404` or "already published".
- Response: stop, confirm scope ownership and token/trusted-publishing setup, then rerun the same tag after the credential problem is fixed.

### Publish step says the package already exists

- Symptom: the workflow reports an already-published package version.
- Response: treat that as rerun-safe when the package contents are expected to match the same tag. Investigate only if the release state is incomplete or inconsistent across packages.

### Published smoke fails

- Symptom: the workflow publishes packages but the published-wrapper smoke or action smoke fails.
- Response: fix the underlying issue in a PR, merge it, and create a new version/tag. Do not mutate an existing published release into a different artifact.

### Draft notes need refresh

- Symptom: the draft release notes on GitHub are stale or missing merged PRs.
- Response: rerun Release Drafter on `master` only. Keep that rerun separate from npm publication.

## Maintainer Notes

- Keep release-related docs aligned: `README.md`, `README.en.md`, `CONTRIBUTING.md`, this runbook, and the release workflows should describe the same release model.
- If a change touches release wiring, package naming, or packed-install behavior, re-run the full preflight checklist before asking a maintainer to tag a release.
- Do not run tag creation, tag movement, release workflow dispatch, or npm publication without an explicit maintainer confirmation for that operation.
