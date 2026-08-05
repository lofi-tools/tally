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
//!   `company` identity block, an `accounts` sub-object and the flat
//!   accounts-metadata fields).  The file becomes a [`config::FileConfig`]
//!   and is merged with the captured environment ([`config::RawEnvConfig`])
//!   and the subcommand's CLI values into a [`config::ResolvedConfig`] by
//!   [`config::Ct600Config::resolve`]: every identity field is optional —
//!   the company name is resolved from Companies House at runtime when an
//!   API key is configured (the registration date too, when the config
//!   carries no identity details at all), the company number falls back on
//!   `COMPANY_NUMBER`, the Corporation Tax reference (UTR) comes from
//!   `UNIQUE_TAXPAYER_REF` (winning) or `company.tax_reference`, the return
//!   period is the config's `accounts.period` or is deduced from
//!   `--accounts-made-up-to`, defaulting to the next accounting period from
//!   Companies House, and anything still missing is reported clearly (see
//!   [`config`]);
//! * `--book` — the GnuCash ledger (`input.gnucash`);
//! * `--out` — the output directory; the CT600 GovTalk message is written
//!   to `<out>/ct600.xml`.

mod config;

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use config::{Ct600Config, RawEnvConfig};
use ct600::{CompaniesHouseClientType, Ct600Return};
use ixbrl::reports::uk_frs105_accounts::Frs105Accounts;
use ixbrl::reports::uk_frs105_corp_tax::Frs105CorpTax;
use ixbrl::GnucashBook;

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
                let args = parse_ct600_args()?;
                run_ct600(args).await
            }
        }
    }
}

/// Parse the `ct600` flags into the subcommand's [`Ct600Config`].
///
/// `--config-path` is required here (the config file cannot be read without
/// it); `--book` and `--out` are optional because the resolution stage
/// ([`config::Ct600Config::resolve`]) is what reports them missing, alongside
/// any other still-unresolved config.
fn parse_ct600_args() -> Result<Ct600Config> {
    let mut rest = std::env::args().skip(2); // skip program + subcommand
    let mut config_path = None;
    let mut accounts_made_up_to = None;
    let mut book_path = None;
    let mut out_dir = None;

    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--config-path" => config_path = Some(next_value(&mut rest, &arg)?),
            "--accounts-made-up-to" => accounts_made_up_to = Some(next_date(&mut rest, &arg)?),
            "--book" => book_path = Some(next_value(&mut rest, &arg)?),
            "--out" => out_dir = Some(next_value(&mut rest, &arg)?),
            "-h" | "--help" => {
                println!("{USAGE}");
                std::process::exit(0);
            }
            other => bail!("unknown argument '{other}'\n\n{USAGE}"),
        }
    }

    Ok(Ct600Config {
        config_path: config_path.context("missing --config-path")?,
        accounts_made_up_to,
        book_path,
        out_dir,
    })
}

const USAGE: &str = "\
usage: tally ct600 --config-path <config> --book <book> --out <dir> [--accounts-made-up-to <date>]

Produce (not submit) the CT600 corporation-tax return.

  --config-path <config>    JSON config: company + accounts metadata
  --book <book>             GnuCash ledger (input.gnucash)
  --out <dir>               output directory; writes <dir>/ct600.xml
  --accounts-made-up-to <date>
                            date at which the accounts are made (YYYY-MM-DD);
                            deduce the return period as the 12 months ending
                            on it, instead of the config's period or the
                            Companies House default";

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

/// The date following a flag, parsed as `YYYY-MM-DD`.
fn next_date(rest: &mut impl Iterator<Item = String>, flag: &str) -> Result<NaiveDate> {
    let value = rest
        .next()
        .with_context(|| format!("missing value for {flag}"))?;
    NaiveDate::parse_from_str(&value, "%Y-%m-%d")
        .with_context(|| format!("invalid date '{value}' for {flag} (expected YYYY-MM-DD)"))
}

/// The `ct600` subcommand: config + GnuCash book -> CT600 message.
async fn run_ct600(args: Ct600Config) -> Result<()> {
    // Parse and enrich the subcommand's inputs: the company identity is
    // resolved from the config file, the captured environment, and Companies
    // House; anything still missing errors with an explanation (see
    // `config`).
    let resolved = args.resolve(&RawEnvConfig::from_env()).await?;

    // Print the resolved values for this run.
    println!("resolved: company '{}'", resolved.company.name);
    println!("  company number: {}", resolved.company.company_number);
    println!("  tax reference (UTR): {}", resolved.company.tax_reference);
    // The epoch is the unset sentinel: the registration date is only known
    // once resolved from Companies House.
    let registration_date = if resolved.company.registration_date == NaiveDate::default() {
        "unknown (resolve it with a Companies House API key)".to_string()
    } else {
        resolved.company.registration_date.to_string()
    };
    println!("  registration date: {registration_date}");
    let period = resolved.accounts.period();
    println!("  return period: {} to {}", period.start, period.end);
    println!(
        "  financial years: FY{} at {}%, FY{} at {}%",
        resolved.accounts.fy1_year,
        resolved.accounts.fy1_rate,
        resolved.accounts.fy2_year,
        resolved.accounts.fy2_rate
    );
    println!("  book: {}", resolved.book_path.display());
    println!("  out: {}", resolved.out_dir.display());
    let api = match resolved.companies_house_client_type {
        Some(CompaniesHouseClientType::Live) => "live",
        Some(CompaniesHouseClientType::Sandbox) => "sandbox",
        None => "none",
    };
    println!("  Companies House API: {api}");

    let book_path = resolved.book_path.to_string_lossy().into_owned();
    let book = GnucashBook::try_from_gnucash_file(&book_path)
        .await
        .with_context(|| format!("load GnuCash book '{book_path}'"))?;

    // FRS 105 inputs to the return.
    let accounts =
        Frs105Accounts::new(&book, &resolved.company, &resolved.metadata, &resolved.accounts);
    let corp_tax = Frs105CorpTax::builder(&book, &resolved.company, &resolved.accounts).build();

    // The CT600 GovTalk message.
    let filing = Ct600Return::from_inputs(&accounts, &corp_tax);
    let xml = filing.to_xml();

    std::fs::create_dir_all(&resolved.out_dir)
        .with_context(|| format!("create output directory '{}'", resolved.out_dir.display()))?;
    let out_path = resolved.out_dir.join("ct600.xml");
    std::fs::write(&out_path, xml)
        .with_context(|| format!("write '{}'", out_path.display()))?;
    println!("wrote {}", out_path.display());

    Ok(())
}
