use sqlx::Row;
use uuid::Uuid;

use crate::models::Preferences;

use super::{
    DEFAULT_MEETING_REMINDER_MINUTES, DEFAULT_MORNING_BRIEF_LOCAL_TIME, DEFAULT_QUIET_HOURS_END,
    DEFAULT_QUIET_HOURS_START, Store, StoreError,
};

impl Store {
    pub async fn get_or_create_preferences(
        &self,
        user_id: Uuid,
    ) -> Result<Preferences, StoreError> {
        self.ensure_user(user_id).await?;

        if let Some(row) = sqlx::query(
            "SELECT meeting_reminder_minutes, morning_brief_local_time, quiet_hours_start,
                    quiet_hours_end, high_risk_requires_confirm
             FROM user_preferences
             WHERE user_id = $1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?
        {
            return row_to_preferences(&row);
        }

        sqlx::query(
            "INSERT INTO user_preferences (
                user_id,
                meeting_reminder_minutes,
                morning_brief_local_time,
                quiet_hours_start,
                quiet_hours_end,
                high_risk_requires_confirm
             ) VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(user_id)
        .bind(DEFAULT_MEETING_REMINDER_MINUTES)
        .bind(DEFAULT_MORNING_BRIEF_LOCAL_TIME)
        .bind(DEFAULT_QUIET_HOURS_START)
        .bind(DEFAULT_QUIET_HOURS_END)
        .bind(true)
        .execute(&self.pool)
        .await?;

        Ok(Preferences {
            meeting_reminder_minutes: DEFAULT_MEETING_REMINDER_MINUTES as u32,
            morning_brief_local_time: DEFAULT_MORNING_BRIEF_LOCAL_TIME.to_string(),
            quiet_hours_start: DEFAULT_QUIET_HOURS_START.to_string(),
            quiet_hours_end: DEFAULT_QUIET_HOURS_END.to_string(),
            high_risk_requires_confirm: true,
        })
    }

    pub async fn upsert_preferences(
        &self,
        user_id: Uuid,
        preferences: &Preferences,
    ) -> Result<(), StoreError> {
        self.ensure_user(user_id).await?;

        sqlx::query(
            "INSERT INTO user_preferences (
                user_id,
                meeting_reminder_minutes,
                morning_brief_local_time,
                quiet_hours_start,
                quiet_hours_end,
                high_risk_requires_confirm
             ) VALUES ($1, $2, $3, $4, $5, $6)
             ON CONFLICT (user_id)
             DO UPDATE SET
               meeting_reminder_minutes = EXCLUDED.meeting_reminder_minutes,
               morning_brief_local_time = EXCLUDED.morning_brief_local_time,
               quiet_hours_start = EXCLUDED.quiet_hours_start,
               quiet_hours_end = EXCLUDED.quiet_hours_end,
               high_risk_requires_confirm = EXCLUDED.high_risk_requires_confirm,
               updated_at = NOW()",
        )
        .bind(user_id)
        .bind(preferences.meeting_reminder_minutes as i32)
        .bind(&preferences.morning_brief_local_time)
        .bind(&preferences.quiet_hours_start)
        .bind(&preferences.quiet_hours_end)
        .bind(preferences.high_risk_requires_confirm)
        .execute(&self.pool)
        .await?;

        Ok(())
    }
}

fn row_to_preferences(row: &sqlx::postgres::PgRow) -> Result<Preferences, StoreError> {
    let meeting_minutes: i32 = row.try_get("meeting_reminder_minutes")?;
    let meeting_minutes = u32::try_from(meeting_minutes).map_err(|_| {
        StoreError::InvalidData("meeting_reminder_minutes out of range".to_string())
    })?;

    Ok(Preferences {
        meeting_reminder_minutes: meeting_minutes,
        morning_brief_local_time: row.try_get("morning_brief_local_time")?,
        quiet_hours_start: row.try_get("quiet_hours_start")?,
        quiet_hours_end: row.try_get("quiet_hours_end")?,
        high_risk_requires_confirm: row.try_get("high_risk_requires_confirm")?,
    })
}
