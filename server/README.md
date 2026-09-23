# Envbyte API server

The server is a zero-knowledge coordination service for the Envbyte CLI. It stores ciphertext, per-member sealed project keys it cannot open, project membership, authentication state, immutable versions, and audit metadata. It never receives an openable key or decrypted environment content.

See [`../docs/ARCHITECTURE.md`](../docs/ARCHITECTURE.md) for the module layout and how to add a feature.

## Local development

From the repository root:

```bash
cp .env.example .env
docker compose up --build -d
docker compose logs -f api
```

The API listens on `http://localhost:3030` and PostgreSQL is bound to loopback port `5432`. Database migrations run automatically during startup.

In local `ENVBYTE_EMAIL_MODE=log`, invitation tokens appear in API logs. This mode is rejected when `ENVBYTE_ENV=production`.

## Configuration

| Variable | Required/default | Purpose |
|---|---|---|
| `DATABASE_URL` | required | PostgreSQL connection string |
| `ENVBYTE_JWT_SECRET` | required, minimum 32 chars | HS256 signing secret |
| `ENVBYTE_ENV` | `development` | Set to `production` in deployments |
| `ENVBYTE_BIND` | `0.0.0.0:3030` | Listen address |
| `ENVBYTE_JWT_ISSUER` | `envbyte` | Required JWT issuer |
| `ENVBYTE_ACCESS_TTL_SECONDS` | `3600` | Access-token lifetime |
| `ENVBYTE_REFRESH_TTL_DAYS` | `30` | Refresh-session lifetime |
| `ENVBYTE_INVITE_TTL_HOURS` | `24` | Invitation lifetime |
| `ENVBYTE_EMAIL_VERIFICATION_TTL_HOURS` | `2` | Account verification lifetime |
| `ENVBYTE_PASSWORD_RESET_TTL_MINUTES` | `30` | Password reset lifetime |
| `DATABASE_MAX_CONNECTIONS` | `20` | SQL connection pool size |
| `ENVBYTE_RATE_LIMIT_PER_MINUTE` | `300` | General per-instance/IP limit |
| `ENVBYTE_AUTH_RATE_LIMIT_PER_MINUTE` | `20` | Authentication per-instance/IP limit |
| `ENVBYTE_EMAIL_MODE` | `log` | `log` or `smtp` |
| `SMTP_HOST`, `SMTP_PORT` | SMTP mode | SMTP endpoint |
| `SMTP_USERNAME`, `SMTP_PASSWORD` | SMTP mode | SMTP credentials |
| `SMTP_FROM` | SMTP mode | Invitation sender mailbox |
| `RUST_LOG` | server defaults | Tracing filter |

## Production deployment

- Terminate HTTPS at a trusted reverse proxy or managed load balancer.
- Set `ENVBYTE_ENV=production` and use SMTP mode.
- Inject database, JWT, and SMTP credentials from a secret manager.
- Use a PostgreSQL role restricted to the Envbyte database.
- Back up PostgreSQL and continuously test restoration.
- Rotate the JWT secret through a controlled session-invalidating deployment.
- Put a distributed rate limiter at the edge for multi-replica deployments.
- Export container logs and health probes to the platform's monitoring system.
- Keep the API container non-root and read-only, as configured in Compose.

## Tests

```bash
docker compose up -d postgres
DATABASE_URL=postgres://envbyte:envbyte-local-change-me@localhost:5432/envbyte \
  cargo test --workspace --locked
```

The SQLx integration test creates and drops isolated test databases. The configured PostgreSQL test role therefore needs `CREATEDB`; do not grant that permission to the production application role.
