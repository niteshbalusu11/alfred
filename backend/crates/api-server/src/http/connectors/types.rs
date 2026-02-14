use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub(super) struct GoogleTokenResponse {
    pub(super) refresh_token: Option<String>,
    pub(super) scope: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(super) struct GoogleOAuthErrorResponse {
    pub(super) error: String,
    pub(super) error_description: Option<String>,
}
