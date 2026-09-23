# Envbyte API contract

This document describes the server interface consumed by CLI v0.3. All request and response bodies are JSON. Remote deployments must use HTTPS.

## Common behavior

- Authenticated endpoints accept `Authorization: Bearer <token>`.
- Errors use an appropriate HTTP status and `{ "error": "safe message" }` or `{ "message": "safe message" }`.
- The server must enforce account and project authorization rather than trusting client-supplied IDs.
- Unknown JSON fields should be ignored to preserve forward compatibility.
- IDs are opaque strings.

## Authentication

### `POST /auth/signup`

Request: `{ "username", "email", "password" }`. Creates an unverified account, emails a single-use verification token, and returns `{ "verification_required": true, "email" }`.

### `POST /auth/verify-email`

Request: `{ "email", "token" }`. Consumes the verification token and returns the authentication response.

### `POST /auth/login`

Request: `{ "email", "password" }`.

Login and email verification return `{ "token", "refresh_token", "user_id", "username", "email" }`.

### `POST /auth/refresh`

Request: `{ "refresh_token" }`. Atomically revokes the presented refresh token and returns a new authentication response with a rotated refresh token.

### `POST /auth/logout`

Request: `{ "refresh_token" }`. Revokes the refresh session. The operation is idempotent.

### `POST /auth/forgot-password`

Request: `{ "email" }`. Always returns a generic success response and emails a single-use reset token when a verified account exists.

### `POST /auth/reset-password`

Request: `{ "email", "token", "password" }`. Consumes the reset token, replaces the password hash, and revokes every existing refresh session for the account.

### `GET /users/me`

Returns `{ "id", "username", "email", "public_key" }`. `public_key` is `null`
until the client publishes one.

### `PUT /users/me/key`

Authenticated. Request: `{ "public_key" }` - a base64 32-byte X25519 key.
Publishes the caller's identity public key so collaborators can seal project
keys to it. The key is public by design. Replacing it does not revoke existing
grants, which were sealed to the previous key and can no longer be opened; an
admin re-grants afterwards.

Returns `{ "ok": true, "changed": <bool>, "replaced_existing": <bool> }`.

## Projects

### `POST /projects`

Authenticated. Request: `{ "name", "wrapped_key" }`, where `wrapped_key` is the
project data key sealed to the creator's own identity key. The server stores it
opaquely and cannot open it.

Returns `{ "project_id", "key_version", "soft_token" }`. New projects start at
key version 1.

Project names are unique **per owner**, not across the installation. A duplicate
under the same owner returns `409 Conflict`; the same name under a different
owner is accepted.

### `GET /projects`

Authenticated. Returns the caller's projects as
`{ "id", "name", "owner_username", "role", "key_version" }`. `owner_username` is
required to display a name unambiguously, since names are scoped per owner.

### `POST /projects/join`

Authenticated. Request: `{ "ott", "refresher_token", "mac_address" }`, where
`mac_address` is optional. Atomically consumes the OTT and returns
`{ "project_id", "project_name", "owner_username", "key_version", "soft_token" }`.

The project is identified by the invitation alone. It must not be named in the
path: doing so made project names a globally probeable namespace, and nothing
about joining requires the caller to know the name in advance.

An invitation proves that its holder was invited, never that they are the invitee, so this endpoint must reject a caller whose session belongs to any account other than the one named on the invitation, and must never return account credentials. A forwarded or intercepted invitation therefore grants nothing on its own.

### `POST /projects/{project_id}/collaborators/lookup`

Owner/admin operation. Request: `{ "email" }`. Returns
`{ "user_id", "username", "public_key" }` so the inviting admin can seal the
project key to the invitee.

`404` if no verified account has that address; `409` if the account exists but
has published no identity key. Restricted to project admins, who could already
learn whether an address is registered by attempting the invite itself.

### `POST /projects/{project_id}/collaborators`

Owner/admin operation. Request: `{ "email", "wrapped_key" }`, where
`wrapped_key` is the project's **current** data key sealed to the invitee's
published identity key.

The invitation and the key grant are written in one transaction: an invitee who
can join but cannot decrypt is a worse outcome than a failed invite. Returns
`{ "ok": true, "expires_in_hours" }`.

### `GET /projects/{project_id}/collaborators`

Returns members with `user_id`, `username`, `email`, `role`, `public_key`, the
highest `key_version` they hold, and `joined_at`. An admin needs `public_key` to
re-seal the data key during a rotation, and `key_version` to see who would be
left behind.

### `GET /projects/{project_id}/collaborators/{user_id}/grants`

Owner/admin operation. Returns
`{ "key_versions", "current_key_version", "holds_current_key" }` for one member.

### `PATCH /projects/{project_id}/collaborators/{user_id}`

Owner/admin operation with `{ "role": "admin|member|viewer" }`. Only owners can grant or revoke the admin role.

### `DELETE /projects/{project_id}/collaborators/{user_id}`

Owner/admin operation. Removes membership, deletes every sealed key held by that
user for the project, and revokes their project device sessions. Owners cannot
be removed through this endpoint.

Returns `{ "ok": true, "rotation_required": true, "detail" }`. Removal cannot
unlearn a data key the member already opened, so the response always points at a
rotation.

## Project keys

### `GET /projects/{project_id}/keys`

Authenticated project-member operation. Returns every grant the **caller** holds
as `[{ "key_version", "wrapped_key" }]`, oldest first. All versions are
returned, not just the current one, so history and rolled-back files written
under a retired key stay readable to members who were present at the time.

`404` if the caller holds no grant - they are a member whose key was never
sealed to them, and an admin must re-grant.

The server must never return a grant belonging to another user.

### `POST /projects/{project_id}/key-rotations`

Owner/admin operation. Request:

```json
{
  "grants": [{ "user_id": "...", "wrapped_key": "greenbyte:wrap:v1:..." }],
  "files":  [{ "filename": ".env", "content": "greenbyte:v3:..." }]
}
```

Applied in a single transaction, and rejected with `400` unless it covers
**every** current member and no one else, and **every** stored file and no
others. A partial rotation is worse than none: a missing grant locks out a
colleague, and a file left on the old key stays readable to whoever the rotation
was meant to shut out.

On success it increments the project's `key_version`, inserts the new grants,
deletes every grant belonging to a non-member at every version, writes a new
version of each file at the new key version, and records an audit event.
Returns `{ "key_version", "members_granted", "files_reencrypted", "grants_revoked" }`.

Earlier key versions held by current members are retained, so history stays
readable to the team.

## Environment files

### `POST /users/me/env`

Authenticated. Request:

```json
{
  "project_id": "opaque-project-id",
  "commit_id": "client-generated-uuid",
  "filename": ".env.production",
  "content": "greenbyte:v3:<base64-envelope>",
  "message": "rotate database password",
  "key_version": 2
}
```

Validate write permission and filename, then atomically create a remote
history entry. Returns `{ "commit_id", "key_version" }`.

`key_version` must equal the project's current version; a mismatch is rejected
with `409 Conflict` naming both. Because the server cannot decrypt, this is the
only defence against a client that missed a rotation replacing the current
version with a payload the rest of the team can no longer read.

`commit_id` makes pushes idempotent. Repeating an identical request with the same ID returns the existing commit; reusing the ID with different content or metadata returns `409 Conflict`.

### `GET /users/me/env?project_id={project_id}`

Authenticated project-member operation. Returns an array of
`{ "filename", "content", "key_version" }` for that project only. Never return
files from another project owned by the same user.

`key_version` tells the client which of its held keys to use - a rolled-back
file may sit on an older version than the project's current one.

## Rollback

### `POST /projects/{project_id}/rollback`

Authenticated. Request: `{ "commit_id" }`. Verify that the commit belongs to the same project, atomically make it current, and retain an audit event.

### `GET /projects/{project_id}/commits`

Authenticated project-member operation. Returns up to 500 newest entries containing `commit_id`, `filename`, `message`,
`author`, `key_version`, and `created_at`.

### `GET /projects/{project_id}/audit`

Owner/admin operation returning up to 500 recent membership and environment mutation audit events. Audit metadata must never contain ciphertext, tokens, key material, or
plaintext secrets. It records identifiers and the shape of a change: filenames,
key versions, roles, and counts.

## Required production controls

The backend implementation is in `server/`. Production operators must provide TLS termination, SMTP credentials, database backups, secret rotation, centralized metrics/logging, and an external distributed rate limiter when running more than one API replica.
