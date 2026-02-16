use std::collections::HashMap;

use shared::config::WorkerConfig;
use shared::llm::LlmGateway;
use shared::repos::Store;
use shared::security::SecretRuntime;

use crate::{NotificationContent, PushSender};

pub(crate) struct JobActionContext<'a> {
    pub(crate) store: &'a Store,
    pub(crate) config: &'a WorkerConfig,
    pub(crate) secret_runtime: &'a SecretRuntime,
    pub(crate) oauth_client: &'a reqwest::Client,
    pub(crate) llm_gateway: &'a dyn LlmGateway,
    pub(crate) push_sender: &'a PushSender,
}

pub(crate) struct JobActionResult {
    pub(crate) notification: Option<NotificationContent>,
    pub(crate) metadata: HashMap<String, String>,
}
