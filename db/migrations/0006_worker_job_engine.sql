ALTER TABLE jobs ADD COLUMN IF NOT EXISTS lease_owner TEXT;
ALTER TABLE jobs ADD COLUMN IF NOT EXISTS lease_expires_at TIMESTAMPTZ;
ALTER TABLE jobs ADD COLUMN IF NOT EXISTS attempts INT NOT NULL DEFAULT 0 CHECK (attempts >= 0);
ALTER TABLE jobs ADD COLUMN IF NOT EXISTS max_attempts INT NOT NULL DEFAULT 5 CHECK (max_attempts > 0);
ALTER TABLE jobs ADD COLUMN IF NOT EXISTS idempotency_key TEXT;
ALTER TABLE jobs ADD COLUMN IF NOT EXISTS last_error_code TEXT;
ALTER TABLE jobs ADD COLUMN IF NOT EXISTS last_error_message TEXT;
ALTER TABLE jobs ADD COLUMN IF NOT EXISTS updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

UPDATE jobs
SET idempotency_key = id::text
WHERE idempotency_key IS NULL;

ALTER TABLE jobs ALTER COLUMN idempotency_key SET NOT NULL;

CREATE TABLE IF NOT EXISTS dead_letter_jobs (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  job_id UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  type TEXT NOT NULL CHECK (type IN ('MEETING_REMINDER', 'MORNING_BRIEF', 'URGENT_EMAIL_CHECK')),
  idempotency_key TEXT NOT NULL,
  attempts INT NOT NULL CHECK (attempts >= 1),
  reason_code TEXT NOT NULL,
  reason_message TEXT NOT NULL,
  payload_ciphertext BYTEA NULL,
  failed_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (job_id)
);

CREATE TABLE IF NOT EXISTS outbound_action_idempotency (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  action_key TEXT NOT NULL,
  job_id UUID NOT NULL REFERENCES jobs(id) ON DELETE CASCADE,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (user_id, action_key)
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_jobs_user_type_idempotency
  ON jobs (user_id, type, idempotency_key);

CREATE INDEX IF NOT EXISTS idx_jobs_claimable
  ON jobs (state, due_at, lease_expires_at);

CREATE INDEX IF NOT EXISTS idx_jobs_running_user_lease
  ON jobs (user_id, state, lease_expires_at);

CREATE INDEX IF NOT EXISTS idx_dead_letter_jobs_user_failed
  ON dead_letter_jobs (user_id, failed_at DESC);
