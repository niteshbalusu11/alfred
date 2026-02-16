use std::collections::HashMap;

#[derive(Debug, Default)]
pub(crate) struct ReplayGuard {
    nonces: HashMap<String, i64>,
}

impl ReplayGuard {
    pub(crate) fn verify_and_record(
        &mut self,
        challenge_nonce: &str,
        expires_at: i64,
        now: i64,
    ) -> Result<(), ()> {
        self.nonces.retain(|_, expiry| *expiry >= now);

        if self.nonces.contains_key(challenge_nonce) {
            return Err(());
        }

        self.nonces
            .insert(challenge_nonce.to_string(), expires_at.max(now));
        Ok(())
    }
}
