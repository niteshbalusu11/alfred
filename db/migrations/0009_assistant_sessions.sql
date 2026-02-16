CREATE TABLE IF NOT EXISTS assistant_sessions (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  session_id UUID NOT NULL,
  last_capability TEXT NOT NULL,
  turn_count INT NOT NULL CHECK (turn_count >= 0),
  memory_ciphertext BYTEA NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  expires_at TIMESTAMPTZ NOT NULL,
  UNIQUE (user_id, session_id)
);

CREATE INDEX IF NOT EXISTS idx_assistant_sessions_user_updated
  ON assistant_sessions (user_id, updated_at DESC);

CREATE INDEX IF NOT EXISTS idx_assistant_sessions_expires_at
  ON assistant_sessions (expires_at);

CREATE INDEX IF NOT EXISTS idx_assistant_sessions_user_expires_at
  ON assistant_sessions (user_id, expires_at);
