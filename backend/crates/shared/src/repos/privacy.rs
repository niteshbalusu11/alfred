use uuid::Uuid;

use super::{Store, StoreError};

impl Store {
    pub async fn queue_delete_all(&self, user_id: Uuid) -> Result<Uuid, StoreError> {
        self.ensure_user(user_id).await?;

        let request_id: Uuid = sqlx::query_scalar(
            "INSERT INTO privacy_delete_requests (user_id, status)
             VALUES ($1, 'QUEUED')
             RETURNING id",
        )
        .bind(user_id)
        .fetch_one(&self.pool)
        .await?;

        Ok(request_id)
    }
}
