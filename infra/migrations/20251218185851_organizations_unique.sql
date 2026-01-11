ALTER TABLE organizations
ADD CONSTRAINT unique_organization_name UNIQUE (name);
