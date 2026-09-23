# Security policy

## Supported versions

Security fixes target the latest release on `main`.

## Reporting a vulnerability

Do not open a public issue containing credentials, ciphertext tied to real
projects, or exploit details. Contact the maintainers privately with the
affected version, reproduction steps, impact, and any suggested mitigation.

## Design

Three layers of key material, each with a different lifetime:

| Key | Where it lives | Who can use it |
|---|---|---|
| Identity key (X25519) | The member's device, or their CI secret store | That device only |
| Project data key (32 bytes) | Sealed once per member, stored by the server | Every current member |
| Derived wrapping key | Never stored | Derived per sealing operation |

**Sealing.** Ephemeral X25519 → HKDF-SHA256 → AES-256-GCM. Both the ephemeral
and recipient public keys are bound into the HKDF `info`, so a sealed key cannot
be replayed against a different recipient.

**Content.** AES-256-GCM under the project data key, with the key version bound
into the authenticated data. Relabelling a payload as a different version fails
to verify, so a rotation cannot be undone by editing a header.

The data key is 256 bits of CSPRNG output and is used directly, with no password
stretching, because there is no password to stretch.

## Threat model

Envbyte protects environment-file contents from the storage service and from
disclosure of stored ciphertext. An attacker holding the full database sees
ciphertext, sealed keys they cannot open, and metadata: account identifiers,
project and file names, payload sizes, timestamps, and the audit trail.

Envbyte does **not** protect against an attacker who obtains a member's
identity key, their decrypted `.env` files, their process memory, or an active
session. It cannot recover a lost identity key — but, unlike a shared master
key, losing one costs nothing permanent: an admin re-grants access.

Invitations are authorization to add a named account to a project, not
credentials for that account. Redeeming one requires an authenticated session
belonging to the invitee and never returns a token, so a forwarded or
intercepted invitation cannot become a login.

Device MAC addresses are identifiers, not secrets or cryptographic factors. They
are retained for enrolment compatibility and for reading the oldest ciphertext
format only. Every current operation succeeds without one, so containers and CI
runners remain supported.

## Revocation and rotation

Removing a member deletes their sealed keys and revokes their device sessions.
It does not, and cannot, unlearn the data key they already opened.

`envbyte rotate` is what retires that key. It mints a new data key,
re-encrypts every stored file under it, and seals it only to current members.
The server applies this in one transaction and rejects it unless it covers
every current member and every stored file, because a partial rotation is worse
than none: a missing grant locks out a colleague, and a file left behind stays
readable with the key being retired.

Grants belonging to former members are deleted at every version. Earlier
versions held by current members are kept, so project history remains readable
to the team.

**Rotate whenever** someone leaves the project, an identity key may have leaked,
a device is lost, or a CI secret store is compromised.

## Operator requirements

- Terminate TLS at the API and reject cleartext remote traffic.
- Hash account passwords with a memory-hard password hash.
- Store only sealed keys and encrypted payloads; never request, log, or derive a
  plaintext key.
- Rate-limit sign-in, registration, join, invitation and refresh endpoints. The
  built-in limiter is per-process and keyed on the socket address, so behind a
  load balancer or across replicas it must be replaced with a shared limiter or
  supplemented at the edge.
- Make one-time tokens single-use, store only their hashes, and expire them.
- Require an authenticated session to redeem an invitation, bound to the invited
  account.
- Reject pushed ciphertext whose key version is not the project's current one.
- Refuse a rotation that does not cover every member and every file.
- Enforce project membership on every environment, key and history operation.
- Keep immutable, secret-free audit events for membership and data mutations.
- Back up the database and routinely test restoration.

## Client guidance

- `~/.envbyte-identity` is as sensitive as an SSH private key. It is written
  `0600` and never transmitted.
- Give CI its own account rather than reusing a person's identity, so revoking
  CI does not mean rotating a human's key. Inject it via
  `ENVBYTE_IDENTITY_KEY` from the runner's secret store.
- Treat `.envbyte`, `.envbyte-logs`, `~/.envbyte-auth` and every `.env*`
  as sensitive. Project initialization adds them to `.gitignore`.
- Check shell debug output and CI logs to confirm environment variables are not
  echoed.
- Confirm a colleague's identity fingerprint out of band when the project
  warrants it. `envbyte add` prints the fingerprint it is sealing to.
