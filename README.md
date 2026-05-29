# 🌿 Greenbyte

> Team `.env` manager — secure, versioned, collaborative. No more WhatsApp.

---

## The Problem

You're tired of maintaining `.env` files locally and sending them to your team over WhatsApp. Greenbyte fixes this with encrypted, versioned, team-synced environment management.

---

## How It Works

```
Your .env
   │
   ▼ encrypt (AES-256-GCM)
   │   key = derive(master_key + MAC address)
   │
   ▼
Server (stores only encrypted blob)
   │
   ▼ decrypt (needs master_key + MAC)
   │
Team member's .env
```

- **Master Key** — generated at project creation. Only you have it. Store it in a password manager.
- **MAC address** — your machine's hardware address. Ties decryption to your device.
- **OTT (One-Time Token)** — sent via email when you're added as a collaborator. Expires in 24h.
- **Refresher Token** — `SHA256(OTT + MAC)`. Device-bound, stored in `.greenbyte`.

---

## Installation

```bash
cargo install --path .
```

Or build manually:
```bash
cargo build --release
# binary at ./target/release/greenbyte
```

---

## Commands

### Account

```bash
greenbyte register       # Create a new account
greenbyte login          # Login to existing account
```

### Project Setup

```bash
# Owner: create a new project
greenbyte create myapp

# Collaborator: join an existing project (needs OTT from email)
greenbyte init myapp
```

### Daily Use

```bash
greenbyte push -m "add stripe keys"   # Encrypt & push .env to server
greenbyte pull                         # Fetch & decrypt .env from server
greenbyte status                       # Show project + auth state
```

### Versioning

```bash
greenbyte commit "before adding prod keys"   # Local encrypted snapshot
greenbyte logs                               # Show all commits
greenbyte logs --file greenbyte.log          # Save logs to a file

greenbyte rollback --address <commit-id>     # Rollback server to a commit
greenbyte rollback --local <commit-id>       # Restore local .env from snapshot
```

### Team

```bash
greenbyte add teammate@company.com   # Sends OTT via email
```

---

## Local Files

| File | Purpose |
|---|---|
| `.greenbyte` | Project config + tokens (per-directory) |
| `.greenbyte-logs` | Local encrypted commit history |
| `~/.greenbyte-auth` | Global login token |

**Add all three to `.gitignore`** — Greenbyte does this automatically on `create`/`init`.

---

## Security Model

- The server **never** sees your plaintext `.env`
- The server **never** sees your master key
- Decryption requires **both** master key **and** the registered MAC address
- Collaborator access is gated by OTT (email) + device MAC binding
- All encryption uses AES-256-GCM with Argon2id key derivation

---

## Environment Variables

| Variable | Default | Description |
|---|---|---|
| `GREENBYTE_SERVER` | `https://api.greenbyte.dev` | Override server URL (for self-hosting) |

---

## Project Structure

```
src/
├── main.rs                  # CLI entry point + command routing
├── commands/
│   ├── auth.rs              # register, login
│   ├── project.rs           # create, init
│   ├── sync.rs              # push, pull
│   ├── commit.rs            # local commit snapshot
│   ├── logs.rs              # show logs
│   ├── rollback.rs          # server + local rollback
│   ├── collaborator.rs      # add collaborator
│   └── status.rs            # project status
└── utils/
    ├── crypto.rs            # AES-256-GCM encrypt/decrypt
    ├── mac.rs               # MAC address + refresher token
    ├── local_store.rs       # .greenbyte + .greenbyte-logs I/O
    └── config.rs            # server URL + .env file helpers
```
