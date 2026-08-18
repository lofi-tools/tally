//! `tally` — command-line front-end for the accounting libraries.
//!
//! Exposes the `ct600` and `ixbrl` crates as a CLI.  For now there is a
//! single subcommand, `ct600`, which produces (not submits) the CT600
//! corporation-tax return:
//!
//! ```text
//! tally ct600 --config-path <config> --book <book> [--out <dir>]
//! ```
//!
//! * `--config-path` — the JSON config (see
//!   `example_data/basic-1/input_config.jsonc`).  A
//!   [`config::ConfigBuilder`] merges it with the captured environment
//!   ([`config::EnvVars`]) and the CLI values ([`config::CliArgs`]) into a
//!   strict [`config::ResolvedInputs`]: identity fields that the config
//!   leaves out are filled from the environment / Companies House, and the
//!   first still-missing input errors with how to resolve it (see
//!   [`config`]);
//! * `--book` — the GnuCash ledger (`input.gnucash`);
//! * `--out` — the output directory (optional; defaults to the tally
//!   repo's `.cache/tally-cli` when run from the checkout, else
//!   `~/.cache/tally-cli`); the CT600 GovTalk message is written to
//!   `<out>/ct600-<company-number>.xml`.

mod config;

use std::path::PathBuf;

use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use config::{CliArgs, ConfigBuilder};
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

/// Parse the `ct600` flags into the subcommand's [`CliArgs`].
fn parse_ct600_args() -> Result<CliArgs> {
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

    Ok(CliArgs {
        config_path: config_path.context("missing --config-path")?,
        accounts_made_up_to,
        book_path,
        out_dir,
    })
}

const USAGE: &str = "\
usage: tally ct600 --config-path <config> --book <book> [--out <dir>] [--accounts-made-up-to <date>]

Produce (not submit) the CT600 corporation-tax return.

  --config-path <config>    JSON config: company + accounts metadata
  --book <book>             GnuCash ledger (input.gnucash)
  --out <dir>               output directory (default: the tally repo's
                            .cache/tally-cli, else ~/.cache/tally-cli);
                            writes <dir>/ct600-<company-number>.xml
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
async fn run_ct600(args: CliArgs) -> Result<()> {
    // Resolve the config (see `config`).
    let resolved = ConfigBuilder::from_cli(args)?.build().await?;

    // Print the resolved values.
    println!("resolved: company '{}'", resolved.company.name);
    println!("  company number: {}", resolved.company.company_number);
    println!("  tax reference (UTR): {}", resolved.company.tax_reference);
    // The epoch is the unset sentinel.
    let registration_date = if resolved.company.registration_date == NaiveDate::default() {
        "unknown (resolve it with a Companies House API key)".to_string()
    } else {
        resolved.company.registration_date.to_string()
    };
    println!("  registration date: {registration_date}");
    let period = resolved.accounts.period();
    println!("  return period: {} to {}", period.start, period.end);
    let fy1_calc = ixbrl::calc_corp_tax::for_fy(resolved.accounts.fy1_year);
    let fy2_calc = ixbrl::calc_corp_tax::for_fy(resolved.accounts.fy2_year);
    println!(
        "  financial years: FY{} ({}), FY{} ({})",
        resolved.accounts.fy1_year,
        fy1_calc.name(),
        resolved.accounts.fy2_year,
        fy2_calc.name()
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
        Frs105Accounts::new(&book, &resolved.company, &resolved.profile, &resolved.accounts);
    let corp_tax = Frs105CorpTax::builder(&book, &resolved.company, &resolved.accounts).build();

    // The CT600 GovTalk message.
    let filing = Ct600Return::from_inputs(&accounts, &corp_tax);
    let xml = filing.to_xml();

    std::fs::create_dir_all(&resolved.out_dir)
        .with_context(|| format!("create output directory '{}'", resolved.out_dir.display()))?;
    // The company number names the output file, so live runs are
    // distinguishable by their data source.
    let out_path = resolved
        .out_dir
        .join(format!("ct600-{}.xml", resolved.company.company_number));
    std::fs::write(&out_path, xml).with_context(|| format!("write '{}'", out_path.display()))?;
    println!("wrote {}", out_path.display());

    Ok(())
}
