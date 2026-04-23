const fs = require("node:fs");
const path = require("node:path");
const { spawn } = require("node:child_process");

const CHANNEL_NAME = "Maximus";
const COMMAND_IDS = {
  audit: "maximus.runAudit",
  doctor: "maximus.runDoctor",
  fix: "maximus.runFix",
};

/**
 * @param {import("vscode").ExtensionContext} context
 */
function activate(context) {
  const vscode = getVscode();
  const output = vscode.window.createOutputChannel(CHANNEL_NAME);

  context.subscriptions.push(output);
  context.subscriptions.push(
    vscode.commands.registerCommand(COMMAND_IDS.audit, (resource) => {
      return runMaximusCommand("audit", output, resource);
    }),
    vscode.commands.registerCommand(COMMAND_IDS.doctor, (resource) => {
      return runMaximusCommand("doctor", output, resource);
    }),
    vscode.commands.registerCommand(COMMAND_IDS.fix, (resource) => {
      return runMaximusCommand("fix", output, resource);
    }),
  );

  output.appendLine("Maximus VS Code extension activated.");
}

function deactivate() {}

/**
 * @param {"audit" | "doctor" | "fix"} mode
 * @param {import("vscode").OutputChannel} output
 * @param {unknown} resource
 */
async function runMaximusCommand(mode, output, resource) {
  const vscode = getVscode();
  const workspaceRoot = await resolveWorkspaceRoot(resource);

  if (!workspaceRoot) {
    await showMissingWorkspaceWarning(output, vscode);
    return;
  }

  const invocation = resolveInvocation(workspaceRoot, mode);
  output.clear();
  output.show(true);
  output.appendLine(`Workspace: ${workspaceRoot}`);
  output.appendLine(`Command: ${formatCommand(invocation.command, invocation.args)}`);
  output.appendLine("");

  await new Promise((resolve) => {
    const child = spawn(invocation.command, invocation.args, {
      cwd: workspaceRoot,
      env: process.env,
      stdio: ["ignore", "pipe", "pipe"],
      ...invocation.options,
    });
    let settled = false;

    const finish = () => {
      if (settled) {
        return;
      }

      settled = true;
      resolve();
    };

    child.stdout.on("data", (chunk) => {
      output.append(chunk.toString());
    });

    child.stderr.on("data", (chunk) => {
      output.append(chunk.toString());
    });

    child.on("error", async (error) => {
      output.appendLine("");
      output.appendLine(`실행 실패: ${error.message}`);
      await vscode.window.showErrorMessage(`Maximus 실행에 실패했습니다: ${error.message}`);
      finish();
    });

    child.on("close", (code) => {
      output.appendLine("");
      output.appendLine(`종료 코드: ${code ?? "unknown"}`);
      finish();
    });
  });
}

/**
 * @param {import("vscode").OutputChannel} output
 * @param {typeof import("vscode")} vscode
 */
async function showMissingWorkspaceWarning(output, vscode = getVscode()) {
  const message = "워크스페이스를 열고 다시 실행해 주세요.";
  output.appendLine(message);
  await vscode.window.showWarningMessage(message);
}

/**
 * @param {unknown} resource
 * @returns {string | undefined}
 */
async function resolveWorkspaceRoot(resource, vscode = getVscode()) {
  const selectedFolder = resolveWorkspaceFolderFromResource(resource, vscode);
  if (selectedFolder) {
    return selectedFolder;
  }

  const activeEditorFolder = resolveActiveEditorWorkspaceRoot(vscode);
  if (activeEditorFolder) {
    return activeEditorFolder;
  }

  const workspaceFolders = vscode.workspace.workspaceFolders ?? [];
  if (workspaceFolders.length === 1) {
    return workspaceFolders[0].uri.fsPath;
  }

  if (workspaceFolders.length > 1) {
    const pickedFolder = await vscode.window.showWorkspaceFolderPick({
      placeHolder: "Maximus를 실행할 워크스페이스를 선택해 주세요.",
    });

    return pickedFolder?.uri.fsPath;
  }

  return undefined;
}

/**
 * @param {unknown} resource
 * @param {typeof import("vscode")} vscode
 * @returns {string | undefined}
 */
function resolveWorkspaceFolderFromResource(resource, vscode = getVscode()) {
  if (!resource || typeof resource !== "object") {
    return undefined;
  }

  const uri = /** @type {{ fsPath?: string, scheme?: string }} */ (resource);
  if (typeof uri.fsPath !== "string" || uri.fsPath.length === 0) {
    return undefined;
  }

  const folder = vscode.workspace.getWorkspaceFolder(vscode.Uri.file(uri.fsPath));
  if (folder) {
    return folder.uri.fsPath;
  }

  return fs.existsSync(uri.fsPath) && fs.statSync(uri.fsPath).isDirectory()
    ? uri.fsPath
    : path.dirname(uri.fsPath);
}

/**
 * @param {typeof import("vscode")} vscode
 * @returns {string | undefined}
 */
function resolveActiveEditorWorkspaceRoot(vscode = getVscode()) {
  const activeEditorUri = vscode.window.activeTextEditor?.document?.uri;
  if (!activeEditorUri) {
    return undefined;
  }

  return vscode.workspace.getWorkspaceFolder(activeEditorUri)?.uri.fsPath;
}

/**
 * @param {string} workspaceRoot
 * @param {"audit" | "doctor" | "fix"} mode
 * @param {NodeJS.Platform} platform
 */
function resolveInvocation(workspaceRoot, mode, platform = process.platform) {
  const localCli = path.join(workspaceRoot, "bin", "maximus.js");
  if (fs.existsSync(localCli)) {
    return {
      command: process.execPath,
      args: [localCli, mode],
      options: {},
    };
  }

  if (platform === "win32") {
    return {
      command: process.env.comspec || "cmd.exe",
      args: ["/d", "/s", "/c", "maximus", mode],
      options: {
        windowsHide: true,
      },
    };
  }

  return {
    command: "maximus",
    args: [mode],
    options: {},
  };
}

/**
 * @param {string} command
 * @param {string[]} args
 */
function formatCommand(command, args) {
  return [command, ...args].join(" ");
}

function getVscode() {
  return require("vscode");
}

module.exports = {
  activate,
  deactivate,
  resolveActiveEditorWorkspaceRoot,
  resolveInvocation,
  resolveWorkspaceFolderFromResource,
  resolveWorkspaceRoot,
  showMissingWorkspaceWarning,
};
