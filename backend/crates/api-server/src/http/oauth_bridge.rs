use std::collections::HashMap;

use axum::extract::Query;
use axum::response::Redirect;
use url::Url;

const IOS_OAUTH_CALLBACK_URI: &str = "alfred://oauth/google/callback";

pub(super) async fn redirect_google_oauth_callback(
    Query(params): Query<HashMap<String, String>>,
) -> Redirect {
    let redirect_url = build_ios_oauth_callback_url(&params);
    // Use 303 for best compatibility when the upstream callback method varies.
    Redirect::to(&redirect_url)
}

fn build_ios_oauth_callback_url(params: &HashMap<String, String>) -> String {
    let mut url =
        Url::parse(IOS_OAUTH_CALLBACK_URI).expect("static iOS callback URI must be valid");
    {
        let mut query = url.query_pairs_mut();
        for key in ["code", "state", "error", "error_description", "scope"] {
            if let Some(value) = params.get(key) {
                query.append_pair(key, value);
            }
        }
    }

    url.to_string()
}

#[cfg(test)]
mod tests {
    use super::build_ios_oauth_callback_url;
    use std::collections::HashMap;

    #[test]
    fn keeps_expected_google_callback_parameters() {
        let mut params = HashMap::new();
        params.insert("code".to_string(), "oauth-code".to_string());
        params.insert("state".to_string(), "state-123".to_string());
        params.insert(
            "error_description".to_string(),
            "consent denied".to_string(),
        );
        params.insert("unexpected".to_string(), "ignored".to_string());

        let redirect = build_ios_oauth_callback_url(&params);

        assert!(redirect.starts_with("alfred://oauth/google/callback?"));
        assert!(redirect.contains("code=oauth-code"));
        assert!(redirect.contains("state=state-123"));
        assert!(redirect.contains("error_description=consent+denied"));
        assert!(!redirect.contains("unexpected="));
    }
}
