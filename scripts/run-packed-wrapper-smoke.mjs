import assert from "node:assert/strict";
import os from "node:os";
import path from "node:path";
import { spawn, spawnSync } from "node:child_process";
import { createReadStream } from "node:fs";
import { access, chmod, copyFile, cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { createServer } from "node:http";
import { fileURLToPath } from "node:url";
import {
  assertInstalledNativeRuntime,
  inspectInstalledNativeRuntime,
} from "./assert-installed-native-runtime.mjs";
import { resolvePackedWrapperLaunch } from "./lib/packed-wrapper-launch.mjs";

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const repoRoot = path.resolve(scriptDir, "..");
const rootPackageName = "@jeremyfellaz/maximus";
const allPlatformPackages = [
  {
    packageName: "@jeremyfellaz/maximus-darwin-arm64",
    directoryName: "maximus-darwin-arm64",
    directory: path.join(repoRoot, "npm", "maximus-darwin-arm64"),
    platform: "darwin",
    arch: "arm64",
  },
  {
    packageName: "@jeremyfellaz/maximus-darwin-x64",
    directoryName: "maximus-darwin-x64",
    directory: path.join(repoRoot, "npm", "maximus-darwin-x64"),
    platform: "darwin",
    arch: "x64",
  },
  {
    packageName: "@jeremyfellaz/maximus-linux-arm64-gnu",
    directoryName: "maximus-linux-arm64-gnu",
    directory: path.join(repoRoot, "npm", "maximus-linux-arm64-gnu"),
    platform: "linux",
    arch: "arm64",
    libc: "glibc",
  },
  {
    packageName: "@jeremyfellaz/maximus-linux-x64-gnu",
    directoryName: "maximus-linux-x64-gnu",
    directory: path.join(repoRoot, "npm", "maximus-linux-x64-gnu"),
    platform: "linux",
    arch: "x64",
    libc: "glibc",
  },
];

const [packJsonPath, fixtureArg] = process.argv.slice(2);

if (!packJsonPath) {
  console.error(
    "Usage: node ./scripts/run-packed-wrapper-smoke.mjs <npm-pack-json-path> <target-dir>",
  );
  process.exit(1);
}

const fixtureDir = path.resolve(fixtureArg ?? path.join(repoRoot, "test/fixtures/clean-project"));
const platformPackage = resolvePlatformPackage();

if (!platformPackage) {
  throw new Error(formatUnsupportedPlatformMessage());
}

const tempRoot = await mkdtemp(path.join(os.tmpdir(), "maximus-packed-wrapper-"));
let localRegistry;
try {
  const rootTarball = await resolveRootTarball(packJsonPath);
  const rustBinary = await ensureRustBinary();
  const platformTarballs = await packAllPlatformPackages(
    tempRoot,
    rustBinary,
    platformPackage.packageName,
  );
  localRegistry = await startLocalRegistry(platformTarballs);
  const installRoot = await installPackedPackages(tempRoot, rootTarball, {
    directoryName: "install-with-optional-runtime",
    registryUrl: localRegistry.url,
  });

  await assertInstalledNativeRuntime(installRoot);
  await removeJsFallback(installRoot);
  await runScenario(installRoot);

  const omitOptionalInstallRoot = await installPackedPackages(tempRoot, rootTarball, {
    directoryName: "install-without-optional-runtime",
    omitOptional: true,
  });

  const omittedRuntime = await inspectInstalledNativeRuntime(omitOptionalInstallRoot);
  assert.equal(
    omittedRuntime.state,
    "missing",
    `Expected omit=optional install to exclude the native runtime package, but observed "${omittedRuntime.state}".`,
  );
  await runScenario(omitOptionalInstallRoot);
  await runFallbackBlockingScenarios(omitOptionalInstallRoot);

  console.log(`Packed wrapper smoke passed via ${path.basename(rootTarball)}.`);
} finally {
  if (localRegistry) {
    await localRegistry.close();
  }
  await rm(tempRoot, { recursive: true, force: true });
}

async function resolveRootTarball(jsonPath) {
  const absoluteJsonPath = path.resolve(jsonPath);
  const packResult = JSON.parse(await readFile(absoluteJsonPath, "utf8"));
  const filename = packResult[0]?.filename;

  assert.equal(typeof filename, "string", "npm pack JSON must include filename");

  if (path.isAbsolute(filename)) {
    return filename;
  }

  const jsonRelativePath = path.join(path.dirname(absoluteJsonPath), filename);
  try {
    await access(jsonRelativePath);
    return jsonRelativePath;
  } catch {
    return path.join(repoRoot, filename);
  }
}

function resolvePlatformPackage() {
  return (
    allPlatformPackages.find((candidate) => {
      if (candidate.platform !== process.platform || candidate.arch !== process.arch) {
        return false;
      }

      if (candidate.libc === "glibc") {
        return hasGlibcRuntime();
      }

      return true;
    }) ?? null
  );
}

function hasGlibcRuntime() {
  return Boolean(process.report?.getReport?.().header?.glibcVersionRuntime);
}

function formatUnsupportedPlatformMessage() {
  if (process.platform === "linux" && !hasGlibcRuntime()) {
    return "Packed wrapper smoke does not support Linux musl yet.";
  }

  return `Packed wrapper smoke does not support ${process.platform}-${process.arch}.`;
}

async function packAllPlatformPackages(tempRoot, rustBinary, currentPlatformPackageName) {
  const tarballs = new Map();
  const platformSourceRoot = path.join(tempRoot, "platform-package-sources");

  for (const platformPackage of allPlatformPackages) {
    const stagedDirectory = path.join(platformSourceRoot, platformPackage.directoryName);
    await cp(platformPackage.directory, stagedDirectory, { recursive: true });
    if (platformPackage.packageName === currentPlatformPackageName) {
      await copyFile(rustBinary, path.join(stagedDirectory, "bin", "maximus"));
      await chmod(path.join(stagedDirectory, "bin", "maximus"), 0o755);
    }

    const manifest = JSON.parse(
      await readFile(path.join(stagedDirectory, "package.json"), "utf8"),
    );
    const packOutput = runCommand(
      "npm",
      ["pack", "--json", "--pack-destination", tempRoot],
      {
        cwd: stagedDirectory,
        env: {
          ...process.env,
          npm_config_cache: path.join(tempRoot, ".npm-cache"),
        },
        encoding: "utf8",
      },
    );
    const packResult = JSON.parse(packOutput.stdout);
    const filename = packResult[0]?.filename;

    assert.equal(typeof filename, "string", "platform npm pack JSON must include filename");
    tarballs.set(platformPackage.packageName, {
      filename,
      manifest,
      tarballPath: path.join(tempRoot, filename),
    });
  }

  return tarballs;
}

async function installPackedPackages(tempRoot, rootTarball, options = {}) {
  const installRoot = path.join(tempRoot, options.directoryName ?? "install");
  const dependencies = {
    [rootPackageName]: `file:${rootTarball}`,
  };

  await mkdir(installRoot, { recursive: true });
  await writeFile(
    path.join(installRoot, "package.json"),
    JSON.stringify(
      {
        name: "maximus-wrapper-smoke",
        private: true,
        dependencies,
      },
      null,
      2,
    ),
    "utf8",
  );

  const installArgs = ["install", "--no-package-lock"];
  if (options.omitOptional) {
    installArgs.push("--offline", "--omit=optional");
  }

  await runCommandAsync("npm", installArgs, {
    cwd: installRoot,
    env: {
      ...process.env,
      npm_config_cache: path.join(tempRoot, ".npm-cache"),
      npm_config_audit: "false",
      npm_config_fund: "false",
      npm_config_update_notifier: "false",
      ...(options.registryUrl ? { npm_config_registry: options.registryUrl } : {}),
    },
    encoding: "utf8",
  });

  return installRoot;
}

async function startLocalRegistry(platformTarballs) {
  const server = createServer((request, response) => {
    if (!request.url) {
      response.statusCode = 404;
      response.end();
      return;
    }

    const url = new URL(request.url, "http://127.0.0.1");
    if (process.env.MAXIMUS_WRAPPER_SMOKE_DEBUG === "1") {
      console.error(`[wrapper-smoke-registry] ${request.method} ${url.pathname}`);
    }

    if (url.pathname === "/-/ping") {
      response.statusCode = 200;
      response.setHeader("content-type", "application/json");
      response.end(request.method === "HEAD" ? undefined : JSON.stringify({ ok: true }));
      return;
    }

    const packageName = decodeURIComponent(url.pathname.slice(1));
    const packageEntry = platformTarballs.get(packageName);

    if (packageEntry && (request.method === "GET" || request.method === "HEAD")) {
      const tarballUrl = `/tarballs/${packageEntry.filename}`;
      const body = JSON.stringify({
        name: packageEntry.manifest.name,
        "dist-tags": {
          latest: packageEntry.manifest.version,
        },
        versions: {
          [packageEntry.manifest.version]: {
            ...packageEntry.manifest,
            dist: {
              tarball: `http://127.0.0.1:${server.address().port}${tarballUrl}`,
            },
          },
        },
      });

      response.statusCode = 200;
      response.setHeader("content-type", "application/json");
      response.end(request.method === "HEAD" ? undefined : body);
      return;
    }

    const tarballEntry = [...platformTarballs.values()].find(
      (candidate) => url.pathname === `/tarballs/${candidate.filename}`,
    );
    if (tarballEntry && (request.method === "GET" || request.method === "HEAD")) {
      response.statusCode = 200;
      response.setHeader("content-type", "application/octet-stream");
      if (request.method === "HEAD") {
        response.end();
      } else {
        createReadStream(tarballEntry.tarballPath).pipe(response);
      }
      return;
    }

    response.statusCode = 404;
    response.end();
  });

  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });

  return {
    url: `http://127.0.0.1:${server.address().port}`,
    close: () =>
      new Promise((resolve, reject) => {
        server.close((error) => {
          if (error) {
            reject(error);
            return;
          }

          resolve();
        });
      }),
  };
}

async function ensureRustBinary() {
  const debugBinary = path.join(repoRoot, "target", "debug", "maximus");

  const build = spawnSync("cargo", ["build", "-q", "-p", "maximus-cli"], {
    cwd: repoRoot,
    encoding: "utf8",
  });
  if (build.status !== 0) {
    throw new Error(build.stderr || build.stdout || "cargo build failed");
  }

  return debugBinary;
}

async function runScenario(installRoot) {
  await runWrapper(["audit", fixtureDir], installRoot);
  await runWrapper(["doctor", fixtureDir], installRoot);
  await runWrapper(["fix", fixtureDir, "--dry-run"], installRoot);
}

async function runFallbackBlockingScenarios(installRoot) {
  const configFixture = path.join(installRoot, "config-fixture");

  await mkdir(configFixture, { recursive: true });
  await writeFile(
    path.join(configFixture, "maximus.config.json"),
    '{ "checks": { "only": ["env"] } }\n',
    "utf8",
  );

  await assertWrapperFails(
    ["audit", configFixture],
    installRoot,
    /A Rust runtime is required when a Maximus config file is present/,
  );
  await assertWrapperFails(
    ["audit", fixtureDir, "--only", "env"],
    installRoot,
    /A Rust runtime is required for options not supported by the frozen JS compatibility path/,
  );
  await assertWrapperFails(
    ["fix", fixtureDir],
    installRoot,
    /fix \(without --dry-run\)/,
  );
}

async function removeJsFallback(installRoot) {
  await rm(path.join(installRoot, "node_modules", "@jeremyfellaz", "maximus", "src"), {
    recursive: true,
    force: true,
  });
}

async function runWrapper(args, cwd) {
  const launch = await resolvePackedWrapperLaunch(cwd);
  await runCommandAsync(launch.command, [...launch.args, ...args], {
    cwd,
    encoding: "utf8",
    env: {
      ...process.env,
      npm_config_cache: path.join(cwd, ".npm-cache"),
    },
  });
}

async function assertWrapperFails(args, cwd, expectedPattern) {
  const launch = await resolvePackedWrapperLaunch(cwd);
  const result = await runCommandAsync(launch.command, [...launch.args, ...args], {
    cwd,
    encoding: "utf8",
    env: {
      ...process.env,
      npm_config_cache: path.join(cwd, ".npm-cache"),
    },
    rejectOnFailure: false,
  });

  if (result.code === 0) {
    throw new Error(`Expected wrapper command to fail: maximus ${args.join(" ")}`);
  }

  const combinedOutput = `${result.stdout}\n${result.stderr}`;
  assert.match(combinedOutput, expectedPattern);
}

function runCommand(command, args, options) {
  const result = spawnSync(command, args, {
    ...options,
  });

  if (result.status !== 0) {
    throw new Error(result.stderr || result.stdout || `${command} ${args.join(" ")} failed`);
  }

  return result;
}

async function runCommandAsync(command, args, options) {
  return await new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: options.cwd,
      env: options.env,
      stdio: ["ignore", "pipe", "pipe"],
    });
    let stdout = "";
    let stderr = "";

    if (options.encoding === "utf8") {
      child.stdout.setEncoding("utf8");
      child.stderr.setEncoding("utf8");
    }

    child.stdout.on("data", (chunk) => {
      stdout += chunk;
    });
    child.stderr.on("data", (chunk) => {
      stderr += chunk;
    });

    child.on("error", (error) => {
      reject(error);
    });
    child.on("close", (code) => {
      if (code !== 0 && options.rejectOnFailure !== false) {
        reject(new Error(stderr || stdout || `${command} ${args.join(" ")} failed`));
        return;
      }

      resolve({
        code,
        stdout,
        stderr,
      });
    });
  });
}
