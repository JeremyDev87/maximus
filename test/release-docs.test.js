import assert from "node:assert/strict";
import path from "node:path";
import test from "node:test";
import { readFile } from "node:fs/promises";
import { updateReleaseDocs } from "../scripts/update-release-docs.mjs";

test("release docs generator preserves the static release-tag example", async () => {
  const readmePath = path.join(process.cwd(), "README.md");
  const readmeText = await readFile(readmePath, "utf8");
  const nextText = updateReleaseDocs(readmePath, readmeText);

  assert.equal(nextText, readmeText);
  assert.match(nextText, /예: `v0\.1\.0`/);
  assert.doesNotMatch(nextText, /예: `v0\.1\.3`/);
});

test("English release docs generator preserves the static release-tag example", async () => {
  const readmePath = path.join(process.cwd(), "README.en.md");
  const readmeText = await readFile(readmePath, "utf8");
  const nextText = updateReleaseDocs(readmePath, readmeText);

  assert.equal(nextText, readmeText);
  assert.match(nextText, /for example `v0\.1\.0`/);
  assert.doesNotMatch(nextText, /for example `v0\.1\.3`/);
});
