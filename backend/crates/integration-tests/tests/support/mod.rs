#![allow(dead_code)]

pub mod api_app;
pub mod assistant_encrypted;
pub mod clerk;
pub mod enclave_mock;

use std::path::PathBuf;

use shared::repos::Store;
use sqlx::postgres::{PgPool, PgPoolOptions};
use tokio::sync::OnceCell;

static MIGRATIONS_APPLIED: OnceCell<()> = OnceCell::const_new();

pub const DEFAULT_DATABASE_URL: &str = "postgres://postgres:postgres@127.0.0.1:5432/alfred_test";
pub const DEFAULT_DATA_ENCRYPTION_KEY: &str = "integration-tests-data-key";
pub const DEFAULT_REDIS_URL: &str = "redis://127.0.0.1:6379/0";

pub async fn test_store() -> Store {
    let database_url = test_database_url();
    assert_test_database_url(database_url.as_str());
    apply_migrations_once(&database_url).await;

    Store::connect(&database_url, 10, DEFAULT_DATA_ENCRYPTION_KEY)
        .await
        .expect("test store connection should succeed")
}

pub async fn reset_database(pool: &PgPool) {
    assert_test_database_pool(pool).await;
    sqlx::query(
        "TRUNCATE TABLE
            outbound_action_idempotency,
            dead_letter_jobs,
            automation_runs,
            automation_rules,
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

pub fn test_redis_url() -> String {
    std::env::var("REDIS_URL").unwrap_or_else(|_| DEFAULT_REDIS_URL.to_string())
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

fn assert_test_database_url(database_url: &str) {
    let base = database_url.split('?').next().unwrap_or(database_url);
    let database_name = base.rsplit('/').next().unwrap_or_default();
    assert!(
        is_isolated_test_database(database_name),
        "integration tests require an isolated test database (*_test or *_ci), got: {database_url}"
    );
}

async fn assert_test_database_pool(pool: &PgPool) {
    let current_database: String = sqlx::query_scalar("SELECT current_database()")
        .fetch_one(pool)
        .await
        .expect("current database lookup should succeed");
    assert!(
        is_isolated_test_database(current_database.as_str()),
        "integration tests may only reset isolated test databases (*_test or *_ci), got: {current_database}"
    );
}

fn is_isolated_test_database(database_name: &str) -> bool {
    database_name.ends_with("_test") || database_name.ends_with("_ci")
}
