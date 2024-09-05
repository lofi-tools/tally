use super::{Account, AccountTag, AllAssets, Asset, AssetId};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::{borrow::Cow, collections::HashMap, sync::LazyLock};

pub struct StaticDb {
    pub accounts: AccountsMap,
    pub assets: AllAssets,
}
#[rustfmt::skip]
pub static DB: LazyLock<StaticDb> = LazyLock::new(|| StaticDb {
    accounts: AccountsMap::default()
        .with_account(Account { id: AccountId(Cow::Borrowed("BANK")), code: 1200, name: "Company bank account",details: "Starling bank",account_tag: AccountTag::Cash,asset_id: AssetId(GBP),loan_apy: None, })
        // Director's loan is an account receivable (debt owed by others to the company) -> it's an asset account
        .with_account(Account { id: AccountId(Cow::Borrowed("DIRECTORS_LOAN")), code: 2301, name: "Director's loan account",details: "Director 1: Nicolas Marshall",account_tag: AccountTag::AccountsReceivable,asset_id: AssetId(GBP),loan_apy: Decimal::from_f64_retain(2.0), })
        .with_account(Account { id: AccountId(Cow::Borrowed("SALES")), code: 4010, name: "Sales - Services",details: "",account_tag: AccountTag::Sales,asset_id: AssetId(GBP),loan_apy: None, })
        // CLIENTS replaces SALES, but amount is negative. total(SALES) is now derived from -total(CLIENTS)
        .with_account(Account { id: AccountId(Cow::Borrowed("CLIENTS")), code: 4040, name: "Sales - Services",details: "",account_tag: AccountTag::Sales,asset_id: AssetId(GBP),loan_apy: None, })
        .with_account(Account { id: AccountId(Cow::Borrowed("NEXO_GBP")), code: 1201, name: "Company spot account - Nexo - GBP",details: "",account_tag: AccountTag::Cash,asset_id: AssetId(GBP),loan_apy: None, })
        .with_account(Account { id: AccountId(Cow::Borrowed("NEXO_EUR")), code: 1202, name: "Company spot account - Nexo - EUR",details: "",account_tag: AccountTag::Cash,asset_id: AssetId(EUR),loan_apy: None, })
        .with_account(Account { id: AccountId(Cow::Borrowed("WAGES_GROSS")), code: 7000, name: "Gross wages paid",details: "",account_tag: AccountTag::ExpensesPaid,asset_id: AssetId(GBP),loan_apy: None, })
        .with_account(Account { id: AccountId(Cow::Borrowed("WAGES_NET")), code: 2220, name: "Net wages paid",details: "",account_tag: AccountTag::ExpensesPaid,asset_id: AssetId(GBP),loan_apy: None, })
        .with_account(Account{ id:AccountId::new("EXPENSES_TO_REPAY"), code:2230, name: "Provision for expenses to repay (within 1 year)", details:"Employee has paid, has not been repaid yet. Repay within 1 year of accounting period end.", account_tag: AccountTag::ExpensesToRepay, asset_id:AssetId(GBP),loan_apy:None })
        // NOTE credit this account when paying PAYE taxes
        .with_account(Account { id: AccountId(Cow::Borrowed("PAYE_PAID")), code: 2210, name: "P.A.Y.E. paid",details: "",account_tag: AccountTag::ExpensesPaid,asset_id: AssetId(GBP),loan_apy: None, })
        // NOTE employer's NI debit is balanced with a PAYE credit
        .with_account(Account { id: AccountId(Cow::Borrowed("EMPLOYERS_NI_PAID")), code: 7006, name: "Employer's N.I.",details: "",account_tag: AccountTag::ExpensesPaid,asset_id: AssetId(GBP),loan_apy: None, })
        .with_account(Account { id: AccountId(Cow::Borrowed("CORP_TAX_PAID")), code: 5500, name: "Corporate tax paid",details: "",account_tag: AccountTag::ExpensesPaid,asset_id: AssetId(GBP),loan_apy: None, })
        .with_account(Account { id: AccountId(Cow::Borrowed("RETAINED_EARNINGS")), code: 3300, name: "Retained earnings",details: "",account_tag: AccountTag::Equity,asset_id: AssetId(GBP),loan_apy: None, }),
    assets: AllAssets::default()
        .with_asset(Asset { id: AssetId(GBP) })
        .with_asset(Asset { id: AssetId(EUR) }),
});

pub static EUR: &'static str = "EUR";
pub static GBP: &'static str = "GBP";

pub static BANK: LazyLock<&'static Account> = LazyLock::new(|| DB.accounts.bank());
pub static DIRECTORS_LOAN: LazyLock<&'static Account> =
    LazyLock::new(|| DB.accounts.directors_loan());
pub static SALES: LazyLock<&'static Account> = LazyLock::new(|| DB.accounts.sales());
pub static CLIENTS: LazyLock<&'static Account> = LazyLock::new(|| DB.accounts.clients());
pub static NEXO_GBP: LazyLock<&'static Account> = LazyLock::new(|| DB.accounts.nexo_gbp());
pub static NEXO_EUR: LazyLock<&'static Account> = LazyLock::new(|| DB.accounts.nexo_eur());
pub static WAGES_GROSS: LazyLock<&'static Account> = LazyLock::new(|| DB.accounts.wages_gross());
pub static WAGES_NET: LazyLock<&'static Account> = LazyLock::new(|| DB.accounts.wages_net());
pub static EXPENSES_TO_REPAY: LazyLock<&'static Account> =
    LazyLock::new(|| DB.accounts.expenses_to_repay());
pub static EMPLOYERS_NI_PAID: LazyLock<&'static Account> =
    LazyLock::new(|| DB.accounts.employers_ni_paid());
pub static PAYE_PAID: LazyLock<&'static Account> = LazyLock::new(|| DB.accounts.paye_paid());

// Test-only accounts
pub static BORROW: Account = Account {
    id: AccountId(Cow::Borrowed("BORROW")),
    code: 1200,
    name: "",
    details: "",
    account_tag: AccountTag::LoansPayable,
    asset_id: AssetId(GBP),
    loan_apy: None,
};

#[derive(Serialize, Deserialize, PartialEq, Eq, Hash, Debug, Clone)]
pub struct AccountId(Cow<'static, str>);
impl AccountId {
    pub fn new(s: impl Into<Cow<'static, str>>) -> Self {
        AccountId(s.into())
    }
    pub fn account(&self) -> anyhow::Result<&'static Account> {
        DB.accounts.try_get(&self)
    }

    // pub fn increase(&self, amount_diff: Decimal) -> TxOutput {
    //     TxOutput {
    //         account_id: self.clone(),
    //         amount_diff,
    //     }
    // }
    // pub fn decrease(&self, amount_diff: Decimal) -> TxOutput {
    //     TxOutput {
    //         account_id: self.clone(),
    //         amount_diff: -amount_diff,
    //     }
    // }
}
impl std::fmt::Display for AccountId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Default, Debug, Clone)]
pub struct AccountsMap {
    pub accounts: HashMap<AccountId, Account>,
}
impl AccountsMap {
    pub fn with_account(mut self, acc: impl Into<Account>) -> Self {
        let acc: Account = acc.into();
        self.accounts.insert(acc.id.clone(), acc);
        self
    }
    pub fn try_get(&self, id: &AccountId) -> anyhow::Result<&Account> {
        self.accounts
            .get(id)
            .ok_or(anyhow::anyhow!("Account not found"))
    }

    pub fn bank(&self) -> &Account {
        self.try_get(&AccountId(Cow::Borrowed("BANK"))).unwrap()
    }
    pub fn directors_loan(&self) -> &Account {
        self.try_get(&AccountId(Cow::Borrowed("DIRECTORS_LOAN")))
            .unwrap()
    }
    pub fn sales(&self) -> &Account {
        self.try_get(&AccountId(Cow::Borrowed("SALES"))).unwrap()
    }
    pub fn clients(&self) -> &Account {
        self.try_get(&AccountId(Cow::Borrowed("CLIENTS"))).unwrap()
    }
    pub fn nexo_gbp(&self) -> &Account {
        self.try_get(&AccountId(Cow::Borrowed("NEXO_GBP"))).unwrap()
    }
    pub fn nexo_eur(&self) -> &Account {
        self.try_get(&AccountId(Cow::Borrowed("NEXO_EUR"))).unwrap()
    }
    pub fn wages_gross(&self) -> &Account {
        self.try_get(&AccountId(Cow::Borrowed("WAGES_GROSS")))
            .unwrap()
    }
    pub fn wages_net(&self) -> &Account {
        self.try_get(&AccountId(Cow::Borrowed("WAGES_NET")))
            .unwrap()
    }
    pub fn expenses_to_repay(&self) -> &Account {
        self.try_get(&AccountId::new("EXPENSES_TO_REPAY")).unwrap()
    }
    pub fn paye_paid(&self) -> &Account {
        self.try_get(&AccountId(Cow::Borrowed("PAYE_PAID")))
            .unwrap()
    }
    pub fn employers_ni_paid(&self) -> &Account {
        self.try_get(&AccountId(Cow::Borrowed("EMPLOYERS_NI_PAID")))
            .unwrap()
    }
    pub fn corp_tax_paid(&self) -> &Account {
        self.try_get(&AccountId(Cow::Borrowed("CORP_TAX_PAID")))
            .unwrap()
    }
    pub fn retained_earnings(&self) -> &Account {
        self.try_get(&AccountId(Cow::Borrowed("RETAINED_EARNINGS")))
            .unwrap()
    }
}
