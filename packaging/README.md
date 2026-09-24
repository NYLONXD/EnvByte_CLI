# Releases and install channels

## Cutting a release

Set the new version in `cli/Cargo.toml` (and `server/Cargo.toml`), commit, and
push to `main`. That is the whole process.

`.github/workflows/release.yml` runs on every push to `main`. When the version
in `cli/Cargo.toml` has no GitHub release yet, it runs the tests, builds the CLI
for six targets, tags the commit `v<version>`, publishes a GitHub release with
every archive and its `.sha256`, and pushes to each channel below that is
switched on. The install scripts on the website always fetch the latest
release, so they need no update.

Each release also carries `envbyte-setup.exe`, the Windows installer built from
`installer/`. The website's Download button links to
`releases/latest/download/envbyte-setup.exe`, so the button only works once a
release made by this workflow exists. Like the install scripts, the installer
downloads whichever release is latest, so it has its own version in
`installer/Cargo.toml` that does not need bumping with the CLI.

Do not tag or `cargo publish` by hand (past the one-time first publish under
crates.io below): the release creates the tag, and publishing to crates.io on
its own is how a version ends up there while the install scripts still have
nothing to download.

If a release fails on a flaky test or a runner outage, use **Re-run failed
jobs** on that run. If it fails on a real bug, push the fix to `main`: the
version still has no release, so that push releases it. Running the workflow by
hand on any other branch is a dry run that builds every target and publishes
nothing.

| File | What it does |
|---|---|
| `npm/envbyte/` | The `envbyte` npm package: a small launcher that runs the platform binary |
| `npm/build.mjs` | Builds the launcher and the five `@nylonxd/envbyte-*` platform packages from a release's archives |
| `render.py` | Writes the Homebrew formula, Scoop manifest and winget manifests from a release's checksums |

## Switching channels on

Each channel after crates.io is off until its repository variable is `true`
(GitHub → Settings → Secrets and variables → Actions → Variables), so a release
never fails on an account that is not set up yet.

The website and `README.md` list only the channels that work today. Once a
channel's first release is published and its install command works on a clean
machine, add it to `INSTALL_METHODS` in `web/src/lib/content.ts` and to the
Install section of `README.md`. Not before: a listed command that fails is the
first thing a new user sees.

### crates.io

1. Publish the first version by hand: `cargo publish -p envbyte`.
2. crates.io → `envbyte` → Settings → Trusted Publishing → add
   owner `NYLONXD`, repository `EnvByte_CLI`, workflow `release.yml`.

From then on each release publishes it. A version already on crates.io is
skipped.

### npm (`PUBLISH_NPM`)

Six packages: the `envbyte` launcher, and one binary per platform under the
`@nylonxd` organization's scope. Releases publish them through Trusted
Publishing, with no npm token stored in GitHub. npm only lets you set that up on
a package that already exists, so the first version goes up by hand:

1. After a release exists, download its archives into a folder and build and
   publish, platform packages first, with a granular access token that can
   write to all packages and to the `nylonxd` organization:

   ```bash
   node packaging/npm/build.mjs v0.4.1 <archives folder> npm-out
   for dir in npm-out/cli-* npm-out/envbyte; do (cd "$dir" && npm publish --access public); done
   ```

2. On npmjs.com, open each of the six packages → Settings → Trusted Publisher →
   GitHub Actions: owner `NYLONXD`, repository `EnvByte_CLI`, workflow
   `release.yml`.
3. Set the variable `PUBLISH_NPM` to `true`, and delete the token from step 1.

### Homebrew and Scoop (`PUBLISH_TAPS`)

1. Create two public repositories: `NYLONXD/homebrew-tap` and
   `NYLONXD/scoop-bucket`.
2. Create a fine-grained personal access token limited to those two
   repositories, with **Contents: read and write**, and save it as the secret
   `TAP_TOKEN`.
3. Set the variable `PUBLISH_TAPS` to `true`.

Users then install with `brew install nylonxd/tap/envbyte`, or
`scoop bucket add nylonxd https://github.com/NYLONXD/scoop-bucket` followed by
`scoop install nylonxd/envbyte`.

### winget (`PUBLISH_WINGET`)

Microsoft reviews every new package, so the first version goes in by hand:

1. After the release exists, download its `.sha256` files into a folder and run
   `python3 packaging/render.py v0.4.1 <that folder> out`.
2. Fork `microsoft/winget-pkgs`, copy `out/winget/manifests` into the fork, and
   open a pull request. `winget validate --manifest <folder>` checks the files
   first; `wingetcreate submit <folder>` does the fork and pull request for you.
3. Once it is merged, keep the fork, create a classic token with the
   `public_repo` scope, save it as `WINGET_TOKEN`, and set `PUBLISH_WINGET` to
   `true`. Later releases open their pull request automatically.

### Website (Vercel)

1. Import the repository on Vercel and set **Root Directory** to `web`. The
   build settings come from `web/vercel.json`.
2. Add the domain `envbyte.trackedge.in` in Vercel, then add the CNAME record
   it shows you (host `envbyte`) in GoDaddy's DNS for `trackedge.in`.
