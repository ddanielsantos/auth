ALTER TABLE projects
ADD CONSTRAINT unique_org_project_name UNIQUE (org_id, name);
