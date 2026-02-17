use std::path::PathBuf;

use shared::repos::Store;
use sqlx::postgres::{PgPool, PgPoolOptions};
use tokio::sync::OnceCell;

static MIGRATIONS_APPLIED: OnceCell<()> = OnceCell::const_new();

const DEFAULT_DATABASE_URL: &str = "postgres://postgres:postgres@127.0.0.1:5432/alfred";
const DEFAULT_DATA_ENCRYPTION_KEY: &str = "integration-tests-data-key";

pub async fn test_store() -> Store {
    let database_url = test_database_url();
    apply_migrations_once(&database_url).await;

    Store::connect(&database_url, 10, DEFAULT_DATA_ENCRYPTION_KEY)
        .await
        .expect("test store connection should succeed")
}

pub async fn reset_database(pool: &PgPool) {
    sqlx::query(
        "TRUNCATE TABLE
            outbound_action_idempotency,
            dead_letter_jobs,
            jobs,
            audit_events,
            oauth_states,
            assistant_encrypted_sessions,
            connectors,
            devices,
            user_preferences,
            privacy_delete_requests,
            users
         RESTART IDENTITY CASCADE",
    )
    .execute(pool)
    .await
    .expect("database reset should succeed");
}

fn test_database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| DEFAULT_DATABASE_URL.to_string())
}

async fn apply_migrations_once(database_url: &str) {
    MIGRATIONS_APPLIED
        .get_or_init(|| async move {
            let pool = PgPoolOptions::new()
                .max_connections(2)
                .connect(database_url)
                .await
                .expect("migration pool connection should succeed");

            let migrations_dir =
                PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../db/migrations");
            let migrator = sqlx::migrate::Migrator::new(migrations_dir)
                .await
                .expect("migrations should load");
            migrator
                .run(&pool)
                .await
                .expect("migrations should apply successfully");
        })
        .await;
}
