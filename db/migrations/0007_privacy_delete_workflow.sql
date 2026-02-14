ALTER TABLE privacy_delete_requests
ADD COLUMN IF NOT EXISTS started_at TIMESTAMPTZ NULL;

ALTER TABLE privacy_delete_requests
ADD COLUMN IF NOT EXISTS completed_at TIMESTAMPTZ NULL;

ALTER TABLE privacy_delete_requests
ADD COLUMN IF NOT EXISTS failed_at TIMESTAMPTZ NULL;

ALTER TABLE privacy_delete_requests
ADD COLUMN IF NOT EXISTS lease_owner TEXT NULL;

ALTER TABLE privacy_delete_requests
ADD COLUMN IF NOT EXISTS lease_expires_at TIMESTAMPTZ NULL;

ALTER TABLE privacy_delete_requests
ADD COLUMN IF NOT EXISTS failure_reason TEXT NULL;

CREATE INDEX IF NOT EXISTS idx_privacy_delete_requests_status_created
  ON privacy_delete_requests (status, created_at ASC);

CREATE INDEX IF NOT EXISTS idx_privacy_delete_requests_running_lease
  ON privacy_delete_requests (status, lease_expires_at)
  WHERE status = 'RUNNING';

CREATE INDEX IF NOT EXISTS idx_privacy_delete_requests_sla
  ON privacy_delete_requests (created_at ASC)
  WHERE status <> 'COMPLETED';
