ALTER TABLE applications
ALTER COLUMN client_id TYPE UUID
USING client_id::UUID;