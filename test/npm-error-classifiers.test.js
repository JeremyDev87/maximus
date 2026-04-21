import assert from "node:assert/strict";
import test from "node:test";
import { classifyNpmLookupError } from "../scripts/classify-npm-lookup-error.mjs";
import { classifyNpmPublishError } from "../scripts/classify-npm-publish-error.mjs";

test("npm lookup classifier distinguishes not-found from registry failures", () => {
  assert.equal(
    classifyNpmLookupError("npm ERR! code E404\nnpm ERR! 404 Not Found - GET https://registry.npmjs.org/pkg"),
    "not-found",
  );
  assert.equal(
    classifyNpmLookupError("npm ERR! code E401\nnpm ERR! Unable to authenticate, your authentication token seems to be invalid."),
    "registry-or-auth-failure",
  );
});

test("npm publish classifier distinguishes already-published from other failures", () => {
  assert.equal(
    classifyNpmPublishError("npm ERR! code EPUBLISHCONFLICT\nnpm ERR! cannot publish over existing version."),
    "already-published",
  );
  assert.equal(
    classifyNpmPublishError("npm ERR! code EOTP\nnpm ERR! This operation requires a one-time password."),
    "publish-failure",
  );
});
