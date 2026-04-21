import assert from "node:assert/strict";
import test from "node:test";

import { parseJsonc } from "../src/lib/jsonc.js";

test("parseJsonc supports comments and trailing commas", () => {
  const parsed = parseJsonc(`
    {
      // comment
      "compilerOptions": {
        "baseUrl": ".",
      },
      "extends": "./tsconfig.base.json",
    }
  `);

  assert.deepEqual(parsed, {
    compilerOptions: {
      baseUrl: ".",
    },
    extends: "./tsconfig.base.json",
  });
});

test("parseJsonc supports CRLF line endings", () => {
  const parsed = parseJsonc("{\r\n  // comment\r\n  \"compilerOptions\": {\r\n    \"baseUrl\": \".\",\r\n  },\r\n  \"extends\": \"./tsconfig.base.json\",\r\n}\r\n");

  assert.deepEqual(parsed, {
    compilerOptions: {
      baseUrl: ".",
    },
    extends: "./tsconfig.base.json",
  });
});
