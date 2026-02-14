ALTER TABLE connectors
ADD COLUMN IF NOT EXISTS token_key_id TEXT NOT NULL DEFAULT '__legacy__';

ALTER TABLE connectors
ADD COLUMN IF NOT EXISTS token_rotated_at TIMESTAMPTZ NOT NULL DEFAULT NOW();

UPDATE connectors
SET token_key_id = '__legacy__'
WHERE token_key_id IS NULL;
