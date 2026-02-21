use std::collections::HashMap;

use shared::enclave::EnclaveRpcClient;
use shared::enclave::EncryptedAutomationNotificationEnvelope;
use shared::repos::Store;

use crate::{NotificationContent, PushSender};

pub(crate) struct JobActionContext<'a> {
    pub(crate) store: &'a Store,
    pub(crate) push_sender: &'a PushSender,
    pub(crate) enclave_client: &'a EnclaveRpcClient,
}

pub(crate) struct JobActionResult {
    pub(crate) notification: Option<NotificationContent>,
    pub(crate) encrypted_envelopes_by_device:
        HashMap<String, EncryptedAutomationNotificationEnvelope>,
    pub(crate) metadata: HashMap<String, String>,
}
