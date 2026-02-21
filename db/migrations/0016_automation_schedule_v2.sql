ALTER TABLE automation_rules
  ADD COLUMN IF NOT EXISTS local_time_minutes INT,
  ADD COLUMN IF NOT EXISTS anchor_day_of_week SMALLINT,
  ADD COLUMN IF NOT EXISTS anchor_day_of_month SMALLINT,
  ADD COLUMN IF NOT EXISTS anchor_month SMALLINT;

UPDATE automation_rules
SET schedule_type = CASE
      WHEN interval_seconds >= 604800 THEN 'WEEKLY'
      ELSE 'DAILY'
    END,
    local_time_minutes = (
      EXTRACT(HOUR FROM (next_run_at AT TIME ZONE time_zone))::INT * 60
      + EXTRACT(MINUTE FROM (next_run_at AT TIME ZONE time_zone))::INT
    ),
    anchor_day_of_week = CASE
      WHEN interval_seconds >= 604800
        THEN EXTRACT(ISODOW FROM (next_run_at AT TIME ZONE time_zone))::SMALLINT
      ELSE NULL
    END,
    anchor_day_of_month = NULL,
    anchor_month = NULL
WHERE local_time_minutes IS NULL;

ALTER TABLE automation_rules
  ALTER COLUMN local_time_minutes SET NOT NULL;

ALTER TABLE automation_rules
  DROP CONSTRAINT IF EXISTS automation_rules_schedule_type_check;

ALTER TABLE automation_rules
  ADD CONSTRAINT automation_rules_schedule_type_check
  CHECK (schedule_type IN ('DAILY', 'WEEKLY', 'MONTHLY', 'ANNUALLY'));

ALTER TABLE automation_rules
  DROP CONSTRAINT IF EXISTS automation_rules_interval_seconds_check;

ALTER TABLE automation_rules
  ADD CONSTRAINT automation_rules_interval_seconds_check
  CHECK (interval_seconds BETWEEN 60 AND 31556952);

ALTER TABLE automation_rules
  DROP CONSTRAINT IF EXISTS automation_rules_local_time_minutes_check;

ALTER TABLE automation_rules
  ADD CONSTRAINT automation_rules_local_time_minutes_check
  CHECK (local_time_minutes BETWEEN 0 AND 1439);

ALTER TABLE automation_rules
  DROP CONSTRAINT IF EXISTS automation_rules_schedule_anchor_check;

ALTER TABLE automation_rules
  ADD CONSTRAINT automation_rules_schedule_anchor_check
  CHECK (
    (schedule_type = 'DAILY'
      AND anchor_day_of_week IS NULL
      AND anchor_day_of_month IS NULL
      AND anchor_month IS NULL)
    OR (schedule_type = 'WEEKLY'
      AND anchor_day_of_week BETWEEN 1 AND 7
      AND anchor_day_of_month IS NULL
      AND anchor_month IS NULL)
    OR (schedule_type = 'MONTHLY'
      AND anchor_day_of_week IS NULL
      AND anchor_day_of_month BETWEEN 1 AND 31
      AND anchor_month IS NULL)
    OR (schedule_type = 'ANNUALLY'
      AND anchor_day_of_week IS NULL
      AND anchor_day_of_month BETWEEN 1 AND 31
      AND anchor_month BETWEEN 1 AND 12)
  );
