//! Ledger upload, ingest and JSON views (spec §9 + §6).
//!
//! Upload: bytes stream to a temp file (size-capped), sha256, parse once via
//! `GnucashBook::try_from_gnucash_file`, then persist to
//! `<repo>/.cache/tally-api/uploads/<uuid>.gnucash` and write the
//! `Account` / `Transaction` / `Split` rows in one transaction.  Reports
//! rebuild the book from those rows via `GnucashBook::from_raw_parts`.

use std::collections::HashMap;
use std::io::Write as _;
use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use chrono::NaiveDateTime;
use ixbrl::{AccountType, GnucashBook};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::app::AppState;
use crate::auth::AuthUser;
use crate::companies::owned_company;
use crate::error::{AppError, FieldIssue};
use crate::extract::{AppMultipart, AppPath, AppQuery};
use crate::models::{Account, Ledger, Split, Transaction};

// ---------------------------------------------------------------------------
// Request / response types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct TxnQuery {
    pub limit: Option<u32>,
    pub offset: Option<u32>,
}

#[derive(Debug, Serialize)]
pub struct TransactionOut {
    pub guid: String,
    pub post_datetime: String,
    /// The GnuCash transaction description ("" when the book had none).
    pub description: String,
    pub splits: Vec<SplitOut>,
}

#[derive(Debug, Serialize)]
pub struct SplitOut {
    pub account_guid: String,
    pub value: String,
}

#[derive(Debug, Serialize)]
pub struct TransactionsPage {
    pub items: Vec<TransactionOut>,
    pub limit: u32,
    pub offset: u32,
}

/// The account tree, mirroring `GnucashBook`'s `Display` (zero subtrees
/// omitted, children sorted by name).
#[derive(Debug, Serialize)]
pub struct AccountNodeOut {
    /// GnuCash account guid — lets the frontend resolve split
    /// `account_guid`s to names/types (web-api-wiring-spec §15).
    pub guid: String,
    pub name: String,
    #[serde(rename = "type")]
    pub account_type: String,
    pub balance: String,
    pub children: Vec<AccountNodeOut>,
}

#[derive(Debug, Serialize)]
pub struct AccountsView {
    pub accounts: Vec<AccountNodeOut>,
    pub net_assets: String,
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

/// `POST /companies/:id/ledgers` — multipart `.gnucash` upload.
pub async fn upload(
    State(state): State<Arc<AppState>>,
    AuthUser { user, .. }: AuthUser,
    AppPath(company_id): AppPath<uuid::Uuid>,
    AppMultipart(mut multipart): AppMultipart,
) -> Result<Json<Ledger>, AppError> {
    let mut db = state.db.clone();
    // Ownership first, before any parsing work.
    owned_company(&mut db, user.id, company_id).await?;
    drop(db);

    // -- pull the `file` field -------------------------------------------------
    // `Field` borrows `multipart` mutably, so scope each iteration's field
    // and move the found one out via the loop value (no borrow across the
    // loop condition).
    let mut file_field = loop {
        let Some(field) = multipart
            .next_field()
            .await
            .map_err(|e| AppError::Multipart { message: e.to_string() })?
        else {
            break None;
        };
        if field.name() == Some("file") {
            break Some(field);
        }
    };
    let mut field = file_field.take().ok_or_else(|| AppError::Validation {
        fields: vec![FieldIssue { field: "file".into(), reason: "required (multipart file field)".into() }],
    })?;

    let filename = field.file_name().unwrap_or("").to_string();
    if !filename.to_ascii_lowercase().ends_with(".gnucash") {
        return Err(AppError::UnsupportedFileType { expected: ".gnucash", got: filename });
    }

    // -- stream to a temp file, size-capped, hashing on the way ----------------
    let tmp_dir = tempfile::tempdir().map_err(|e| AppError::Storage { source: e })?;
    let tmp_path = tmp_dir.path().join(format!("{}.gnucash", uuid::Uuid::new_v4()));
    let mut tmp = std::fs::File::create(&tmp_path).map_err(|e| AppError::Storage { source: e })?;
    let mut hasher = Sha256::new();
    let mut total: u64 = 0;
    while let Some(chunk) = field.chunk().await.map_err(|e| AppError::Multipart { message: e.to_string() })? {
        total += chunk.len() as u64;
        if total > state.max_upload_bytes {
            return Err(AppError::FileTooLarge { limit_bytes: state.max_upload_bytes });
        }
        hasher.update(&chunk);
        tmp.write_all(&chunk).map_err(|e| AppError::Storage { source: e })?;
    }
    drop(tmp);
    let file_sha256 = format!("{:x}", hasher.finalize());

    // -- parse once --------------------------------------------------------------
    let book = GnucashBook::try_from_gnucash_file(&tmp_path.to_string_lossy())
        .await
        .map_err(|source| AppError::LedgerParse { source })?;

    // -- prepare rows -------------------------------------------------------------
    let raw_accounts = book.raw_accounts();
    let raw_txns = book.raw_transactions();
    let raw_splits = book.raw_splits();

    let mut balances: HashMap<&str, rust_decimal::Decimal> = HashMap::new();
    for split in raw_splits {
        *balances.entry(&split.account_guid).or_insert(rust_decimal::Decimal::ZERO) += split.value;
    }

    let post_datetime = |dt: &NaiveDateTime| dt.and_utc().to_rfc3339();

    // -- persist file + rows ------------------------------------------------------
    // Stage the file under a temp name in the upload dir (same filesystem
    // as the final path) and publish it with an atomic `rename` *after* the
    // transaction commits — a crash never leaves a committed ledger pointing
    // at a half-written or missing file, and a DB failure only leaves a
    // harmless `.tmp` staging file (removed below).
    let upload_id = uuid::Uuid::new_v4();
    let final_path = state.upload_dir.join(format!("{upload_id}.gnucash"));
    let staging_path = state.upload_dir.join(format!(".{upload_id}.tmp"));
    if let Some(parent) = staging_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| AppError::Storage { source: e })?;
    }
    std::fs::copy(&tmp_path, &staging_path).map_err(|e| AppError::Storage { source: e })?;

    let mut db = state.db.clone();
    let result = async {
        let mut tx = db.transaction().await?;

        let ledger = toasty::create!(Ledger {
            company_id,
            name: filename,
            file_path: final_path.to_string_lossy().into_owned(),
            file_sha256,
            uploaded_at: chrono::Utc::now().to_rfc3339(),
            accounts_count: raw_accounts.len() as i64,
            transactions_count: raw_txns.len() as i64,
            splits_count: raw_splits.len() as i64,
        })
        .exec(&mut tx)
        .await?;

        let account_builders = raw_accounts.iter().map(|a| {
            let balance = balances.get(a.guid.as_str()).copied().unwrap_or(rust_decimal::Decimal::ZERO);
            toasty::create!(Account {
                ledger_id: ledger.id,
                guid: a.guid.clone(),
                name: a.name.clone(),
                account_type: a.r#type.clone(),
                parent_guid: a.parent_guid.clone(),
                balance,
            })
        });
        let txn_builders = raw_txns.iter().map(|t| {
            toasty::create!(Transaction {
                ledger_id: ledger.id,
                guid: t.guid.clone(),
                post_datetime: post_datetime(&t.post_datetime),
                description: t.description.clone(),
            })
        });
        let split_builders = raw_splits.iter().map(|s| {
            toasty::create!(Split {
                ledger_id: ledger.id,
                tx_guid: s.tx_guid.clone(),
                account_guid: s.account_guid.clone(),
                value: s.value,
            })
        });

        toasty::batch(account_builders.collect::<Vec<_>>()).exec(&mut tx).await?;
        toasty::batch(txn_builders.collect::<Vec<_>>()).exec(&mut tx).await?;
        toasty::batch(split_builders.collect::<Vec<_>>()).exec(&mut tx).await?;

        tx.commit().await?;
        Ok::<Ledger, AppError>(ledger)
    }
    .await;

    match result {
        Ok(ledger) => {
            // Commit succeeded: atomically publish the staged file.
            std::fs::rename(&staging_path, &final_path).map_err(|e| AppError::Storage { source: e })?;
            Ok(Json(ledger))
        }
        Err(e) => {
            // The DB work failed: drop the staging file (no committed row
            // references the final path, so nothing to clean there).
            let _ = std::fs::remove_file(&staging_path);
            Err(e)
        }
    }
}

/// `GET /companies/:id/ledgers` — the company's ledgers.
pub async fn list(
    State(state): State<Arc<AppState>>,
    AuthUser { user, .. }: AuthUser,
    AppPath(company_id): AppPath<uuid::Uuid>,
) -> Result<Json<Vec<Ledger>>, AppError> {
    let mut db = state.db.clone();
    owned_company(&mut db, user.id, company_id).await?;
    let ledgers = Ledger::filter_by_company_id(company_id)
        .order_by(Ledger::fields().uploaded_at().desc())
        .exec(&mut db)
        .await?;
    Ok(Json(ledgers))
}

/// `GET /ledgers/:id` — metadata (ownership-scoped via the company).
pub async fn get(
    State(state): State<Arc<AppState>>,
    AuthUser { user, .. }: AuthUser,
    AppPath(id): AppPath<uuid::Uuid>,
) -> Result<Json<Ledger>, AppError> {
    let mut db = state.db.clone();
    let ledger = owned_ledger(&mut db, user.id, id).await?;
    Ok(Json(ledger))
}

/// `DELETE /ledgers/:id` — remove rows + file.
pub async fn delete(
    State(state): State<Arc<AppState>>,
    AuthUser { user, .. }: AuthUser,
    AppPath(id): AppPath<uuid::Uuid>,
) -> Result<axum::http::StatusCode, AppError> {
    let mut db = state.db.clone();
    let ledger = owned_ledger(&mut db, user.id, id).await?;
    let mut tx = db.transaction().await?;
    Split::filter_by_ledger_id(ledger.id).delete().exec(&mut tx).await?;
    Transaction::filter_by_ledger_id(ledger.id).delete().exec(&mut tx).await?;
    Account::filter_by_ledger_id(ledger.id).delete().exec(&mut tx).await?;
    Ledger::filter_by_id(ledger.id).delete().exec(&mut tx).await?;
    tx.commit().await?;
    if !ledger.file_path.is_empty() {
        std::fs::remove_file(&ledger.file_path).map_err(|e| AppError::Storage { source: e })?;
    }
    Ok(axum::http::StatusCode::NO_CONTENT)
}

/// `GET /ledgers/:id/accounts` — JSON account tree with balances.
pub async fn accounts_view(
    State(state): State<Arc<AppState>>,
    AuthUser { user, .. }: AuthUser,
    AppPath(id): AppPath<uuid::Uuid>,
) -> Result<Json<AccountsView>, AppError> {
    let mut db = state.db.clone();
    let ledger = owned_ledger(&mut db, user.id, id).await?;
    let accounts = Account::filter_by_ledger_id(ledger.id).exec(&mut db).await?;

    let view = build_account_tree(&accounts);
    Ok(Json(view))
}

/// `GET /ledgers/:id/transactions?limit=&offset=` — JSON transactions with
/// their splits.
pub async fn transactions_view(
    State(state): State<Arc<AppState>>,
    AuthUser { user, .. }: AuthUser,
    AppPath(id): AppPath<uuid::Uuid>,
    AppQuery(q): AppQuery<TxnQuery>,
) -> Result<Json<TransactionsPage>, AppError> {
    let limit = q.limit.unwrap_or(50).clamp(1, 200);
    let offset = q.offset.unwrap_or(0);

    let mut db = state.db.clone();
    let ledger = owned_ledger(&mut db, user.id, id).await?;

    let txns = Transaction::filter_by_ledger_id(ledger.id)
        .order_by(Transaction::fields().post_datetime().desc())
        .limit(limit as usize)
        .offset(offset as usize)
        .exec(&mut db)
        .await?;
    let splits = Split::filter_by_ledger_id(ledger.id).exec(&mut db).await?;
    let mut by_tx: HashMap<String, Vec<&Split>> = HashMap::new();
    for split in &splits {
        by_tx.entry(split.tx_guid.clone()).or_default().push(split);
    }

    let items = txns
        .iter()
        .map(|t| TransactionOut {
            guid: t.guid.clone(),
            post_datetime: t.post_datetime.clone(),
            description: t.description.clone(),
            splits: by_tx
                .get(&t.guid)
                .map(|splits| {
                    splits
                        .iter()
                        .map(|s| SplitOut {
                            account_guid: s.account_guid.clone(),
                            value: s.value.to_string(),
                        })
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect();

    Ok(Json(TransactionsPage { items, limit, offset }))
}

// ---------------------------------------------------------------------------
// Shared helpers
// ---------------------------------------------------------------------------

/// Load a ledger owned by `user_id` (through its company), or 404.
pub async fn owned_ledger(
    db: &mut toasty::Db,
    user_id: uuid::Uuid,
    id: uuid::Uuid,
) -> Result<Ledger, AppError> {
    let ledger = Ledger::filter_by_id(id)
        .first()
        .exec(db)
        .await?
        .ok_or_else(|| AppError::NotFound { resource: "ledger", id: id.to_string() })?;
    // Ownership: the ledger's company must belong to the user.
    owned_company(db, user_id, ledger.company_id).await?;
    Ok(ledger)
}

/// Build the JSON account tree from stored rows, mirroring `GnucashBook`'s
/// `Display` (root subtrees flattened, zero subtrees omitted, sorted by
/// name) plus the balance-sheet net assets.
fn build_account_tree(accounts: &[Account]) -> AccountsView {
    let mut children_of: HashMap<&str, Vec<&Account>> = HashMap::new();
    let mut roots: Vec<&Account> = Vec::new();
    for acc in accounts {
        if acc.parent_guid.is_empty() || !accounts.iter().any(|a| a.guid == acc.parent_guid) {
            roots.push(acc);
        } else {
            children_of.entry(acc.parent_guid.as_str()).or_default().push(acc);
        }
    }

    fn node(acc: &Account, children_of: &HashMap<&str, Vec<&Account>>) -> AccountNodeOut {
        let mut children = children_of
            .get(acc.guid.as_str())
            .map(|cs| cs.iter().map(|c| node(c, children_of)).collect::<Vec<_>>())
            .unwrap_or_default();
        children.sort_by(|a, b| a.name.cmp(&b.name));
        AccountNodeOut {
            guid: acc.guid.clone(),
            name: acc.name.clone(),
            account_type: acc.account_type.clone(),
            balance: acc.balance.to_string(),
            children,
        }
    }

    let is_zero = |n: &AccountNodeOut| n.balance == "0";
    fn has_nonzero(n: &AccountNodeOut) -> bool {
        !is_zero_balance(n) || n.children.iter().any(has_nonzero)
    }
    fn is_zero_balance(n: &AccountNodeOut) -> bool {
        n.balance == "0"
    }

    let mut accounts_out: Vec<AccountNodeOut> = Vec::new();
    for root in roots {
        if AccountType::try_from(root.account_type.as_str()).ok() == Some(AccountType::Root) {
            // Flatten ROOT's children into the top level (like Display).
            if let Some(children) = children_of.get(root.guid.as_str()) {
                for child in children {
                    let n = node(child, &children_of);
                    if !(is_zero(&n) && !has_nonzero(&n)) {
                        accounts_out.push(n);
                    }
                }
            }
            continue;
        }
        let n = node(root, &children_of);
        if !(is_zero(&n) && !has_nonzero(&n)) {
            accounts_out.push(n);
        }
    }

    // Net assets: the sum of the balance-sheet top-level accounts (the lib's
    // `is_balance_sheet` set).
    let net_assets: rust_decimal::Decimal = accounts
        .iter()
        .filter(|a| a.parent_guid.is_empty() || !accounts.iter().any(|o| o.guid == a.parent_guid))
        .filter(|a| is_balance_sheet_type(&a.account_type))
        .map(|a| a.balance)
        .sum();

    AccountsView {
        accounts: accounts_out,
        net_assets: net_assets.to_string(),
    }
}

/// The lib's balance-sheet account set (`AccountType::is_balance_sheet`).
fn is_balance_sheet_type(t: &str) -> bool {
    matches!(
        AccountType::try_from(t).unwrap_or(AccountType::Expense),
        AccountType::Asset
            | AccountType::Bank
            | AccountType::Cash
            | AccountType::Receivable
            | AccountType::Liability
            | AccountType::Credit
            | AccountType::Payable
    )
}



