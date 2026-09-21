# Architecture

Two crates in one workspace. The CLI does all the cryptography; the server
stores ciphertext it cannot read and enforces who may touch what.

```
greenbyte/
├── cli/      the `greenbyte` binary — every key operation happens here
└── server/   the API — sealed keys, ciphertext, membership, audit
```

## The rule that keeps this navigable

**Layers point one way.** An outer layer may use an inner one; an inner layer
never reaches back out. When you are unsure where something belongs, ask which
layer would have to know about which, and put it in the inner one.

**Features are vertical, not horizontal.** Adding a capability means adding a
folder, not editing a file that five other capabilities also edit. That is what
stops any one file from becoming the file nobody wants to open.

## Server

```
server/src/
├── app.rs          router assembly — the whole URL surface, readable at once
├── config.rs       environment parsing, with production guardrails
├── state.rs        what every handler gets: pool, config, mailer, limiters
├── error.rs        ApiError → HTTP status, in one place
│
├── features/       one folder per capability
│   ├── accounts/   signup, sign-in, recovery, profile, identity key
│   ├── projects/   create, list, join, rollback
│   ├── members/    invite, roles, removal, key-grant status
│   ├── keys/       data-key grants and rotation
│   ├── env_files/  push, pull, history
│   ├── audit/      reading the trail
│   └── health.rs   liveness and readiness
│
├── security/       session.rs, password.rs, tokens.rs, permissions.rs
├── db/             models.rs (row types), audit.rs (the audit recorder)
└── infra/          email.rs, rate_limit.rs
```

Inside a feature:

| File | Holds |
|---|---|
| `mod.rs` | `routes()` — this feature's URLs, and nothing else |
| `dto.rs` | request and response shapes: the published wire contract |
| `handlers.rs` | axum handlers — validate, call, respond |
| `service.rs` | rules and queries too involved to sit in a handler |

`dto.rs` is separate on purpose: changing a response field should be a
deliberate edit to the contract, not something that happens while moving logic
around.

### Adding a feature

1. `mkdir server/src/features/<name>` with `mod.rs`, `dto.rs`, `handlers.rs`.
2. Declare it in `features/mod.rs`.
3. `.merge(<name>::routes())` in `app.rs`.

Nothing else changes. No shared file grows.

## CLI

```
cli/src/
├── main.rs         parse, dispatch, print the error
├── cli.rs          the clap command surface and dispatch table
│
├── commands/       one module per command group — thin
│   ├── context.rs  resolves project link + session + identity, once
│   ├── account.rs  register, login, logout, whoami, reset-password
│   ├── project.rs  create, init, projects
│   ├── sync.rs     push, pull
│   ├── members.rs  add, members, remove, role
│   ├── rotate.rs   rotate
│   ├── identity.rs identity show/publish/export/replace
│   └── ...         snapshot, history, rollback, status, audit
│
├── core/           no terminal I/O, therefore directly testable
│   ├── crypto/     identity.rs, sealing.rs, envelope.rs, keyring.rs
│   ├── api/        client.rs + one module per resource
│   ├── workspace/  project_config.rs, global_auth.rs, commit_log.rs, paths.rs
│   ├── env_files.rs
│   └── device.rs
│
└── ui/             every prompt, spinner and line of output
```

`core` never prints and never prompts. That is why 50 unit tests run in under a
second with no server and no terminal.

`commands` is glue: gather input, call `core`, print through `ui`. If a command
module starts holding logic worth testing, that logic belongs in `core`.

## How the encryption fits together

```
identity key            X25519, one per device, never leaves it
      │                 ~/.greenbyte-identity, or $GREENBYTE_IDENTITY_KEY
      │ opens
      ▼
project data key        32 random bytes, one version per rotation
      │                 stored only as one sealed copy per member
      │ encrypts
      ▼
.env contents           AES-256-GCM, key version bound into the AAD
```

- **Sealing** (`core/crypto/sealing.rs`) — ephemeral X25519 → HKDF-SHA256 →
  AES-256-GCM. Both public keys go into the HKDF info, so a sealed key cannot be
  replayed at a different recipient.
- **The envelope** (`core/crypto/envelope.rs`) — `greenbyte:v3:` payloads carry
  their key version in authenticated data, so relabelling one as another version
  fails to verify.
- **The keyring** (`core/crypto/keyring.rs`) — a member holds every version they
  were present for. New writes use the newest; older ones keep history readable.

The server sees: sealed keys it cannot open, ciphertext it cannot read, and
metadata (filenames, sizes, timestamps, who did what).

## Rotation

The one flow worth reading end to end, because it is what makes offboarding
real.

1. `greenbyte remove <user>` — deletes that member's sealed keys server-side and
   revokes their device sessions. **They may still hold the key they already
   opened.** The response says so.
2. `greenbyte rotate` —
   - fetches members, files and the current key version;
   - refuses to start if any member has no published identity key, rather than
     silently locking them out;
   - decrypts every file locally, mints a new data key, re-encrypts everything;
   - seals the new key once per remaining member;
   - sends all of it in **one** request.
3. The server (`features/keys/rotation.rs`) applies it in a single transaction
   and rejects it unless it covers **every** current member and **every** stored
   file. A partial rotation is worse than none: a missing grant locks out a
   colleague, a missing file stays readable with the retired key.
4. Grants belonging to people who have left are deleted at every version.
   Earlier versions held by current members are kept, so history stays readable.

After a rotation, a client still holding the old key gets a `409` on push naming
both versions, instead of overwriting a file the team can no longer read.

## Project names

Names are unique **per owner**, not per installation
(`projects_owner_name_lower_uq`). Two teams may each have a `backend`.

Joining is by invitation token alone — `POST /projects/join` — so a project name
is never something a stranger needs to know or can probe for.

Wherever a name is shown to a human, it is shown as `owner/project`.

## Testing

| Where | What it covers |
|---|---|
| `cli/src/**` unit tests | crypto round-trips, tamper detection, key-version binding, file-format compatibility, validation |
| `server/src/**` unit tests | validation, password hashing, rate limiting, token handling |
| `server/tests/api.rs` | the real router over a real Postgres: invite→join→pull, per-owner names, rotation, partial-rotation refusal |

`server/tests/harness/mod.rs` builds accounts and projects, so a new API test is
a few lines rather than a page of setup.

Argon2 and the AEAD crates are compiled with `opt-level = 3` even in debug
builds (see the workspace `Cargo.toml`); without that, password hashing makes
the suite unusably slow.
