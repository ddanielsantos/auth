CREATE TABLE login_methods
(
    id            UUID PRIMARY KEY      DEFAULT gen_random_uuid(),
    identity_id   UUID         NOT NULL REFERENCES identities (id) ON DELETE CASCADE,
    method_type   VARCHAR(50)  NOT NULL,
    identifier    VARCHAR(255) NOT NULL,
    password_hash VARCHAR(255),
    is_verified   BOOLEAN      NOT NULL DEFAULT FALSE,
    UNIQUE (method_type, identifier)
);

CREATE INDEX idx_login_lookup ON login_methods (identifier, method_type);
