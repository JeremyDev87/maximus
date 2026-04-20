import assert from "node:assert/strict";
import test from "node:test";
import { validateRustReleaseWiring } from "../scripts/validate-rust-release-wiring.mjs";

test("Rust release wiring validation passes for the checked-in GitHub automation files", async () => {
  const summary = await validateRustReleaseWiring(process.cwd());

  assert.equal(summary.checkedFiles.length, 10);
  assert.deepEqual(summary.platformPackages, [
    "maximus-darwin-arm64",
    "maximus-darwin-x64",
    "maximus-linux-arm64-gnu",
    "maximus-linux-x64-gnu",
  ]);
});
