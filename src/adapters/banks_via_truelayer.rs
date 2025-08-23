use crate::config::expect_env_var;
use crate::models::List;
use crate::utils;
use crate::utils::api_client_utils::{ApiClient, RequestBuilderExt};
use anyhow::{anyhow as err, bail};
use chrono::{DateTime, Utc};
use file_cache::Cacheable;
use file_cache::FileBytes;
use http_body_util::Full;
use hyper::body::Bytes;
use hyper::server::conn::http1::{self};
use hyper::service::service_fn;
use reqwest::RequestBuilder;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::TcpListener;

// pub const ID_TRUELAYER: &str = "TRUELAYER_TLGB"; // Placeholder ID, adjust if TrueLayer has a specific institution ID
const TRUELAYER_ACCESS_TOKEN_CACHE_FILE_NAME: &str = "truelayer_access_token";

#[derive(Debug)]
pub struct TrueLayerClient {
    pub config: TruelayerConfig,
    pub http_client: reqwest::Client,
    pub access_token: Option<String>,
}

#[derive(Debug)]
pub struct TruelayerConfig {
    pub client_id: String,
    pub client_secret: String,
    pub redirect_uri: String,
    pub recv_code_port: u16, // Port to listen for redirect code
}
impl TruelayerConfig {
    pub fn from_env() -> Self {
        TruelayerConfig {
            client_id: expect_env_var("TRUELAYER_CLIENT_ID"),
            client_secret: expect_env_var("TRUELAYER_CLIENT_SECRET"),
            redirect_uri: expect_env_var("TRUELAYER_REDIRECT_URI"),
            recv_code_port: 5473,
        }
    }
}

impl ApiClient for TrueLayerClient {
    fn base_url(&self) -> &str {
        "https://api.truelayer.com/data/v1"
    }
    fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }
    fn default_params(&self, request_builder: RequestBuilder) -> RequestBuilder {
        let mut req_builder = request_builder.timeout(Duration::new(5, 0));
        if let Some(token) = &self.access_token {
            req_builder = req_builder.header("Authorization", format!("Bearer {token}"));
        }
        req_builder
    }
}

impl TrueLayerClient {
    pub fn new() -> Self {
        TrueLayerClient {
            config: TruelayerConfig::from_env(),
            http_client: reqwest::Client::new(),
            access_token: None,
        }
    }

    async fn expect_login(&self) -> anyhow::Result<()> {
        if self.access_token.is_none() {
            return Err(err!("Not logged in to TrueLayer"));
        }
        Ok(())
    }
    async fn ensure_login(&mut self) -> anyhow::Result<()> {
        if self.access_token.is_none() {
            self.login().await?;
        }
        Ok(())
    }

    pub async fn login(&mut self) -> anyhow::Result<()> {
        // Check if we already have a valid access token in cache
        // if let Ok(cached_token) = TrueLayerAccessToken::from_cache() {
        //     self.access_token = Some(cached_token.token);
        //     return Ok(());
        // }

        // If not, initiate OAuth2 Authorization Code Flow
        const SCOPES: &str = "info accounts cards transactions offline_access"; // Request offline_access for refresh tokens
        let auth_url = format!(
            "https://auth.truelayer.com/?response_type=code&client_id={}&redirect_uri={}&scope={}&nonce={}&provider_id=ob-starling",
            self.config.client_id,
            self.config.redirect_uri,
            urlencoding::encode(SCOPES),
            uuid::Uuid::new_v4().to_string() // Nonce for security
        );

        println!(
            "Please open the following URL in your browser to authorize TrueLayer:\n{}",
            auth_url
        );

        let code = self.wait_for_redirect_code().await?;
        self.exchange_code_for_token(&code).await?;

        Ok(())
    }

    async fn wait_for_redirect_code(&self) -> anyhow::Result<String> {
        use hyper::body::Incoming;
        use hyper::{Request, Response};
        use tokio::sync::mpsc::Sender;

        async fn handle_redirect_req(
            req: hyper::Request<impl hyper::body::Body>,
            snd_code: Arc<Sender<Result<String, String>>>,
        ) -> Result<Response<Full<Bytes>>, String> {
            let query = req
                .uri()
                .query()
                .ok_or("No query string in request".to_string())?;
            let code = query
                .split('&')
                .find(|pair| pair.starts_with(&format!("{}=", "code")))
                .and_then(|pair| pair.splitn(2, '=').nth(1))
                .map(|value| value.to_string())
                .ok_or("Failed extracting code from query".to_string())?;
            snd_code
                .send(Ok(code.clone()))
                .await
                .map_err(|_| "Failed sending code".to_string())?;

            Ok(hyper::Response::new(Full::new(Bytes::from(
                "Accounting is now authenticated. Close this page and return to the terminal where you launched it.",
            ))))
        }

        let addr: SocketAddr = ([0, 0, 0, 0], self.config.recv_code_port).into();
        let (snd_code, mut recv_code) = tokio::sync::mpsc::channel::<Result<String, String>>(1);
        let listener = TcpListener::bind(addr).await?;
        println!("Listening on http://{}", addr);
        let (tcp_stream, _) = listener.accept().await?;
        let io = hyper_util::rt::TokioIo::new(tcp_stream); // Wrapper to implement Hyper IO traits for Tokio types.

        let snd_code = Arc::new(snd_code);
        if let Err(err) = http1::Builder::new()
            .serve_connection(
                io,
                service_fn(move |req: Request<Incoming>| {
                    let snd_code = Arc::clone(&snd_code);
                    async move {
                        handle_redirect_req(req, snd_code)
                            .await
                            .map_err(|e| e.to_string())
                    }
                }),
            )
            .await
        {
            println!("Failed to serve connection: {:?}", err);
        }

        let code = match recv_code.recv().await {
            Some(Ok(code)) => code,
            Some(Err(err)) => bail!("Error receiving code: {}", err),
            None => bail!("No code received"),
        };

        Ok(code)
    }

    pub async fn exchange_code_for_token(&mut self, code: &str) -> anyhow::Result<()> {
        let body = json!({
            "grant_type": "authorization_code",
            "client_id": self.config.client_id,
            "client_secret": self.config.client_secret,
            "redirect_uri": self.config.redirect_uri,
            "code": code.to_string(),
        });
        dbg!(&body);

        let token_data = self
            .post("https://auth.truelayer.com/connect/token")
            .header("Content-Type", "application/json")
            .json(&body)
            .fetch_json::<TrueLayerAccessToken>()
            .await?;

        // let access_token = response_json["access_token"]
        //     .as_str()
        //     .ok_or(err!("No access_token in TrueLayer response"))?;
        // let refresh_token = response_json["refresh_token"]
        //     .as_str()
        //     .map(|s| s.to_string());
        // let expires_in = response_json["expires_in"].as_i64();

        // self.access_token = Some(access_token.to_string());

        // let mut token_data =
        //     TrueLayerAccessToken::new(access_token.to_string(), refresh_token, expires_in);
        token_data.to_cache()?;

        Ok(())
    }

    // pub async fn refresh_access_token(&mut self) -> anyhow::Result<()> {
    //     let cached_token = TrueLayerAccessToken::from_cache()?;
    //     let refresh_token = cached_token
    //         .refresh_token
    //         .ok_or(err!("No refresh token available"))?;

    //     let response_json = self
    //         .post("https://auth.truelayer.com/connect/token")
    //         .form(&json!({
    //             "grant_type": "refresh_token",
    //             "client_id": CONFIG.truelayer_client_id,
    //             "client_secret": CONFIG.truelayer_client_secret,
    //             "refresh_token": refresh_token,
    //         }))
    //         .fetch_json::<serde_json::Value>()
    //         .await?;

    //     let access_token = response_json["access_token"]
    //         .as_str()
    //         .ok_or(err!("No access_token in TrueLayer refresh response"))?;
    //     let new_refresh_token = response_json["refresh_token"]
    //         .as_str()
    //         .map(|s| s.to_string());
    //     let expires_in = response_json["expires_in"].as_i64();

    //     self.access_token = Some(access_token.to_string());

    //     let mut token_data =
    //         TrueLayerAccessToken::new(access_token.to_string(), new_refresh_token, expires_in);
    //     token_data.to_cache()?;

    //     Ok(())
    // }

    pub async fn list_accounts(&self) -> anyhow::Result<Vec<TrueLayerAccount>> {
        self.expect_login().await?;

        let accounts_resp = self
            .get(&format!("{}/data/v1/accounts", self.base_url())) // Placeholder path
            .fetch_json::<ListResp<TrueLayerAccount>>()
            .await?;

        Ok(accounts_resp.data)
    }

    pub async fn list_transactions(
        &self,
        account_id: &str,
    ) -> anyhow::Result<Vec<TrueLayerTransaction>> {
        self.expect_login().await?;

        let transactions_resp = self
            .get(&format!(
                "{}/data/v1/accounts/{}/transactions", // Placeholder path
                self.base_url(),
                account_id
            ))
            .fetch_json::<ListResp<TrueLayerTransaction>>()
            .await?;

        Ok(transactions_resp.data)
    }

    pub async fn list_all_transactions(&mut self) -> anyhow::Result<List<TrueLayerTransaction>> {
        // impl FileBytes for List<TrueLayerTransaction> {
        //     fn as_file_bytes(&self) -> anyhow::Result<Vec<u8>> {
        //         Ok(serde_json::to_vec_pretty(self)?)
        //     }
        //     fn from_file_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        //         Ok(serde_json::from_slice(&bytes)?)
        //     }
        // }

        async fn fetch(
            tlclient: &mut TrueLayerClient,
        ) -> anyhow::Result<List<TrueLayerTransaction>> {
            tlclient.ensure_login().await?;

            let accounts = tlclient.list_accounts().await?;
            let futures = accounts
                .iter()
                .map(|account| tlclient.list_transactions(&account.account_id));
            let all_transactions = futures::future::join_all(futures)
                .await
                .into_iter()
                .collect::<Result<Vec<Vec<_>>, _>>()?
                .into_iter()
                .flatten()
                .collect::<Vec<TrueLayerTransaction>>();

            Ok(List::new(all_transactions))
        }

        todo!()
        // List::from_file_or_save_new("truelayer_transactions.json", fetch(self)).await
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct TrueLayerAccessToken {
    pub token: String,
    pub refresh_token: Option<String>,
    pub expires_in: Option<i64>,
    // Add field for when the token was issued to calculate expiry
    pub issued_at: DateTime<Utc>,
}

impl TrueLayerAccessToken {
    pub fn new(token: String, refresh_token: Option<String>, expires_in: Option<i64>) -> Self {
        Self {
            token,
            refresh_token,
            expires_in,
            issued_at: Utc::now(),
        }
    }

    pub fn is_expired(&self) -> bool {
        if let Some(expires_in) = self.expires_in {
            // Add a small buffer for network latency and clock skew
            self.issued_at + chrono::Duration::seconds(expires_in - 300) < Utc::now()
        } else {
            // Assume expired if no expiry information
            true
        }
    }
}

impl FileBytes for TrueLayerAccessToken {
    fn as_file_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(self)?)
    }
    fn from_file_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(&bytes)?)
    }
}

impl Cacheable for TrueLayerAccessToken {
    // type CacheType = CacheInRepo;
    fn static_relative_path_str() -> &'static str {
        TRUELAYER_ACCESS_TOKEN_CACHE_FILE_NAME
    }
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TrueLayerAccount {
    pub account_id: String,
    pub account_type: String,
    pub display_name: String,
    pub currency: String,
    // Add more fields as per TrueLayer's Account API response
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TrueLayerTransaction {
    pub transaction_id: String,
    pub timestamp: DateTime<Utc>,
    #[serde(deserialize_with = "utils::deserialize_decimal")]
    pub amount: Decimal,
    pub currency: String,
    pub description: String,
    // Add more fields as per TrueLayer's Transaction API response
}

#[derive(Deserialize, Debug)]
pub struct ListResp<T> {
    pub data: Vec<T>,
}

#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    // #[error(transparent)]
    // Ssooidc(Box<aws_sdk_ssooidc::Error>),
    // #[error(transparent)]
    // SdkRegisterClient(Box<SdkError<RegisterClientError>>),
    // #[error(transparent)]
    // SdkCreateToken(Box<SdkError<CreateTokenError>>),
    // #[error(transparent)]
    // SdkStartDeviceAuthorization(Box<SdkError<StartDeviceAuthorizationError>>),
    // #[error(transparent)]
    // Io(#[from] std::io::Error),
    // // #[error(transparent)]
    // // TimeComponentRange(#[from] time::error::ComponentRange),
    // // #[error(transparent)]
    // // Directories(#[from] crate::util::directories::DirectoryError),
    // #[error(transparent)]
    // SerdeJson(#[from] serde_json::Error),
    // // #[error(transparent)]
    // // DbOpenError(#[from] crate::database::DbOpenError),
    // #[error("No token")]
    // NoToken,
    // #[error("OAuth state mismatch. Actual: {} | Expected: {}", .actual, .expected)]
    // OAuthStateMismatch { actual: String, expected: String },
    // #[error("Timeout waiting for authentication to complete")]
    // OAuthTimeout,
    // #[error("No code received on redirect")]
    // OAuthMissingCode,
    // #[error("OAuth error: {0}")]
    // OAuthCustomError(String),
    // #[error(transparent)]
    // DatabaseError(#[from] crate::database::DatabaseError),
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[tokio::test]
    // #[ignore = "Requires TrueLayer API credentials and setup"]
    async fn test_truelayer_login_and_fetch_accounts() -> anyhow::Result<()> {
        // Ensure CONFIG.truelayer_client_id and CONFIG.truelayer_client_secret are set in .env
        let mut tlclient = TrueLayerClient::new();
        tlclient.login().await?;
        assert!(tlclient.access_token.is_some());

        let accounts = tlclient.list_accounts().await?;
        dbg!(&accounts);
        assert!(!accounts.is_empty());

        Ok(())
    }

    #[tokio::test]
    #[ignore = "Requires TrueLayer API credentials and setup, and an account_id"]
    async fn test_truelayer_fetch_transactions() -> anyhow::Result<()> {
        let mut tlclient = TrueLayerClient::new();
        tlclient.login().await?;

        // Replace with a valid account_id from your TrueLayer accounts
        let accounts = tlclient.list_accounts().await?;
        let Some(account_id) = accounts.first().map(|a| a.account_id.clone()) else {
            anyhow::bail!("No accounts found to fetch transactions for.");
        };

        let transactions = tlclient.list_transactions(&account_id).await?;
        dbg!(&transactions);
        assert!(!transactions.is_empty());

        Ok(())
    }

    // #[tokio::test]
    // #[ignore = "Requires TrueLayer API credentials and setup"]
    // async fn test_truelayer_list_all_transactions() -> anyhow::Result<()> {
    //     let mut tlclient = TrueLayerClient::new();
    //     let all_transactions = tlclient.list_all_transactions().await?;
    //     dbg!(&all_transactions);
    //     assert!(!all_transactions.is_empty());
    //     Ok(())
    // }
}
