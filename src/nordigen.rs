use crate::CONFIG;
use anyhow::anyhow as err;
use reqwest::{Client, RequestBuilder};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::env;
use std::error::Error;
use std::time::Duration;

#[derive(Debug, Serialize, Deserialize)]
struct Account {
    id: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Transaction {
    id: String,
    amount: f32,
    currency: String,
    date: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct TransactionsResponse {
    transactions: Vec<Transaction>,
}

#[derive(Debug)]
pub struct NordigenClient {
    pub http_client: Client,
    pub access_jwt: Option<String>,
}
impl NordigenClient {
    const API_URL_BASE: &str = "https://ob.nordigen.com/api/v2";

    pub fn new() -> Self {
        NordigenClient {
            http_client: Client::new(),
            access_jwt: None,
        }
    }
    fn post(&self, url: &str) -> anyhow::Result<RequestBuilder> {
        let req_builder = self
            .http_client
            .post(url)
            .header("Content-Type", "application/json")
            .header("Accept", "application/json")
            .timeout(Duration::new(5, 0));

        match self.access_jwt {
            Some(access_jwt) => {
                Ok(req_builder.header("Authorization", format!("Bearer {access_jwt}")))
            }
            None => Ok(req_builder),
        }
    }
    // fn post_auth(&self, url: &str) -> anyhow::Result<RequestBuilder> {
    //     let access_jwt = self.access_jwt.ok_or(err!("Not logged in"))?;
    //     Ok(self
    //         .post(url)
    //         .header("Authorization", format!("Bearer {access_jwt}")))
    // }
    // async fn fetch()

    async fn require_login(&mut self) -> anyhow::Result<()> {
        if self.access_jwt.is_none() {
            self.login().await?;
        }
        Ok(())
    }

    pub async fn login(&mut self) -> anyhow::Result<()> {
        let response = self
            .post("https://ob.nordigen.com/api/v2/token/new/")
            .json(&json!({
                "secret_id": CONFIG.nordigen_api_secret_id,
                "secret_key": CONFIG.nordigen_api_secret_key,
            }))
            .send()
            .await?;

        // Parse the response and extract the access token
        let response_json: serde_json::Value = response.json().await?;
        let access_jwt = response_json["access"]
            .as_str()
            .ok_or(err!("no access token in response"))?;
        if access_jwt.is_empty() {
            return Err(err!("no access token in response"));
        }
        self.access_jwt = Some(access_jwt.to_string());

        Ok(())
    }
    // TODO create user agreement for more than 90 days transaction history
    pub async fn create_agreement(&mut self) -> anyhow::Result<String> {
        self.require_login().await?;
        let response = self
            .post_auth("https://ob.nordigen.com/api/v2/agreements/enduser/")?
            .json(&CreateAgreementBody {
                institution_id: "STARLING_SRLGGB3L".into(),
                max_historical_days: 180,
                access_valid_for_days: 90,
                access_scope: vec!["balances".into(), "details".into(), "transaction".into()],
            })
            .send()
            .await?;

        todo!()
    }
}

#[derive(Serialize, Debug)]
pub struct CreateAgreementBody {
    pub institution_id: String,
    pub max_historical_days: u32,
    pub access_valid_for_days: u32,
    pub access_scope: Vec<String>,
}

pub mod starling {
    // {"id":"STARLING_SRLGGB3L","name":"Starling Bank","bic":"SRLGGB3L","transaction_total_days":"730","countries":["GB"],"logo":"https://cdn.nordigen.com/ais/STARLING_SRLGGB3L.png"}
}

// pub trait WithAuth {
//     fn with_jwt_auth(self) -> Self;
// }

//     fn with_jwt_auth(self) -> Self {
//         self.header(format!("Authorization: Bearer {}"))
//     }
// }
#[async_trait::async_trait]
pub trait RequestBuilderExt {
    async fn fetch<D: DeserializeOwned>(self) -> D;
}
#[async_trait::async_trait]
impl RequestBuilderExt for RequestBuilder {
    async fn fetch<D: DeserializeOwned>(self) -> anyhow::Result<D> {
        let req = req.build()?;
        let method = req.method().clone();
        let resp = reqwest::Client::new().execute(req).await?;
        let status = resp.status();
        let url = resp.url().clone();
        let resp_text = resp.text().await?;

        if !status.is_success() {
            return Err(err!(
            "{method} {url} \n expected response with status ok, got: status: {status}, resp: {resp_text:?}",
          ));
        }
    }
}

async fn fetch_bank_transactions() -> Result<(), Box<dyn Error>> {
    // Read the API key from the environment variable
    let api_key = env::var("NORDIGEN_API_KEY")?;

    // Set up the request
    let account_id = "123456"; // Replace with your account ID
    let url = format!(
        "https://ob.nordigen.com/api/accounts/{}/transactions",
        account_id
    );
    let client = reqwest::Client::new();
    let response = client
        .get(&url)
        .header("Authorization", &format!("Bearer {}", api_key))
        .send()
        .await?;

    // Parse the response
    let response_text = response.text().await?;
    let transactions_response: TransactionsResponse = serde_json::from_str(&response_text)?;

    // Print the transactions
    for transaction in transactions_response.transactions {
        println!("{:?}", transaction);
    }

    Ok(())
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[tokio::test]
    async fn test_login_jwt_ok() -> anyhow::Result<()> {
        let mut nclient = NordigenClient::new();
        nclient.login().await?;
        assert!(nclient.access_jwt.is_some());

        // let jwt = obtain_jwt().await?;
        Ok(())
    }
}
