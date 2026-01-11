CREATE TABLE applications
(
    id                 UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    project_id         UUID                NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    client_id          VARCHAR(100) UNIQUE NOT NULL,
    client_secret_hash VARCHAR(255)        NOT NULL,
    redirect_uris      TEXT[]              NOT NULL
);
