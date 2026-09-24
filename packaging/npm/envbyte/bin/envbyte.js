#!/usr/bin/env node
// Runs the envbyte binary from the platform package npm installed alongside
// this one (see optionalDependencies). Nothing is downloaded at install time,
// so it works with --ignore-scripts and behind proxies that block postinstall.
"use strict";

const { spawnSync } = require("node:child_process");

const PACKAGES = {
  "darwin arm64": "@envbyte/cli-darwin-arm64",
  "darwin x64": "@envbyte/cli-darwin-x64",
  "linux arm64": "@envbyte/cli-linux-arm64",
  "linux x64": "@envbyte/cli-linux-x64",
  "win32 x64": "@envbyte/cli-win32-x64",
  // Windows on ARM runs the x64 build through emulation.
  "win32 arm64": "@envbyte/cli-win32-x64",
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

const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit", windowsHide: false });
if (result.error) {
  console.error(`envbyte: could not start ${binary}: ${result.error.message}`);
  process.exit(1);
}
if (result.signal) {
  process.kill(process.pid, result.signal);
} else {
  process.exit(result.status ?? 1);
}
