CREATE TABLE admin_org_memberships (
	id uuid PRIMARY KEY,
	admin_user_id uuid NOT NULL REFERENCES admin_users (id) ON DELETE CASCADE,
	org_id uuid NOT NULL REFERENCES organizations (id) ON DELETE CASCADE,
	role text NOT NULL CHECK (role IN ('owner', 'admin')),
	created_at timestamptz NOT NULL DEFAULT NOW(),
	UNIQUE (admin_user_id, org_id)
);

CREATE INDEX admin_org_memberships_admin_user_id_idx ON admin_org_memberships (admin_user_id);
CREATE INDEX admin_org_memberships_org_id_idx ON admin_org_memberships (org_id);
