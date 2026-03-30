CREATE TABLE admin_project_memberships (
	id uuid PRIMARY KEY,
	admin_user_id uuid NOT NULL REFERENCES admin_users (id) ON DELETE CASCADE,
	project_id uuid NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
	role text NOT NULL CHECK (role IN ('owner', 'admin')),
	created_at timestamptz NOT NULL DEFAULT NOW(),
	UNIQUE (admin_user_id, project_id)
);

CREATE INDEX admin_project_memberships_admin_user_id_idx ON admin_project_memberships (admin_user_id);
CREATE INDEX admin_project_memberships_project_id_idx ON admin_project_memberships (project_id);
