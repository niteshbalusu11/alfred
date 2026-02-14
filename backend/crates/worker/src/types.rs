use chrono::{DateTime, Utc};

#[derive(Debug, Clone, Copy)]
pub(crate) enum FailureClass {
    Transient,
    Permanent,
}

#[derive(Debug)]
pub(crate) struct JobExecutionError {
    pub(crate) class: FailureClass,
    pub(crate) code: String,
    pub(crate) message: String,
}

impl JobExecutionError {
    pub(crate) fn transient(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            class: FailureClass::Transient,
            code: code.into(),
            message: message.into(),
        }
    }

    pub(crate) fn permanent(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            class: FailureClass::Permanent,
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Default)]
pub(crate) struct WorkerTickMetrics {
    pub(crate) claimed_jobs: usize,
    pub(crate) processed_jobs: usize,
    pub(crate) successful_jobs: usize,
    pub(crate) retryable_failures: usize,
    pub(crate) permanent_failures: usize,
    pub(crate) dead_lettered_jobs: usize,
    pub(crate) push_attempts: usize,
    pub(crate) push_delivered: usize,
    pub(crate) push_quiet_hours_suppressed: usize,
    pub(crate) push_transient_failures: usize,
    pub(crate) push_permanent_failures: usize,
    pub(crate) total_lag_seconds: i64,
    pub(crate) max_lag_seconds: i64,
}

impl WorkerTickMetrics {
    pub(crate) fn record_lag(&mut self, due_at: DateTime<Utc>, now: DateTime<Utc>) {
        let lag_seconds = (now - due_at).num_seconds().max(0);
        self.total_lag_seconds += lag_seconds;
        self.max_lag_seconds = self.max_lag_seconds.max(lag_seconds);
    }

    pub(crate) fn average_lag_seconds(&self) -> f64 {
        if self.processed_jobs == 0 {
            return 0.0;
        }

        self.total_lag_seconds as f64 / self.processed_jobs as f64
    }

    pub(crate) fn success_rate(&self) -> f64 {
        if self.processed_jobs == 0 {
            return 1.0;
        }

        self.successful_jobs as f64 / self.processed_jobs as f64
    }
}
