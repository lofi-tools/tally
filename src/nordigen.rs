use crate::utils::errors_debug::WrapErr;
use crate::utils::rand_string;
use crate::CONFIG;
use anyhow::anyhow as err;
use reqwest::{Client, RequestBuilder};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

pub mod starling {
    pub const ID_STARLING: &str = "STARLING_SRLGGB3L";
    // {"id":"STARLING_SRLGGB3L","name":"Starling Bank","bic":"SRLGGB3L","transaction_total_days":"730","countries":["GB"],"logo":"https://cdn.nordigen.com/ais/STARLING_SRLGGB3L.png"}
}
use self::starling::ID_STARLING;

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
    // const API_URL_BASE: &str = "https://ob.nordigen.com/api/v2";

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
        match &self.access_jwt {
            Some(access_jwt) => {
                Ok(req_builder.header("Authorization", format!("Bearer {access_jwt}")))
            }
            None => Ok(req_builder),
        }
    }
    fn get(&self, url: &str) -> anyhow::Result<RequestBuilder> {
        let req_builder = self
            .http_client
            .get(url)
            .header("Accept", "application/json")
            .timeout(Duration::new(5, 0));
        match &self.access_jwt {
            Some(access_jwt) => {
                Ok(req_builder.header("Authorization", format!("Bearer {access_jwt}")))
            }
            None => Ok(req_builder),
        }
    }

    async fn expect_login(&self) -> anyhow::Result<()> {
        if self.access_jwt.is_none() {
            // self.login().await?;
            return Err(err!("not logged in"));
        }
        Ok(())
    }

    pub async fn login(&mut self) -> anyhow::Result<()> {
        let response = self
            .post("https://ob.nordigen.com/api/v2/token/new/")?
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

    pub async fn create_agreement(&self) -> anyhow::Result<CreateAgreementResp> {
        self.expect_login().await?;
        let agreement = self
            .post("https://ob.nordigen.com/api/v2/agreements/enduser/")?
            .json(&CreateAgreementBody {
                institution_id: ID_STARLING.into(),
                max_historical_days: 180,
                access_valid_for_days: 90,
                access_scope: vec!["balances".into(), "details".into(), "transactions".into()],
            })
            .fetch::<CreateAgreementResp>()
            .await?;

        Ok(agreement)
    }
    pub async fn create_requisition(
        &self,
        agreement_id: &str,
    ) -> anyhow::Result<CreateRequisitionResp> {
        self.expect_login().await?;
        let requisition = self
            .post("https://ob.nordigen.com/api/v2/requisitions/")?
            .json(&CreateRequisitionBody {
                redirect: "http://localhost:2345".to_string(),
                institution_id: ID_STARLING.to_string(),
                reference: rand_string(8),
                agreement: agreement_id.to_string(),
                user_language: "EN".to_string(),
            })
            .fetch::<CreateRequisitionResp>()
            .await?;

        Ok(requisition)
    }

    pub async fn list_accounts(
        &self,
        requisition_id: &str,
    ) -> anyhow::Result<(Vec<AccountId>, ListAccountsResp)> {
        self.expect_login().await?;

        let resp_list_accounts = self
            .get(&format!(
                "https://ob.nordigen.com/api/v2/requisitions/{requisition_id}/"
            ))?
            .fetch::<ListAccountsResp>()
            .await?;

        Ok((resp_list_accounts.accounts.clone(), resp_list_accounts))
    }

    pub async fn list_transactions(
        &self,
        account_id: &str,
    ) -> anyhow::Result<Vec<BookedTransaction>> {
        self.expect_login().await?;

        let list_transactions = self
            .get(&format!(
                "https://ob.nordigen.com/api/v2/accounts/{account_id}/transactions/"
            ))?
            .fetch::<ListTransactionsResp>()
            .await?;

        Ok(list_transactions.transactions.booked)
    }
}

#[derive(Serialize, Debug)]
pub struct CreateAgreementBody {
    pub institution_id: String,
    pub max_historical_days: u32,
    pub access_valid_for_days: u32,
    pub access_scope: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateAgreementResp {
    pub id: String,
    // pub created: NaiveDateTime,
    pub institution_id: String,
    // pub max_historical_days: u32,
    // pub access_valid_for_days: u32,
    // pub access_scope: Vec<String>,
    // pub accepted: Option<String>,
}

#[derive(Serialize, Debug)]
pub struct CreateRequisitionBody {
    pub redirect: String,
    pub institution_id: String,
    pub reference: String,
    pub agreement: String,
    pub user_language: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CreateRequisitionResp {
    pub id: String,
    pub redirect: String,
    pub status: String,
    pub agreements: Option<String>,
    pub accounts: Vec<serde_json::Value>,
    pub reference: String,
    pub user_language: String,
    pub link: String,
}
// #[derive(Debug, Serialize, Deserialize)]
// pub struct CreateRequisitionRespStatus {
//     pub short: String,
//     pub long: String,
//     pub description: String,
// }

#[derive(Deserialize, Debug)]
pub struct ListAccountsResp {
    pub id: String,
    pub status: String,
    pub agreements: Option<String>,
    pub accounts: Vec<AccountId>,
    pub reference: String,
}
pub type AccountId = String;

#[derive(Deserialize, Debug)]
pub struct ListTransactionsResp {
    pub transactions: Transactions,
}

#[derive(Deserialize, Debug)]
pub struct Transactions {
    pub booked: Vec<BookedTransaction>,
    // pub pending: Vec<PendingTransaction>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct BookedTransaction {
    pub transaction_id: String,
    pub debtor_name: Option<String>,
    pub debtor_account: Option<DebtorAccount>,
    pub transaction_amount: TransactionAmount,
    pub booking_date: String,
    pub value_date: String,
    pub remittance_information_unstructured: String,
    pub bank_transaction_code: Option<String>,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct DebtorAccount {
    pub iban: String,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct TransactionAmount {
    pub currency: String,
    pub amount: String,
}

// #[derive(Serialize, Deserialize, Debug)]
// pub struct PendingTransaction {
//     pub transaction_amount: TransactionAmount,
//     pub value_date: String,
//     pub remittance_information_unstructured: String,
// }

#[async_trait::async_trait]
pub trait RequestBuilderExt {
    async fn fetch<D: DeserializeOwned>(self) -> anyhow::Result<D>;
}
#[async_trait::async_trait]
impl RequestBuilderExt for RequestBuilder {
    async fn fetch<D: DeserializeOwned>(self) -> anyhow::Result<D> {
        let req = self.build()?;
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
        let deserialized: D = serde_json::from_str(&resp_text)
            .wrap_err(&format!("failed deserializing. resp_text: {resp_text}"))?;
        Ok(deserialized)
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[tokio::test]
    async fn test_main() -> anyhow::Result<()> {
        let mut nclient = NordigenClient::new();
        nclient.login().await?;
        assert!(nclient.access_jwt.is_some());

        if CONFIG.nordigen_starling_requisition_id.is_empty() {
            let agreement = nclient.create_agreement().await?;
            let requisition = nclient.create_requisition(&agreement.id).await?;
            dbg!(&requisition);
            return Ok(());
        }

        let (accounts, _) = nclient
            .list_accounts(&CONFIG.nordigen_starling_requisition_id)
            .await?;
        for account_id in accounts.iter() {
            let transactions = nclient.list_transactions(account_id).await?;
            dbg!(&transactions);
        }

        Ok(())
    }
}
