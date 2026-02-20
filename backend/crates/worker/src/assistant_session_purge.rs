use chrono::Utc;
use shared::config::WorkerConfig;
use shared::repos::Store;
use tracing::{debug, error, info};
use uuid::Uuid;

pub(crate) async fn purge_expired_sessions(
    store: &Store,
    config: &WorkerConfig,
    worker_id: Uuid,
) -> u64 {
    let now = Utc::now();
    let purged_rows = match store
        .purge_expired_assistant_encrypted_sessions_batch(
            now,
            i64::from(config.assistant_session_purge_batch_size),
        )
        .await
    {
        Ok(purged_rows) => purged_rows,
        Err(err) => {
            error!(
                worker_id = %worker_id,
                "failed to purge expired assistant encrypted sessions: {err}"
            );
            return 0;
        }
    };

    if purged_rows > 0 {
        info!(
            worker_id = %worker_id,
            purged_rows,
            batch_size = config.assistant_session_purge_batch_size,
            "assistant encrypted session purge tick"
        );
    } else {
        debug!(
            worker_id = %worker_id,
            batch_size = config.assistant_session_purge_batch_size,
            "assistant encrypted session purge tick found no expired rows"
        );
    }

    purged_rows
}
