import { constants as fsConstants } from "node:fs";
import { access } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

export async function resolvePackedWrapperLaunch(installRoot) {
  const binShimPath = path.join(installRoot, "node_modules", ".bin", "maximus");
  if (await isAccessible(binShimPath, fsConstants.X_OK)) {
    return {
      command: binShimPath,
      args: [],
    };
  }

  const packageEntrypointPath = path.join(
    installRoot,
    "node_modules",
    "@jeremyfellaz",
    "maximus",
    "bin",
    "maximus.js",
  );
  if (await isAccessible(packageEntrypointPath, fsConstants.R_OK)) {
    return {
      command: process.execPath,
      args: [packageEntrypointPath],
    };
  }

  throw new Error(
    [
      `Packed install at "${installRoot}" did not expose a runnable Maximus entrypoint.`,
      'Neither "node_modules/.bin/maximus" nor "node_modules/@jeremyfellaz/maximus/bin/maximus.js" exists.',
    ].join(" "),
  );
}

async function isAccessible(targetPath, mode) {
  try {
    await access(targetPath, mode);
    return true;
  } catch {
    return false;
  }
}
