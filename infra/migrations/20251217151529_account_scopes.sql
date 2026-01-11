CREATE TABLE account_scopes
(
    account_id    UUID NOT NULL REFERENCES user_accounts (id) ON DELETE CASCADE,
    permission_id UUID NOT NULL REFERENCES permissions (id) ON DELETE CASCADE,
    PRIMARY KEY (account_id, permission_id)
);
