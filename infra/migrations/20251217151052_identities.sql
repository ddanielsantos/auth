CREATE TABLE identities
(
    id         UUID PRIMARY KEY     DEFAULT gen_random_uuid(),
    is_active  BOOLEAN     NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
