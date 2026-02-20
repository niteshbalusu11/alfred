DELETE FROM dead_letter_jobs
WHERE type <> 'AUTOMATION_RUN';

DELETE FROM jobs
WHERE type <> 'AUTOMATION_RUN';

ALTER TABLE jobs
  DROP CONSTRAINT IF EXISTS jobs_type_check;

ALTER TABLE dead_letter_jobs
  DROP CONSTRAINT IF EXISTS dead_letter_jobs_type_check;

ALTER TABLE jobs
  ADD CONSTRAINT jobs_type_check
  CHECK (type IN ('AUTOMATION_RUN'));

ALTER TABLE dead_letter_jobs
  ADD CONSTRAINT dead_letter_jobs_type_check
  CHECK (type IN ('AUTOMATION_RUN'));
