CREATE TABLE IF NOT EXISTS oauth_states (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  state_hash BYTEA NOT NULL UNIQUE,
  redirect_uri TEXT NOT NULL,
  expires_at TIMESTAMPTZ NOT NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  consumed_at TIMESTAMPTZ NULL
);

CREATE INDEX IF NOT EXISTS idx_oauth_states_user_id ON oauth_states (user_id);
CREATE INDEX IF NOT EXISTS idx_oauth_states_expires_at ON oauth_states (expires_at);
