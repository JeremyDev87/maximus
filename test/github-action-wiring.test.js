import assert from "node:assert/strict";
import test from "node:test";
import { validateRustReleaseWiring } from "../scripts/validate-rust-release-wiring.mjs";

test("Rust release wiring validation passes for the checked-in GitHub automation files", async () => {
  const summary = await validateRustReleaseWiring(process.cwd());

  assert.equal(summary.checkedFiles.length, 24);
  assert.deepEqual(summary.platformPackages, [
    "@jeremyfellaz/maximus-darwin-arm64",
    "@jeremyfellaz/maximus-darwin-x64",
    "@jeremyfellaz/maximus-linux-arm64-gnu",
    "@jeremyfellaz/maximus-linux-x64-gnu",
  ]);
});
