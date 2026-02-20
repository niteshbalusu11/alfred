CREATE TABLE IF NOT EXISTS automation_rules (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  status TEXT NOT NULL CHECK (status IN ('ACTIVE', 'PAUSED')),
  schedule_type TEXT NOT NULL CHECK (schedule_type IN ('INTERVAL_SECONDS')),
  interval_seconds INT NOT NULL CHECK (interval_seconds BETWEEN 60 AND 604800),
  time_zone TEXT NOT NULL CHECK (char_length(trim(time_zone)) BETWEEN 1 AND 128),
  next_run_at TIMESTAMPTZ NOT NULL,
  last_run_at TIMESTAMPTZ NULL,
  prompt_ciphertext BYTEA NOT NULL,
  prompt_sha256 TEXT NOT NULL CHECK (prompt_sha256 ~ '^[A-Fa-f0-9]{64}$'),
  lease_owner TEXT NULL,
  lease_expires_at TIMESTAMPTZ NULL,
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_automation_rules_claimable
  ON automation_rules (status, next_run_at, lease_expires_at);

CREATE INDEX IF NOT EXISTS idx_automation_rules_user_created
  ON automation_rules (user_id, created_at DESC, id DESC);

CREATE TABLE IF NOT EXISTS automation_runs (
  id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
  rule_id UUID NOT NULL REFERENCES automation_rules(id) ON DELETE CASCADE,
  user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
  scheduled_for TIMESTAMPTZ NOT NULL,
  job_id UUID NULL REFERENCES jobs(id) ON DELETE SET NULL,
  idempotency_key TEXT NOT NULL,
  state TEXT NOT NULL CHECK (state IN ('MATERIALIZED', 'ENQUEUED', 'FAILED')) DEFAULT 'MATERIALIZED',
  created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
  UNIQUE (rule_id, scheduled_for),
  UNIQUE (user_id, idempotency_key)
);

CREATE INDEX IF NOT EXISTS idx_automation_runs_rule_scheduled
  ON automation_runs (rule_id, scheduled_for DESC, id DESC);

CREATE INDEX IF NOT EXISTS idx_automation_runs_user_created
  ON automation_runs (user_id, created_at DESC, id DESC);
