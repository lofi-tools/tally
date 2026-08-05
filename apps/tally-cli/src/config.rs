//! Configuration for the `tally` CLI.
//!
//! Configuration is split across the input sources, each captured in its own
//! permissive type, and merged by the [`ConfigBuilder`] into a single strict
//! [`ResolvedInputs`]:
//!
//! * [`EnvVars`] — the environment, captured once ([`EnvVars::from_env`]):
//!   `UNIQUE_TAXPAYER_REF`, `COMPANY_NUMBER`, the Companies House API keys,
//!   `CT600_CACHE_DIR`.
//! * [`ConfigFile`] — the JSON config file (`--config-path`): the nested
//!   `company` block (identity + descriptive profile) plus the `accounts`
//!   sub-object (period, financial-year parameters, report metadata).
//! * [`CliArgs`] — the `ct600` subcommand's command-line values:
//!   `--config-path`, `--book`, `--out`, `--accounts-made-up-to`.  (Each
//!   future subcommand gets its own args struct here.)
//! * [`ConfigBuilder`] — loads the three sources
//!   ([`ConfigBuilder::from_cli`], in precedence order: the CLI flags first,
//!   then the environment overriding the config file) and resolves them
//!   ([`ConfigBuilder::build`]) into the strict [`ResolvedInputs`],
//!   enriching missing company data from the Companies House API (cache-first)
//!   when a client is configured.
//! * [`ResolvedInputs`] — strict.  Every field is present at this point, or
//!   resolution has already errored on the first input that was still
//!   missing, with how to resolve it.
//!
//! The company-identity fields are optional because each has an alternative
//! source to fall back on — the environment, Companies House (the name and
//! registration date), or defaults ([`Company::new`]); the return period
//! falls back on the next accounting period from Companies House or is
//! deduced from a made-up-to date; the remaining fields (the company
//! profile, the report metadata, the CLI paths) have no alternative source
//! and stay required.
//!
//! Resolution order, field by field:
//!
//! | Field | Sources (priority order) |
//! |-------|--------------------------|
//! | `company.name` | config file → Companies House (needs an API key + company number) |
//! | `company.company_number` | `COMPANY_NUMBER` (env, wins) → config file |
//! | `company.tax_reference` (UTR) | `UNIQUE_TAXPAYER_REF` (env, wins) → config file |
//! | `accounts.period` | config file (both dates) → deduced from the made-up-to date (`--accounts-made-up-to` wins over `accounts.accounts_made_up_to`; the 12 months ending on it) → Companies House next accounting period |
//! | `accounts.accounts_made_up_to` / `--accounts-made-up-to` | command line (flag, wins) → config file; deduces the return period as the 12 months ending on the date |
//! | `accounts.fy1_year` / `fy2_year` / `fy1_rate` / `fy2_rate` | config file → defaults (2019 / 2020 at 19%) |
//! | `company.registration_date` | Companies House (only when the config carries no identity at all) → [`Company::new`] default |
//! | Companies House layer | `COMPANIES_HOUSE_API_KEY` (live) / `COMPANIES_HOUSE_SANDBOX_API_KEY` (sandbox); response cache in `CT600_CACHE_DIR` (env) |
//! | `company.profile.*` (directors, contacts, accountant/auditor, ...) | config file only |
//! | `accounts.*` report metadata (report_title, dates, employees, ...) | config file only |
//! | `--book`, `--out` | command line only |

use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context, Result};
use chrono::{Duration, Months, NaiveDate};
use ct600::{CompaniesHouseClient, CompaniesHouseClientType};
use ixbrl::company::{AccountingPeriod, AccountsMeta, Company, CompanyProfile};
use serde::Deserialize;

/// The values `tally` reads from the environment.
///
/// Captured once (via [`EnvVars::from_env`]) into the builder so resolution
/// is a pure merge of the config file, the environment and the CLI.  Empty
/// environment variables count as unset, matching the libraries' conventions.
#[derive(Debug, Clone, Default)]
pub struct EnvVars {
    /// `UNIQUE_TAXPAYER_REF` — the Corporation Tax reference (UTR); wins
    /// over the config file's `company.tax_reference`.
    pub unique_taxpayer_ref: Option<String>,
    /// `COMPANY_NUMBER` — the company number; wins over the config file's
    /// `company.company_number`.
    pub company_number: Option<String>,
    /// `COMPANIES_HOUSE_API_KEY` — the live Companies House API key.
    pub companies_house_api_key: Option<String>,
    /// `COMPANIES_HOUSE_SANDBOX_API_KEY` — the sandbox Companies House API key.
    pub companies_house_sandbox_api_key: Option<String>,
    /// `CT600_CACHE_DIR` — the Companies House response-cache directory.
    pub cache_dir: Option<PathBuf>,
}

impl EnvVars {
    /// Snapshot the environment: every non-empty matching variable is
    /// captured (empty variables count as unset).
    pub fn from_env() -> Self {
        Self {
            unique_taxpayer_ref: non_empty_env("UNIQUE_TAXPAYER_REF"),
            company_number: non_empty_env("COMPANY_NUMBER"),
            companies_house_api_key: non_empty_env("COMPANIES_HOUSE_API_KEY"),
            companies_house_sandbox_api_key: non_empty_env("COMPANIES_HOUSE_SANDBOX_API_KEY"),
            cache_dir: non_empty_env("CT600_CACHE_DIR").map(PathBuf::from),
        }
    }

    /// Which Companies House API the config will use: the live key
    /// (`COMPANIES_HOUSE_API_KEY`) preferred over the sandbox key
    /// (`COMPANIES_HOUSE_SANDBOX_API_KEY`), matching
    /// `ct600::Config::from_env`.  `None` when no key is set.
    pub fn companies_house_client_type(&self) -> Option<CompaniesHouseClientType> {
        if self.companies_house_api_key.is_some() {
            Some(CompaniesHouseClientType::Live)
        } else if self.companies_house_sandbox_api_key.is_some() {
            Some(CompaniesHouseClientType::Sandbox)
        } else {
            None
        }
    }

    /// Whether a Companies House API key is configured.
    pub fn api_key_configured(&self) -> bool {
        self.companies_house_client_type().is_some()
    }

    /// The Companies House client configuration resolved from the captured
    /// values alone (never re-reads the environment).
    pub fn ch_config(&self) -> ct600::Config {
        // `Config::default()` carries no API layer; the company number and
        // cache dir are overridden with the captured values so the ambient
        // environment cannot leak in.
        let mut config = ct600::Config::default()
            .with_company_number(self.company_number.clone().unwrap_or_default());
        if let Some(cache_dir) = &self.cache_dir {
            config = config.with_cache_dir(cache_dir.clone());
        }
        if let Some(client_type) = self.companies_house_client_type() {
            let api_key = match client_type {
                CompaniesHouseClientType::Live => self.companies_house_api_key.as_deref(),
                CompaniesHouseClientType::Sandbox => self.companies_house_sandbox_api_key.as_deref(),
            };
            if let Some(api_key) = api_key {
                config = config.with_api(api_key, client_type == CompaniesHouseClientType::Sandbox);
            }
        }
        config
    }
}

/// A non-empty environment variable, if set.
fn non_empty_env(name: &str) -> Option<String> {
    std::env::var(name).ok().filter(|value| !value.is_empty())
}

/// The JSON config file's contents (`--config-path`): the nested `company`
/// block and the `accounts` sub-object.
///
/// The `company` block's identity fields (`name`, `tax_reference`,
/// `company_number`) are optional — the company name is resolved from
/// Companies House at runtime when an API key is configured, the company
/// number is overridden by the `COMPANY_NUMBER` environment variable, and
/// the Corporation Tax reference (UTR) is overridden by the
/// `UNIQUE_TAXPAYER_REF` environment variable.  The `company` block's
/// descriptive profile fields ([`CompanyProfile`]) and the `accounts`
/// sub-object's report metadata are required — they have no alternative
/// source.  The `accounts` sub-object's return period is optional: an
/// explicit `accounts.period` wins, otherwise the period is deduced from
/// `accounts.accounts_made_up_to` or comes from Companies House, and the
/// financial-year parameters default.
#[derive(Debug, Clone, Deserialize)]
pub struct ConfigFile {
    /// The config file's `company` block: optional identity fields plus the
    /// required descriptive profile ([`CompanyProfile`], flattened).
    pub company: RawCompanyConfig,
    /// The config file's `accounts` sub-object ([`AccountsMeta`], the exact
    /// shape of the `accounts.*` keys): the optional return period and
    /// financial-year parameters, plus the required report metadata.
    pub accounts: AccountsMeta,
}

impl ConfigFile {
    /// Parse the config file at `path` (`--config-path`).
    pub fn from_file(path: &Path) -> Result<Self> {
        let data = std::fs::read_to_string(path)
            .with_context(|| format!("read config '{}'", path.display()))?;
        serde_json::from_str(&data)
            .with_context(|| format!("parse config '{}'", path.display()))
    }
}

/// The company block of the config file (`company.*`).
///
/// The identity fields are optional: the company name is resolved from
/// Companies House at runtime when an API key is configured, the company
/// number is overridden by the `COMPANY_NUMBER` environment variable, and
/// the Corporation Tax reference (UTR) is overridden by the
/// `UNIQUE_TAXPAYER_REF` environment variable.  The descriptive profile
/// fields ([`CompanyProfile`]) are flattened in and are all required —
/// nothing is resolved from Companies House for them.  The return period
/// and financial-year parameters are not here — they live in the config
/// file's `accounts.*` sub-object ([`AccountsMeta`]).
#[derive(Debug, Clone, Deserialize)]
pub struct RawCompanyConfig {
    pub name: Option<String>,
    pub tax_reference: Option<String>,
    pub company_number: Option<String>,
    /// The company's descriptive profile (directors, contacts, accountant /
    /// auditor, ...), flattened into the `company.*` keys; all required.
    #[serde(flatten)]
    pub profile: CompanyProfile,
}

/// The `ct600` subcommand's command-line values, before resolution.
///
/// [`ConfigBuilder::from_cli`] loads these together with the environment and
/// the config file, and [`ConfigBuilder::build`] resolves the merged inputs
/// into a strict [`ResolvedInputs`].  (Each future subcommand gets its own
/// args struct here.)
#[derive(Debug, Clone)]
pub struct CliArgs {
    /// The `--config-path` value: the JSON config file to load (required).
    pub config_path: PathBuf,
    /// The `--accounts-made-up-to` value: a date at which the accounts are
    /// made; the return period is deduced as the 12 months ending on it
    /// (wins over the config file's `accounts.accounts_made_up_to`).
    pub accounts_made_up_to: Option<NaiveDate>,
    /// The `--book` value (required; no other source).
    pub book_path: Option<PathBuf>,
    /// The `--out` value (required; no other source).
    pub out_dir: Option<PathBuf>,
}

/// Loads and resolves the configuration for a `tally` subcommand.
///
/// The builder holds the three input sources as sub-structs — the captured
/// environment ([`EnvVars`]), the JSON config file ([`ConfigFile`]) and the
/// subcommand's command-line values ([`CliArgs`]) — and resolves them into
/// a strict [`ResolvedInputs`].
///
/// [`ConfigBuilder::from_cli`] loads the sources in precedence order: the
/// CLI flags first, then the environment overriding the config file values.
/// [`ConfigBuilder::build`] then resolves what the sources still left open —
/// creating a Companies House client when a key is configured and enriching
/// the missing company data from its cached responses / the live API — and
/// errors on the first input that is still missing, with how to resolve it.
#[derive(Debug)]
pub struct ConfigBuilder {
    /// The captured environment.
    pub env: EnvVars,
    /// The parsed config file.
    pub file: ConfigFile,
    /// The subcommand's CLI values.
    pub cli: CliArgs,
}

impl ConfigBuilder {
    /// Load every source for the subcommand: the environment is captured
    /// now, and the config file is read from `cli.config_path`.
    ///
    /// Loading is done in precedence order — the CLI flags (first) and the
    /// environment (overriding the config file) are the sources `build()`
    /// prefers when a value appears in more than one place.
    pub fn from_cli(cli: CliArgs) -> Result<Self> {
        let env = EnvVars::from_env();
        let file = ConfigFile::from_file(&cli.config_path)?;
        Ok(Self { env, file, cli })
    }

    /// Resolve the merged inputs into a strict [`ResolvedInputs`].
    ///
    /// The resolution is split into logical chunks, one sub-method per
    /// concern, so the overall flow stays readable:
    ///
    /// 1. [`ConfigBuilder::resolve_company_number`] — the company number the
    ///    lookups run against (the `COMPANY_NUMBER` environment variable
    ///    wins over the config file);
    /// 2. the Companies House client, created when an API key is configured
    ///    (its responses are served cache-first);
    /// 3. [`ConfigBuilder::resolve_identity`] — the UTR (`UNIQUE_TAXPAYER_REF`
    ///    wins), the name (enriched from the profile when absent) and the
    ///    registration date;
    /// 4. [`ConfigBuilder::resolve_period`] — the return period: config,
    ///    made-up-to date (CLI flag wins), or the Companies House next
    ///    accounting period;
    /// 5. [`ConfigBuilder::resolve_paths`] — the required `--book` / `--out`.
    ///
    /// Each step errors on the first input it cannot resolve, describing
    /// that particular problem and how to fix it.
    pub async fn build(self) -> Result<ResolvedInputs> {
        let ch = self.env.ch_config();

        // 1. The company number used for the lookups and the client.
        let company_number = self.resolve_company_number()?;

        // 2. The Companies House client, when a key is configured.  It
        //    serves from the local cache first.
        let api = self.env.api_key_configured().then(|| {
            CompaniesHouseClient::new(ch.with_company_number(company_number.clone()))
        });

        // 3. The company identity: the name (enriched from the profile when
        //    absent), the UTR and the registration date.
        let (name, tax_reference, mut registration_date) =
            self.resolve_identity(&company_number, api.as_ref()).await?;

        // 4. The return period: an explicit config period, else the
        //    made-up-to date (CLI flag wins), else the Companies House next
        //    accounting period (which also fills the registration date).
        let period =
            self.resolve_period(api.as_ref(), &company_number, &mut registration_date).await?;

        // 5. The CLI paths — they have no alternative source.
        let (book_path, out_dir) = self.resolve_paths()?;

        let mut accounts = self.file.accounts.clone();
        accounts.period = Some(period);

        let mut company = Company::new(name, tax_reference, company_number);
        if let Some(date) = registration_date {
            company.registration_date = date;
        }

        Ok(ResolvedInputs {
            company,
            profile: self.file.company.profile,
            accounts,
            book_path,
            out_dir,
            companies_house_client_type: self.env.companies_house_client_type(),
        })
    }

    /// The company number used for Companies House lookups: the `COMPANY_NUMBER`
    /// environment variable wins over the config file's
    /// `company.company_number`.
    fn resolve_company_number(&self) -> Result<String> {
        self.env
            .company_number
            .as_deref()
            .filter(|s| !s.is_empty())
            .or_else(|| {
                self.file
                    .company
                    .company_number
                    .as_deref()
                    .filter(|s| !s.is_empty())
            })
            .map(str::to_string)
            .ok_or_else(|| {
                missing(
                    &self.cli.config_path,
                    "company.company_number",
                    "set the COMPANY_NUMBER environment variable (which wins) or \
                     company.company_number in the config file",
                )
            })
    }

    /// Resolve the company identity: the company name, the Corporation Tax
    /// reference (UTR) and the registration date.
    ///
    /// The name comes from the config file, or is enriched from the
    /// company's profile (cache-first) when absent and a client is
    /// available.  The UTR comes from the `UNIQUE_TAXPAYER_REF` environment
    /// variable (winning) or the config's `company.tax_reference` — Companies
    /// House cannot provide it.  The registration date is filled in from the
    /// profile only when the config carried no identity details at all, so
    /// the accounting periods are not skewed by partial inputs (same gating
    /// as the libraries' `fill_from_profile`).
    async fn resolve_identity(
        &self,
        company_number: &str,
        api: Option<&CompaniesHouseClient>,
    ) -> Result<(String, String, Option<NaiveDate>)> {
        // Empty strings count as absent, matching the libraries' company
        // resolution conventions (see `Config::enrichment_number`).
        let name_from_config = self.file.company.name.as_deref().filter(|s| !s.is_empty());
        let number_from_config = self
            .file
            .company
            .company_number
            .as_deref()
            .filter(|s| !s.is_empty());

        // Enrich the name from Companies House when the config has none.
        // The registration date is only filled when the config carried no
        // identity details at all.
        let mut name = name_from_config.map(str::to_string);
        let mut registration_date = None;
        let details_fully_absent = name_from_config.is_none() && number_from_config.is_none();
        if name.is_none()
            && let Some(client) = api
        {
            let profile = client
                .get_company_profile_cached(company_number)
                .await
                .with_context(|| {
                    format!("resolve company '{company_number}' from Companies House")
                })?;
            name = Some(profile.company_name.clone());
            if details_fully_absent
                && let Some(created) = profile.date_of_creation.as_deref()
                && let Ok(date) = NaiveDate::parse_from_str(created, "%Y-%m-%d")
            {
                registration_date = Some(date);
            }
        }
        let name = name.ok_or_else(|| {
            missing(
                &self.cli.config_path,
                "company.name",
                "no Companies House API key is set, so it cannot be resolved from \
                 Companies House (set COMPANIES_HOUSE_API_KEY or \
                 COMPANIES_HOUSE_SANDBOX_API_KEY), or add company.name to the config file",
            )
        })?;

        // The Corporation Tax reference (UTR): the captured
        // `UNIQUE_TAXPAYER_REF` wins, falling back on the config file's
        // `company.tax_reference`.  (Captured env values are non-empty by
        // construction.)
        let tax_reference = self
            .env
            .unique_taxpayer_ref
            .as_deref()
            .or_else(|| {
                self.file
                    .company
                    .tax_reference
                    .as_deref()
                    .filter(|s| !s.is_empty())
            })
            .map(str::to_string)
            .ok_or_else(|| {
                missing(
                    &self.cli.config_path,
                    "company.tax_reference",
                    "the Corporation Tax reference (UTR) cannot be resolved from \
                     Companies House; set the UNIQUE_TAXPAYER_REF environment variable \
                     or add company.tax_reference to the config file",
                )
            })?;

        Ok((name, tax_reference, registration_date))
    }

    /// Resolve the return period.
    ///
    /// An explicit `accounts.period` (both dates) in the config wins;
    /// otherwise a made-up-to date (the `--accounts-made-up-to` flag,
    /// winning, or the config's `accounts.accounts_made_up_to`) gives the 12
    /// months ending on that date; otherwise the next accounting period to
    /// file is resolved from Companies House ([`CompaniesHouseClient::next_accounting_period`],
    /// needing an API key and company number), which also supplies the
    /// registration date when still unknown — anchoring the
    /// registration-date schedule used as its fallback.
    async fn resolve_period(
        &self,
        api: Option<&CompaniesHouseClient>,
        company_number: &str,
        registration_date: &mut Option<NaiveDate>,
    ) -> Result<AccountingPeriod> {
        if let Some(period) = self.file.accounts.period {
            return Ok(period);
        }

        let made_up_to = self
            .cli
            .accounts_made_up_to
            .or(self.file.accounts.accounts_made_up_to);
        if let Some(end) = made_up_to {
            return Ok(AccountingPeriod {
                start: end - Months::new(12) + Duration::days(1),
                end,
            });
        }

        if let Some(client) = api {
            let profile = client
                .get_company_profile_cached(company_number)
                .await
                .with_context(|| format!("resolve company '{company_number}' from Companies House"))?;
            if registration_date.is_none()
                && let Some(created) = profile.date_of_creation.as_deref()
                && let Ok(created) = NaiveDate::parse_from_str(created, "%Y-%m-%d")
            {
                *registration_date = Some(created);
            }
            let today = chrono::Utc::now().date_naive();
            let mut provisional = Company::new("", "", company_number.to_string());
            provisional.registration_date = registration_date.unwrap_or(today);
            let next = client
                .next_accounting_period(&provisional)
                .await
                .with_context(|| {
                    format!(
                        "resolve the next accounting period for '{company_number}' from \
                         Companies House"
                    )
                })?;
            return Ok(next.period);
        }

        Err(missing(
            &self.cli.config_path,
            "accounts.period",
            "the return period cannot be resolved from Companies House without an API \
             key; set COMPANIES_HOUSE_API_KEY (or COMPANIES_HOUSE_SANDBOX_API_KEY), or \
             add accounts.period (or accounts.accounts_made_up_to) to the config file",
        ))
    }

    /// The required CLI paths, `--book` and `--out`: no alternative source.
    fn resolve_paths(&self) -> Result<(PathBuf, PathBuf)> {
        let book_path = self.cli.book_path.clone().ok_or_else(|| {
            missing(
                &self.cli.config_path,
                "--book",
                "the GnuCash ledger cannot be resolved from the environment; \
                 set the --book flag",
            )
        })?;
        let out_dir = self.cli.out_dir.clone().ok_or_else(|| {
            missing(
                &self.cli.config_path,
                "--out",
                "the output directory cannot be resolved from the environment; \
                 set the --out flag",
            )
        })?;
        Ok((book_path, out_dir))
    }
}

/// Resolved (strict) inputs: every field is present, or resolution errored
/// on the first input that was still missing.
#[derive(Debug)]
pub struct ResolvedInputs {
    /// The fully-resolved company identity (name, UTR, number, registration
    /// date).
    pub company: Company,
    /// The company's descriptive profile (directors, contacts, accountant /
    /// auditor, ...) from the config's `company.*` sub-object.
    pub profile: CompanyProfile,
    /// The set of accounts being produced: the return period (resolved), the
    /// financial-year tax parameters and the report metadata.
    pub accounts: AccountsMeta,
    /// The GnuCash ledger to read.
    pub book_path: PathBuf,
    /// The output directory; the CT600 GovTalk message is written to
    /// `<out>/ct600.xml`.
    pub out_dir: PathBuf,
    /// Which Companies House API the config will use: live or sandbox,
    /// depending on which key is set; `None` when no key is configured.
    pub companies_house_client_type: Option<CompaniesHouseClientType>,
}

/// An error for a config input that no source could provide, explaining
/// that particular problem and how to resolve it.
fn missing(config_path: &Path, field: &str, reason: impl Into<String>) -> anyhow::Error {
    anyhow!(
        "cannot resolve the config from '{}': {} is missing — {}",
        config_path.display(),
        field,
        reason.into()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn company_config(
        name: Option<&str>,
        tax_reference: Option<&str>,
        company_number: Option<&str>,
    ) -> RawCompanyConfig {
        RawCompanyConfig {
            name: name.map(str::to_string),
            tax_reference: tax_reference.map(str::to_string),
            company_number: company_number.map(str::to_string),
            // The descriptive profile is irrelevant to resolution; use the
            // empty default.
            profile: CompanyProfile::default(),
        }
    }

    /// An `accounts` sub-object with the 2020 calendar-year return period
    /// (or none) and an optional made-up-to date.
    fn accounts_meta(with_period: bool, made_up_to: Option<NaiveDate>) -> AccountsMeta {
        AccountsMeta {
            period: with_period.then(|| AccountingPeriod {
                start: date(2020, 1, 1),
                end: date(2020, 12, 31),
            }),
            accounts_made_up_to: made_up_to,
            ..AccountsMeta::default()
        }
    }

    fn env(
        utr: Option<&str>,
        company_number: Option<&str>,
        api_key: Option<&str>,
        sandbox_api_key: Option<&str>,
    ) -> EnvVars {
        EnvVars {
            unique_taxpayer_ref: utr.map(str::to_string),
            company_number: company_number.map(str::to_string),
            companies_house_api_key: api_key.map(str::to_string),
            companies_house_sandbox_api_key: sandbox_api_key.map(str::to_string),
            cache_dir: None,
        }
    }

    /// A `ConfigBuilder` over the given sources with dummy CLI paths, so the
    /// tests exercise resolution without a real config file or command line.
    fn builder(env: EnvVars, company: RawCompanyConfig, accounts: AccountsMeta) -> ConfigBuilder {
        ConfigBuilder {
            env,
            file: ConfigFile { company, accounts },
            cli: CliArgs {
                config_path: PathBuf::from("test.json"),
                accounts_made_up_to: None,
                book_path: Some(PathBuf::from("book.gnucash")),
                out_dir: Some(PathBuf::from("out")),
            },
        }
    }

    /// The resolution paths that never touch the network.  The environment
    /// is passed explicitly as an [`EnvVars`], so no environment-variable
    /// mutation (and no contention between parallel tests) is involved: a
    /// complete config resolves (with or without an API key — a complete
    /// identity is never looked up), and resolution errors on the first
    /// field that is missing, naming it and how to resolve it.
    #[tokio::test]
    async fn resolve_company_offline_paths() {
        let empty_env = env(None, None, None, None);

        // Complete config: resolves with no lookup.
        let complete = company_config(
            Some("Example Biz Ltd."),
            Some("8596148860"),
            Some("12345678"),
        );
        let company = builder(empty_env.clone(), complete.clone(), accounts_meta(true, None))
            .build()
            .await
            .expect("complete config resolves")
            .company;
        assert_eq!(company.name, "Example Biz Ltd.");
        assert_eq!(company.tax_reference, "8596148860");
        assert_eq!(company.company_number, "12345678");
        // No Companies House lookup happened, so the registration date is
        // the unset default (Company::new's).
        assert_eq!(company.registration_date, NaiveDate::default());

        // Incomplete config without an API key: resolution errors on the
        // first missing field — the company number — with how to resolve it.
        let incomplete = company_config(None, None, None);
        let err = builder(empty_env.clone(), incomplete, accounts_meta(false, None))
            .build()
            .await
            .expect_err("incomplete config must error");
        let msg = format!("{err:#}");
        assert!(msg.contains("company.company_number"), "{msg}");
        assert!(msg.contains("COMPANY_NUMBER"), "{msg}");

        // With a number but no name, the error names `company.name` and the
        // key needed to resolve it.
        let no_name = company_config(None, Some("8596148860"), Some("12345678"));
        let err = builder(empty_env.clone(), no_name, accounts_meta(true, None))
            .build()
            .await
            .expect_err("a missing name must error");
        let msg = format!("{err:#}");
        assert!(msg.contains("company.name"), "{msg}");
        assert!(msg.contains("COMPANIES_HOUSE_API_KEY"), "{msg}");

        // With a name but no UTR, the error names `company.tax_reference`
        // and the `UNIQUE_TAXPAYER_REF` alternative.
        let no_utr = company_config(Some("Example Biz Ltd."), None, Some("12345678"));
        let err = builder(empty_env.clone(), no_utr, accounts_meta(true, None))
            .build()
            .await
            .expect_err("a missing UTR must error");
        let msg = format!("{err:#}");
        assert!(msg.contains("company.tax_reference"), "{msg}");
        assert!(msg.contains("UNIQUE_TAXPAYER_REF"), "{msg}");

        // Empty-string fields count as absent (matching the libraries'
        // resolution conventions): an empty name or UTR still errors.
        let empty_name = company_config(Some(""), Some("8596148860"), Some("12345678"));
        let err = builder(empty_env.clone(), empty_name, accounts_meta(true, None))
            .build()
            .await
            .expect_err("an empty company.name must error");
        assert!(format!("{err:#}").contains("company.name"));
        let empty_utr = company_config(Some("Example Biz Ltd."), Some(""), Some("12345678"));
        let err = builder(empty_env.clone(), empty_utr, accounts_meta(true, None))
            .build()
            .await
            .expect_err("an empty company.tax_reference must error");
        assert!(format!("{err:#}").contains("company.tax_reference"));

        // The captured `UNIQUE_TAXPAYER_REF` wins over the config's
        // `company.tax_reference`.
        let company = builder(
            env(Some("1111111111"), None, None, None),
            complete.clone(),
            accounts_meta(true, None),
        )
        .build()
        .await
        .expect("the env UTR wins over the config's")
        .company;
        assert_eq!(company.tax_reference, "1111111111");

        // The captured `COMPANY_NUMBER` fills an absent config number.
        let no_number = company_config(Some("Example Biz Ltd."), Some("8596148860"), None);
        let company = builder(
            env(None, Some("12345678"), None, None),
            no_number,
            accounts_meta(true, None),
        )
        .build()
        .await
        .expect("the env company number fills the absent config's")
        .company;
        assert_eq!(company.company_number, "12345678");

        // The captured `COMPANY_NUMBER` wins over the config's number.
        let company = builder(
            env(None, Some("99999999"), None, None),
            complete.clone(),
            accounts_meta(true, None),
        )
        .build()
        .await
        .expect("the env number wins over the config's")
        .company;
        assert_eq!(company.company_number, "99999999");

        // An API key set with a *complete* config: still no lookup (the
        // name is present, so nothing needs fetching) and no network.
        let company = builder(
            env(None, None, Some("test-key"), None),
            complete,
            accounts_meta(true, None),
        )
        .build()
        .await
        .expect("complete config resolves even with an API key set")
        .company;
        assert_eq!(company.name, "Example Biz Ltd.");
    }

    /// `EnvVars::from_env` snapshots every non-empty matching variable
    /// (empty variables count as unset), and the live API key is preferred
    /// over the sandbox key.  The only test that touches the environment;
    /// the resolution tests above pass the env in explicitly.
    #[test]
    fn env_snapshot_captures_non_empty_variables() {
        // The env lock serialises this mutation against the live test's
        // `EnvVars::from_env()` (see `ENV_LOCK`).
        let _guard = ENV_LOCK.lock().unwrap();
        unsafe {
            std::env::set_var("UNIQUE_TAXPAYER_REF", "1111111111");
            std::env::set_var("COMPANY_NUMBER", "12345678");
            std::env::set_var("COMPANIES_HOUSE_API_KEY", "live-key");
            std::env::set_var("COMPANIES_HOUSE_SANDBOX_API_KEY", "sandbox-key");
            std::env::set_var("CT600_CACHE_DIR", "/tmp/ch-cache");
        }
        let env = EnvVars::from_env();
        assert_eq!(env.unique_taxpayer_ref.as_deref(), Some("1111111111"));
        assert_eq!(env.company_number.as_deref(), Some("12345678"));
        assert_eq!(env.companies_house_api_key.as_deref(), Some("live-key"));
        assert_eq!(env.companies_house_sandbox_api_key.as_deref(), Some("sandbox-key"));
        assert_eq!(env.cache_dir.as_deref(), Some(Path::new("/tmp/ch-cache")));
        assert!(env.api_key_configured());
        // Both keys set: the live key is preferred.
        assert_eq!(env.companies_house_client_type(), Some(CompaniesHouseClientType::Live));

        // An empty variable counts as unset.
        unsafe {
            std::env::set_var("UNIQUE_TAXPAYER_REF", "");
            std::env::set_var("COMPANIES_HOUSE_API_KEY", "");
            std::env::remove_var("COMPANIES_HOUSE_SANDBOX_API_KEY");
        }
        let env = EnvVars::from_env();
        assert_eq!(env.unique_taxpayer_ref, None);
        assert_eq!(env.companies_house_api_key, None);
        assert_eq!(env.companies_house_sandbox_api_key, None);
        assert!(!env.api_key_configured());
        assert_eq!(env.companies_house_client_type(), None);

        // Only the sandbox key: Sandbox.
        unsafe {
            std::env::set_var("COMPANIES_HOUSE_SANDBOX_API_KEY", "sandbox-key");
        }
        let env = EnvVars::from_env();
        assert_eq!(env.companies_house_client_type(), Some(CompaniesHouseClientType::Sandbox));
        unsafe {
            std::env::remove_var("COMPANIES_HOUSE_SANDBOX_API_KEY");
        }

        unsafe {
            std::env::remove_var("UNIQUE_TAXPAYER_REF");
            std::env::remove_var("COMPANY_NUMBER");
            std::env::remove_var("COMPANIES_HOUSE_API_KEY");
            std::env::remove_var("COMPANIES_HOUSE_SANDBOX_API_KEY");
            std::env::remove_var("CT600_CACHE_DIR");
        }
    }

    /// End-to-end through the per-source types: the example config file
    /// parses into a [`ConfigFile`], and a [`ConfigBuilder`] resolves it
    /// (with an explicit [`EnvVars`], so the test never depends on the
    /// ambient environment) — erroring on the first missing CLI path, and
    /// succeeding — with everything carried through — once they are present.
    #[tokio::test]
    async fn ct600_config_resolves_and_requires_paths() {
        let path = Path::new("../../libs/ixbrl/example_data/example2/input_config.json");
        let empty_env = EnvVars::default();

        // The file parses into a ConfigFile carrying the company block
        // (identity + profile) and the accounts sub-object.
        let file = ConfigFile::from_file(path).expect("parse example config");
        assert_eq!(file.company.name.as_deref(), Some("Example Biz Ltd."));
        assert_eq!(file.company.profile.directors, vec!["A Bloggs", "B Smith", "C Jones"]);
        assert_eq!(file.accounts.period().start, date(2020, 1, 1));
        assert_eq!(file.accounts.period().end, date(2020, 12, 31));
        assert_eq!(file.accounts.report_title, "Unaudited Micro-Entity Accounts");

        // Without the CLI paths, resolution errors on the first missing one.
        let cli = CliArgs {
            config_path: path.to_path_buf(),
            accounts_made_up_to: None,
            book_path: None,
            out_dir: None,
        };
        let err = ConfigBuilder { env: empty_env.clone(), file: file.clone(), cli }
            .build()
            .await
            .expect_err("missing paths must error");
        let msg = format!("{err:#}");
        assert!(msg.contains("--book"), "{msg}");
        assert!(!msg.contains("--out"), "resolution fails fast on the first missing path: {msg}");

        // With only one path present, the error names the missing one.
        let cli = CliArgs {
            config_path: path.to_path_buf(),
            accounts_made_up_to: None,
            book_path: Some(PathBuf::from("input.gnucash")),
            out_dir: None,
        };
        let err = ConfigBuilder { env: empty_env.clone(), file: file.clone(), cli }
            .build()
            .await
            .expect_err("missing --out must error");
        let msg = format!("{err:#}");
        assert!(msg.contains("--out"), "{msg}");
        assert!(!msg.contains("--book"), "the present path must not be reported: {msg}");

        // With both, resolution succeeds and the resolved inputs carry the
        // company, metadata and paths through.
        let cli = CliArgs {
            config_path: path.to_path_buf(),
            accounts_made_up_to: None,
            book_path: Some(PathBuf::from("input.gnucash")),
            out_dir: Some(PathBuf::from("out")),
        };
        let resolved = ConfigBuilder { env: empty_env.clone(), file: file.clone(), cli }
            .build()
            .await
            .expect("resolves with paths");
        assert_eq!(resolved.company.name, "Example Biz Ltd.");
        assert_eq!(resolved.company.tax_reference, "8596148860");
        assert_eq!(resolved.company.company_number, "12345678");
        assert_eq!(resolved.company.registration_date, NaiveDate::default());
        // The accounts sub-object is carried through with the period.
        assert_eq!(resolved.accounts.period().start, date(2020, 1, 1));
        assert_eq!(resolved.accounts.period().end, date(2020, 12, 31));
        assert_eq!(resolved.accounts.fy1_year, 2019);
        assert_eq!(resolved.book_path, PathBuf::from("input.gnucash"));
        assert_eq!(resolved.out_dir, PathBuf::from("out"));
        assert_eq!(resolved.accounts.report_title, "Unaudited Micro-Entity Accounts");
        // The company profile is carried through from the config file.
        assert_eq!(resolved.profile.directors, vec!["A Bloggs", "B Smith", "C Jones"]);
        // The empty captured env configures no Companies House client.
        assert_eq!(resolved.companies_house_client_type, None);

        // With a sandbox key in the captured env, the resolved inputs report
        // it.
        let cli = CliArgs {
            config_path: path.to_path_buf(),
            accounts_made_up_to: None,
            book_path: Some(PathBuf::from("input.gnucash")),
            out_dir: Some(PathBuf::from("out")),
        };
        let resolved = ConfigBuilder {
            env: env(None, None, None, Some("sandbox-key")),
            file: file.clone(),
            cli,
        }
        .build()
        .await
        .expect("resolves with paths");
        assert_eq!(resolved.companies_house_client_type, Some(CompaniesHouseClientType::Sandbox));
    }

    /// The return period deduced from a made-up-to date, entirely offline:
    /// the 12 months ending on the date, with the flag winning over the
    /// config's `accounts.accounts_made_up_to`, and an explicit
    /// `accounts.period` still winning over both.
    #[tokio::test]
    async fn made_up_to_deduces_the_return_period() {
        let empty_env = env(None, None, None, None);
        let identity = company_config(
            Some("Example Biz Ltd."),
            Some("8596148860"),
            Some("12345678"),
        );

        // The config's `accounts.accounts_made_up_to`: the 12 months ending
        // on it.
        let accounts = builder(
            empty_env.clone(),
            identity.clone(),
            accounts_meta(false, Some(date(2020, 12, 31))),
        )
        .build()
        .await
        .expect("resolves")
        .accounts;
        assert_eq!(accounts.period().start, date(2020, 1, 1));
        assert_eq!(accounts.period().end, date(2020, 12, 31));

        // The flag (the override) wins over the config's date.
        let file = ConfigFile {
            company: identity.clone(),
            accounts: accounts_meta(false, Some(date(2020, 12, 31))),
        };
        let cli = CliArgs {
            config_path: PathBuf::from("test.json"),
            accounts_made_up_to: Some(date(2021, 3, 31)),
            book_path: Some(PathBuf::from("book.gnucash")),
            out_dir: Some(PathBuf::from("out")),
        };
        let accounts = ConfigBuilder {
            env: empty_env.clone(),
            file,
            cli,
        }
        .build()
        .await
        .expect("resolves")
        .accounts;
        assert_eq!(accounts.period().start, date(2020, 4, 1));
        assert_eq!(accounts.period().end, date(2021, 3, 31));

        // An explicit `accounts.period` still wins over the made-up-to date
        // (config or flag).
        let file = ConfigFile {
            company: identity,
            accounts: accounts_meta(true, Some(date(2020, 12, 31))),
        };
        let cli = CliArgs {
            config_path: PathBuf::from("test.json"),
            accounts_made_up_to: Some(date(2021, 3, 31)),
            book_path: Some(PathBuf::from("book.gnucash")),
            out_dir: Some(PathBuf::from("out")),
        };
        let accounts = ConfigBuilder {
            env: empty_env,
            file,
            cli,
        }
        .build()
        .await
        .expect("resolves")
        .accounts;
        assert_eq!(accounts.period().start, date(2020, 1, 1));
        assert_eq!(accounts.period().end, date(2020, 12, 31));
    }

    /// Without any period input and no API key, the error names the period
    /// and the options for resolving it.
    #[tokio::test]
    async fn missing_period_without_api_key_errors_with_options() {
        let empty_env = env(None, None, None, None);
        let raw = company_config(Some("Example Biz Ltd."), Some("8596148860"), Some("12345678"));
        let err = builder(empty_env, raw, accounts_meta(false, None))
            .build()
            .await
            .expect_err("the missing period must error");
        let msg = format!("{err:#}");
        assert!(msg.contains("accounts.period"), "{msg}");
        assert!(msg.contains("accounts.accounts_made_up_to"), "{msg}");
        assert!(msg.contains("COMPANIES_HOUSE_API_KEY"), "{msg}");
    }

    /// With an API key but no company number, resolution errors on the
    /// missing number first — it is needed for every Companies House lookup
    /// (the name and the return period both fall back on it).
    #[tokio::test]
    async fn missing_number_with_api_key_errors() {
        let env = env(None, None, Some("test-key"), None);
        let raw = company_config(Some("Example Biz Ltd."), Some("8596148860"), None);
        let err = builder(env, raw, accounts_meta(false, None))
            .build()
            .await
            .expect_err("the missing number must error");
        let msg = format!("{err:#}");
        assert!(msg.contains("company.company_number"), "{msg}");
        assert!(msg.contains("COMPANY_NUMBER"), "{msg}");
    }

    /// Fail-fast order across concerns: with an incomplete identity *and*
    /// missing CLI paths, the identity field errors first (the number is
    /// resolved before the paths), not the other way round.
    #[tokio::test]
    async fn identity_errors_before_paths() {
        let cli = CliArgs {
            config_path: PathBuf::from("test.json"),
            accounts_made_up_to: None,
            book_path: None,
            out_dir: None,
        };
        let err = ConfigBuilder {
            env: EnvVars::default(),
            file: ConfigFile {
                company: company_config(None, None, None),
                accounts: accounts_meta(false, None),
            },
            cli,
        }
        .build()
        .await
        .expect_err("the identity error surfaces first");
        let msg = format!("{err:#}");
        assert!(msg.contains("company.company_number"), "{msg}");
        assert!(!msg.contains("--book"), "the identity is resolved before the paths: {msg}");
    }

    /// With an API key, a company number and a cached profile, the return
    /// period defaults to the profile's next accounting period — no network
    /// access (the profile is seeded into a scratch cache directory).
    #[tokio::test]
    async fn missing_period_defaults_to_companies_house_next_accounting_period() {
        let cache_dir = std::env::temp_dir().join(format!(
            "tally-cli-config-ch-default-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&cache_dir).expect("create scratch cache dir");
        std::fs::write(
            cache_dir.join("companies-house-12345678.json"),
            serde_json::json!({
                "company_number": "12345678",
                "company_name": "CACHED CORP LTD",
                "date_of_creation": "2001-01-01",
                "accounts": {
                    "next_accounts": {
                        "period_start_on": "2025-01-01",
                        "period_end_on": "2025-12-31",
                        "due_on": "2026-09-30"
                    }
                }
            })
            .to_string(),
        )
        .expect("seed the profile cache");

        let mut env = env(None, None, Some("test-key"), None);
        env.cache_dir = Some(cache_dir.clone());

        let raw = company_config(Some("Example Biz Ltd."), Some("8596148860"), Some("12345678"));
        let resolved = builder(env, raw, accounts_meta(false, None))
            .build()
            .await
            .expect("resolves from the cached profile");
        let accounts = resolved.accounts;
        assert_eq!(accounts.period().start, date(2025, 1, 1));
        assert_eq!(accounts.period().end, date(2025, 12, 31));
        let company = resolved.company;
        // The profile also supplies the registration date, anchoring the
        // registration-date schedule used as the fallback inside
        // `next_accounting_period`.
        assert_eq!(company.registration_date, date(2001, 1, 1));

        let _ = std::fs::remove_dir_all(&cache_dir);
    }
}

// ============================================================================
// Live enrichment tests (part of the default-enabled `api_tests` feature)
// ============================================================================

/// Serialises the tests that read or mutate the process environment: the
/// live test's `EnvVars::from_env()` and the env-mutation test in [`tests`]
/// below.  Without the lock they race in a parallel run and the live test
/// could capture the fixture values (`live-key` / `12345678` …) and 401
/// against the real API.
#[cfg(test)]
static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// Live Companies House enrichment tests (part of the default-enabled
/// `api_tests` feature).
#[cfg(test)]
mod live_tests {
    use super::*;

    /// Live end-to-end enrichment of the *minimum* config from the Companies
    /// House API, and the cache-first second run.
    ///
    /// With `COMPANIES_HOUSE_API_KEY` (or `COMPANIES_HOUSE_SANDBOX_API_KEY`),
    /// `COMPANY_NUMBER` and `UNIQUE_TAXPAYER_REF` exported, a config with an
    /// empty `company` block is enriched entirely from the company's live
    /// profile: the name and registration date come from the API response,
    /// and the return period defaults to the next accounting period to file.
    /// The fetched profile is cached on disk in a scratch directory, and a
    /// second resolution against that cache — with a placeholder key that
    /// could never fetch the real profile — resolves to the same company,
    /// proving the second run never touches the network.
    #[tokio::test]
    #[cfg_attr(
        not(feature = "api_tests"),
        ignore = "requires a Companies House API key, COMPANY_NUMBER and UNIQUE_TAXPAYER_REF"
    )]
    async fn live_minimal_config_enriched_from_api_and_cached() {
        // The ambient environment is captured exactly as the CLI captures
        // it, with the response cache redirected to a scratch directory the
        // test owns (so the first run really hits the network and the cache
        // state is deterministic).
        let cache_dir = tempfile::tempdir().expect("create the scratch cache dir");
        // Snapshot under the env lock: the env-mutation test in `mod tests`
        // runs in a parallel thread and would otherwise race this read (see
        // `ENV_LOCK`).
        let mut env = {
            let _guard = ENV_LOCK.lock().unwrap();
            EnvVars::from_env()
        };
        env.cache_dir = Some(cache_dir.path().to_path_buf());

        assert!(
            env.companies_house_api_key.is_some() || env.companies_house_sandbox_api_key.is_some(),
            "the api_tests feature needs COMPANIES_HOUSE_API_KEY (live) or \
             COMPANIES_HOUSE_SANDBOX_API_KEY (sandbox)"
        );
        let number = env
            .company_number
            .as_deref()
            .expect("the api_tests feature needs COMPANY_NUMBER: use a real company for the \
                     live API, a sandbox test company for the sandbox API");
        let utr = env
            .unique_taxpayer_ref
            .as_deref()
            .expect("the api_tests feature needs UNIQUE_TAXPAYER_REF (the Corporation Tax \
                     reference is never resolved from Companies House)");

        // The minimum config: an empty `company` block and an empty
        // `accounts` sub-object, nothing to resolve from.
        let file = ConfigFile {
            company: RawCompanyConfig {
                name: None,
                tax_reference: None,
                company_number: None,
                profile: CompanyProfile::default(),
            },
            accounts: AccountsMeta::default(),
        };
        let cli = CliArgs {
            config_path: PathBuf::from("live.json"),
            accounts_made_up_to: None,
            book_path: Some(PathBuf::from("book.gnucash")),
            out_dir: Some(PathBuf::from("out")),
        };

        // Run 1: everything is enriched from the live API response.
        let resolved = ConfigBuilder {
            env: env.clone(),
            file: file.clone(),
            cli: cli.clone(),
        }
        .build()
        .await
        .expect("resolve the minimum config from the live API");
        let company = resolved.company;
        assert_eq!(company.company_number, number);
        assert_eq!(company.tax_reference, utr);
        assert!(!company.name.is_empty(), "the name is filled from the profile");
        let today = chrono::Utc::now().date_naive();
        assert!(
            company.registration_date < today,
            "the registration date comes from the profile (created in the past), got {}",
            company.registration_date
        );
        // The return period defaults to the next accounting period to file
        // and lands on the resolved accounts (a company behind on filings
        // can have a past-due period, so only the ordering is pinned).
        let period = resolved.accounts.period();
        assert!(period.start < period.end, "the return period is ordered");

        // The profile is cached for the next run.
        let cache_file = cache_dir.path().join(format!("companies-house-{number}.json"));
        assert!(cache_file.exists(), "the fetched profile is cached on disk");
        let cached = std::fs::read_to_string(&cache_file).expect("read the cache file");
        assert!(cached.contains("company_name"), "the cache holds the profile JSON");

        // Run 2: the same scratch cache, but a placeholder key that could
        // never fetch the real profile — resolution is served entirely from
        // the cache.
        let mut cached_env = env.clone();
        cached_env.companies_house_api_key = Some("placeholder-cached-run".to_string());
        cached_env.companies_house_sandbox_api_key = None;
        let from_cache = ConfigBuilder {
            env: cached_env,
            file,
            cli,
        }
        .build()
        .await
        .expect("resolve the minimum config from the cache");
        let company2 = from_cache.company;
        assert_eq!(company2.name, company.name);
        assert_eq!(company2.registration_date, company.registration_date);
        let period2 = from_cache.accounts.period();
        assert_eq!(period2.start, period.start, "the return period is the same on the cached run");
        assert_eq!(period2.end, period.end, "the return period is the same on the cached run");
    }
}
