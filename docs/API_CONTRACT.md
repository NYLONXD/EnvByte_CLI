# Greenbyte API contract

This document describes the server interface consumed by CLI v0.2. All request and response bodies are JSON. Remote deployments must use HTTPS.

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

Returns at least `{ "username", "email" }`.

## Projects

### `POST /projects`

Authenticated. Request: `{ "name", "master_key_hash" }`. The hash is a high-entropy key verifier, not an encryption key. Return `{ "project_id", "soft_token" }`.

### `POST /projects/{project_name}/join`

Authenticated. Request: `{ "ott", "refresher_token", "mac_address" }`, where `mac_address` is optional. Atomically consume the OTT and return `{ "project_id", "soft_token" }`.

An invitation proves that its holder was invited, never that they are the invitee, so this endpoint must reject a caller whose session belongs to any account other than the one named on the invitation, and must never return account credentials. A forwarded or intercepted invitation therefore grants nothing on its own.

### `POST /projects/{project_id}/collaborators`

Authenticated owner/admin operation. Request: `{ "email" }`. Generate a single-use, hashed-at-rest OTT and deliver it out of band.

### `GET /projects/{project_id}/collaborators`

Returns project members and their user IDs, usernames, verified emails, roles, and join timestamps.

### `PATCH /projects/{project_id}/collaborators/{user_id}`

Owner/admin operation with `{ "role": "admin|member|viewer" }`. Only owners can grant or revoke the admin role.

### `DELETE /projects/{project_id}/collaborators/{user_id}`

Owner/admin operation that removes membership and revokes the user's project device sessions. Owners cannot be removed through this endpoint.

## Environment files

### `POST /users/me/env`

Authenticated. Request:

```json
{
  "project_id": "opaque-project-id",
  "commit_id": "client-generated-uuid",
  "filename": ".env.production",
  "content": "greenbyte:v2:<base64-envelope>",
  "message": "rotate database password",
  "master_key_hash": "verifier-for-the-key-the-content-was-encrypted-under"
}
```

Validate project membership and filename, then atomically create a remote history entry. Return `{ "commit_id": "opaque-id" }`.

`master_key_hash` is required and must equal the verifier registered when the project was created; compare it in constant time and reject a mismatch with `409 Conflict`. Because the server cannot decrypt, this is the only defence against a member holding the wrong key replacing the current version with a payload nobody else can read.

`commit_id` makes pushes idempotent. Repeating an identical request with the same ID returns the existing commit; reusing the ID with different content or metadata returns `409 Conflict`.

### `GET /users/me/env?project_id={project_id}`

Authenticated. Return an array of `{ "filename", "content" }` for that project only. Never return files from another project owned by the same user.

## Rollback

### `POST /projects/{project_id}/rollback`

Authenticated. Request: `{ "commit_id" }`. Verify that the commit belongs to the same project, atomically make it current, and retain an audit event.

### `GET /projects/{project_id}/commits`

Authenticated project-member operation. Returns up to 500 newest entries containing `commit_id`, `filename`, `message`, `author`, and `created_at`.

### `GET /projects/{project_id}/audit`

Owner/admin operation returning up to 500 recent membership and environment mutation audit events. Audit metadata must never contain ciphertext, tokens, master-key material, or plaintext secrets.

## Required production controls

The backend implementation is in `server/`. Production operators must provide TLS termination, SMTP credentials, database backups, secret rotation, centralized metrics/logging, and an external distributed rate limiter when running more than one API replica.
