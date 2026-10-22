# Envbyte project review

Date: 2026-10-03

Scope: the server, CLI, crypto, migrations, CI and install scripts. The website
was only skimmed for its claims. Nothing was run. The 2 MB limit (#7) was
checked against axum's source code. The race conditions (#4, #5, #6) come from
reading the transaction code and have not been reproduced.

All paths are relative to the repository root.

---

## 1. The LinkedIn question

> How are you handling conflicts when the same secret gets updated from two devices?

### What the code does today

Envbyte uses **last-write-wins on the whole file**. If two devices push the
same `.env`, the second push becomes the current version. Nothing is lost, but
the conflict isn't detected yet.

- **Every push is kept.** Each push adds a new encrypted version to the
  history; it never overwrites one. Recording that version and moving the
  "current" pointer happen in one database transaction with a row lock
  (`server/src/features/env_files/handlers.rs:54-56`). Two pushes at the same
  moment take turns and can't corrupt each other.
- **There's no "were you up to date?" check.** A push doesn't say which
  version it started from, so the server can't tell that device B is
  overwriting changes it never saw.
- **The older change can be recovered.** It stays in history:
  `envbyte logs --remote`, then `envbyte rollback --address <commit>`.
- **There's already a 409 for one case.** After a key rotation, a push
  encrypted with the old key is rejected
  (`server/src/features/env_files/service.rs:24`).
- **Retries are safe.** The CLI picks the commit ID before sending, so a push
  retried after a network error isn't stored twice.
- **The server can't merge for you.** It only ever sees encrypted data, so any
  merge has to happen on the device.

### Reply to paste

> Thanks! Honest answer: right now it's last-write-wins at the file level.
>
> Every push is stored as a new immutable, encrypted version, and the "current"
> pointer moves inside a single locked transaction, so simultaneous pushes
> can't corrupt each other. But if two devices edit from the same starting
> point, the second push becomes current.
>
> Nothing is lost, though. The other version stays in history, and
> `envbyte logs --remote` + `envbyte rollback` bring it back.
>
> The interesting constraint is the end-to-end encryption. The server only ever
> sees ciphertext, so it *can't* merge two versions. Any merge has to happen on
> the client.
>
> Next step I'm planning: each push says which version it was based on. If the
> server has moved on since then, it rejects the push, and the CLI pulls the
> latest, decrypts it locally and merges key by key. It only asks you when the
> same key changed on both devices. Key rotation already uses this
> reject-and-retry pattern, so a device holding an old key can't overwrite a
> file the team can no longer read.

Only post the "Next step" paragraph if you mean to build it.

This reply is accurate if two teammates edit the same file. If they meant one
person on two laptops, finding #1 below is what that person would actually
hit, so don't promise that works smoothly yet.

---

## 2. Will the server check add load?

No. The check costs almost nothing:

- **No extra query.** A push already locks that file's row
  (`server/src/features/env_files/handlers.rs:56`). The check reads one more
  column from that row and compares two IDs.
- **Conflicts get cheaper.** On a conflict the server says no before writing
  anything.
- **The client does the heavy work.** Decrypting and merging run on the user's
  machine, and only when there's a real conflict.
- **It has to happen on the server.** If device A checks "am I up to date?"
  and then pushes, device B can push in between those two steps. Only the
  server can check and write in one step. Git, HTTP `If-Match` and S3
  conditional writes all work this way.

So doing the server check first is the right order. It's useful on its own:
the server can reject with "pull first", like git does. The merge can come
later.

**Prerequisite:** pull doesn't return a commit ID
(`server/src/db/models.rs:26`), and the CLI doesn't remember which version it
pulled. Without that, a push has nothing to compare against.

### Planned steps

1. The server returns `commit_id` with each file on pull.
2. The CLI records the last commit it pulled or pushed for each file, in
   `.envbyte`.
3. Push sends `base_commit_id`.
4. Inside the push transaction, the server compares `base_commit_id` with the
   file's current version and returns 409 if they differ.
5. Later: on a 409, the CLI pulls the latest version, decrypts it and merges
   key by key, asking only when the same key changed on both sides.

---

## 3. Findings

### Fix first

#### 1. Logging in on a second device breaks the first one

On a new machine, `envbyte login` creates a new identity key and uploads it,
replacing the account's key on the server (`cli/src/commands/account.rs:148`,
`cli/src/commands/context.rs:68`).

- The new machine can't open any existing project keys.
- The old machine keeps working until the next rotation or invite. Those now
  lock keys to the new machine's key, and the old machine never notices
  (`cli/src/commands/context.rs:75`).
- The only workaround is running `identity export` before logging in on the
  second machine, and nothing tells the user to.

**Fix:** if the server already has a different key, don't overwrite it. Add
`envbyte identity import` instead. Longer term: one key per device.

#### 2. A second device can't connect to a project you're already in

`envbyte init` needs an invitation token (`cli/src/commands/project.rs:68`).
Inviting an existing member returns 409
(`server/src/features/members/handlers.rs:97`). `.envbyte` is gitignored, so a
fresh clone has no project link.

**Fix:** add an `envbyte link owner/project` command that picks from
`GET /projects`.

#### 3. The CLI sends your login tokens to whatever server a `.envbyte` file names

The saved login doesn't record which server it belongs to
(`cli/src/core/workspace/global_auth.rs:13`). Project commands use the server
URL from the local `.envbyte` (`cli/src/commands/context.rs:34`, `:105`).

- A repo that ships a `.envbyte` pointing at an attacker's server gets your
  access and refresh tokens as soon as you run `envbyte status` there.
- With the refresh token they can sign in as you, upload their own identity
  key, and receive the next project key that's handed out.

**Fix:** store the server URL with the tokens, and never send them anywhere
else.

#### 4. A push can be saved under the old key during a key rotation

The key-version check runs before the push transaction starts, with no lock
(`server/src/features/env_files/handlers.rs:41`,
`server/src/features/env_files/service.rs:19`). If a rotation finishes in that
gap, the push becomes the current version, encrypted with the retired key. For
a new filename the push doesn't even wait for the rotation.
`docs/ARCHITECTURE.md:149` says this can't happen.

**Fix:** do the check inside the push transaction, with
`SELECT key_version FROM projects WHERE id = $1 FOR SHARE`. Pushes still won't
block each other; only a rotation blocks them.

#### 5. A rotation can silently undo a teammate's push

`rotate` downloads the files, re-encrypts them locally, then uploads them
(`cli/src/commands/rotate.rs:255`). Anything pushed in between gets replaced
(`server/src/features/keys/rotation.rs:95`). This is the same lost-update
problem as the LinkedIn question.

**Fix:** include the commit each file was based on, and have the server reject
the rotation if a file changed since. This uses the same mechanism as the
conflict check in section 2.

#### 6. Rotating while an invite is pending locks the invitee out

The invitee's copy of the key is stored before they become a member. Rotation
deletes copies held by non-members
(`server/src/features/keys/rotation.rs:65`).

- When they join, they have no key. The error tells them to ask for a re-invite
  (`server/src/features/keys/handlers.rs:44`), but a re-invite now returns 409
  because they're a member.
- Only another rotation fixes it.

**Fix:** rotation also re-locks the new key for users with pending
invitations, or keeps their copies and re-locks them.

### Should fix

#### 7. The real request size limit is 2 MB, not 16 MB

axum applies its own 2 MB JSON limit, separate from the 20 MB one in
`server/src/app.rs:50`.

- The ciphertext is base64-encoded twice
  (`cli/src/core/crypto/envelope.rs:84-90`), so a push can carry only about
  1.1 MB of `.env` content.
- Rotation sends every file in one request, so a project whose files add up to
  about 1.1 MB can never be rotated.
- The CLI allows files up to 5 MB (`cli/src/core/env_files.rs:3`).

**Fix:** add `DefaultBodyLimit::max(...)` and make the CLI and server limits
agree.

#### 8. Rollback can bring back content locked with a retired key

Rollback just points back to an old version
(`server/src/features/env_files/service.rs:68`). If that version is from
before a rotation, members who joined later can't read it.

**Fix:** do rollback as a new commit: decrypt the old version, re-encrypt it
with the current key and push it. History also stays append-only.

#### 9. An invite can mislabel the key version

The CLI locks its newest key for the invitee (`cli/src/commands/members.rs:23`),
but the server labels it with a version it reads separately
(`server/src/features/members/handlers.rs:102`, `:137`). During a rotation
these can differ.

**Fix:** send `key_version` with the invite and check it inside the
transaction.

#### 10. Dead code that sends your machine's MAC address to the server

`device_sessions`, `soft_token` and `refresher_token` are written
(`server/src/features/projects/handlers.rs:63`, `:169`) and never read. The CLI
still sends the MAC address on join (`cli/src/commands/project.rs:85`). That's
personal data stored for no reason.

**Fix:** remove it.

#### 11. The invitation email is out of date

It tells people to "obtain the project master key through a separate secure
channel" (`server/src/infra/email.rs:145`). Master keys no longer exist. The
email also says the invite expires in 24 hours, but that time is configurable.

#### 12. Users have to trust the server to hand out the right public keys

`add` shows the invitee's fingerprint but doesn't remember it
(`cli/src/commands/members.rs:31`). `rotate` encrypts for whatever key the
server returns (`cli/src/commands/rotate.rs:326`). Key copies aren't signed, so
anyone who can write to the database can slip one in.

"The server can't read your secrets" holds against a database leak, but not
against a compromised server.

**Later fix:** remember each member's key fingerprint and warn when it
changes, and sign key copies.

### Smaller issues

- **`forgot-password`:** there's no cooldown, so anyone can flood an inbox and
  use up your Resend quota. It also responds more slowly for real accounts,
  because it sends the email before replying
  (`server/src/features/accounts/session.rs:244`). Signup and invite have the
  same timing issue. Send email from a background task.
- **Signup** returns 409 for a taken email. That reveals which addresses are
  registered, which login and forgot-password were built to hide.
- **`publish_key`** takes a row lock outside a transaction, so the lock does
  nothing (`server/src/features/accounts/profile.rs:48`).
- **Two CLI commands at once:** if both try to refresh the login, one fails
  with 401, because refresh tokens are single-use
  (`cli/src/commands/context.rs:104`).
- **Nothing is ever deleted:** old tokens, refresh sessions, audit rows and
  every version's full ciphertext stay forever. That matters on Supabase's free
  500 MB.
- **Please check Render:** if `ENVBYTE_TRUSTED_PROXY_HOPS` isn't set, every
  user shares one rate limit of 20 auth requests per minute.
- **Swappable files:** the encryption doesn't tie a file to its filename or
  project (`cli/src/core/crypto/envelope.rs:154`), so a server could swap
  `.env` and `.env.production` without the CLI noticing.
- **Pull** always downloads every file, even with `--file`
  (`server/src/features/env_files/handlers.rs:118`).
- **`envbyte commit`** only reads `.env`, while `push` handles every `.env*`
  file.

### What's already solid

- Each change and its audit entry are saved together in one transaction.
- Tokens are stored only as hashes.
- Login timing doesn't reveal which emails exist.
- Retried pushes are safe.
- Local files are written atomically with private permissions.
- Rotation refuses to apply partially.
- The key version is part of the encrypted data's integrity check.
- CI checks dependencies for known vulnerabilities and checks the minimum Rust
  version.
- The installer verifies checksums.

---

## 4. Suggested order

1. **#3**: security fix, small change.
2. **#4**: move one query into the transaction.
3. **#1 and #2**: multi-device.
4. **Conflict check (section 2) together with #5**: both use the same "based
   on commit X" mechanism.
5. **#6 to #11**.
6. **#12 and the smaller issues**.
