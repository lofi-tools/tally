use super::{Account, AccountTag, AssetId};
use rust_decimal::Decimal;
use std::{collections::HashMap, sync::LazyLock};

pub static EUR: &'static str = "EUR";
pub static GBP: &'static str = "GBP";

pub static ALL_ACCOUNTS: LazyLock<AllAccounts> = LazyLock::new(|| {
    AllAccounts::default()
        .with_account(Account {
            id: AccountId("BANK"),
            code: 1200,
            name: "Company bank account",
            details: "Starling bank",
            account_tag: AccountTag::Cash,
            asset: AssetId(GBP),
            loan_apy: None,
        })
        .with_account(Account {
            // Director's loan is an account receivable (debt owed by others to the company) -> it's an asset account
            id: AccountId("DIRECTORS_LOAN"),
            code: 2301,
            name: "Director's loan account",
            details: "Director 1: Nicolas Marshall",
            account_tag: AccountTag::AccountsReceivable,
            asset: AssetId(GBP),
            loan_apy: Decimal::from_f64_retain(2.0),
        })
        .with_account(Account {
            id: AccountId("SALES"),
            code: 4010,
            name: "Sales - Services",
            details: "",
            account_tag: AccountTag::Sales,
            asset: AssetId(GBP),
            loan_apy: None,
        })
        .with_account(Account {
            id: AccountId("NEXO_GBP"),
            code: 1201,
            name: "Company spot account - Nexo - GBP",
            details: "",
            account_tag: AccountTag::Cash,
            asset: AssetId(GBP),
            loan_apy: None,
        })
        .with_account(Account {
            id: AccountId("NEXO_EUR"),
            code: 1202,
            name: "Company spot account - Nexo - EUR",
            details: "",
            account_tag: AccountTag::Cash,
            asset: AssetId(EUR),
            loan_apy: None,
        })
        .with_account(Account {
            id: AccountId("WAGES_GROSS"),
            code: 7000,
            name: "Gross wages paid",
            details: "",
            account_tag: AccountTag::Expenses,
            asset: AssetId(GBP),
            loan_apy: None,
        })
        .with_account(Account {
            id: AccountId("WAGES_NET"),
            code: 2220,
            name: "Net wages paid",
            details: "",
            account_tag: AccountTag::Expenses,
            asset: AssetId(GBP),
            loan_apy: None,
        })
        .with_account(Account {
            // NOTE credit this account when paying PAYE taxes
            id: AccountId("PAYE_PAID"),
            code: 2210,
            name: "P.A.Y.E. paid",
            details: "",
            account_tag: AccountTag::Expenses,
            asset: AssetId(GBP),
            loan_apy: None,
        })
        .with_account(Account {
            // NOTE employer's NI debit is balanced with a PAYE credit
            id: AccountId("EMPLOYERS_NI_PAID"),
            code: 7006,
            name: "Employer's N.I.",
            details: "",
            account_tag: AccountTag::Expenses,
            asset: AssetId(GBP),
            loan_apy: None,
        })
        .with_account(Account {
            id: AccountId("CORP_TAX_PAID"),
            code: 5500,
            name: "Corporate tax paid",
            details: "",
            account_tag: AccountTag::Expenses,
            asset: AssetId(GBP),
            loan_apy: None,
        })
        .with_account(Account {
            id: AccountId("RETAINED_EARNINGS"),
            code: 3300,
            name: "Retained earnings",
            details: "",
            account_tag: AccountTag::Equity,
            asset: AssetId(GBP),
            loan_apy: None,
        })
});
pub static BANK: LazyLock<&'static Account> = LazyLock::new(|| ALL_ACCOUNTS.bank());
pub static DIRECTORS_LOAN: LazyLock<&'static Account> =
    LazyLock::new(|| ALL_ACCOUNTS.directors_loan());
pub static SALES: LazyLock<&'static Account> = LazyLock::new(|| ALL_ACCOUNTS.sales());
pub static NEXO_GBP: LazyLock<&'static Account> = LazyLock::new(|| ALL_ACCOUNTS.nexo_gbp());
pub static NEXO_EUR: LazyLock<&'static Account> = LazyLock::new(|| ALL_ACCOUNTS.nexo_eur());
pub static WAGES_GROSS: LazyLock<&'static Account> = LazyLock::new(|| ALL_ACCOUNTS.wages_gross());
pub static WAGES_NET: LazyLock<&'static Account> = LazyLock::new(|| ALL_ACCOUNTS.wages_net());
pub static EMPLOYERS_NI_PAID: LazyLock<&'static Account> =
    LazyLock::new(|| ALL_ACCOUNTS.employers_ni_paid());
pub static PAYE_PAID: LazyLock<&'static Account> = LazyLock::new(|| ALL_ACCOUNTS.paye_paid());

// Test-only accounts
pub static BORROW: Account = Account {
    id: AccountId("BORROW"),
    code: 1200,
    name: "",
    details: "",
    account_tag: AccountTag::LoansPayable,
    asset: AssetId(GBP),
    loan_apy: None,
};

#[derive(PartialEq, Eq, Hash, Debug, Clone, Copy)]
pub struct AccountId(&'static str);

#[derive(Default)]
pub struct AllAccounts {
    pub accounts: HashMap<AccountId, Account>,
}
impl AllAccounts {
    pub fn with_account(mut self, account: Account) -> Self {
        self.accounts.insert(AccountId(account.asset.0), account);
        self
    }
    pub fn try_get(&self, id: &AccountId) -> anyhow::Result<&Account> {
        self.accounts
            .get(id)
            .ok_or(anyhow::anyhow!("Account not found"))
    }

    pub fn bank(&self) -> &Account {
        self.try_get(&AccountId("BANK")).unwrap()
    }
    pub fn directors_loan(&self) -> &Account {
        self.try_get(&AccountId("DIRECTORS_LOAN")).unwrap()
    }
    pub fn sales(&self) -> &Account {
        self.try_get(&AccountId("SALES")).unwrap()
    }
    pub fn nexo_gbp(&self) -> &Account {
        self.try_get(&AccountId("NEXO_GBP")).unwrap()
    }
    pub fn nexo_eur(&self) -> &Account {
        self.try_get(&AccountId("NEXO_EUR")).unwrap()
    }
    pub fn wages_gross(&self) -> &Account {
        self.try_get(&AccountId("WAGES_GROSS")).unwrap()
    }
    pub fn wages_net(&self) -> &Account {
        self.try_get(&AccountId("WAGES_NET")).unwrap()
    }
    pub fn paye_paid(&self) -> &Account {
        self.try_get(&AccountId("PAYE_PAID")).unwrap()
    }
    pub fn employers_ni_paid(&self) -> &Account {
        self.try_get(&AccountId("EMPLOYERS_NI_PAID")).unwrap()
    }
    pub fn corp_tax_paid(&self) -> &Account {
        self.try_get(&AccountId("CORP_TAX_PAID")).unwrap()
    }
    pub fn retained_earnings(&self) -> &Account {
        self.try_get(&AccountId("RETAINED_EARNINGS")).unwrap()
    }
}
