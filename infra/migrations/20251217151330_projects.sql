CREATE TABLE projects
(
    id                      UUID PRIMARY KEY      DEFAULT gen_random_uuid(),
    org_id                  UUID         NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
    name                    VARCHAR(255) NOT NULL,
    shared_identity_context BOOLEAN      NOT NULL DEFAULT FALSE
);
