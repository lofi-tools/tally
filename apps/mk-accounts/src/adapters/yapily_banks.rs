use base64::{Engine, prelude::BASE64_STANDARD};
use reqwest::RequestBuilder;

use crate::{
    config::expect_env_var,
    utils::api_client_utils::{ApiClient, RequestBuilderExt},
};

#[derive(Debug)]
pub struct YapilyConfig {
    pub app_id: String,
    pub app_secret: String,
    // pub redirect_uri: String,
    // pub recv_code_port: u16, // Port to listen for redirect code
}
impl YapilyConfig {
    pub fn from_env() -> Self {
        YapilyConfig {
            app_id: expect_env_var("YAPILY_APP_ID"),
            app_secret: expect_env_var("YAPILY_APP_SECRET"),
            // redirect_uri: expect_env_var("YAPILY_REDIRECT_URI"),
            // recv_code_port: 5473,
        }
    }
}

pub struct YapilyClient {
    pub config: YapilyConfig,
    pub http_client: reqwest::Client,
    pub base_url: String,
    // pub user_id: String, // Added for applicationUserId
    pub access_token: Option<String>,
}
impl ApiClient for YapilyClient {
    fn base_url(&self) -> &str {
        // "https://auth.yapily.com" // Changed base URL to Yapily API
        &self.base_url
    }
    fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }
    fn default_params(&self, mut req_builder: RequestBuilder) -> RequestBuilder {
        let basic_auth =
            BASE64_STANDARD.encode(format!("{}:{}", self.config.app_id, self.config.app_secret));
        req_builder = req_builder.header("Authorization", format!("Basic {basic_auth}"));
        req_builder
    }
    // fn with_base_url(&mut self, url: &str) -> &mut Self {
    //     self.base_url = url.to_string();
    //     self
    // }
}
impl YapilyClient {
    pub fn new() -> Self {
        YapilyClient {
            config: YapilyConfig::from_env(),
            http_client: reqwest::Client::new(),
            // user_id: "accounting_app_user".to_string(), // Dummy user ID
            access_token: None, // Initialize access_token as None
            base_url: "https://api.yapily.com".to_string(), // Default base URL for Yapily
        }
    }

    pub async fn login(&mut self) -> anyhow::Result<()> {
        let request_body = serde_json::json!({
            // "applicationUserId": self.user_id,
            "applicationUserId": "accounting_app_user", // Dummy user ID for Yapily
            // "institutionId": "mock-institution", // Replace with a real institution ID if needed
            // "applicationUserId": "string",
            "institutionId": "starling-bank",
            "callback": "https://localhost:5473",
        });

        // let basic_auth =
        //     BASE64_STANDARD.encode(format!("{}:{}", self.config.app_id, self.config.app_secret));

        let response = self
            .http_client
            .post(format!("{}/account-auth-requests", self.base_url()))
            // .header("Authorization", format!("Basic {basic_auth}"))
            .json(&request_body)
            .send()
            .await?
            .error_for_status()?;

        let response_text = response.text().await?;
        println!("Yapily login response: {}", response_text);

        Ok(())
    }

    pub async fn fetch_institutions(&self) -> anyhow::Result<Vec<Institution>> {
        // let basic_auth =
        //     BASE64_STANDARD.encode(format!("{}:{}", self.config.app_id, self.config.app_secret));
        let response = self
            .get("https://api.yapily.com/institutions")
            .fetch_json::<ListInstitutionsResp>()
            .await?;

        // dbg!(&response);
        Ok(response.institutions)
    }
}

#[derive(serde::Deserialize, Debug)]
pub struct ListInstitutionsResp {
    #[serde(rename = "data")]
    pub institutions: Vec<Institution>,
}
#[derive(serde::Deserialize, Debug)]
pub struct Institution {
    pub id: String,
    pub name: String,
    #[serde(rename = "fullName")]
    pub full_name: String,
    pub media: Vec<serde_json::Value>,
    pub countries: Vec<serde_json::Value>,
    pub features: Vec<String>,
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "Test needs manual web login"]
    async fn yapily_login_and_fetch_accounts() -> anyhow::Result<()> {
        let mut client = YapilyClient::new();
        client.login().await?;
        assert!(client.access_token.is_some());

        // let accounts = client.list_accounts().await?;
        // dbg!(&accounts);
        // assert!(!accounts.is_empty());

        Ok(())
    }

    #[tokio::test]
    #[ignore = "Institutions empty in response. WHY ?"]
    async fn yapily_fetch_institutions() -> anyhow::Result<()> {
        let client = YapilyClient::new();
        let institutions = client.fetch_institutions().await?;
        dbg!(&institutions);
        assert!(!institutions.is_empty());
        Ok(())
    }
}
