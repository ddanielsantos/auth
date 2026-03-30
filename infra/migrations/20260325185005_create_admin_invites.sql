CREATE TABLE admin_invites (
	id uuid PRIMARY KEY,
	invited_by_admin_user_id uuid NOT NULL REFERENCES admin_users (id) ON DELETE CASCADE,
	org_id uuid REFERENCES organizations (id) ON DELETE CASCADE,
	project_id uuid REFERENCES projects (id) ON DELETE CASCADE,
	invitee_username varchar(50) NOT NULL,
	role text NOT NULL CHECK (role IN ('owner', 'admin')),
	status text NOT NULL CHECK (status IN ('pending', 'accepted', 'declined', 'expired', 'revoked')),
	expires_at timestamptz NOT NULL,
	responded_at timestamptz,
	created_at timestamptz NOT NULL DEFAULT NOW(),
	CHECK (
		(org_id IS NOT NULL AND project_id IS NULL)
		OR (org_id IS NULL AND project_id IS NOT NULL)
	)
);

CREATE INDEX admin_invites_invited_by_admin_user_id_idx ON admin_invites (invited_by_admin_user_id);
CREATE INDEX admin_invites_invitee_username_idx ON admin_invites (invitee_username);
CREATE INDEX admin_invites_org_id_idx ON admin_invites (org_id);
CREATE INDEX admin_invites_project_id_idx ON admin_invites (project_id);
CREATE INDEX admin_invites_status_expires_at_idx ON admin_invites (status, expires_at DESC);
