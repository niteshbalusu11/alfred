ALTER TABLE automation_rules
  ADD COLUMN IF NOT EXISTS title TEXT;

UPDATE automation_rules
SET title = 'Task'
WHERE title IS NULL OR btrim(title) = '';

ALTER TABLE automation_rules
  ALTER COLUMN title SET NOT NULL;

ALTER TABLE automation_rules
  DROP CONSTRAINT IF EXISTS automation_rules_title_check;

ALTER TABLE automation_rules
  ADD CONSTRAINT automation_rules_title_check
  CHECK (char_length(btrim(title)) BETWEEN 1 AND 120);
