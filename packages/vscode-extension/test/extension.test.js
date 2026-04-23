const path = require("node:path");
const test = require("node:test");
const assert = require("node:assert/strict");

const {
  resolveInvocation,
  resolveWorkspaceRoot,
  showMissingWorkspaceWarning,
} = require("../src/extension.ts");

function createFakeVscode({
  workspaceFolders = [],
  activeEditorPath,
  onPick,
} = {}) {
  return {
    Uri: {
      file(fsPath) {
        return { fsPath };
      },
    },
    workspace: {
      workspaceFolders: workspaceFolders.map((folderPath) => ({
        uri: { fsPath: folderPath },
      })),
      getWorkspaceFolder(uri) {
        const matched = workspaceFolders.find((folderPath) => {
          const relative = path.relative(folderPath, uri.fsPath);
          return relative === "" || (!relative.startsWith("..") && !path.isAbsolute(relative));
        });

        return matched ? { uri: { fsPath: matched } } : undefined;
      },
    },
    window: {
      activeTextEditor: activeEditorPath
        ? {
            document: {
              uri: { fsPath: activeEditorPath },
            },
          }
        : undefined,
      async showWorkspaceFolderPick() {
        return onPick?.();
      },
    },
  };
}

test("resolveWorkspaceRoot prefers the active editor workspace in multi-root windows", async () => {
  const fakeVscode = createFakeVscode({
    workspaceFolders: ["/repo-one", "/repo-two"],
    activeEditorPath: "/repo-two/src/index.ts",
  });

  const resolved = await resolveWorkspaceRoot(undefined, fakeVscode);

  assert.equal(resolved, "/repo-two");
});

test("resolveWorkspaceRoot prompts when multiple workspaces exist without a resource or active editor", async () => {
  const fakeVscode = createFakeVscode({
    workspaceFolders: ["/repo-one", "/repo-two"],
    onPick: () => ({ uri: { fsPath: "/repo-two" } }),
  });

  const resolved = await resolveWorkspaceRoot(undefined, fakeVscode);

  assert.equal(resolved, "/repo-two");
});

test("resolveWorkspaceRoot returns undefined when the workspace picker is canceled", async () => {
  const fakeVscode = createFakeVscode({
    workspaceFolders: ["/repo-one", "/repo-two"],
    onPick: () => undefined,
  });

  const resolved = await resolveWorkspaceRoot(undefined, fakeVscode);

  assert.equal(resolved, undefined);
});

test("resolveInvocation routes PATH fallback through cmd on Windows", () => {
  const invocation = resolveInvocation("/workspace", "audit", "win32");

  assert.equal(invocation.command.endsWith("cmd.exe") || invocation.command === "cmd.exe", true);
  assert.deepEqual(invocation.args, ["/d", "/s", "/c", "maximus", "audit"]);
  assert.deepEqual(invocation.options, { windowsHide: true });
});

test("showMissingWorkspaceWarning appends the message and notifies VS Code", async () => {
  const messages = [];
  const warnings = [];

  await showMissingWorkspaceWarning(
    {
      appendLine(message) {
        messages.push(message);
      },
    },
    {
      window: {
        async showWarningMessage(message) {
          warnings.push(message);
        },
      },
    },
  );

  assert.deepEqual(messages, ["워크스페이스를 열고 다시 실행해 주세요."]);
  assert.deepEqual(warnings, ["워크스페이스를 열고 다시 실행해 주세요."]);
});
