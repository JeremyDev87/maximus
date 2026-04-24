import assert from "node:assert/strict";
import test from "node:test";

import {
  assertReleaseUpgrade,
  compareReleaseVersions,
  resolveReleasePlan,
} from "../scripts/lib/release.mjs";

test("resolveReleasePlan maps stable and prerelease tags to npm dist-tags", () => {
  assert.deepEqual(resolveReleasePlan("v1.2.3"), {
    tag: "v1.2.3",
    version: "1.2.3",
    isPrerelease: false,
    npmDistTag: "latest",
  });
  assert.deepEqual(resolveReleasePlan("v1.2.3-beta.1"), {
    tag: "v1.2.3-beta.1",
    version: "1.2.3-beta.1",
    isPrerelease: true,
    npmDistTag: "next",
  });
});

test("release helpers reject invalid semver prerelease identifiers", () => {
  for (const tag of [
    "v01.2.3",
    "v1.02.3",
    "v1.2.03",
    "v1.2.3-.",
    "v1.2.3-alpha..1",
    "v1.2.3-01",
  ]) {
    assert.throws(() => resolveReleasePlan(tag), /Tag must look like/);
  }
});

test("compareReleaseVersions follows stable and prerelease ordering", () => {
  assert.equal(compareReleaseVersions("1.2.3-alpha.1", "1.2.3-alpha.2"), -1);
  assert.equal(compareReleaseVersions("1.2.3-alpha.2", "1.2.3-beta.1"), -1);
  assert.equal(compareReleaseVersions("1.2.3-beta.1", "1.2.3"), -1);
  assert.equal(compareReleaseVersions("1.2.3", "1.2.4"), -1);
  assert.equal(compareReleaseVersions("1.2.3", "1.2.3"), 0);
});

test("assertReleaseUpgrade rejects equal or lower target versions", () => {
  assert.doesNotThrow(() => assertReleaseUpgrade("1.2.3", "1.2.4"));
  assert.throws(() => assertReleaseUpgrade("1.2.3", "1.2.3"), /must be greater/);
  assert.throws(() => assertReleaseUpgrade("1.2.3", "1.2.3-beta.1"), /must be greater/);
});
