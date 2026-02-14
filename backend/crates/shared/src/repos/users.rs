use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use super::{Store, StoreError};

impl Store {
    pub async fn connect(
        database_url: &str,
        max_connections: u32,
        data_encryption_key: &str,
    ) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(database_url)
            .await?;

        Ok(Self {
            pool,
            data_encryption_key: data_encryption_key.to_string(),
        })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn ping(&self) -> Result<(), StoreError> {
        let _: i32 = sqlx::query_scalar("SELECT 1").fetch_one(&self.pool).await?;
        Ok(())
    }

    pub async fn create_user(&self) -> Result<Uuid, StoreError> {
        let user_id: Uuid = sqlx::query_scalar("INSERT INTO users DEFAULT VALUES RETURNING id")
            .fetch_one(&self.pool)
            .await?;
        Ok(user_id)
    }

    pub async fn ensure_user(&self, user_id: Uuid) -> Result<(), StoreError> {
        sqlx::query("INSERT INTO users (id) VALUES ($1) ON CONFLICT (id) DO NOTHING")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
