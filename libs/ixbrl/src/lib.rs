use std::fmt;

use snafu::Snafu;

pub mod company;
pub mod ixbrl_fmt;
pub mod reports {
    pub mod uk_frs105_accounts;
    pub mod uk_frs105_corp_tax;
}
#[cfg(test)]
pub mod test_utils;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AccountType {
    Root,
    Asset,
    Bank,
    Cash,
    Credit,
    Equity,
    Expense,
    Income,
    Liability,
    Payable,
    Receivable,
}

impl AccountType {
    fn is_balance_sheet(self) -> bool {
        matches!(
            self,
            Self::Asset
                | Self::Bank
                | Self::Cash
                | Self::Receivable
                | Self::Liability
                | Self::Credit
                | Self::Payable
        )
    }
}

impl TryFrom<&str> for AccountType {
    type Error = String;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        match s {
            "ROOT" => Ok(Self::Root),
            "ASSET" => Ok(Self::Asset),
            "BANK" => Ok(Self::Bank),
            "CASH" => Ok(Self::Cash),
            "CREDIT" => Ok(Self::Credit),
            "EQUITY" => Ok(Self::Equity),
            "EXPENSE" => Ok(Self::Expense),
            "INCOME" => Ok(Self::Income),
            "LIABILITY" => Ok(Self::Liability),
            "PAYABLE" => Ok(Self::Payable),
            "RECEIVABLE" => Ok(Self::Receivable),
            other => Err(other.to_string()),
        }
    }
}

pub struct AccountNode {
    name: String,
    account_type: AccountType,
    balance: rucash::Num,
    children: Vec<AccountNode>,
}

#[derive(Debug, Clone)]
pub struct RawAccount {
    pub guid: String,
    pub name: String,
    pub r#type: String,
    pub parent_guid: String,
}

#[derive(Debug, Clone)]
pub struct RawTransaction {
    pub guid: String,
    pub post_datetime: chrono::NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct RawSplit {
    pub tx_guid: String,
    pub account_guid: String,
    pub value: rucash::Num,
}

pub struct GnucashBook {
    accounts: Vec<AccountNode>,
    net_assets: rucash::Num,
    raw_accounts: Vec<RawAccount>,
    raw_txns: Vec<RawTransaction>,
    raw_splits: Vec<RawSplit>,
}

#[derive(Debug, Snafu)]
pub enum GnucashError {
    #[snafu(display("IO error: {source}"))]
    Io { source: std::io::Error },

    #[snafu(display("GnuCash parse error: {source}"))]
    Rucash { source: rucash::Error },
}

impl From<std::io::Error> for GnucashError {
    fn from(e: std::io::Error) -> Self {
        GnucashError::Io { source: e }
    }
}

impl From<rucash::Error> for GnucashError {
    fn from(e: rucash::Error) -> Self {
        GnucashError::Rucash { source: e }
    }
}

impl AccountNode {
    fn is_zero(&self) -> bool {
        self.balance == rucash::Num::from(0)
    }

    fn has_nonzero_descendant(&self) -> bool {
        !self.children.is_empty()
            && self
                .children
                .iter()
                .any(|c| !c.is_zero() || c.has_nonzero_descendant())
    }
}

fn write_tree(f: &mut fmt::Formatter<'_>, nodes: &[AccountNode], depth: usize) -> fmt::Result {
    let mut sorted: Vec<&AccountNode> = nodes.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));

    for node in &sorted {
        if node.account_type == AccountType::Root {
            write_tree(f, &node.children, depth)?;
            continue;
        }
        if node.is_zero() && !node.has_nonzero_descendant() {
            continue;
        }
        let prefix = "  ".repeat(depth);
        writeln!(f, "{prefix}{}: {}", node.name, node.balance)?;
        write_tree(f, &node.children, depth + 1)?;
    }
    Ok(())
}

impl fmt::Display for GnucashBook {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_tree(f, &self.accounts, 0)?;
        writeln!(f, "---")?;
        write!(f, "Net assets: {}", self.net_assets)
    }
}

fn build_tree_from_raw(
    raw: &[RawAccount],
    balances: &[rucash::Num],
    children_of: &[Vec<usize>],
    idx: usize,
) -> AccountNode {
    let account_type =
        AccountType::try_from(raw[idx].r#type.as_str()).unwrap_or(AccountType::Expense);
    AccountNode {
        name: raw[idx].name.clone(),
        account_type,
        balance: balances[idx],
        children: children_of[idx]
            .iter()
            .map(|&child| build_tree_from_raw(raw, balances, children_of, child))
            .collect(),
    }
}

impl GnucashBook {
    pub async fn try_from_book(
        book: &rucash::Book<rucash::XMLQuery>,
    ) -> Result<Self, rucash::Error> {
        let accounts = book.accounts().await?;
        let txns = book.transactions().await?;
        let splits = book.splits().await?;

        let mut guid_to_idx: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for (i, acc) in accounts.iter().enumerate() {
            guid_to_idx.insert(acc.guid.clone(), i);
        }

        let mut balances = vec![rucash::Num::from(0); accounts.len()];
        for (i, acc) in accounts.iter().enumerate() {
            if acc.commodity_guid.is_empty() {
                continue;
            }
            balances[i] = acc.balance(book).await?;
        }

        let mut children_of: Vec<Vec<usize>> = vec![Vec::new(); accounts.len()];
        let mut roots = Vec::new();
        for (i, acc) in accounts.iter().enumerate() {
            match guid_to_idx.get(&acc.parent_guid) {
                Some(&parent_idx) => children_of[parent_idx].push(i),
                None => roots.push(i),
            }
        }

        let raw_accounts: Vec<RawAccount> = accounts
            .iter()
            .map(|a| RawAccount {
                guid: a.guid.clone(),
                name: a.name.clone(),
                r#type: a.r#type.clone(),
                parent_guid: a.parent_guid.clone(),
            })
            .collect();

        let tree: Vec<AccountNode> = roots
            .iter()
            .map(|&idx| build_tree_from_raw(&raw_accounts, &balances, &children_of, idx))
            .collect();

        let top_level: Vec<usize> = roots
            .iter()
            .flat_map(|&idx| &children_of[idx])
            .copied()
            .collect();

        let net_assets: rucash::Num = top_level
            .iter()
            .map(|&idx| {
                let account_type = AccountType::try_from(raw_accounts[idx].r#type.as_str())
                    .unwrap_or(AccountType::Expense);
                (account_type, &balances[idx])
            })
            .filter(|(t, _)| t.is_balance_sheet())
            .map(|(_, bal)| bal)
            .sum();

        let raw_txns: Vec<RawTransaction> = txns
            .iter()
            .map(|t| RawTransaction {
                guid: t.guid.clone(),
                post_datetime: t.post_datetime,
            })
            .collect();
        let raw_splits: Vec<RawSplit> = splits
            .iter()
            .map(|s| RawSplit {
                tx_guid: s.tx_guid.clone(),
                account_guid: s.account_guid.clone(),
                value: s.value,
            })
            .collect();

        Ok(GnucashBook {
            accounts: tree,
            net_assets,
            raw_accounts,
            raw_txns,
            raw_splits,
        })
    }

    pub async fn try_from_sqlite_book(
        book: &rucash::Book<rucash::SQLiteQuery>,
    ) -> Result<Self, rucash::Error> {
        let accounts = book.accounts().await?;
        let txns = book.transactions().await?;
        let splits = book.splits().await?;

        let mut guid_to_idx: std::collections::HashMap<String, usize> =
            std::collections::HashMap::new();
        for (i, acc) in accounts.iter().enumerate() {
            guid_to_idx.insert(acc.guid.clone(), i);
        }

        let mut balances = vec![rucash::Num::from(0); accounts.len()];
        for (i, acc) in accounts.iter().enumerate() {
            if acc.commodity_guid.is_empty() {
                continue;
            }
            balances[i] = acc.balance(book).await?;
        }

        let mut children_of: Vec<Vec<usize>> = vec![Vec::new(); accounts.len()];
        let mut roots = Vec::new();
        for (i, acc) in accounts.iter().enumerate() {
            match guid_to_idx.get(&acc.parent_guid) {
                Some(&parent_idx) => children_of[parent_idx].push(i),
                None => roots.push(i),
            }
        }

        let raw_accounts: Vec<RawAccount> = accounts
            .iter()
            .map(|a| RawAccount {
                guid: a.guid.clone(),
                name: a.name.clone(),
                r#type: a.r#type.clone(),
                parent_guid: a.parent_guid.clone(),
            })
            .collect();

        let tree: Vec<AccountNode> = roots
            .iter()
            .map(|&idx| build_tree_from_raw(&raw_accounts, &balances, &children_of, idx))
            .collect();

        let top_level: Vec<usize> = roots
            .iter()
            .flat_map(|&idx| &children_of[idx])
            .copied()
            .collect();

        let net_assets: rucash::Num = top_level
            .iter()
            .map(|&idx| {
                let account_type = AccountType::try_from(raw_accounts[idx].r#type.as_str())
                    .unwrap_or(AccountType::Expense);
                (account_type, &balances[idx])
            })
            .filter(|(t, _)| t.is_balance_sheet())
            .map(|(_, bal)| bal)
            .sum();

        let raw_txns: Vec<RawTransaction> = txns
            .iter()
            .map(|t| RawTransaction {
                guid: t.guid.clone(),
                post_datetime: t.post_datetime,
            })
            .collect();
        let raw_splits: Vec<RawSplit> = splits
            .iter()
            .map(|s| RawSplit {
                tx_guid: s.tx_guid.clone(),
                account_guid: s.account_guid.clone(),
                value: s.value,
            })
            .collect();

        Ok(GnucashBook {
            accounts: tree,
            net_assets,
            raw_accounts,
            raw_txns,
            raw_splits,
        })
    }

    pub async fn try_from_gnucash_file(path: &str) -> Result<Self, GnucashError> {
        if path.ends_with(".gnucash") {
            let magic = std::fs::read(path).map(|b| {
                if b.len() >= 4 {
                    [b[0], b[1], b[2], b[3]]
                } else {
                    [0u8; 4]
                }
            })?;

            if magic == [0x53, 0x51, 0x4c, 0x69] {
                let query = rucash::SQLiteQuery::new(path)?;
                let book = rucash::Book::new(query).await?;
                Ok(GnucashBook::try_from_sqlite_book(&book).await?)
            } else {
                let query = rucash::XMLQuery::new(path)?;
                let book = rucash::Book::new(query).await?;
                Ok(GnucashBook::try_from_book(&book).await?)
            }
        } else {
            Err(GnucashError::Io {
                source: std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("unsupported file extension for {path}"),
                ),
            })
        }
    }

    pub fn raw_accounts(&self) -> &[RawAccount] {
        &self.raw_accounts
    }

    pub fn raw_transactions(&self) -> &[RawTransaction] {
        &self.raw_txns
    }

    pub fn raw_splits(&self) -> &[RawSplit] {
        &self.raw_splits
    }
}
