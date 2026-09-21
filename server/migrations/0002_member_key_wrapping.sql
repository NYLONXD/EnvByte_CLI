-- Envelope encryption, key rotation, and per-owner project namespaces.
--
-- Before this migration a project had a single master key that every member
-- typed in, shared out of band, and could never rotate. Now each user owns an
-- X25519 identity key, each project owns a versioned data key (DEK), and the
-- DEK is wrapped to every member's public key. Rotation is then mechanical:
-- mint a new DEK, re-wrap it for the members who remain, and re-encrypt.

-- ── Identity keys ────────────────────────────────────────────────────────────
-- base64 X25519 public key. NULL until the client uploads one, so that accounts
-- created before this migration keep working until their next login.
ALTER TABLE users ADD COLUMN public_key TEXT;
ALTER TABLE users ADD COLUMN public_key_updated_at TIMESTAMPTZ;

-- ── Versioned project data keys ──────────────────────────────────────────────
ALTER TABLE projects ADD COLUMN key_version INTEGER NOT NULL DEFAULT 1;
-- Only legacy (pre-v3) projects still carry a master key verifier.
ALTER TABLE projects ALTER COLUMN master_key_hash DROP NOT NULL;

CREATE TABLE project_key_grants (
    id UUID PRIMARY KEY,
    project_id UUID NOT NULL REFERENCES projects(id) ON DELETE CASCADE,
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    key_version INTEGER NOT NULL,
    -- The project DEK sealed to this member's public key. The server cannot
    -- open it; it only stores and hands it back to the member it belongs to.
    wrapped_key TEXT NOT NULL,
    granted_by UUID REFERENCES users(id) ON DELETE SET NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (project_id, user_id, key_version)
);
CREATE INDEX project_key_grants_member_idx ON project_key_grants(project_id, user_id);

-- Which key version each stored ciphertext was written under, so a member can
-- pick the right grant when reading history or a rolled-back file.
ALTER TABLE env_versions ADD COLUMN key_version INTEGER NOT NULL DEFAULT 1;

-- ── Per-owner project namespace ──────────────────────────────────────────────
-- Project names used to be unique across the entire installation, so the first
-- account to take "backend" held it forever and anyone could probe which names
-- existed. Names are now unique only within one owner's account.
DROP INDEX projects_name_lower_uq;
CREATE UNIQUE INDEX projects_owner_name_lower_uq ON projects (owner_id, lower(name));
