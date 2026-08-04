# Security policy

## Supported version

Security fixes target the latest release on the `main` branch.

## Reporting a vulnerability

Do not open a public issue containing credentials, ciphertext samples tied to real projects, or exploit details. Contact the maintainers privately and include the affected version, reproduction steps, impact, and any suggested mitigation.

## Threat model

Greenbyte protects environment-file contents from an honest-but-curious storage service and from disclosure of stored ciphertext. AES-256-GCM authenticates content, and Argon2id derives a per-envelope key from the project master key and a random salt.

Greenbyte does not protect a device after an attacker obtains the project master key, decrypted `.env` files, process memory, or an active user session. It also cannot recover a lost project key.

Device MAC addresses are identifiers, not secrets or cryptographic factors. They are retained for enrollment compatibility and legacy ciphertext only.

## Operator requirements

- Terminate TLS at the API and reject cleartext remote traffic.
- Hash account passwords with a memory-hard password hash.
- Store only encrypted environment payloads; never request or log master keys.
- Rate-limit login, registration, join, invitation, and refresh endpoints.
- Make OTTs single-use, store only their hashes, and expire them within 24 hours.
- Rotate signing keys and short-lived access tokens; revoke refresh tokens on logout or device removal.
- Enforce project membership on every environment and history operation.
- Keep immutable, secret-free audit events for membership and data mutations.
- Back up the database and routinely test restoration.

## Client operational guidance

- Keep the project key in a password manager and share it separately from invitations.
- Treat `.greenbyte`, `.greenbyte-auth`, `.greenbyte-logs`, and all `.env*` files as sensitive.
- Use `GREENBYTE_MASTER_KEY` only through a protected CI secret store.
- Review shell debug output and CI logs to ensure environment variables are not echoed.
