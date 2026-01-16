CREATE TABLE admin_users
(
    id            UUID PRIMARY KEY,
    username      VARCHAR(50)  NOT NULL,
    password_hash VARCHAR(255) NOT NULL,
    UNIQUE (username)
);