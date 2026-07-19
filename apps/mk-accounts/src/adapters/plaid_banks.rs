use crate::config::expect_env_var;
use crate::utils::api_client_utils::{ApiClient, RequestBuilderExt};
use anyhow::anyhow as err;
use file_cache::{Cacheable, FileBytes};
use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};
use serde_json::json;

#[derive(Debug)]
pub struct PlaidConfig {
    pub app_id: String,
    pub app_secret: String,
    // pub redirect_uri: String,
    // pub recv_code_port: u16, // Port to listen for redirect code
}
impl PlaidConfig {
    pub fn from_env() -> Self {
        PlaidConfig {
            app_id: expect_env_var("PLAID_APP_ID"),
            app_secret: expect_env_var("PLAID_APP_SECRET"),
            // redirect_uri: expect_env_var("PLAID_APP_REDIRECT_URI"),
            // recv_code_port: 5473,
        }
    }
}

pub struct PlaidClient {
    pub config: PlaidConfig,
    pub http_client: reqwest::Client,
    pub access_token: Option<String>,
}
impl ApiClient for PlaidClient {
    fn base_url(&self) -> &str {
        "https://sandbox.plaid.com"
    }
    fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }
    fn default_params(&self, mut req_builder: RequestBuilder) -> RequestBuilder {
        req_builder = req_builder.header("Content-Type", "application/json");
        req_builder = req_builder.header("Accept", "application/json");
        req_builder
    }
}
impl PlaidClient {
    pub fn new() -> Self {
        PlaidClient {
            config: PlaidConfig::from_env(),
            http_client: reqwest::Client::new(),
            access_token: None, // Initialize access_token as None
        }
    }

    pub async fn login(&mut self) -> anyhow::Result<()> {
        // try from cache
        if let Ok(cached_token) = TokenResp::from_cache(TokenResp::uniq_relative_path()) {
            self.access_token = Some(cached_token.access_token);
            return Ok(());
        }

        let body = json!({
            "client_id": self.config.app_id,
            "client_secret": self.config.app_secret,
            "grant_type": "client_credentials",
            "scope": "user:read",
        });

        let response = self
            .post("/oauth/token")
            .json(&body)
            .fetch_json::<TokenResp>()
            .await?;

        response.to_cache()?;
        self.access_token = Some(response.access_token);

        Ok(())
    }
    async fn _expect_login(&self) -> anyhow::Result<()> {
        if self.access_token.is_none() {
            return Err(err!("Not logged in"));
        }
        Ok(())
    }
    async fn _ensure_login(&mut self) -> anyhow::Result<()> {
        if self.access_token.is_none() {
            self.login().await?;
        }
        Ok(())
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub struct TokenResp {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_in: u64,
    pub token_type: String,
    pub request_id: String,
}
impl FileBytes for TokenResp {
    fn as_file_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(self)?)
    }
    fn from_file_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(bytes)?)
    }
}
impl Cacheable for TokenResp {
    fn is_expired(&self) -> bool {
        false // TODO on parse resp, turn expires in into expires_at
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Test needs manual web login"]
    async fn test_plaid_login_and_fetch_accounts() -> anyhow::Result<()> {
        let mut client = PlaidClient::new();
        client.login().await?;
        assert!(client.access_token.is_some());

        // let accounts = client.list_accounts().await?;
        // dbg!(&accounts);
        // assert!(!accounts.is_empty());

        Ok(())
    }
}
