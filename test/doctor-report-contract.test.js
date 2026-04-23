import assert from "node:assert/strict";
import test from "node:test";

import { formatDoctorReport } from "../src/core/format-report.js";

test("JS doctor formatter includes the Top 3 priorities section", () => {
  const report = formatDoctorReport({
    rootDir: "/tmp/project",
    summary: {
      status: "blocking issues",
      fixesAvailable: 1,
    },
    structure: {
      isMonorepo: false,
      packageCount: 1,
      configFiles: 2,
      envDirectories: 1,
      recommendations: [],
    },
    findings: [
      {
        severity: "error",
        title: "Missing example env file",
        file: "/tmp/project/.env",
        detail: "A committed .env.example file is missing.",
        hint: "Create .env.example with safe defaults.",
        fixable: false,
      },
      {
        severity: "warn",
        title: "Path alias target does not exist",
        file: "/tmp/project/tsconfig.json",
        detail: "@app/* points to src/missing/*.",
        hint: "Update the alias to an existing directory.",
        fixable: false,
      },
      {
        severity: "info",
        title: "Package scripts are tidy",
        file: null,
        detail: "No extra work is needed.",
        hint: "",
        fixable: false,
      },
    ],
  });

  assert.match(report, /Top 3 priorities/);
  assert.match(report, /1\. \[error\] Missing example env file/);
  assert.match(report, /   file: \.env/);
  assert.match(report, /   next: Create \.env\.example with safe defaults\./);
  assert.match(report, /2\. \[warn\] Path alias target does not exist/);
  assert.match(report, /3\. \[info\] Package scripts are tidy/);
});
