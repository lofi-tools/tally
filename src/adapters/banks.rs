use crate::utils;
use crate::utils::api_client_utils::{ApiClient, RequestBuilderExt};
use crate::utils::rand_string;
use crate::CONFIG;
use anyhow::anyhow as err;
use chrono::{DateTime, NaiveDate, Utc};
use file_cache::{FileBytes, FromFileOrNew};
use reqwest::RequestBuilder;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::time::Duration;

pub mod map_starling {
    use crate::adapters::banks::nordigen_client::BookedTransaction;
    use crate::models::static_data::{BANK, CLIENTS, DIRECTORS_LOAN, PAYE_PAID, SALES, WAGES_NET};
    use crate::models::tx2::Transaction2;
    use crate::models::{Account, DateAndAmount};

    impl Transaction2 {
        pub fn from_starling(starling_tx: &BookedTransaction) -> anyhow::Result<Transaction2> {
            if starling_tx.is_sale() {
                return Ok(Transaction2::tx_to_bank_from(&CLIENTS, starling_tx));
            }
            if starling_tx.is_wage() {
                // return Ok(Transaction2::wage(starling_tx));
                return Ok(Transaction2::tx_to_bank_from(&WAGES_NET, starling_tx));
            }
            if starling_tx.is_director_borrows() {
                // return Ok(Transaction2::director_borrows_gbp(starling_tx));
                return Ok(Transaction2::tx_to_bank_from(&DIRECTORS_LOAN, starling_tx));
            }
            if starling_tx.is_paye() {
                // return Ok(Transaction2::paye(starling_tx));
                return Ok(Transaction2::tx_to_bank_from(&PAYE_PAID, starling_tx));
            }
            anyhow::bail!("no match for tx: {starling_tx:?}")
        }
        pub fn is_director_borrows(&self) -> bool {
            self.outputs.iter().any(|output| {
                output.account_id == DIRECTORS_LOAN.id && output.amount_diff.is_sign_positive()
            })
        }
    }

    impl From<&BookedTransaction> for DateAndAmount {
        fn from(tx: &BookedTransaction) -> Self {
            DateAndAmount {
                date: tx.value_date,
                amount: tx.amount.amount,
            }
        }
    }
    impl BookedTransaction {
        pub fn is_sale(&self) -> bool {
            if let Some(debtor_name) = &self.debtor_name {
                return debtor_name == "Nigel Frank Intern" || debtor_name.contains("Gravitas");
            }
            return false;
        }
        pub fn is_director_borrows(&self) -> bool {
            self.remittance_information_unstructured
                .contains("Director's loan")
        }
        pub fn is_wage(&self) -> bool {
            self.remittance_information_unstructured.contains("Salary")
        }
        pub fn is_paye(&self) -> bool {
            self.remittance_information_unstructured
                .contains("120PZ028811752312")
        }
    }

    #[cfg(test)]
    pub mod tests {
        use crate::adapters::banks::nordigen_client::NordigenClient;

        #[tokio::test]
        #[ignore = "API is rate limited"]
        async fn test_main() -> anyhow::Result<()> {
            let mut nclient = NordigenClient::new();
            nclient.login().await?;
            assert!(nclient.access_jwt.is_some());

            let requisition = nclient.ensure_starling_requisition().await?;

            let (accounts, _) = nclient.list_accounts(&requisition.id).await?;
            for account_id in accounts.iter() {
                let _transactions = nclient.list_transactions(account_id).await?;
            }

            Ok(())
        }
    }
}

pub mod nordigen_client {
    use file_cache::{CacheInRepo, Cacheable};

    use super::*;
    use crate::models::List;
    use std::{path::PathBuf, sync::LazyLock};

    pub const ID_STARLING: &str = "STARLING_SRLGGB3L";
    pub const MAX_TRANSACTION_TOTAL_DAYS: u32 = 730;
    const STARLING_REQUISITION_CACHE_FILE_NAME: &str = "nordigen_starling_requisition";
    // static STARLING_REQUISISTION_FILE_PATH: LazyLock<PathBuf> =
    //     LazyLock::new(|| PathBuf::from(STARLING_REQUISITION_CACHE_FILE_NAME));

    #[derive(Default, Debug)]
    pub struct NordigenClient {
        pub http_client: reqwest::Client,
        pub access_jwt: Option<String>,
    }
    impl ApiClient for NordigenClient {
        fn base_url(&self) -> &str {
            "https://ob.nordigen.com/api/v2"
        }
        fn http_client(&self) -> &reqwest::Client {
            &self.http_client
        }
        fn default_params(&self, request_builder: RequestBuilder) -> RequestBuilder {
            let mut req_builder = request_builder.timeout(Duration::new(5, 0));
            if let Some(access_jwt) = &self.access_jwt {
                req_builder = req_builder.header("Authorization", format!("Bearer {access_jwt}"));
            }
            req_builder
        }
    }
    impl NordigenClient {
        pub fn new() -> Self {
            Self::default()
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
            let response_json = self
                .post("https://ob.nordigen.com/api/v2/token/new/")
                .json(&json!({
                    "secret_id": CONFIG.nordigen_api_secret_id,
                    "secret_key": CONFIG.nordigen_api_secret_key,
                }))
                .fetch_json::<serde_json::Value>()
                .await?;

            // Parse the response and extract the access token
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
                .post("https://ob.nordigen.com/api/v2/agreements/enduser/")
                .json(&CreateAgreementBody {
                    institution_id: ID_STARLING.into(),
                    max_historical_days: MAX_TRANSACTION_TOTAL_DAYS,
                    access_valid_for_days: 90,
                    access_scope: vec!["balances".into(), "details".into(), "transactions".into()],
                })
                .fetch_json::<Agreement>()
                .await?;

            Ok(agreement)
        }
        pub async fn list_agreements(&self) -> anyhow::Result<Vec<Agreement>> {
            self.expect_login().await?;

            let agreements = self
                .get("/agreements/enduser")
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

            match active_starling_agreements.next() {
                Some(agreement) => Ok(agreement),
                None => Ok(self.create_starling_agreement().await?),
            }
        }

        pub async fn create_requisition(
            &self,
            agreement_id: &str,
        ) -> anyhow::Result<RequisitionFull> {
            self.expect_login().await?;

            let created = self
                .post("https://ob.nordigen.com/api/v2/requisitions/")
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
            got_requisition.to_cache()?;
            // RequisitionFull::overwrite_file(
            //     STARLING_REQUISITION_CACHE_FILE_NAME,
            //     got_requisition.clone(),
            // )
            // .await?;

            println!(
                "Created new requisition, to activate it go to {}",
                created.link
            );

            Ok(got_requisition)
        }
        async fn list_requisitions(&self) -> anyhow::Result<Vec<RequisitionFull>> {
            self.expect_login().await?;

            let requisitions = self
                .get("https://ob.nordigen.com/api/v2/requisitions/")
                .fetch_json::<ListResp<RequisitionFull>>()
                .await?;

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
                .get(&format!("/requisitions/{requisition_id}"))
                .fetch_json::<RequisitionFull>()
                .await?;

            // if !requisition.is_active() {
            //     return Err(anyhow::Error::msg("Requisition is not active")); // TODO display requisition
            // }

            Ok(requisition)
        }

        pub async fn ensure_starling_requisition(&self) -> anyhow::Result<RequisitionFull> {
            self.expect_login().await?;

            let requisition = RequisitionFull::from_file_or_save_new(
                STARLING_REQUISITION_CACHE_FILE_NAME,
                async {
                    let agreement = self.ensure_starling_agreement().await?;
                    let requisition = self.create_requisition(&agreement.id).await?;
                    dbg!(&requisition); // TODO message: go to requisition URL
                    Ok::<_, anyhow::Error>(requisition)
                },
            )
            .await?;

            let got_requisition = match self.get_requisition(&requisition.id).await {
                Ok(requisition) => {
                    requisition.to_cache()?;
                    // RequisitionFull::overwrite_file(
                    //     STARLING_REQUISITION_CACHE_FILE_NAME,
                    //     requisition.clone(),
                    // )
                    // .await?;
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
                ))
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
                ))
                .fetch_json::<ListTransactionsResp>()
                .await?;

            Ok(list_transactions.transactions.booked)
        }

        pub async fn list_starling_transactions(
            &mut self,
        ) -> anyhow::Result<List<BookedTransaction>> {
            impl FileBytes for List<BookedTransaction> {
                fn as_file_bytes(&self) -> anyhow::Result<Vec<u8>> {
                    Ok(serde_json::to_vec_pretty(self)?)
                }
                fn from_file_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
                    Ok(serde_json::from_slice(&bytes)?)
                }
            }

            async fn fetch(
                nclient: &mut NordigenClient,
            ) -> anyhow::Result<List<BookedTransaction>> {
                nclient.ensure_login().await?;

                let requisition = nclient.ensure_starling_requisition().await?;

                let (accounts, _) = nclient.list_accounts(&requisition.id).await?;
                let futures = accounts
                    .iter()
                    .map(|account_id| nclient.list_transactions(&account_id));
                let all_transactions = futures::future::join_all(futures)
                    .await
                    .into_iter()
                    .collect::<Result<Vec<Vec<_>>, _>>()?
                    .into_iter()
                    .flatten()
                    .collect::<Vec<BookedTransaction>>();

                Ok(List::new(all_transactions))
            }

            List::from_file_or_save_new("nordigen_starling_transactions.json", fetch(self)).await
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
            self.created + chrono::Duration::days(i64::from(self.access_valid_for_days))
                > Utc::now()
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
    impl Cacheable for RequisitionFull {
        type CacheType = CacheInRepo;
        fn static_relative_path_str() -> &'static str {
            STARLING_REQUISITION_CACHE_FILE_NAME
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

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct BookedTransaction {
        pub transaction_id: String,
        pub debtor_name: Option<String>,
        pub debtor_account: Option<DebtorAccount>,
        #[serde(rename = "transactionAmount")]
        pub amount: TransactionAmount,
        pub booking_date: NaiveDate,
        pub value_date: NaiveDate,
        #[serde(rename = "remittanceInformationUnstructured")]
        pub remittance_information_unstructured: String,
        pub bank_transaction_code: Option<String>,
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct DebtorAccount {
        pub iban: String,
    }

    #[derive(Serialize, Deserialize, Debug)]
    #[serde(rename_all = "camelCase")]
    pub struct TransactionAmount {
        pub currency: String,
        #[serde(deserialize_with = "utils::deserialize_decimal")]
        pub amount: Decimal,
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
}
