use crate::models::profit_and_loss::ProfitAndLoss;
use crate::models::Transaction;
use anyhow::anyhow as err;
pub use config::CONFIG;
use models::sheet3::BalanceSheet3;
use models::static_data::{BANK, DIRECTORS_LOAN, PAYE_PAID, SALES, WAGES_NET};
use models::Account;
use nordigen::{BookedTransaction, NordigenClient};

pub mod config;
pub mod models;
pub mod nordigen;
pub mod utils;

// TODO calculate interest on director's loan -> add interest in transaction
// TODO transaction::repay_loan_in_currency
// TODO ensure director's loan account is zero

// TODO struct ListTransactions -> get_balance_sheet_at(date): adds interest transactions at end of months
// TODO struct ListTransactions:

// TODO BalanceSheet contains Accounts, Accounts contain BalanceChanges
// -> calculating the balance sheet ? means classifying BalanceChanges, then going over loan accounts again for interest

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let transactions = fetch_starling_transactions().await?;
    let extra_transactions: Vec<Transaction> = vec![
        // Transaction::sale(9240.0, (2023, 01, 13)),
        // Transaction::sale(8820.0, (2023, 02, 14)),
        // Transaction::withdraw_loan(1400.0, (2023, 02, 26)),
    ];

    // let totals = BalanceTotals::from_transactions(&transactions);
    // let balance_sheet = totals.balance_sheet();
    let balance_sheet = BalanceSheet3::now_from_transactions(&transactions);
    println!("{balance_sheet}");

    // TODO profit and loss
    let profit_and_loss = ProfitAndLoss::from_transactions(&transactions);
    // print!("{profit_and_loss}");

    // TODO task for bank account transfers without corresponding accounting
    // TODO assert invoice file per transaction (store in dir in git)

    Ok(())
}

async fn fetch_starling_transactions() -> anyhow::Result<Vec<Transaction>> {
    let mut nordigen_client = NordigenClient::new();
    let transactions = nordigen_client.list_starling_transactions().await?;

    let mapped_transactions: Vec<Transaction> = transactions
        .iter()
        .map(|tx| Transaction::from_starling(tx))
        .collect::<Result<Vec<_>, _>>()?;

    Ok(mapped_transactions)
}

impl Transaction {
    fn from_starling(starling_tx: &BookedTransaction) -> anyhow::Result<Transaction> {
        match starling_tx {
            tx if tx.debtor_name == Some("Nigel Frank Intern".to_string()) => Ok(
                // Transaction::sale(tx.transaction_amount.amount, tx.booking_date),
                Transaction::to_bank(starling_tx, &SALES),
            ),
            tx if tx.remittance_information_unstructured.contains("Salary") => Ok(
                // Transaction::salary(tx.transaction_amount.amount, tx.booking_date),
                Transaction::to_bank(starling_tx, &WAGES_NET),
            ),
            tx if tx
                .remittance_information_unstructured
                .contains("Director's loan") =>
            {
                Ok(Transaction::to_bank(starling_tx, &DIRECTORS_LOAN))
            }
            tx if tx
                .remittance_information_unstructured
                .contains("120PZ028811752312") =>
            {
                Ok(Transaction::to_bank(starling_tx, &PAYE_PAID))
            }
            other_tx => return Err(err!("no match for tx: {other_tx:?}")),
        }
    }

    fn to_bank(nordigen_tx: &BookedTransaction, from_account: &'static Account) -> Transaction {
        Transaction {
            from: from_account,
            to: &BANK, // all nordigen transactions are to the bank: positive amounts for incoming, negative amounts for outgoing
            amount_gbp: nordigen_tx.transaction_amount.amount,
            date: nordigen_tx.booking_date,
        }
        .to_positive_direction() // this reverses the direction of the transaction so that amounts are positive
    }
}
