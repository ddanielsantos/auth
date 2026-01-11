CREATE TABLE user_accounts
(
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    identity_id        UUID NOT NULL REFERENCES identities (id) ON DELETE CASCADE,
    project_id         UUID NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    local_profile_data JSONB            DEFAULT '{}',

    UNIQUE (identity_id, project_id)
);
