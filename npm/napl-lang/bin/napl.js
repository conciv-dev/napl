#!/usr/bin/env node
"use strict";

const { spawnSync } = require("node:child_process");

function platformSuffix() {
  const { platform, arch } = process;
  if (platform === "darwin" && arch === "arm64") return "darwin-arm64";
  if (platform === "darwin" && arch === "x64") return "darwin-x64";
  if (platform === "linux" && arch === "x64") return "linux-x64-gnu";
  if (platform === "linux" && arch === "arm64") return "linux-arm64-gnu";
  if (platform === "win32" && arch === "x64") return "win32-x64";
  return null;
}

function resolveBinary() {
  const suffix = platformSuffix();
  if (suffix === null) return null;
  const binary = process.platform === "win32" ? "napl.exe" : "napl";
  try {
    return require.resolve("@napl-lang/binary-" + suffix + "/" + binary);
  } catch (_error) {
    return null;
  }
}

const UNSUPPORTED =
  "napl: no prebuilt binary is available for your platform (" +
  process.platform +
  "-" +
  process.arch +
  ").\n\n" +
  "Install from source with cargo:\n" +
  "  cargo install --git https://github.com/conciv-dev/napl napl-cli\n\n" +
  "Or run the install script:\n" +
  "  curl -fsSL https://raw.githubusercontent.com/conciv-dev/napl/main/install.sh | sh\n";

const binary = resolveBinary();
if (binary === null) {
  process.stderr.write(UNSUPPORTED);
  process.exit(1);
}

const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });
if (result.error) {
  process.stderr.write("napl: failed to launch binary: " + result.error.message + "\n");
  process.exit(1);
}
process.exit(result.status === null ? 1 : result.status);
