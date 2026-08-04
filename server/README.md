# Greenbyte API server

The server is a zero-knowledge coordination service for the Greenbyte CLI. It stores ciphertext, project membership, authentication state, immutable versions, and audit metadata. It never receives project master keys or decrypted environment content.

## Local development

From the repository root:

```bash
cp .env.example .env
docker compose up --build -d
docker compose logs -f api
```

The API listens on `http://localhost:3030` and PostgreSQL is bound to loopback port `5432`. Database migrations run automatically during startup.

In local `GREENBYTE_EMAIL_MODE=log`, invitation tokens appear in API logs. This mode is rejected when `GREENBYTE_ENV=production`.

## Configuration

| Variable | Required/default | Purpose |
|---|---|---|
| `DATABASE_URL` | required | PostgreSQL connection string |
| `GREENBYTE_JWT_SECRET` | required, minimum 32 chars | HS256 signing secret |
| `GREENBYTE_ENV` | `development` | Set to `production` in deployments |
| `GREENBYTE_BIND` | `0.0.0.0:3030` | Listen address |
| `GREENBYTE_JWT_ISSUER` | `greenbyte` | Required JWT issuer |
| `GREENBYTE_ACCESS_TTL_SECONDS` | `3600` | Access-token lifetime |
| `GREENBYTE_REFRESH_TTL_DAYS` | `30` | Refresh-session lifetime |
| `GREENBYTE_INVITE_TTL_HOURS` | `24` | Invitation lifetime |
| `GREENBYTE_EMAIL_VERIFICATION_TTL_HOURS` | `2` | Account verification lifetime |
| `GREENBYTE_PASSWORD_RESET_TTL_MINUTES` | `30` | Password reset lifetime |
| `DATABASE_MAX_CONNECTIONS` | `20` | SQL connection pool size |
| `GREENBYTE_RATE_LIMIT_PER_MINUTE` | `300` | General per-instance/IP limit |
| `GREENBYTE_AUTH_RATE_LIMIT_PER_MINUTE` | `20` | Authentication per-instance/IP limit |
| `GREENBYTE_EMAIL_MODE` | `log` | `log` or `smtp` |
| `SMTP_HOST`, `SMTP_PORT` | SMTP mode | SMTP endpoint |
| `SMTP_USERNAME`, `SMTP_PASSWORD` | SMTP mode | SMTP credentials |
| `SMTP_FROM` | SMTP mode | Invitation sender mailbox |
| `RUST_LOG` | server defaults | Tracing filter |

## Production deployment

- Terminate HTTPS at a trusted reverse proxy or managed load balancer.
- Set `GREENBYTE_ENV=production` and use SMTP mode.
- Inject database, JWT, and SMTP credentials from a secret manager.
- Use a PostgreSQL role restricted to the Greenbyte database.
- Back up PostgreSQL and continuously test restoration.
- Rotate the JWT secret through a controlled session-invalidating deployment.
- Put a distributed rate limiter at the edge for multi-replica deployments.
- Export container logs and health probes to the platform's monitoring system.
- Keep the API container non-root and read-only, as configured in Compose.

## Tests

```bash
docker compose up -d postgres
DATABASE_URL=postgres://greenbyte:greenbyte-local-change-me@localhost:5432/greenbyte \
  cargo test --workspace --locked
```

The SQLx integration test creates and drops isolated test databases. The configured PostgreSQL test role therefore needs `CREATEDB`; do not grant that permission to the production application role.
