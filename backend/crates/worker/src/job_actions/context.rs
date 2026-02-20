use std::collections::HashMap;

use shared::repos::Store;

use crate::{NotificationContent, PushSender};

pub(crate) struct JobActionContext<'a> {
    pub(crate) store: &'a Store,
    pub(crate) push_sender: &'a PushSender,
}

pub(crate) struct JobActionResult {
    pub(crate) notification: Option<NotificationContent>,
    pub(crate) metadata: HashMap<String, String>,
}
