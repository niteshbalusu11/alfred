CREATE INDEX IF NOT EXISTS idx_devices_user_id ON devices (user_id);
CREATE INDEX IF NOT EXISTS idx_connectors_user_provider ON connectors (user_id, provider);
CREATE INDEX IF NOT EXISTS idx_jobs_state_due_at ON jobs (state, due_at);
CREATE INDEX IF NOT EXISTS idx_jobs_user_id ON jobs (user_id);
CREATE INDEX IF NOT EXISTS idx_audit_events_user_created_id ON audit_events (user_id, created_at DESC, id DESC);
CREATE INDEX IF NOT EXISTS idx_privacy_delete_requests_user_created ON privacy_delete_requests (user_id, created_at DESC);
