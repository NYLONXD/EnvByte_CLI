# envbyte

End-to-end encrypted `.env` sharing for teams, with rotatable keys.

```bash
curl -fsSL https://envbyte.trackedge.in/install.sh | sh                  # macOS, Linux
powershell -c "irm https://envbyte.trackedge.in/install.ps1 | iex"       # Windows
cargo install envbyte                                                    # from source
```

All methods: https://envbyte.trackedge.in

Formerly published as `greenbyte`. The first time `envbyte` runs it copies your
identity key and moves your sign-in and project links to their new names.

## Why

Sharing `.env` files usually means pasting secrets into chat, or sharing one
master passphrase that can never be changed. Envbyte does neither.

Every member has an **identity key** that never leaves their device. Every
project has a **data key** that encrypts its files, stored as one sealed copy
per member. The server holds ciphertext and sealed keys, and can open neither.

Because the key exists as per-member copies rather than one shared passphrase,
it can be **rotated** — which is what makes offboarding actually work.

## Getting started

```bash
envbyte register              # creates your identity key on first use
cd my-app
envbyte create my-app         # project key is sealed to you; nothing to copy down
envbyte push -m "initial"
```

Add a colleague — the project key is sealed to them in the same step:

```bash
envbyte add teammate@example.com
```

They join with the token from their email. No name needed; the invitation
identifies the project:

```bash
envbyte login
envbyte init
envbyte pull
```

## Offboarding

Removing someone revokes their sessions and deletes their sealed key, but they
may have kept the key they already opened. Rotating retires it:

```bash
envbyte remove <user-id>
envbyte rotate
```

`rotate` mints a new key, re-encrypts every file, and seals it only to the
people who remain. The server applies it all-or-nothing: it refuses a rotation
that would miss a member or leave a file on the old key.

## Commands

```text
envbyte register | verify | login | logout | whoami | reset-password
envbyte identity [show | publish | export | replace]
envbyte create <project> | init [project] | projects
envbyte push [-m <msg>] [-f <file>] | pull [-f <file>] [--force]
envbyte commit <msg> | logs [--remote] | rollback --local <id> | --address <id>
envbyte add <email> | members | role <id> <role> | remove <id> | rotate [-y]
envbyte status | audit
```

## Configuration

| Variable | Default | Purpose |
|---|---|---|
| `ENVBYTE_SERVER` | `https://api.envbyte.trackedge.in` | API URL; remote servers must use HTTPS |
| `ENVBYTE_IDENTITY_KEY` | unset | Identity secret for CI, instead of a file |
| `ENVBYTE_IDENTITY_FILE` | `~/.envbyte-identity` | Where the identity key is stored |

For CI, give the runner its own account and inject its identity key from the
secret store:

```bash
ENVBYTE_IDENTITY_KEY="$ENVBYTE_CI_IDENTITY" envbyte pull -f .env.ci --force
```

## Crypto

- Content: AES-256-GCM under the project data key, key version bound into the AAD
- Key sealing: ephemeral X25519 → HKDF-SHA256 → AES-256-GCM, both public keys bound into the HKDF info
- Passwords: Argon2id, server-side
- Identity and data keys are zeroized on drop

The server needs running too — see the
[repository](https://github.com/NYLONXD/EnvByte_CLI) for the API, its
container image, and `SECURITY.md`.

## License

MIT or Apache-2.0, at your option.
