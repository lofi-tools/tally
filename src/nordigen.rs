use crate::utils;
use crate::utils::errors_debug::WrapErr;
use crate::utils::rand_string;
use crate::CONFIG;
use anyhow::anyhow as err;
use chrono::{DateTime, NaiveDate, Utc};
use file_cache::{FileBytes, FromFileOrNew};
use reqwest::{Client, RequestBuilder};
use rust_decimal::Decimal;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

pub mod starling {
    pub const ID_STARLING: &str = "STARLING_SRLGGB3L";
    pub const MAX_TRANSACTION_TOTAL_DAYS: u32 = 730;
    // {"id":"STARLING_SRLGGB3L","name":"Starling Bank","bic":"SRLGGB3L","transaction_total_days":"730","countries":["GB"],"logo":"https://cdn.nordigen.com/ais/STARLING_SRLGGB3L.png"}
}
use self::starling::{ID_STARLING, MAX_TRANSACTION_TOTAL_DAYS};

const STARLING_REQUISITION_CACHE_FILE_NAME: &str = "nordigen_starling_requisition";

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
    pub base_url: String,
    pub access_jwt: Option<String>,
}
impl NordigenClient {
    // const API_URL_BASE: &str = "https://ob.nordigen.com/api/v2";
    pub fn new() -> Self {
        NordigenClient {
            http_client: Client::new(),
            base_url: "https://ob.nordigen.com/api/v2".to_string(),
            access_jwt: None,
        }
    }
    fn path(&self, url_path: &str) -> String {
        if url_path.starts_with("http") {
            return url_path.to_string();
        }

        let base = &self.base_url;
        let path = url_path.trim().trim_start_matches('/');
        format!("{base}/{path}")
    }

    fn post(&self, url_path: &str) -> anyhow::Result<RequestBuilder> {
        let req_builder = self
            .http_client
            .post(self.path(url_path))
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
    fn get(&self, url_path: &str) -> anyhow::Result<RequestBuilder> {
        let req_builder = self
            .http_client
            .get(self.path(url_path))
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
            return Err(err!("Not logged in"));
        }
        Ok(())
    }
    async fn ensure_login(&mut self) -> anyhow::Result<()> {
        if self.access_jwt.is_none() {
            self.login().await?;
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
            .ok_or(err!("No access token in response"))?;
        if access_jwt.is_empty() {
            return Err(err!("Empty access token in response"));
        }
        self.access_jwt = Some(access_jwt.to_string());

        Ok(())
    }

    pub async fn create_starling_agreement(&self) -> anyhow::Result<Agreement> {
        self.expect_login().await?;
        let agreement = self
            .post("https://ob.nordigen.com/api/v2/agreements/enduser/")?
            .json(&CreateAgreementBody {
                institution_id: ID_STARLING.into(),
                max_historical_days: MAX_TRANSACTION_TOTAL_DAYS,
                access_valid_for_days: 90,
                access_scope: vec!["balances".into(), "details".into(), "transactions".into()],
            })
            .fetch_json::<Agreement>()
            .await?;
        dbg!(&agreement);

        Ok(agreement)
    }
    pub async fn list_agreements(&self) -> anyhow::Result<Vec<Agreement>> {
        self.expect_login().await?;

        let agreements = self
            .get("/agreements/enduser")?
            .fetch_json::<ListResp<Agreement>>()
            .await?;

        Ok(agreements.results)
    }

    pub async fn ensure_starling_agreement(&self) -> anyhow::Result<Agreement> {
        self.expect_login().await?;

        // TODO list agreements, find current ?
        let agreements = self.list_agreements().await?;
        let mut active_starling_agreements = agreements
            .into_iter()
            .filter(|a| a.institution_id == ID_STARLING)
            .filter(|a| a.is_active())
            .filter(|a| a.max_historical_days == MAX_TRANSACTION_TOTAL_DAYS);

        // TODO cache in file
        // {
        //     Agreement::from_file_or_save_new("nordigen_starling_agreement", self.create_agreement())
        //         .await
        // }

        match active_starling_agreements.next() {
            Some(agreement) => Ok(agreement),
            None => Ok(self.create_starling_agreement().await?),
        }
    }

    pub async fn create_requisition(&self, agreement_id: &str) -> anyhow::Result<RequisitionFull> {
        self.expect_login().await?;

        let created = self
            .post("https://ob.nordigen.com/api/v2/requisitions/")?
            .json(&CreateRequisitionBody {
                redirect: "http://localhost:2345".to_string(),
                institution_id: ID_STARLING.to_string(),
                reference: rand_string(8),
                agreement: agreement_id.to_string(),
                user_language: "EN".to_string(),
            })
            .fetch_json::<CreateRequisitionResp>()
            .await?;
        let got_requisition = self.get_requisition(&created.id).await?;

        // overwrite cache
        RequisitionFull::overwrite_file(
            STARLING_REQUISITION_CACHE_FILE_NAME,
            got_requisition.clone(),
        )
        .await?;

        println!(
            "Created new requisition, to activate it go to {}",
            created.link
        );

        Ok(got_requisition)
    }
    async fn list_requisitions(&self) -> anyhow::Result<Vec<RequisitionFull>> {
        self.expect_login().await?;

        let requisitions = self
            .get("https://ob.nordigen.com/api/v2/requisitions/")?
            .fetch_json::<ListResp<RequisitionFull>>()
            .await?;
        dbg!(&requisitions);

        let active_requisitions = requisitions
            .results
            .into_iter()
            .filter(|r| r.is_active())
            .collect::<Vec<_>>();
        Ok(active_requisitions)
    }
    async fn get_requisition(&self, requisition_id: &str) -> anyhow::Result<RequisitionFull> {
        self.expect_login().await?;

        let requisition = self
            .get(&format!("/requisitions/{requisition_id}"))?
            .fetch_json::<RequisitionFull>()
            .await?;

        // if !requisition.is_active() {
        //     return Err(anyhow::Error::msg("Requisition is not active")); // TODO display requisition
        // }

        Ok(requisition)
    }
    // async fn is_requisition_active(&self, requisition_id: &str) -> anyhow::Result<bool> {
    //     let requisition = self.get_requisition(requisition_id).await?;
    //     Ok(requisition.is_active())
    // }

    async fn ensure_starling_requisition(&self) -> anyhow::Result<RequisitionFull> {
        self.expect_login().await?;

        let requisition =
            RequisitionFull::from_file_or_save_new(STARLING_REQUISITION_CACHE_FILE_NAME, async {
                let agreement = self.ensure_starling_agreement().await?;
                let requisition = self.create_requisition(&agreement.id).await?;
                dbg!(&requisition); // TODO message: go to requisition URL
                Ok::<_, anyhow::Error>(requisition)
            })
            .await?;

        let got_requisition = match self.get_requisition(&requisition.id).await {
            Ok(requisition) => {
                RequisitionFull::overwrite_file(
                    STARLING_REQUISITION_CACHE_FILE_NAME,
                    requisition.clone(),
                )
                .await?;
                requisition
            }
            Err(err) => {
                println!("Error getting requisition: {err}. Will create new Requisition...");
                let agreement = self.ensure_starling_agreement().await?;
                self.create_requisition(&agreement.id).await?
            }
        };

        if got_requisition.is_active() {
            Ok(requisition)
        } else {
            println!(
                "Requisition exists but is not active. To activate it go to {}",
                got_requisition.link
            );
            std::process::exit(0)
        }
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
            .fetch_json::<ListAccountsResp>()
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
            .fetch_json::<ListTransactionsResp>()
            .await?;

        Ok(list_transactions.transactions.booked)
    }

    pub async fn list_starling_transactions(&mut self) -> anyhow::Result<Vec<BookedTransaction>> {
        self.ensure_login().await?;

        let requisition = self.ensure_starling_requisition().await?;

        let (accounts, _) = self.list_accounts(&requisition.id).await?;
        let futures = accounts
            .iter()
            .map(|account_id| self.list_transactions(&account_id));
        let all_transactions = futures::future::join_all(futures)
            .await
            .into_iter()
            .collect::<Result<Vec<Vec<_>>, _>>()?
            .into_iter()
            .flatten()
            .collect::<Vec<BookedTransaction>>();

        Ok(all_transactions)
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
pub struct Agreement {
    pub id: String,
    pub created: DateTime<Utc>,
    pub institution_id: String,
    pub max_historical_days: u32,
    pub access_valid_for_days: u32,
    pub access_scope: Vec<String>,
    pub accepted: Option<String>,
}
impl Agreement {
    pub fn is_active(&self) -> bool {
        self.created + chrono::Duration::days(i64::from(self.access_valid_for_days)) > Utc::now()
    }
}
impl FileBytes for Agreement {
    fn as_file_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(self)?)
    }
    fn from_file_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(&bytes)?)
    }
}

#[derive(Serialize, Debug)]
pub struct CreateRequisitionBody {
    pub redirect: String,
    pub institution_id: String,
    pub reference: String,
    pub agreement: String,
    pub user_language: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CreateRequisitionResp {
    pub id: String,
    pub redirect: String,
    pub status: RequisitionStatus,
    pub agreements: Option<String>,
    pub accounts: Vec<serde_json::Value>,
    pub reference: String,
    pub user_language: String,
    pub link: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum RequisitionStatus {
    #[serde(rename = "CR")]
    Created,
    #[serde(rename = "LN")]
    Linked,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct RequisitionFull {
    pub id: String,
    pub created: String,
    pub redirect: String,
    pub status: RequisitionStatus,
    pub institution_id: String,
    pub agreement: String,
    pub reference: String,
    pub accounts: Vec<String>,
    pub user_language: String,
    pub link: String,
    pub ssn: Option<()>,
    pub account_selection: bool,
    pub redirect_immediate: bool,
}
impl RequisitionFull {
    pub fn is_active(&self) -> bool {
        match self.status {
            RequisitionStatus::Created => false,
            RequisitionStatus::Linked => true,
        }
    }
}
impl FileBytes for RequisitionFull {
    fn as_file_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(self)?)
    }
    fn from_file_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(&bytes)?)
    }
}

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
    pub booking_date: NaiveDate,
    pub value_date: NaiveDate,
    #[serde(rename = "remittanceInformationUnstructured")]
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
    #[serde(deserialize_with = "utils::deserialize_decimal")]
    pub amount: Decimal,
}

#[async_trait::async_trait]
pub trait RequestBuilderExt {
    async fn fetch_json<D: DeserializeOwned>(self) -> anyhow::Result<D>;
}
#[async_trait::async_trait]
impl RequestBuilderExt for RequestBuilder {
    async fn fetch_json<D: DeserializeOwned>(self) -> anyhow::Result<D> {
        let req = self.build()?;
        let method = req.method().clone();
        let resp = reqwest::Client::new().execute(req).await?;
        let status = resp.status();
        let url = resp.url().clone();
        let resp_text = resp.text().await?;

        if !status.is_success() {
            return Err(err!(
            "{method} {url} \n Expected response with status ok, got: status: {status}, resp: {resp_text:?}",
          ));
        }
        let deserialized: D = serde_json::from_str(&resp_text)
            .wrap_err(&format!("Failed deserializing. resp_text: {resp_text}"))?;
        Ok(deserialized)
    }
}

#[derive(Deserialize, Debug)]
pub struct ListResp<T> {
    pub count: u32,
    pub next: Option<String>,
    pub previous: Option<String>,
    pub results: Vec<T>,
}

#[cfg(test)]
pub mod tests {
    use super::*;

    #[tokio::test]
    async fn test_main() -> anyhow::Result<()> {
        let mut nclient = NordigenClient::new();
        nclient.login().await?;
        assert!(nclient.access_jwt.is_some());

        let requisition = nclient.ensure_starling_requisition().await?;

        let (accounts, _) = nclient.list_accounts(&requisition.id).await?;
        for account_id in accounts.iter() {
            let transactions = nclient.list_transactions(account_id).await?;
            dbg!(&transactions);
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_ensure_starling_agreement() -> anyhow::Result<()> {
        let mut nclient = NordigenClient::new();
        nclient.login().await?;
        let agreement = nclient.ensure_starling_agreement().await?;
        assert!(agreement.is_active());

        Ok(())
    }

    #[tokio::test]
    async fn test_ensure_requisition() -> anyhow::Result<()> {
        let mut nclient = NordigenClient::new();
        nclient.login().await?;
        let requisition = nclient.ensure_starling_requisition().await?;
        assert!(requisition.is_active());
        Ok(())
    }
}
