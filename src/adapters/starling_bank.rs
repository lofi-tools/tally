use crate::ListTxns;
use crate::config::expect_env_var;
use crate::utils::api_client_utils::{ApiClient, RequestBuilderExt};
use anyhow::anyhow as err;
use chrono::{DateTime, Utc};
use file_cache::{Cacheable, FileBytes};
use reqwest::RequestBuilder;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs::File;

// TODO pub mod starling_client

#[derive(Debug)]
pub struct StarlingConfig {
    pub personal_access_token: String,
    // pub app_secret: String,
    // pub redirect_uri: String,
    // pub recv_code_port: u16, // Port to listen for redirect code
}
impl StarlingConfig {
    pub fn from_env() -> anyhow::Result<Self> {
        Ok(StarlingConfig {
            personal_access_token: std::env::var("STARLING_PAT")?,
            // app_secret: expect_env_var("STARLING_APP_SECRET"),
            // redirect_uri: expect_env_var("STARLING_APP_REDIRECT_URI"),
            // recv_code_port: 5473,
        })
    }
}

pub struct StarlingClient {
    pub config: StarlingConfig,
    pub http_client: reqwest::Client,
    pub access_token: Option<String>,
}
impl ApiClient for StarlingClient {
    fn base_url(&self) -> &str {
        "https://api.starlingbank.com/api/v2/"
    }
    fn http_client(&self) -> &reqwest::Client {
        &self.http_client
    }
    fn default_params(&self, mut req_builder: RequestBuilder) -> RequestBuilder {
        req_builder = req_builder.header("Accept", "application/json");
        if let Some(token) = &self.access_token {
            req_builder = req_builder.header("Authorization", format!("Bearer {}", token));
        }
        req_builder
    }
}
impl StarlingClient {
    pub fn new() -> anyhow::Result<Self> {
        Ok(StarlingClient {
            config: StarlingConfig::from_env()?,
            http_client: reqwest::Client::new(),
            access_token: None, // Initialize access_token as None
        })
    }

    pub async fn login(&mut self) -> anyhow::Result<()> {
        // temporary solution: use personal access token
        self.access_token = Some(self.config.personal_access_token.clone());

        // if let Ok(cached_token) = TokenResp::from_cache(TokenResp::static_relative_path()) {
        //     self.access_token = Some(cached_token.access_token);
        //     return Ok(());
        // }

        // let body = json!({
        //     "client_id": self.config.app_id,
        //     "client_secret": self.config.app_secret,
        //     "grant_type": "client_credentials",
        //     "scope": "user:read",
        // });

        // let response = self
        //     .post("/oauth/token")
        //     .json(&body)
        //     .fetch_json::<TokenResp>()
        //     .await?;

        // response.to_cache()?;
        // self.access_token = Some(response.access_token);

        Ok(())
    }
    async fn expect_login(&self) -> anyhow::Result<()> {
        if self.access_token.is_none() {
            return Err(err!("Not logged in"));
        }
        Ok(())
    }
    async fn ensure_login(&mut self) -> anyhow::Result<()> {
        if self.access_token.is_none() {
            self.login().await?;
        }
        Ok(())
    }

    pub async fn list_accounts(&mut self) -> anyhow::Result<Vec<Account>> {
        self.ensure_login().await?;
        let response = self
            .get("/accounts")
            .fetch_json::<ListAccountsResp>()
            .await?;
        Ok(response.accounts)
    }
    pub async fn refetch_primary_account(&mut self) -> anyhow::Result<Account> {
        let accounts = self.list_accounts().await?;
        if let Some(account) = accounts.into_iter().find(|a| a.account_type == "PRIMARY") {
            Ok(account)
        } else {
            Err(err!("No primary account found"))
        }
    }
    pub async fn primary_account(&mut self) -> anyhow::Result<Account> {
        if let Ok(cached_account) = Account::from_cache(Account::static_relative_path()) {
            return Ok(cached_account);
        }
        let account = self.refetch_primary_account().await?;
        account.to_cache()?;
        Ok(account)
    }

    pub async fn refetch_transactions(&mut self) -> anyhow::Result<ListTransactionsResp> {
        self.ensure_login().await?;
        let account = self.primary_account().await?;

        let url = format!(
            "/feed/account/{}/category/{}?changesSince=2022-06-09T16%3A58%3A20.583Z",
            account.id, account.default_category
        );

        let req = self.get(&url);
        let resp = req.fetch_json::<ListTransactionsResp>().await?;
        Ok(resp)
    }
    // pub async fn transactions(&mut self) -> anyhow::Result<ListTxs> {
    //     if let Ok(cached_txs) = ListTxs::from_cache(ListTxs::static_relative_path()) {
    //         return Ok(cached_txs);
    //     }

    //     self.ensure_login().await?;
    //     let account = self.primary_account().await?;

    //     let url = format!(
    //         "/feed/account/{}/category/{}?changesSince=2022-06-09T16%3A58%3A20.583Z",
    //         account.id, account.default_category
    //     );

    //     let req = self.get(&url);
    //     let resp = req.fetch_json::<ListTransactionsResp>().await?;
    //     dbg!(&resp);
    //     // Ok(response.transactions)
    //     todo!()
    // }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListAccountsResp {
    pub accounts: Vec<Account>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Account {
    pub account_type: String,
    #[serde(rename = "accountUid")]
    pub id: String,
    pub created_at: String,
    pub currency: String,
    pub default_category: String,
    pub name: String,
}
impl FileBytes for Account {
    fn as_file_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(self)?)
    }
    fn from_file_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(bytes)?)
    }
}
impl Cacheable for Account {
    fn static_relative_path_str() -> &'static str {
        "starling_account.json"
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListTransactionsResp {
    #[serde(rename = "feedItems")]
    pub transactions: Vec<StTransaction>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StTransaction {
    pub amount: StAmount,
    pub category_uid: String,
    pub counter_party_name: String,
    pub counter_party_sub_entity_identifier: Option<String>,
    pub counter_party_sub_entity_name: Option<String>,
    pub counter_party_sub_entity_sub_identifier: Option<String>,
    pub counter_party_sub_entity_uid: String,
    pub counter_party_type: String,
    pub counter_party_uid: String,
    pub country: String,
    pub direction: String,
    pub feed_item_uid: String,
    pub has_attachment: bool,
    pub has_receipt: bool,
    pub reference: String,
    pub settlement_time: DateTime<Utc>,
    pub source: String,
    pub source_amount: StAmount,
    pub spending_category: String,
    pub status: String,
    pub transacting_application_user_uid: Option<String>,
    pub transaction_time: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StAmount {
    pub currency: String,
    pub minor_units: i64, // Use i64 for minor units to handle large values
}

impl FileBytes for ListTransactionsResp {
    fn as_file_bytes(&self) -> anyhow::Result<Vec<u8>> {
        Ok(serde_json::to_vec_pretty(self)?)
    }
    fn from_file_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
        Ok(serde_json::from_slice(bytes)?)
    }
}
impl Cacheable for ListTransactionsResp {
    fn static_relative_path_str() -> &'static str {
        "starling_transactions.json"
    }
}

pub mod map_starling {
    use std::sync::Arc;

    use super::*;
    use crate::ListTxns;
    use crate::models::static_data::{
        BANK, CLIENTS, DIRECTORS_LOAN, EXPENSES_PAID, EXPENSES_TO_REPAY, GBP, PAYE_PAID,
        TAXES_PAID, WAGES_NET,
    };
    use crate::models::tx2::{Transaction2, TxEffect};
    use crate::models::{
        Account, Asset, Asset2, AssetAmount2, HasAssetAmount, HasDatetime, HasFromTo,
    };

    impl StarlingClient {
        pub async fn transactions(&mut self) -> anyhow::Result<ListTxns> {
            if let Ok(cached_mapped_txns) =
                MappedTxns::from_cache(MappedTxns::static_relative_path())
            {
                return Ok(cached_mapped_txns.transactions);
            }

            let starling_txns = match ListTransactionsResp::from_cache(
                ListTransactionsResp::static_relative_path(),
            ) {
                Ok(cached) => cached.transactions,
                Err(_) => {
                    let resp = self.refetch_transactions().await?;
                    resp.to_cache()?;
                    resp.transactions
                }
            };

            let mapped_txns = MappedTxns {
                transactions: ListTxns {
                    txs: starling_txns
                        .into_iter()
                        .map(|st_txn| Transaction2::from_starling_direct(st_txn))
                        .collect::<Result<Vec<_>, _>>()?,
                },
            };
            mapped_txns.to_cache()?;
            Ok(mapped_txns.transactions)
        }
    }
    impl Transaction2 {
        pub fn from_starling_direct(starling_tx: StTransaction) -> anyhow::Result<Transaction2> {
            Transaction2::from_details(starling_tx)
        }
    }
    impl StTransaction {
        pub fn is_sale(&self) -> bool {
            let is_gravitas = self.counter_party_name.contains("Gravitas Recruit")
                && self.reference.contains("GVT");
            let is_frank = self.reference.contains("FRANK RECRUIT");

            self.direction == "IN"
                && (is_gravitas || is_frank)
                && self.spending_category == "REVENUE"
        }
        pub fn is_wage(&self) -> bool {
            self.reference.to_lowercase().contains("salary")
                && self.counter_party_name == "Me Monzo"
        }
        pub fn is_director_borrows(&self) -> bool {
            self.counter_party_name == "Me Monzo"
                && self.reference.to_lowercase() == "director's loan"
                && self.direction == "OUT"
            // && self.spending_category == "LOAN_PRINCIPAL"
        }
        pub fn is_director_repays(&self) -> bool {
            dbg!(self.reference.to_lowercase());
            self.counter_party_name == "Me Monzo"
                && self.reference.to_lowercase().contains("loan repayment")
                && self.spending_category == "LOAN_PRINCIPAL"
                || self.reference == "For corporate tax"
        }
        pub fn is_paye(&self) -> bool {
            self.counter_party_name.contains("HMRC") && self.reference.contains("120PZ")
        }
        pub fn is_expense_reimbursement(&self) -> bool {
            self.reference.contains("Exp insur") || self.reference.contains("Exp: nrg")
        }
        pub fn is_tax(&self) -> bool {
            let has_oneof_refs =
                self.reference == "4396519254A00101A" || self.reference == "4396519254A00102A";
            self.counter_party_name.contains("HMRC") && has_oneof_refs
        }
    }
    impl HasAssetAmount for StTransaction {
        fn asset_amount_positive(&self) -> Result<AssetAmount2, String> {
            Ok(AssetAmount2 {
                asset: Arc::new(Asset2::Id(GBP.clone())), // TODO handle currencies
                amount: self.amount.minor_units.abs() as u64, // Convert minor units to major units
            })
        }
    }
    impl HasDatetime for StTransaction {
        fn datetime(&self) -> Result<DateTime<Utc>, String> {
            Ok(self.transaction_time)
        }
    }
    impl HasFromTo for StTransaction {
        fn from_to(&self) -> Result<(&Account, &Account), String> {
            if self.is_sale() {
                return Ok((*CLIENTS, *BANK));
            }
            if self.is_wage() {
                return Ok((*BANK, *WAGES_NET));
            }
            if self.is_director_borrows() {
                return Ok((*BANK, *DIRECTORS_LOAN));
            }
            if self.is_director_repays() {
                return Ok((*DIRECTORS_LOAN, *BANK));
            }
            if self.is_paye() {
                return Ok((*BANK, *PAYE_PAID));
            }
            if self.is_expense_reimbursement() {
                return Ok((*BANK, *EXPENSES_PAID));
            }
            if self.is_tax() {
                return Ok((*BANK, *TAXES_PAID));
            }
            Err(format!(
                "No matching FROM/TO account for transaction: {:?}",
                self
            ))
        }
    }

    pub struct MappedTxns {
        pub transactions: ListTxns,
    }
    impl FileBytes for MappedTxns {
        fn as_file_bytes(&self) -> anyhow::Result<Vec<u8>> {
            Ok(serde_json::to_vec_pretty(&self.transactions)?)
        }
        fn from_file_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
            Ok(MappedTxns {
                transactions: serde_json::from_slice(bytes)?,
            })
        }
    }
    impl Cacheable for MappedTxns {
        fn static_relative_path_str() -> &'static str {
            "starling_mapped_transactions.json"
        }
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    // #[tokio::test]
    // // #[ignore = "Test needs manual web login"]
    // async fn test_Starling_login_and_fetch_accounts() -> anyhow::Result<()> {
    //     let mut client = StarlingClient::new();
    //     client.login().await?;
    //     assert!(client.access_token.is_some());

    //     // let accounts = client.list_accounts().await?;
    //     // dbg!(&accounts);
    //     // assert!(!accounts.is_empty());

    //     Ok(())
    // }

    #[tokio::test]
    #[ignore = "Uses external API call"]
    async fn test_starling_fetch_account() -> anyhow::Result<()> {
        let mut client = StarlingClient::new()?;
        let accounts = client.list_accounts().await?;

        let primary_account = client.primary_account().await?;
        dbg!(&primary_account);

        Ok(())
    }

    #[tokio::test]
    #[ignore = "Uses external API call"]
    async fn test_starling_fetch_transactions() -> anyhow::Result<()> {
        let mut client = StarlingClient::new()?;
        let transactions = client.refetch_transactions().await?;
        dbg!(&transactions);

        let cached_transactions = client.transactions().await?;
        dbg!(&cached_transactions);

        Ok(())
    }
    #[tokio::test]
    async fn test_starling_cached_transactions() -> anyhow::Result<()> {
        let mut client = StarlingClient::new()?;
        let transactions = client.transactions().await?;
        dbg!(&transactions);
        Ok(())
    }
}
