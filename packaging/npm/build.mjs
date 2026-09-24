#!/usr/bin/env node
// Assembles the npm packages for one release from its archives.
//
//   node packaging/npm/build.mjs <version> <archives-dir> <out-dir>
//
// <archives-dir> holds the release archives (envbyte-<target>.tar.gz / .zip).
// <out-dir> receives one directory per package, ready for `npm publish`:
// a package per platform carrying the binary, then the `envbyte` launcher that
// depends on all of them as optionalDependencies, so npm installs only the one
// matching the user's OS and CPU.

import { execFileSync } from "node:child_process";
import { chmodSync, copyFileSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { basename, dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const PLATFORMS = [
  { name: "@nylonxd/envbyte-darwin-arm64", os: "darwin", cpu: ["arm64"], target: "aarch64-apple-darwin" },
  { name: "@nylonxd/envbyte-darwin-x64", os: "darwin", cpu: ["x64"], target: "x86_64-apple-darwin" },
  // Statically linked: runs on glibc and musl (Alpine) alike.
  { name: "@nylonxd/envbyte-linux-x64", os: "linux", cpu: ["x64"], target: "x86_64-unknown-linux-musl" },
  { name: "@nylonxd/envbyte-linux-arm64", os: "linux", cpu: ["arm64"], target: "aarch64-unknown-linux-gnu" },
  // Also installed on Windows ARM64, which runs x64 programs under emulation.
  { name: "@nylonxd/envbyte-win32-x64", os: "win32", cpu: ["x64", "arm64"], target: "x86_64-pc-windows-msvc" },
];

const [version, archives, out] = process.argv.slice(2);
if (!version || !archives || !out) {
  console.error("usage: node build.mjs <version> <archives-dir> <out-dir>");
  process.exit(2);
}
const cleanVersion = version.replace(/^v/, "");
const here = dirname(fileURLToPath(import.meta.url));
const launcher = JSON.parse(readFileSync(join(here, "envbyte", "package.json"), "utf8"));
const shared = {
  license: launcher.license,
  homepage: launcher.homepage,
  repository: launcher.repository,
};

rmSync(out, { recursive: true, force: true });
mkdirSync(out, { recursive: true });

// The archive is passed by name from its own directory: GNU tar reads a
// Windows path such as C:\... as host "C" and tries to connect to it.
function extract(archive, into) {
  const options = { cwd: dirname(archive), stdio: "pipe" };
  const file = basename(archive);
  if (file.endsWith(".zip")) {
    try {
      execFileSync("unzip", ["-q", file, "-d", into], options);
    } catch {
      execFileSync("tar", ["-xf", file, "-C", into], options); // bsdtar (Windows, macOS) reads zip
    }
  } else {
    execFileSync("tar", ["-xzf", file, "-C", into], options);
  }
}

for (const platform of PLATFORMS) {
  const windows = platform.os === "win32";
  const archive = join(archives, `envbyte-${platform.target}.${windows ? "zip" : "tar.gz"}`);
  if (!existsSync(archive)) throw new Error(`missing ${archive}`);

  const scratch = mkdtempSync(join(tmpdir(), "envbyte-npm-"));
  extract(archive, scratch);
  const exe = windows ? "envbyte.exe" : "envbyte";
  const directory = join(out, `cli-${platform.os}-${platform.cpu[0]}`);
  mkdirSync(join(directory, "bin"), { recursive: true });
  copyFileSync(join(scratch, `envbyte-${platform.target}`, exe), join(directory, "bin", exe));
  chmodSync(join(directory, "bin", exe), 0o755);
  rmSync(scratch, { recursive: true, force: true });

  writeFileSync(
    join(directory, "package.json"),
    JSON.stringify(
      {
        name: platform.name,
        version: cleanVersion,
        description: `The envbyte binary for ${platform.os} ${platform.cpu[0]}. Install \`envbyte\` instead of this package.`,
        ...shared,
        os: [platform.os],
        cpu: platform.cpu,
        files: [`bin/${exe}`],
        preferUnplugged: true,
      },
      null,
      2,
    ) + "\n",
  );
  console.log(`built ${platform.name}@${cleanVersion}`);
}

const main = join(out, "envbyte");
cpSync(join(here, "envbyte"), main, { recursive: true });
launcher.version = cleanVersion;
launcher.optionalDependencies = Object.fromEntries(PLATFORMS.map((p) => [p.name, cleanVersion]));
writeFileSync(join(main, "package.json"), JSON.stringify(launcher, null, 2) + "\n");
console.log(`built envbyte@${cleanVersion}`);
