use std::fmt;

use snafu::Snafu;

// The company/accounting-period model and the iXBRL intermediate
// representation moved to the shared leaf crates (`core_model`,
// `ixbrl-ir`); re-export them at their historical paths so callers keep
// working unchanged.
pub use core_model::company;
pub use core_model::{
    AccountingPeriod, AccountsMeta, Company, CompanyProfile, EmployeePeriod, Employees,
};
pub use ixbrl_ir::ixbrl_fmt;

pub mod calc_corp_tax;
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

#[derive(Debug, Clone)]
pub struct AccountNode {
    name: String,
    account_type: AccountType,
    balance: rucash::Num,
    children: Vec<AccountNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawAccount {
    pub guid: String,
    pub name: String,
    pub r#type: String,
    pub parent_guid: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawTransaction {
    pub guid: String,
    pub post_datetime: chrono::NaiveDateTime,
    /// GnuCash transaction description/memo ("" when the book has none).
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawSplit {
    pub tx_guid: String,
    pub account_guid: String,
    pub value: rucash::Num,
}

/// One previous-period start-balance adjustment: a transaction whose splits
/// reference accounts (by full path, resolved against the previous period's
/// chart of accounts) — e.g. correcting a prior-period error the
/// comparative column must reflect, such as restoring a liability the filed
/// balance sheet omitted.  Applied to a report with
/// [`reports::uk_frs105_accounts::Frs105Accounts::with_previous_year_adjustments`].
#[derive(Debug, Clone)]
pub struct AdjustmentTransaction {
    /// The posting date, within the previous period.
    pub post_datetime: chrono::NaiveDateTime,
    /// The transaction's description/memo.
    pub description: String,
    /// The transaction's splits — they must balance (each account's sign
    /// follows the GnuCash convention the reports read: assets and
    /// liabilities stored as-is, equity / income / expense negated).
    pub splits: Vec<AdjustmentSplit>,
}

/// One leg of an [`AdjustmentTransaction`]: a value posted against an
/// account, referenced by its full `":"`-separated path (e.g.
/// `"Liabilities:Owed Corporation Tax"`) and resolved against the previous
/// period's chart of accounts.  The referenced account must exist in the
/// previous book and fall on a balance-sheet line's account paths (the
/// `LINE_ACCOUNTS` of the FRS 105 accounts module — a split on an account
/// outside them is silently ignored by the line computations).
#[derive(Debug, Clone)]
pub struct AdjustmentSplit {
    /// The full account path, e.g. `"Liabilities:Owed Corporation Tax"`.
    pub account: String,
    /// The posted value (the same sign convention as [`RawSplit::value`]).
    pub value: rucash::Num,
}

#[derive(Debug, Clone)]
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

/// Assemble a [`GnucashBook`] from the pre-parsed parts and the per-account
/// balances: the account tree, net assets and the struct itself.  Shared by
/// the file/SQLite parsers and [`GnucashBook::from_raw_parts`].
fn assemble(
    raw_accounts: Vec<RawAccount>,
    balances: Vec<rucash::Num>,
    raw_txns: Vec<RawTransaction>,
    raw_splits: Vec<RawSplit>,
) -> GnucashBook {
    let guid_to_idx: std::collections::HashMap<String, usize> = raw_accounts
        .iter()
        .enumerate()
        .map(|(i, a)| (a.guid.clone(), i))
        .collect();

    let mut children_of: Vec<Vec<usize>> = vec![Vec::new(); raw_accounts.len()];
    let mut roots = Vec::new();
    for (i, acc) in raw_accounts.iter().enumerate() {
        match guid_to_idx.get(&acc.parent_guid) {
            Some(&parent_idx) => children_of[parent_idx].push(i),
            None => roots.push(i),
        }
    }

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

    GnucashBook {
        accounts: tree,
        net_assets,
        raw_accounts,
        raw_txns,
        raw_splits,
    }
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

        let mut balances = vec![rucash::Num::from(0); accounts.len()];
        for (i, acc) in accounts.iter().enumerate() {
            if acc.commodity_guid.is_empty() {
                continue;
            }
            balances[i] = acc.balance(book).await?;
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
        let raw_txns: Vec<RawTransaction> = txns
            .iter()
            .map(|t| RawTransaction {
                guid: t.guid.clone(),
                post_datetime: t.post_datetime,
                description: t.description.clone(),
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

        Ok(assemble(raw_accounts, balances, raw_txns, raw_splits))
    }

    pub async fn try_from_sqlite_book(
        book: &rucash::Book<rucash::SQLiteQuery>,
    ) -> Result<Self, rucash::Error> {
        let accounts = book.accounts().await?;
        let txns = book.transactions().await?;
        let splits = book.splits().await?;

        let mut balances = vec![rucash::Num::from(0); accounts.len()];
        for (i, acc) in accounts.iter().enumerate() {
            if acc.commodity_guid.is_empty() {
                continue;
            }
            balances[i] = acc.balance(book).await?;
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
        let raw_txns: Vec<RawTransaction> = txns
            .iter()
            .map(|t| RawTransaction {
                guid: t.guid.clone(),
                post_datetime: t.post_datetime,
                description: t.description.clone(),
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

        Ok(assemble(raw_accounts, balances, raw_txns, raw_splits))
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

    /// Build a [`GnucashBook`] from pre-parsed parts — the raw accounts,
    /// transactions and splits — skipping the GnuCash file/SQLite parsing.
    ///
    /// The account tree and net assets are derived from the parts (each
    /// account's balance is the sum of its splits), so the book is
    /// equivalent to one parsed from a file.  Useful for tests that need a
    /// ledger for dates the example books don't cover, and for feeding a
    /// book from another ledger source.
    pub fn from_raw_parts(
        raw_accounts: Vec<RawAccount>,
        raw_txns: Vec<RawTransaction>,
        raw_splits: Vec<RawSplit>,
    ) -> Self {
        let guid_to_idx: std::collections::HashMap<String, usize> = raw_accounts
            .iter()
            .enumerate()
            .map(|(i, a)| (a.guid.clone(), i))
            .collect();

        let mut balances = vec![rucash::Num::from(0); raw_accounts.len()];
        for split in &raw_splits {
            if let Some(&idx) = guid_to_idx.get(&split.account_guid) {
                balances[idx] += split.value;
            }
        }

        assemble(raw_accounts, balances, raw_txns, raw_splits)
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

    /// Serialise the book back into a minimal GnuCash v2 XML document that
    /// [`Self::try_from_gnucash_file`] (via rucash) parses back into the
    /// same raw parts.  The output is deterministic, so it doubles as the
    /// generator for the committed example books under `example_data/`
    /// (e.g. `example_data/ctm03955-marginal-relief/input.gnucash`, a gzip of this
    /// document).
    ///
    /// Only the parts the parser round-trips are written: the accounts
    /// (guid, name, type, parent, currency), the transactions (guid,
    /// post/enter dates, currency) and their splits (value/quantity in
    /// `numerator/denominator` form).  Everything else — prices, slots,
    /// book metadata — is omitted.
    pub fn to_gnucash_xml(&self) -> String {
        use std::fmt::Write;

        let mut out = String::new();
        writeln!(out, r#"<?xml version="1.0" encoding="utf-8" ?>"#).unwrap();
        writeln!(out, "<gnc-v2").unwrap();
        for (prefix, ns) in [
            ("gnc", "http://www.gnucash.org/XML/gnc"),
            ("act", "http://www.gnucash.org/XML/act"),
            ("cmdty", "http://www.gnucash.org/XML/cmdty"),
            ("split", "http://www.gnucash.org/XML/split"),
            ("trn", "http://www.gnucash.org/XML/trn"),
            ("ts", "http://www.gnucash.org/XML/ts"),
        ] {
            writeln!(out, "     xmlns:{prefix}=\"{ns}\"").unwrap();
        }
        writeln!(out, ">").unwrap();
        writeln!(out, "  <gnc:book>").unwrap();

        // The only commodity the books carry: pound sterling.
        writeln!(out, "    <gnc:commodity version=\"2.0.0\">").unwrap();
        writeln!(out, "      <cmdty:space>CURRENCY</cmdty:space>").unwrap();
        writeln!(out, "      <cmdty:id>GBP</cmdty:id>").unwrap();
        writeln!(out, "    </gnc:commodity>").unwrap();

        for account in &self.raw_accounts {
            writeln!(out, "    <gnc:account version=\"2.0.0\">").unwrap();
            writeln!(
                out,
                "      <act:name>{}</act:name>",
                xml_escape(&account.name)
            )
            .unwrap();
            writeln!(
                out,
                "      <act:id type=\"guid\">{}</act:id>",
                xml_escape(&account.guid)
            )
            .unwrap();
            writeln!(
                out,
                "      <act:type>{}</act:type>",
                xml_escape(&account.r#type)
            )
            .unwrap();
            writeln!(out, "      <act:commodity>").unwrap();
            writeln!(out, "        <cmdty:space>CURRENCY</cmdty:space>").unwrap();
            writeln!(out, "        <cmdty:id>GBP</cmdty:id>").unwrap();
            writeln!(out, "      </act:commodity>").unwrap();
            writeln!(out, "      <act:commodity-scu>100</act:commodity-scu>").unwrap();
            if !account.parent_guid.is_empty() {
                writeln!(
                    out,
                    "      <act:parent type=\"guid\">{}</act:parent>",
                    xml_escape(&account.parent_guid)
                )
                .unwrap();
            }
            writeln!(out, "    </gnc:account>").unwrap();
        }

        for txn in &self.raw_txns {
            let posted = txn.post_datetime.format("%Y-%m-%d %H:%M:%S +0000");
            writeln!(out, "    <gnc:transaction version=\"2.0.0\">").unwrap();
            writeln!(
                out,
                "      <trn:id type=\"guid\">{}</trn:id>",
                xml_escape(&txn.guid)
            )
            .unwrap();
            writeln!(out, "      <trn:currency>").unwrap();
            writeln!(out, "        <cmdty:space>CURRENCY</cmdty:space>").unwrap();
            writeln!(out, "        <cmdty:id>GBP</cmdty:id>").unwrap();
            writeln!(out, "      </trn:currency>").unwrap();
            writeln!(out, "      <trn:date-posted>").unwrap();
            writeln!(out, "        <ts:date>{posted}</ts:date>").unwrap();
            writeln!(out, "      </trn:date-posted>").unwrap();
            writeln!(out, "      <trn:date-entered>").unwrap();
            writeln!(out, "        <ts:date>{posted}</ts:date>").unwrap();
            writeln!(out, "      </trn:date-entered>").unwrap();
            writeln!(
                out,
                "      <trn:description>{}</trn:description>",
                xml_escape(&txn.description)
            )
            .unwrap();
            writeln!(out, "      <trn:splits>").unwrap();
            for (i, split) in self
                .raw_splits
                .iter()
                .enumerate()
                .filter(|(_, s)| s.tx_guid == txn.guid)
            {
                let value = num_ratio(split.value);
                writeln!(out, "        <trn:split>").unwrap();
                writeln!(
                    out,
                    "          <split:id type=\"guid\">{}-{i}</split:id>",
                    xml_escape(&txn.guid)
                )
                .unwrap();
                writeln!(
                    out,
                    "          <split:reconciled-state>n</split:reconciled-state>"
                )
                .unwrap();
                writeln!(out, "          <split:value>{value}</split:value>").unwrap();
                writeln!(out, "          <split:quantity>{value}</split:quantity>").unwrap();
                writeln!(
                    out,
                    "          <split:account type=\"guid\">{}</split:account>",
                    xml_escape(&split.account_guid)
                )
                .unwrap();
                writeln!(out, "        </trn:split>").unwrap();
            }
            writeln!(out, "      </trn:splits>").unwrap();
            writeln!(out, "    </gnc:transaction>").unwrap();
        }

        writeln!(out, "  </gnc:book>").unwrap();
        writeln!(out, "</gnc-v2>").unwrap();
        out
    }
}

/// Escape `&`, `<`, `>` and `"` for XML text/attribute content.
fn xml_escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            _ => out.push(c),
        }
    }
    out
}

/// A `rucash::Num` (a `rust_decimal::Decimal` under the `decimal` feature)
/// as the `numerator/denominator` pair the GnuCash XML split elements use.
///
/// The denominator is `10^scale`, which assumes a scale of at most 18 —
/// true for money amounts, and the largest denominator rucash (which
/// parses the pair as `i64`) can represent anyway.
fn num_ratio(n: rucash::Num) -> String {
    format!("{}/{}", n.mantissa(), 10u64.pow(n.scale()))
}
