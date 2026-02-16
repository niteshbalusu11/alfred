CREATE TABLE IF NOT EXISTS assistant_encrypted_sessions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  session_id UUID NOT NULL,
  state_json TEXT NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  expires_at TIMESTAMPTZ NOT NULL,
  UNIQUE (user_id, session_id)
);

CREATE INDEX IF NOT EXISTS idx_assistant_encrypted_sessions_user_updated
  ON assistant_encrypted_sessions (user_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_assistant_encrypted_sessions_user_expires_at
  ON assistant_encrypted_sessions (user_id, expires_at);
