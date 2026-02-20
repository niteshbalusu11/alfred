CREATE INDEX IF NOT EXISTS idx_assistant_encrypted_sessions_expires_at_id
  ON assistant_encrypted_sessions (expires_at, id);
