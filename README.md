# Greenbyte

Greenbyte is a client-side encrypted `.env` manager for teams. It gives environment files project-scoped synchronization, version history, rollback, invitations, roles, and an audit trail without sending plaintext secrets or project master keys to the backend.

This repository contains the complete application:

- A Rust CLI that encrypts, decrypts, pushes, pulls, versions, and restores `.env*` files.
- A Rust/Axum API for authentication, projects, membership, synchronization, and audit events.
- PostgreSQL storage, automatic SQL migrations, Docker packaging, and CI checks.

## How it works

```text
.env file ──> Greenbyte CLI ──> AES-256-GCM ciphertext ──> API ──> PostgreSQL
                  │
                  └── project master key stays on the client
```

The CLI derives an encryption key from the project master key with Argon2id, then encrypts each payload with AES-256-GCM using a fresh random salt and nonce. The server stores only opaque encrypted payloads plus the metadata needed for accounts, projects, permissions, versions, and audits.

Share a project master key with approved collaborators through a separate secure channel such as an enterprise password manager. The server cannot recover a lost master key or decrypt stored environment files.

## Features

- Client-side authenticated encryption
- Signup, email verification, login, refresh sessions, logout, and password reset
- Project creation and device/project initialization
- Owner, admin, member, and viewer permissions
- Expiring single-use team invitations
- Safe `.env*` push and pull
- Local snapshots and immutable remote version history
- Local and server-side rollback
- Retry-safe, idempotent pushes
- Project security audit log
- Rate limits, request limits, structured logs, and health probes
- SMTP support for deployed environments
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

Or install the `greenbyte` command locally:

```bash
cargo install --path . --locked
```

The CLI defaults to `http://localhost:3030`. It can also be set explicitly:

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

`greenbyte create` displays the project master key once. Store it in a secure password manager. Never commit it or send it through ordinary chat or email.

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

The teammate retrieves the invitation token from their email, or from API logs in local mode, then runs:

```bash
greenbyte register
greenbyte init my-application
```

The owner must share the project master key separately through a secure channel. Invitation tokens authorize membership but do not contain the encryption key.

Manage roles and membership with:

```bash
greenbyte role <user-id> <admin|member|viewer>
greenbyte remove <user-id>
greenbyte audit
```

## CLI command reference

```text
greenbyte register
greenbyte login
greenbyte logout
greenbyte reset-password
greenbyte create <project>
greenbyte init <project>
greenbyte add <email>
greenbyte members
greenbyte role <user-id> <admin|member|viewer>
greenbyte remove <user-id>
greenbyte push [-m <message>] [-f <.env-file>]
greenbyte pull [-f <.env-file>] [--force]
greenbyte commit <message>
greenbyte logs [--file <output>]
greenbyte logs --remote
greenbyte rollback --local <snapshot-id>
greenbyte rollback --address <remote-commit-id>
greenbyte status
greenbyte audit
```

## Architecture

| Component | Technology | Responsibility |
|---|---|---|
| CLI | Rust, Tokio, Reqwest | Encryption, local state, snapshots, and user commands |
| API | Rust, Axum | Authentication, authorization, projects, versions, and audits |
| Database | PostgreSQL 16 | Users, memberships, encrypted versions, sessions, and events |
| Deployment | Docker Compose | Local API and database orchestration |

The API source is in [`server/src`](server/src), database migrations are in [`server/migrations`](server/migrations), and the endpoint contract is documented in [`docs/API_CONTRACT.md`](docs/API_CONTRACT.md).

## Local files

| Path | Purpose | Protection |
|---|---|---|
| `.greenbyte` | Linked project identity and scoped tokens | Atomic write; mode `0600` on Unix |
| `.greenbyte-logs` | Encrypted local snapshots | Atomic write; mode `0600` on Unix |
| `~/.greenbyte-auth` | Global login session | Atomic write; mode `0600` on Unix |
| `.env*` | Decrypted environment files | Atomic write; mode `0600` on Unix |

Project initialization adds `.greenbyte`, `.greenbyte-logs`, `.env`, and `.env.*` to the application directory's `.gitignore`.

## Configuration

### CLI

| Variable | Default | Description |
|---|---|---|
| `GREENBYTE_SERVER` | `http://localhost:3030` | API URL; non-local servers must use HTTPS |
| `GREENBYTE_MASTER_KEY` | unset | Project key for non-interactive automation |

For CI, inject the master key through the runner's secret environment:

```bash
GREENBYTE_MASTER_KEY="$PROJECT_SECRET" greenbyte pull --file .env.ci --force
```

Do not put the actual key directly into a script, command history, repository file, or CI log.

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

No license file is currently included. Add an explicit license before distributing or accepting external contributions.
