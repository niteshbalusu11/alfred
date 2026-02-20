ALTER TABLE devices
  ADD COLUMN IF NOT EXISTS notification_key_algorithm TEXT NULL,
  ADD COLUMN IF NOT EXISTS notification_public_key_ciphertext BYTEA NULL;

DO $$
BEGIN
  IF NOT EXISTS (
    SELECT 1
    FROM pg_constraint
    WHERE conname = 'devices_notification_key_fields_check'
  ) THEN
    ALTER TABLE devices
      ADD CONSTRAINT devices_notification_key_fields_check
      CHECK (
        (
          notification_key_algorithm IS NULL
          AND notification_public_key_ciphertext IS NULL
        )
        OR (
          notification_key_algorithm = ('x25519' || '-chacha20poly1305')
          AND notification_public_key_ciphertext IS NOT NULL
        )
      );
  END IF;
END $$;
