# Greenbyte

Greenbyte is an end-to-end encrypted `.env` manager for teams. It gives environment files versioned history, rollback, invitations, roles and an audit trail, without any secret ever reaching the server and without anyone having to send a key to anyone.

This repository contains the complete application:

- A Rust CLI that encrypts, decrypts, pushes, pulls, versions, and restores `.env*` files.
- A Rust/Axum API for authentication, projects, membership, synchronization, and audit events.
- PostgreSQL storage, automatic SQL migrations, Docker packaging, and CI checks.

## How it works

```text
.env  ──>  CLI  ──>  AES-256-GCM  ──>  API  ──>  PostgreSQL
            │
            └── project key, sealed to each member's identity key
```

Every member has an **identity key** (X25519) that never leaves their device.
Every project has a **data key** that encrypts its `.env` files. The data key is
sealed once per member, to their identity key, and the sealed copies are what
the server stores. The server can open none of them.

Nothing secret is ever shared by hand. Inviting a colleague seals the project
key to them in the same step; they run `greenbyte init` and can read the files.

Because the project key is stored as per-member copies rather than one shared
passphrase, it can be **rotated**. Removing someone and running
`greenbyte rotate` mints a new key, re-encrypts every file under it, and seals
it only to the people who remain — so the copy the departing member already had
opens nothing new.

## Features

- End-to-end encryption: the server stores ciphertext and sealed keys, never plaintext or an openable key
- Per-member key wrapping — no master key to copy into chat or a password manager
- One-command key rotation for offboarding, applied all-or-nothing
- Project names are unique per account, so two teams can both have a `backend`
- Signup, email verification, sign-in, rotating refresh sessions, password reset
- Owner, admin, member and viewer roles
- Expiring, single-use invitations that are not credentials and cannot be forwarded
- Versioned history, local snapshots, and rollback on either side
- Retry-safe idempotent pushes
- Per-project audit log
- Rate limits, request limits, structured JSON logs, health probes
- Non-root, read-only API container

## Prerequisites

Install the following before starting:

- [Rust and Cargo](https://rustup.rs/) with a current stable toolchain
- Docker Engine or Docker Desktop with Docker Compose
- Git

Confirm the tools are available:

```bash
rustc --version
cargo --version
docker --version
docker compose version
```

## Run the complete project locally

All commands below are run from the repository root.

### 1. Configure local secrets

```bash
cp .env.example .env
```

For local development the example values work, but changing `POSTGRES_PASSWORD` and `GREENBYTE_JWT_SECRET` is recommended. The `.env` file is ignored by Git.

If PostgreSQL was previously started with another password, either keep that original password in `.env` or intentionally recreate the local database volume. Recreating the volume permanently deletes its local Greenbyte data.

### 2. Start PostgreSQL and the API

```bash
docker compose up --build -d
```

Wait until both services are healthy:

```bash
docker compose ps
curl http://localhost:3030/health/live
curl http://localhost:3030/health/ready
```

Expected health responses are `{"status":"ok"}` and `{"status":"ready"}`. Database migrations run automatically when the API starts.

Follow backend logs with:

```bash
docker compose logs -f api
```

### 3. Build or install the CLI

Run it directly during development:

```bash
cargo run -p greenbyte -- --help
```

Or install the `greenbyte` command. Any of these work:

```bash
cargo install greenbyte              # from crates.io
cargo binstall greenbyte             # prebuilt binary, no compile
cargo install --path cli --locked    # from this checkout
```

Prebuilt binaries for Linux (gnu and musl, x86_64 and aarch64), macOS (Intel
and Apple Silicon) and Windows are attached to every
[GitHub release](https://github.com/NYLONXD/GreenByte_CLI/releases), each with a
`.sha256` file to verify against.

The CLI talks to the hosted server at `https://greenbyte-cli.onrender.com` by
default, so a fresh install needs no configuration. To use the local API
started above instead, point it at localhost:

```bash
export GREENBYTE_SERVER=http://localhost:3030
```

### 4. Register an account

```bash
greenbyte register
```

Local development uses `GREENBYTE_EMAIL_MODE=log`, so verification, reset, and invitation tokens are printed to the API logs instead of being emailed:

```bash
docker compose logs api
```

Copy the requested token from the logs and paste it into the CLI prompt. Production mode rejects log-based email delivery and requires SMTP.

If the token is lost or expires before you use it, run `greenbyte verify`. It
asks for the email and password you registered with, sends a new token, and
finishes signing you in. An unverified registration whose token expired no
longer holds its email or username, so registering again also works.

If the CLI was not installed, replace `greenbyte` in any example with:

```bash
cargo run -p greenbyte --
```

For example, `greenbyte status` becomes `cargo run -p greenbyte -- status`.

### 5. Create and synchronize a project

Run these commands inside the application directory containing your `.env` file:

```bash
greenbyte create my-application
greenbyte status
greenbyte push --file .env --message "initial configuration"
greenbyte logs --remote
```

`greenbyte create` mints the project key and seals it to your identity. There is
nothing to write down and nothing to store in a password manager — the sealed
copy lives on the server and only your device can open it.

Your identity key is created on first use at `~/.greenbyte-identity`. Treat it
like an SSH private key. If you lose it, an admin can grant you access again; if
it leaks, rotate the project key.

Pull the current encrypted version back to disk:

```bash
greenbyte pull --file .env
```

Greenbyte asks before replacing an existing file. Use `--force` only for intentional non-interactive replacement:

```bash
greenbyte pull --file .env --force
```

### 6. Invite a teammate

The project owner runs:

```bash
greenbyte add teammate@example.com
greenbyte members
```

The teammate retrieves the invitation token from their email, or from API logs in local mode, then signs in and joins:

```bash
greenbyte register   # or `greenbyte login` for an existing account
greenbyte init
```

`greenbyte init` takes no project name: the invitation identifies the project.

Joining is performed as the signed-in account, and the server accepts the token only from the account it was issued to. An invitation is therefore not a credential: forwarding the email to somebody else gives them nothing, and the response contains no login tokens.

The project key was sealed to the teammate's identity when they were invited, so
they can `greenbyte pull` immediately. Nothing is sent out of band.

If `greenbyte add` reports that the account has no identity key, they have not
signed in with a current CLI yet. One `greenbyte login` publishes it.

Manage roles and membership with:

```bash
greenbyte members
greenbyte role <user-id> <admin|member|viewer>
greenbyte remove <user-id>
greenbyte audit
```

### 7. Offboard someone

Removing a member deletes their sealed key on the server and revokes their
sessions, but they may have kept the key they already opened. Rotating is what
makes that copy worthless:

```bash
greenbyte remove <user-id>
greenbyte rotate
```

`greenbyte rotate` shows exactly what will happen before asking to proceed: the
version it moves to, who receives the new key, and how many files will be
re-encrypted. It refuses to start if any remaining member has no identity key to
seal to, rather than silently locking them out.

The server applies a rotation in one transaction and rejects it unless it covers
every current member and every stored file — a half-applied rotation is worse
than none. Afterwards, a client still holding the retired key gets a clear error
on push instead of overwriting a file the team can no longer read.

Everyone else runs `greenbyte pull` to pick up the new key. Project history
stays readable: members keep their older key versions for reading the past.

Use `greenbyte rotate -y` to skip the prompt in a script.

## CLI command reference

```text
Account
  greenbyte register                        create an account
  greenbyte verify                          resend the verification token and finish signing up
  greenbyte login                           sign in (also publishes your identity key)
  greenbyte logout                          sign out on this machine
  greenbyte whoami                          show the signed-in account
  greenbyte reset-password                  request a token and set a new password

Identity
  greenbyte identity                        show this device's key and whether it is published
  greenbyte identity publish                publish this device's public key
  greenbyte identity export                 print the secret key, for another machine or CI
  greenbyte identity replace                replace this device's key (needs re-granting)

Projects
  greenbyte create <project>                start a project here
  greenbyte init [project]                  join a project you were invited to
  greenbyte projects                        list projects you can reach

Syncing
  greenbyte push [-m <message>] [-f <file>] encrypt and upload a .env file
  greenbyte pull [-f <file>] [--force]      download and decrypt
  greenbyte commit <message>                take an encrypted local snapshot
  greenbyte logs [--file <output>]          list local snapshots
  greenbyte logs --remote                   show the project's server-side history
  greenbyte rollback --local <id>           restore from a local snapshot
  greenbyte rollback --address <commit>     roll the server back to a commit

Collaboration
  greenbyte add <email>                     invite, sealing the project key to them
  greenbyte members                         list members, roles and key versions held
  greenbyte role <user-id> <role>           set admin, member or viewer
  greenbyte remove <user-id>                revoke access
  greenbyte rotate [-y]                     retire the project key and issue a new one

Inspection
  greenbyte status                          what this directory is linked to
  greenbyte audit                           the project's security audit log
```

## Architecture

| Component | Technology | Responsibility |
|---|---|---|
| CLI | Rust, Tokio, Reqwest | Encryption, local state, snapshots, and user commands |
| API | Rust, Axum | Authentication, authorization, projects, versions, and audits |
| Database | PostgreSQL 16 | Users, memberships, encrypted versions, sessions, and events |
| Deployment | Docker Compose | Local API and database orchestration |

### Project layout

Both crates are laid out as vertical slices: one folder per capability, so
adding a feature means adding a folder rather than growing a shared file.

```text
cli/src/
  cli.rs          the command surface and dispatch
  commands/       one module per command group, thin over core
  core/           crypto, API client, on-disk state - no terminal I/O
  ui/             every prompt, spinner and line of output
server/src/
  app.rs          the whole URL surface, in one readable place
  features/       accounts, projects, members, keys, env_files, audit, health
  security/       sessions, passwords, tokens, permission checks
  db/             row types and the audit recorder
  infra/          email delivery and rate limiting
  migrations/     database schema
```

[`docs/ARCHITECTURE.md`](docs/ARCHITECTURE.md) explains the layering rules, what
belongs in each file, how to add a feature, and walks through key rotation end
to end. [`docs/API_CONTRACT.md`](docs/API_CONTRACT.md) is the endpoint
reference.

## Local files

| Path | Purpose | Protection |
|---|---|---|
| `.greenbyte` | Linked project identity and scoped tokens | Atomic write; mode `0600` on Unix |
| `.greenbyte-logs` | Encrypted local snapshots | Atomic write; mode `0600` on Unix |
| `~/.greenbyte-auth` | Global login session | Atomic write; mode `0600` on Unix |
| `~/.greenbyte-identity` | This device's identity key | Atomic write; mode `0600` on Unix |
| `.env*` | Decrypted environment files | Atomic write; mode `0600` on Unix |

Project initialization adds `.greenbyte`, `.greenbyte-logs`, `.env`, and `.env.*` to the application directory's `.gitignore`.

## Configuration

### CLI

| Variable | Default | Description |
|---|---|---|
| `GREENBYTE_SERVER` | `https://greenbyte-cli.onrender.com` | API URL; non-local servers must use HTTPS |
| `GREENBYTE_IDENTITY_KEY` | unset | Identity secret key for CI, instead of a file on disk |
| `GREENBYTE_IDENTITY_FILE` | `~/.greenbyte-identity` | Where the identity key is stored |
| `GREENBYTE_MASTER_KEY` | unset | Only for reading snapshots written before key wrapping |

### API behind a reverse proxy

The API rate-limits by client IP. Behind a proxy or load balancer (Render,
Fly, Heroku, nginx), every connection arrives from the proxy, so all users
would share one limit. Set `GREENBYTE_TRUSTED_PROXY_HOPS` to the number of
proxies in front of the API — `1` for Render — and the client address is read
from `X-Forwarded-For` instead. Leave it at the default `0` when clients
connect directly, because the header is then client-controlled.

Every push declares which project key version it encrypted under, and the server
rejects anything but the current one. A client that missed a rotation fails
immediately with a message naming both versions, instead of replacing the file
with a payload the rest of the team cannot read.

For CI, give the runner its own account and inject that account's identity key
from the secret store:

```bash
GREENBYTE_IDENTITY_KEY="$GREENBYTE_CI_IDENTITY" greenbyte pull --file .env.ci --force
```

Generate it once with `greenbyte identity export` on a machine signed in as the
CI account. Because CI is a member like any other, revoking it is
`greenbyte remove` followed by `greenbyte rotate` — the same as for a person.

Do not put the key directly into a script, shell history, repository file or CI
log.

### Backend

The local Compose stack reads `POSTGRES_PASSWORD`, `GREENBYTE_JWT_SECRET`, `GREENBYTE_EMAIL_MODE`, and `RUST_LOG` from `.env`. All backend settings, including SMTP and token lifetimes, are documented in [`server/README.md`](server/README.md).

## Tests and quality checks

Start PostgreSQL, load the same password used by Compose, and run the complete suite:

```bash
docker compose up -d postgres
set -a
source ./.env
set +a
DATABASE_URL="postgres://greenbyte:${POSTGRES_PASSWORD}@localhost:5432/greenbyte" \
  cargo test --workspace --locked
```

The integration suite uses isolated temporary databases and tests registration, verification, authorization boundaries, project creation, invitation and joining, idempotent encrypted pushes, history, and collaborator pulls.

Run the remaining checks with:

```bash
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo build --workspace --release --locked
docker compose config --quiet
```

## Stop or restart the project

Stop the containers while retaining PostgreSQL data:

```bash
docker compose down
```

Start them again:

```bash
docker compose up -d
```

Restart only the API:

```bash
docker compose restart api
```

## Troubleshooting

### API is not reachable

```bash
docker compose ps
docker compose logs --tail=200 api
curl http://localhost:3030/health/ready
```

### PostgreSQL password authentication fails

The password stored inside an existing PostgreSQL volume may differ from the current `.env`. Restore the password that was used when the volume was first created. Delete and recreate the volume only when losing local data is acceptable.

### Docker reports no space left on device

Inspect Docker usage first:

```bash
docker system df
```

Unused build cache can be removed with `docker builder prune`, but review what Docker plans to remove before confirming.

## Production deployment

Before exposing Greenbyte publicly:

- Use HTTPS through a trusted reverse proxy or load balancer.
- Set `GREENBYTE_ENV=production`.
- Configure `GREENBYTE_EMAIL_MODE=smtp` and real SMTP credentials.
- Generate strong, unique database and JWT secrets and inject them from a secret manager.
- Use a restricted production PostgreSQL role and tested encrypted backups.
- Add edge-level distributed rate limiting for multiple API replicas.
- Send structured logs and health signals to centralized monitoring.
- Regularly test restore, token rotation, access removal, and incident-response procedures.

See [`SECURITY.md`](SECURITY.md) for the threat model and security reporting guidance.

## License

Dual-licensed under [MIT](LICENSE-MIT) or [Apache 2.0](LICENSE-APACHE), at your
option — the Rust ecosystem default. Change it before the first public release
if you want different terms for the server.
