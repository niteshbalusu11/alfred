ALTER TABLE user_preferences
ADD COLUMN IF NOT EXISTS time_zone TEXT NOT NULL DEFAULT 'UTC';

UPDATE user_preferences
SET time_zone = 'UTC'
WHERE trim(time_zone) = '';
