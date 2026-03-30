CREATE TABLE auth_events (
	id uuid PRIMARY KEY,
	event_type text NOT NULL,
	success boolean NOT NULL,
	route text NOT NULL,
	admin_user_id uuid REFERENCES admin_users (id) ON DELETE SET NULL,
	application_id uuid REFERENCES applications (id) ON DELETE SET NULL,
	identifier text,
	ip_address text,
	http_status integer,
	occurred_at timestamptz NOT NULL DEFAULT NOW()
);

CREATE INDEX auth_events_occurred_at_idx ON auth_events (occurred_at DESC);
CREATE INDEX auth_events_success_occurred_at_idx ON auth_events (success, occurred_at DESC);
CREATE INDEX auth_events_route_occurred_at_idx ON auth_events (route, occurred_at DESC);
