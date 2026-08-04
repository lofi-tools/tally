//! `tally` — command-line front-end for the accounting libraries.
//!
//! Exposes the `ct600` and `ixbrl` crates as a CLI.  For now there is a
//! single subcommand, `ct600`, which produces (not submits) the CT600
//! corporation-tax return:
//!
//! ```text
//! tally ct600 --config-path <config> --book <book> --out <dir>
//! ```
//!
//! * `--config-path` — a JSON config file describing the company and the
//!   accounts metadata (same shape as
//!   `libs/ixbrl/example_data/example2/input-company.json`: a nested
//!   `company` identity block plus the flat [`AccountsMetadata`] fields);
//! * `--book` — the GnuCash ledger (`input.gnucash`);
//! * `--out` — the output directory; the CT600 GovTalk message is written
//!   to `<out>/ct600.xml`.

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use ct600::Ct600Return;
use ixbrl::company::Company;
use ixbrl::reports::uk_frs105_accounts::{AccountsMetadata, Frs105Accounts};
use ixbrl::reports::uk_frs105_corp_tax::Frs105CorpTax;
use ixbrl::GnucashBook;
use serde::Deserialize;

/// The top-level shape of the config file: a nested `company` identity
/// block plus the flat accounts-metadata fields (incl. logo/signature).
#[derive(Deserialize)]
struct Config {
    company: CompanyConfig,
    #[serde(flatten)]
    metadata: AccountsMetadata,
}

/// The company identity block of the config file.
#[derive(Deserialize)]
struct CompanyConfig {
    name: String,
    tax_reference: String,
    company_number: String,
    accounting_period_start: NaiveDate,
    accounting_period_end: NaiveDate,
}

/// The `tally` subcommands.
enum Command {
    /// Produce the CT600 corporation-tax return.
    Ct600,
}

impl Command {
    /// Parse the subcommand from the command line (`tally ct600 ...`).
    fn parse_cmd_args() -> Result<Command> {
        let mut rest = std::env::args().skip(1);
        let command = rest.next().unwrap_or_default();
        if matches!(command.as_str(), "-h" | "--help") {
            println!("{USAGE}");
            std::process::exit(0);
        }
        match command.as_str() {
            "ct600" => Ok(Command::Ct600),
            other => bail!("unknown command '{other}'\n\n{USAGE}"),
        }
    }

    /// Run the subcommand.
    async fn run(&self) -> Result<()> {
        match self {
            Command::Ct600 => {
                let args = Ct600Args::parse_args()?;
                run_ct600(args).await
            }
        }
    }
}

/// Parsed arguments for the `ct600` subcommand.
struct Ct600Args {
    config_path: PathBuf,
    book_path: PathBuf,
    out_dir: PathBuf,
}

impl Ct600Args {
    /// Parse the `ct600` flags (`--config-path`, `--book`, `--out`).
    fn parse_args() -> Result<Ct600Args> {
        let mut rest = std::env::args().skip(2); // skip program + subcommand
        let mut config_path = None;
        let mut book_path = None;
        let mut out_dir = None;

        while let Some(arg) = rest.next() {
            match arg.as_str() {
                "--config-path" => config_path = Some(next_value(&mut rest, &arg)?),
                "--book" => book_path = Some(next_value(&mut rest, &arg)?),
                "--out" => out_dir = Some(next_value(&mut rest, &arg)?),
                "-h" | "--help" => {
                    println!("{USAGE}");
                    std::process::exit(0);
                }
                other => bail!("unknown argument '{other}'\n\n{USAGE}"),
            }
        }

        Ok(Ct600Args {
            config_path: config_path.context("missing --config-path")?,
            book_path: book_path.context("missing --book")?,
            out_dir: out_dir.context("missing --out")?,
        })
    }
}

const USAGE: &str = "\
usage: tally ct600 --config-path <config> --book <book> --out <dir>

Produce (not submit) the CT600 corporation-tax return.

  --config-path <config>   JSON config: company + accounts metadata
  --book <book>            GnuCash ledger (input.gnucash)
  --out <dir>              output directory; writes <dir>/ct600.xml";

/// Entry point: parse the subcommand, run it, and print a concise error
/// (no backtrace) on failure.
#[tokio::main]
async fn main() {
    let result: Result<()> = async {
        let command = Command::parse_cmd_args()?;
        command.run().await
    }
    .await;
    if let Err(err) = result {
        eprintln!("error: {err:#}");
        std::process::exit(1);
    }
}

/// The value following a flag, or an error naming the flag.
fn next_value(rest: &mut impl Iterator<Item = String>, flag: &str) -> Result<PathBuf> {
    rest.next()
        .map(PathBuf::from)
        .with_context(|| format!("missing value for {flag}"))
}

/// The `ct600` subcommand: config + GnuCash book -> CT600 message.
async fn run_ct600(args: Ct600Args) -> Result<()> {
    let config_json = std::fs::read_to_string(&args.config_path)
        .with_context(|| format!("read config '{}'", args.config_path.display()))?;
    let config: Config = serde_json::from_str(&config_json)
        .with_context(|| format!("parse config '{}'", args.config_path.display()))?;

    let company = Company::new(
        config.company.name,
        config.company.tax_reference,
        config.company.company_number,
        config.company.accounting_period_start,
        config.company.accounting_period_end,
    );

    let book_path = args.book_path.to_string_lossy().into_owned();
    let book = GnucashBook::try_from_gnucash_file(&book_path)
        .await
        .with_context(|| format!("load GnuCash book '{book_path}'"))?;

    // FRS 105 inputs to the return.
    let accounts = Frs105Accounts::new(&book, &company, &config.metadata);
    let corp_tax = Frs105CorpTax::builder(&book, &company).build();

    // The CT600 GovTalk message.
    let filing = Ct600Return::from_inputs(&accounts, &corp_tax);
    let xml = filing.to_xml();

    std::fs::create_dir_all(&args.out_dir)
        .with_context(|| format!("create output directory '{}'", args.out_dir.display()))?;
    let out_path = args.out_dir.join("ct600.xml");
    std::fs::write(&out_path, xml)
        .with_context(|| format!("write '{}'", out_path.display()))?;
    println!("wrote {}", out_path.display());

    Ok(())
}
