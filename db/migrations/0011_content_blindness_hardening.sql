-- Issue #149: remove residual host-readable assistant content surfaces and
-- harden job payload storage for content-blind privacy boundaries.

-- Legacy host-readable assistant session storage is removed in favor of
-- enclave-only encrypted session envelopes.
DROP TABLE IF EXISTS assistant_sessions;

-- Historical payload bytes may have been written without encryption semantics.
-- Clear these values pre-launch so no legacy plaintext payload artifacts remain.
UPDATE jobs
SET payload_ciphertext = NULL
WHERE payload_ciphertext IS NOT NULL;

UPDATE dead_letter_jobs
SET payload_ciphertext = NULL
WHERE payload_ciphertext IS NOT NULL;
