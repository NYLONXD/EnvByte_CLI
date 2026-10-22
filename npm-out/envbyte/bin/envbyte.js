#!/usr/bin/env node
// Runs the envbyte binary from the platform package npm installed alongside
// this one (see optionalDependencies). Nothing is downloaded at install time,
// so it works with --ignore-scripts and behind proxies that block postinstall.
"use strict";

const { spawnSync } = require("node:child_process");
const { chmodSync } = require("node:fs");

const PACKAGES = {
  "darwin arm64": "@nylonxd/envbyte-darwin-arm64",
  "darwin x64": "@nylonxd/envbyte-darwin-x64",
  "linux arm64": "@nylonxd/envbyte-linux-arm64",
  "linux x64": "@nylonxd/envbyte-linux-x64",
  "win32 x64": "@nylonxd/envbyte-win32-x64",
  // Windows on ARM runs the x64 build through emulation.
  "win32 arm64": "@nylonxd/envbyte-win32-x64",
};

const platform = `${process.platform} ${process.arch}`;
const packageName = PACKAGES[platform];
if (!packageName) {
  console.error(`envbyte: there is no prebuilt binary for ${platform}.`);
  console.error("Install from source instead: cargo install envbyte");
  process.exit(1);
}

let binary;
try {
  binary = require.resolve(`${packageName}/bin/envbyte${process.platform === "win32" ? ".exe" : ""}`);
} catch {
  console.error(`envbyte: ${packageName} is not installed.`);
  console.error("Reinstall without --omit=optional / --no-optional: npm install -g envbyte");
  process.exit(1);
}

const run = () => spawnSync(binary, process.argv.slice(2), { stdio: "inherit", windowsHide: false });
let result = run();

// A package packed on Windows loses the executable bit, so the binary can
// arrive as 0644. Put the bit back and try once more.
if (result.error?.code === "EACCES" && process.platform !== "win32") {
  try {
    chmodSync(binary, 0o755);
    result = run();
  } catch {
    // Not ours to change, e.g. installed with sudo; the hint below covers it.
  }
}
if (result.error) {
  console.error(`envbyte: could not start ${binary}: ${result.error.message}`);
  if (result.error.code === "EACCES") console.error(`Make it executable: sudo chmod +x ${binary}`);
  process.exit(1);
}
if (result.signal) {
  process.kill(process.pid, result.signal);
} else {
  process.exit(result.status ?? 1);
}
