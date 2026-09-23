# greenbyte

End-to-end encrypted `.env` sharing for teams, with rotatable keys.

```bash
cargo install greenbyte      # or: cargo binstall greenbyte
```

## Why

Sharing `.env` files usually means pasting secrets into chat, or sharing one
master passphrase that can never be changed. Greenbyte does neither.

Every member has an **identity key** that never leaves their device. Every
project has a **data key** that encrypts its files, stored as one sealed copy
per member. The server holds ciphertext and sealed keys, and can open neither.

Because the key exists as per-member copies rather than one shared passphrase,
it can be **rotated** — which is what makes offboarding actually work.

## Getting started

```bash
greenbyte register              # creates your identity key on first use
cd my-app
greenbyte create my-app         # project key is sealed to you; nothing to copy down
greenbyte push -m "initial"
```

Add a colleague — the project key is sealed to them in the same step:

```bash
greenbyte add teammate@example.com
```

They join with the token from their email. No name needed; the invitation
identifies the project:

```bash
greenbyte login
greenbyte init
greenbyte pull
```

## Offboarding

Removing someone revokes their sessions and deletes their sealed key, but they
may have kept the key they already opened. Rotating retires it:

```bash
greenbyte remove <user-id>
greenbyte rotate
```

`rotate` mints a new key, re-encrypts every file, and seals it only to the
people who remain. The server applies it all-or-nothing: it refuses a rotation
that would miss a member or leave a file on the old key.

## Commands

```text
greenbyte register | login | logout | whoami | reset-password
greenbyte identity [show | publish | export | replace]
greenbyte create <project> | init [project] | projects
greenbyte push [-m <msg>] [-f <file>] | pull [-f <file>] [--force]
greenbyte commit <msg> | logs [--remote] | rollback --local <id> | --address <id>
greenbyte add <email> | members | role <id> <role> | remove <id> | rotate [-y]
greenbyte status | audit
```

## Configuration

| Variable | Default | Purpose |
|---|---|---|
| `GREENBYTE_SERVER` | `https://greenbyte-cli.onrender.com` | API URL; remote servers must use HTTPS |
| `GREENBYTE_IDENTITY_KEY` | unset | Identity secret for CI, instead of a file |
| `GREENBYTE_IDENTITY_FILE` | `~/.greenbyte-identity` | Where the identity key is stored |

For CI, give the runner its own account and inject its identity key from the
secret store:

```bash
GREENBYTE_IDENTITY_KEY="$GREENBYTE_CI_IDENTITY" greenbyte pull -f .env.ci --force
```

## Crypto

- Content: AES-256-GCM under the project data key, key version bound into the AAD
- Key sealing: ephemeral X25519 → HKDF-SHA256 → AES-256-GCM, both public keys bound into the HKDF info
- Passwords: Argon2id, server-side
- Identity and data keys are zeroized on drop

The server needs running too — see the
[repository](https://github.com/NYLONXD/GreenByte_CLI) for the API, its
container image, and `SECURITY.md`.

## License

MIT or Apache-2.0, at your option.
