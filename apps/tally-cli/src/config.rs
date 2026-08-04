//! Configuration for the `tally` CLI.
//!
//! Configuration is split across the input sources, each captured in its own
//! permissive type, and merged by the per-subcommand resolver into a single
//! strict [`ResolvedConfig`]:
//!
//! * [`RawEnvConfig`] — the environment, captured once
//!   ([`RawEnvConfig::from_env`]): `UNIQUE_TAXPAYER_REF`, `COMPANY_NUMBER`,
//!   the Companies House API keys, `CT600_CACHE_DIR`.
//! * [`FileConfig`] — the JSON config file (`--config-path`): the nested
//!   `company` identity block plus the flat accounts-metadata fields.
//! * [`Ct600Config`] — the `ct600` subcommand's inputs: `--config-path`,
//!   `--book`, `--out`.  Its [`Ct600Config::resolve`] loads the
//!   [`FileConfig`], merges it with a [`RawEnvConfig`] and these CLI values,
//!   and enriches the result.  (Each future subcommand gets its own struct
//!   and resolver here.)
//! * [`ResolvedConfig`] — strict.  Every field is present at this point, or
//!   resolution has already returned an error explaining exactly which input
//!   was still missing and how to provide it.
//!
//! The company-identity fields are optional because each has an alternative
//! source to fall back on — the environment, Companies House (the name and
//! registration date), or defaults ([`Company::new`]); the remaining fields
//! have no alternative source and stay required.
//!
//! Resolution order, field by field:
//!
//! | Field | Sources (priority order) |
//! |-------|--------------------------|
//! | `company.name` | config file → Companies House (needs an API key + company number) |
//! | `company.company_number` | config file → `COMPANY_NUMBER` (env) |
//! | `company.tax_reference` (UTR) | `UNIQUE_TAXPAYER_REF` (env) → config file |
//! | `company.accounting_period_start` / `end` | config file only (Companies House cannot provide them) |
//! | `company.registration_date` | Companies House (only when the config carries no identity at all) → [`Company::new`] default (period start) |
//! | Companies House layer | `COMPANIES_HOUSE_API_KEY` (live) / `COMPANIES_HOUSE_API_KEY_TEST` (sandbox); response cache in `CT600_CACHE_DIR` (env) |
//! | `metadata.*` | config file only |
//! | `--book`, `--out` | command line only |

use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use chrono::NaiveDate;
use ixbrl::clients::{CompaniesHouseClient, CompaniesHouseClientType};
use ixbrl::company::Company;
use ixbrl::reports::uk_frs105_accounts::AccountsMetadata;
use serde::Deserialize;

/// The values `tally` reads from the environment.
///
/// Captured once (via [`RawEnvConfig::from_env`]) into the raw config so
/// resolution is a pure merge of the config file, the environment and the
/// CLI.  Empty environment variables count as unset, matching the libraries'
/// conventions.
#[derive(Debug, Clone, Default)]
pub struct RawEnvConfig {
    /// `UNIQUE_TAXPAYER_REF` — the Corporation Tax reference (UTR); wins
    /// over the config file's `company.tax_reference`.
    pub unique_taxpayer_ref: Option<String>,
    /// `COMPANY_NUMBER` — fallback company number when the config file's
    /// `company.company_number` is absent.
    pub company_number: Option<String>,
    /// `COMPANIES_HOUSE_API_KEY` — the live Companies House API key.
    pub companies_house_api_key: Option<String>,
    /// `COMPANIES_HOUSE_API_KEY_TEST` — the sandbox Companies House API key.
    pub companies_house_api_key_test: Option<String>,
    /// `CT600_CACHE_DIR` — the Companies House response-cache directory.
    pub cache_dir: Option<PathBuf>,
}

impl RawEnvConfig {
    /// Snapshot the environment: every non-empty matching variable is
    /// captured (empty variables count as unset).
    pub fn from_env() -> Self {
        Self {
            unique_taxpayer_ref: non_empty_env("UNIQUE_TAXPAYER_REF"),
            company_number: non_empty_env("COMPANY_NUMBER"),
            companies_house_api_key: non_empty_env("COMPANIES_HOUSE_API_KEY"),
            companies_house_api_key_test: non_empty_env("COMPANIES_HOUSE_API_KEY_TEST"),
            cache_dir: non_empty_env("CT600_CACHE_DIR").map(PathBuf::from),
        }
    }

    /// Which Companies House API the config will use: the live key
    /// (`COMPANIES_HOUSE_API_KEY`) preferred over the sandbox key
    /// (`COMPANIES_HOUSE_API_KEY_TEST`), matching
    /// `ixbrl::clients::Config::from_env`.  `None` when no key is set.
    pub fn companies_house_client_type(&self) -> Option<CompaniesHouseClientType> {
        if self.companies_house_api_key.is_some() {
            Some(CompaniesHouseClientType::Live)
        } else if self.companies_house_api_key_test.is_some() {
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
    pub fn ch_config(&self) -> ixbrl::clients::Config {
        // `Config::default()` carries no API layer; the company number and
        // cache dir are overridden with the captured values so the ambient
        // environment cannot leak in.
        let mut config = ixbrl::clients::Config::default()
            .with_company_number(self.company_number.clone().unwrap_or_default());
        if let Some(cache_dir) = &self.cache_dir {
            config = config.with_cache_dir(cache_dir.clone());
        }
        if let Some(client_type) = self.companies_house_client_type() {
            let api_key = match client_type {
                CompaniesHouseClientType::Live => self.companies_house_api_key.as_deref(),
                CompaniesHouseClientType::Sandbox => self.companies_house_api_key_test.as_deref(),
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
/// identity block plus the flat accounts-metadata fields.
///
/// Every field of `company` is optional — the company name is resolved from
/// Companies House at runtime when an API key is configured, the company
/// number falls back on the `COMPANY_NUMBER` environment variable, and the
/// Corporation Tax reference (UTR) falls back on the `UNIQUE_TAXPAYER_REF`
/// environment variable (which wins over `company.tax_reference`).  The
/// accounting-period dates cannot come from Companies House and must be
/// present in the config file.
#[derive(Deserialize)]
pub struct FileConfig {
    /// The config file's company identity block; every field optional.
    pub company: RawCompanyConfig,
    /// The config file's flat accounts-metadata fields (all required).
    #[serde(flatten)]
    pub metadata: AccountsMetadata,
}

impl FileConfig {
    /// Parse the config file at `path` (`--config-path`).
    pub fn from_file(path: &Path) -> Result<Self> {
        let data = std::fs::read_to_string(path)
            .with_context(|| format!("read config '{}'", path.display()))?;
        serde_json::from_str(&data)
            .with_context(|| format!("parse config '{}'", path.display()))
    }
}

/// The company identity block of the config file.
///
/// Every field is optional: the company name is resolved from Companies
/// House at runtime when an API key is configured, the company number falls
/// back on the `COMPANY_NUMBER` environment variable, and the Corporation
/// Tax reference (UTR) falls back on the `UNIQUE_TAXPAYER_REF` environment
/// variable (which wins over `company.tax_reference`).  The accounting-period
/// dates cannot come from Companies House and must be present in the config
/// file.
///
/// [`resolve_company`] validates the resolved identity and errors clearly
/// when it is incomplete.
#[derive(Deserialize)]
pub struct RawCompanyConfig {
    pub name: Option<String>,
    pub tax_reference: Option<String>,
    pub company_number: Option<String>,
    pub accounting_period_start: Option<NaiveDate>,
    pub accounting_period_end: Option<NaiveDate>,
}

/// Resolved (strict) configuration: every field is present, or resolution
/// errored with an explanation of the input that was still missing.
#[derive(Debug)]
pub struct ResolvedConfig {
    /// The fully-resolved company: identity, registration date and return
    /// period.
    pub company: Company,
    /// The accounts metadata for the report.
    pub metadata: AccountsMetadata,
    /// The GnuCash ledger to read.
    pub book_path: PathBuf,
    /// The output directory; the CT600 GovTalk message is written to
    /// `<out>/ct600.xml`.
    pub out_dir: PathBuf,
    /// Which Companies House API the config will use: live or sandbox,
    /// depending on which key is set; `None` when no key is configured.
    pub companies_house_client_type: Option<CompaniesHouseClientType>,
}

/// The `ct600` subcommand's inputs, before resolution.
///
/// [`Self::resolve`] loads the [`FileConfig`] pointed at by `config_path`,
/// merges it with the captured [`RawEnvConfig`] and these CLI values, and
/// enriches the result into a strict [`ResolvedConfig`].  (Each future
/// subcommand gets its own struct and resolver here.)
pub struct Ct600Config {
    /// The `--config-path` value: the JSON config file to load.
    pub config_path: PathBuf,
    /// The `--book` value (required; no other source).
    pub book_path: Option<PathBuf>,
    /// The `--out` value (required; no other source).
    pub out_dir: Option<PathBuf>,
}

impl Ct600Config {
    /// Parse and enrich this subcommand's inputs into a [`ResolvedConfig`].
    ///
    /// Resolution merges the three sources — the config file, the captured
    /// environment, and the CLI paths — and errors listing *every*
    /// still-missing input (company identity and CLI paths) and how to
    /// resolve it.
    pub async fn resolve(self, env: &RawEnvConfig) -> Result<ResolvedConfig> {
        let file = FileConfig::from_file(&self.config_path)?;
        let resolved = resolve_company_inputs(&file.company, env).await?;

        // The CLI paths have no alternative source; collect them alongside the
        // missing company identity so the error names everything in one go.
        let mut missing = resolved.missing;
        if self.book_path.is_none() {
            missing.push((
                "--book",
                "the GnuCash ledger cannot be resolved from the environment; \
                 set the --book flag"
                    .to_string(),
            ));
        }
        if self.out_dir.is_none() {
            missing.push((
                "--out",
                "the output directory cannot be resolved from the environment; \
                 set the --out flag"
                    .to_string(),
            ));
        }
        if !missing.is_empty() {
            bail!("{}", format_missing(&self.config_path, &missing));
        }

        Ok(ResolvedConfig {
            company: resolved
                .company
                .expect("no missing inputs means the company is resolved"),
            metadata: file.metadata,
            book_path: self.book_path.expect("book_path validated present"),
            out_dir: self.out_dir.expect("out_dir validated present"),
            companies_house_client_type: env.companies_house_client_type(),
        })
    }
}

/// A still-missing config input, with a per-input reason for the error.
type MissingInputs = Vec<(&'static str, String)>;

/// The company identity, resolved as far as the sources allow.
struct ResolvedCompanyInputs {
    /// The resolved company; present when no identity input is missing.
    company: Option<Company>,
    /// Every identity input still missing, with how to resolve it.
    missing: MissingInputs,
}

/// Resolve the company identity from the config file's `company` block and
/// the captured environment, returning the result together with the inputs
/// the sources could not provide.
///
/// The company number comes from `company.company_number`, falling back on
/// the environment's `COMPANY_NUMBER` ([`RawEnvConfig::company_number`]).
/// When a Companies House API key is configured
/// ([`RawEnvConfig::companies_house_api_key`] / `_test`) and the company name
/// is absent, the profile for that number is fetched (cache-first) and the
/// name and registration date are filled in from it.  A complete identity is
/// never looked up, and with no API key no lookup happens at all.
///
/// The fields Companies House cannot provide — the Corporation Tax reference
/// (UTR) and the accounting-period dates — are always required: the UTR comes
/// from the environment's `UNIQUE_TAXPAYER_REF` (winning) or
/// `company.tax_reference`, and the dates from the config file.
async fn resolve_company_inputs(raw: &RawCompanyConfig, env: &RawEnvConfig) -> Result<ResolvedCompanyInputs> {
    let ch = env.ch_config();
    let api_key_configured = env.api_key_configured();

    // Empty strings count as absent, matching the libraries' company
    // resolution conventions (see `Config::enrichment_number`).
    let name_from_config = raw.name.as_deref().filter(|s| !s.is_empty());
    let number_from_config = raw.company_number.as_deref().filter(|s| !s.is_empty());

    // The Corporation Tax reference (UTR): the captured `UNIQUE_TAXPAYER_REF`
    // wins, falling back on the config file's `company.tax_reference`.
    // (Captured env values are non-empty by construction.)
    let tax_reference = match env.unique_taxpayer_ref.as_deref() {
        Some(value) => Some(value.to_string()),
        _ => raw
            .tax_reference
            .as_deref()
            .filter(|s| !s.is_empty())
            .map(str::to_string),
    };

    // The company number used for resolution: the config's own value wins,
    // otherwise the environment's `COMPANY_NUMBER`.
    let company_number = number_from_config
        .map(str::to_string)
        .or_else(|| ch.company_number().filter(|n| !n.is_empty()).map(str::to_string));

    // Enrich the name from Companies House.  The registration date is only
    // filled when the config carried no identity details at all, so the
    // accounting periods are not skewed by partial inputs (same gating as
    // the libraries' `fill_from_profile`).
    let mut name = name_from_config.map(str::to_string);
    let mut registration_date = None;
    let details_fully_absent = name_from_config.is_none() && number_from_config.is_none();
    if api_key_configured && name.is_none() && let Some(number) = &company_number {
        let client = CompaniesHouseClient::new(ch.with_company_number(number.clone()));
        let profile = client
            .get_company_profile_cached(number)
            .await
            .with_context(|| format!("resolve company '{number}' from Companies House"))?;
        name = Some(profile.company_name.clone());
        if details_fully_absent
            && let Some(created) = profile.date_of_creation.as_deref()
            && let Ok(date) = NaiveDate::parse_from_str(created, "%Y-%m-%d")
        {
            registration_date = Some(date);
        }
    }

    // Collect everything still missing, with a per-field reason.
    let mut missing: MissingInputs = Vec::new();
    if name.is_none() {
        missing.push((
            "company.name",
            if api_key_configured {
                "no company number to resolve it from (set company.company_number or COMPANY_NUMBER)"
                    .to_string()
            } else {
                "no Companies House API key is set, so it cannot be resolved from Companies House \
                 (set COMPANIES_HOUSE_API_KEY or COMPANIES_HOUSE_API_KEY_TEST), or add \
                 company.name to the config file"
                    .to_string()
            },
        ));
    }
    if company_number.is_none() {
        missing.push((
            "company.company_number",
            "set it in the config file or in the COMPANY_NUMBER environment variable".to_string(),
        ));
    }
    if tax_reference.is_none() {
        missing.push((
            "company.tax_reference",
            "the Corporation Tax reference (UTR) cannot be resolved from Companies House; \
             set the UNIQUE_TAXPAYER_REF environment variable or add company.tax_reference \
             to the config file"
                .to_string(),
        ));
    }
    if raw.accounting_period_start.is_none() {
        missing.push((
            "company.accounting_period_start",
            "the return period cannot be resolved from Companies House; add it to the config file"
                .to_string(),
        ));
    }
    if raw.accounting_period_end.is_none() {
        missing.push((
            "company.accounting_period_end",
            "the return period cannot be resolved from Companies House; add it to the config file"
                .to_string(),
        ));
    }

    let company = if missing.is_empty() {
        let mut company = Company::new(
            name.expect("name validated present"),
            tax_reference.expect("tax_reference validated present"),
            company_number.expect("company_number validated present"),
            raw.accounting_period_start
                .expect("accounting_period_start validated present"),
            raw.accounting_period_end
                .expect("accounting_period_end validated present"),
        );
        if let Some(date) = registration_date {
            company.registration_date = date;
        }
        Some(company)
    } else {
        None
    };

    Ok(ResolvedCompanyInputs { company, missing })
}

/// Resolve the company identity, erroring with every missing input listed.
///
/// Test-facing convenience: [`Ct600Config::resolve`] gathers the missing
/// inputs itself so it can report the CLI paths alongside them.
#[cfg(test)]
async fn resolve_company(
    config_path: &Path,
    raw: &RawCompanyConfig,
    env: &RawEnvConfig,
) -> Result<Company> {
    let resolved = resolve_company_inputs(raw, env).await?;
    if !resolved.missing.is_empty() {
        bail!("{}", format_missing(config_path, &resolved.missing));
    }
    Ok(resolved.company.expect("no missing inputs means the company is resolved"))
}

/// Format the missing-inputs list as a single error message naming each one.
fn format_missing(config_path: &Path, missing: &MissingInputs) -> String {
    let mut message = format!(
        "cannot resolve the config from '{}': {} input{} still missing",
        config_path.display(),
        missing.len(),
        if missing.len() == 1 { " is" } else { "s are" },
    );
    for (field, reason) in missing {
        message.push_str(&format!("\n  - {field}: {reason}"));
    }
    message
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
        with_period: bool,
    ) -> RawCompanyConfig {
        RawCompanyConfig {
            name: name.map(str::to_string),
            tax_reference: tax_reference.map(str::to_string),
            company_number: company_number.map(str::to_string),
            accounting_period_start: with_period.then(|| date(2020, 1, 1)),
            accounting_period_end: with_period.then(|| date(2020, 12, 31)),
        }
    }

    fn env(
        utr: Option<&str>,
        company_number: Option<&str>,
        api_key: Option<&str>,
        api_key_test: Option<&str>,
    ) -> RawEnvConfig {
        RawEnvConfig {
            unique_taxpayer_ref: utr.map(str::to_string),
            company_number: company_number.map(str::to_string),
            companies_house_api_key: api_key.map(str::to_string),
            companies_house_api_key_test: api_key_test.map(str::to_string),
            cache_dir: None,
        }
    }

    /// The resolution paths that never touch the network.  The environment
    /// is passed explicitly as a [`RawEnvConfig`], so no environment-variable
    /// mutation (and no contention between parallel tests) is involved: a
    /// complete config resolves (with or without an API key — a complete
    /// identity is never looked up), and an incomplete config without an API
    /// key errors with a message listing every missing field and how to
    /// resolve it.
    #[tokio::test]
    async fn resolve_company_offline_paths() {
        let empty_env = env(None, None, None, None);

        // Complete config: resolves with no lookup.
        let complete = company_config(
            Some("Example Biz Ltd."),
            Some("8596148860"),
            Some("12345678"),
            true,
        );
        let company = resolve_company(Path::new("test.json"), &complete, &empty_env)
            .await
            .expect("complete config resolves");
        assert_eq!(company.name, "Example Biz Ltd.");
        assert_eq!(company.tax_reference, "8596148860");
        assert_eq!(company.company_number, "12345678");
        // No Companies House lookup happened, so the registration date is
        // the accounting-period start (Company::new's default).
        assert_eq!(company.registration_date, date(2020, 1, 1));

        // Incomplete config without an API key: clear error naming every
        // missing field and the key needed to resolve them.
        let incomplete = company_config(None, None, None, false);
        let err = resolve_company(Path::new("test.json"), &incomplete, &empty_env)
            .await
            .expect_err("incomplete config must error");
        let msg = format!("{err:#}");
        for field in [
            "company.name",
            "company.tax_reference",
            "company.company_number",
            "company.accounting_period_start",
            "company.accounting_period_end",
        ] {
            assert!(msg.contains(field), "error should mention {field}: {msg}");
        }
        assert!(msg.contains("COMPANIES_HOUSE_API_KEY"));
        assert!(msg.contains("COMPANY_NUMBER"));
        assert!(msg.contains("UNIQUE_TAXPAYER_REF"));

        // Empty-string fields count as absent (matching the libraries'
        // resolution conventions): an empty name or UTR still errors.
        let empty_name = company_config(Some(""), Some("8596148860"), Some("12345678"), true);
        let err = resolve_company(Path::new("test.json"), &empty_name, &empty_env)
            .await
            .expect_err("an empty company.name must error");
        assert!(format!("{err:#}").contains("company.name"));
        let empty_utr = company_config(Some("Example Biz Ltd."), Some(""), Some("12345678"), true);
        let err = resolve_company(Path::new("test.json"), &empty_utr, &empty_env)
            .await
            .expect_err("an empty company.tax_reference must error");
        assert!(format!("{err:#}").contains("company.tax_reference"));

        // The captured `UNIQUE_TAXPAYER_REF` wins over the config's
        // `company.tax_reference`.
        let company = resolve_company(
            Path::new("test.json"),
            &complete,
            &env(Some("1111111111"), None, None, None),
        )
        .await
        .expect("the env UTR wins over the config's");
        assert_eq!(company.tax_reference, "1111111111");

        // No UTR anywhere (no env value, no config field): the error names
        // `UNIQUE_TAXPAYER_REF` and the config fallback.
        let no_utr = company_config(Some("Example Biz Ltd."), None, Some("12345678"), true);
        let err = resolve_company(Path::new("test.json"), &no_utr, &empty_env)
            .await
            .expect_err("a missing UTR must error");
        let msg = format!("{err:#}");
        assert!(msg.contains("company.tax_reference"));
        assert!(msg.contains("UNIQUE_TAXPAYER_REF"));

        // The captured `COMPANY_NUMBER` fills an absent config number.
        let no_number = company_config(Some("Example Biz Ltd."), Some("8596148860"), None, true);
        let company = resolve_company(
            Path::new("test.json"),
            &no_number,
            &env(None, Some("12345678"), None, None),
        )
        .await
        .expect("the env company number fills the absent config's");
        assert_eq!(company.company_number, "12345678");

        // A config number wins over the captured `COMPANY_NUMBER`.
        let company = resolve_company(
            Path::new("test.json"),
            &complete,
            &env(None, Some("99999999"), None, None),
        )
        .await
        .expect("the config number wins over the env's");
        assert_eq!(company.company_number, "12345678");

        // An API key set with a *complete* config: still no lookup (the
        // name is present, so nothing needs fetching) and no network.
        let company = resolve_company(
            Path::new("test.json"),
            &complete,
            &env(None, None, Some("test-key"), None),
        )
        .await
        .expect("complete config resolves even with an API key set");
        assert_eq!(company.name, "Example Biz Ltd.");
    }

    /// `RawEnvConfig::from_env` snapshots every non-empty matching variable
    /// (empty variables count as unset), and the live API key is preferred
    /// over the sandbox key.  The only test that touches the environment;
    /// the resolution tests above pass the env in explicitly.
    #[test]
    fn env_snapshot_captures_non_empty_variables() {
        unsafe {
            std::env::set_var("UNIQUE_TAXPAYER_REF", "1111111111");
            std::env::set_var("COMPANY_NUMBER", "12345678");
            std::env::set_var("COMPANIES_HOUSE_API_KEY", "live-key");
            std::env::set_var("COMPANIES_HOUSE_API_KEY_TEST", "sandbox-key");
            std::env::set_var("CT600_CACHE_DIR", "/tmp/ch-cache");
        }
        let env = RawEnvConfig::from_env();
        assert_eq!(env.unique_taxpayer_ref.as_deref(), Some("1111111111"));
        assert_eq!(env.company_number.as_deref(), Some("12345678"));
        assert_eq!(env.companies_house_api_key.as_deref(), Some("live-key"));
        assert_eq!(env.companies_house_api_key_test.as_deref(), Some("sandbox-key"));
        assert_eq!(env.cache_dir.as_deref(), Some(Path::new("/tmp/ch-cache")));
        assert!(env.api_key_configured());
        // Both keys set: the live key is preferred.
        assert_eq!(env.companies_house_client_type(), Some(CompaniesHouseClientType::Live));

        // An empty variable counts as unset.
        unsafe {
            std::env::set_var("UNIQUE_TAXPAYER_REF", "");
            std::env::set_var("COMPANIES_HOUSE_API_KEY", "");
            std::env::remove_var("COMPANIES_HOUSE_API_KEY_TEST");
        }
        let env = RawEnvConfig::from_env();
        assert_eq!(env.unique_taxpayer_ref, None);
        assert_eq!(env.companies_house_api_key, None);
        assert_eq!(env.companies_house_api_key_test, None);
        assert!(!env.api_key_configured());
        assert_eq!(env.companies_house_client_type(), None);

        // Only the sandbox key: Sandbox.
        unsafe {
            std::env::set_var("COMPANIES_HOUSE_API_KEY_TEST", "sandbox-key");
        }
        let env = RawEnvConfig::from_env();
        assert_eq!(env.companies_house_client_type(), Some(CompaniesHouseClientType::Sandbox));
        unsafe {
            std::env::remove_var("COMPANIES_HOUSE_API_KEY_TEST");
        }

        unsafe {
            std::env::remove_var("UNIQUE_TAXPAYER_REF");
            std::env::remove_var("COMPANY_NUMBER");
            std::env::remove_var("COMPANIES_HOUSE_API_KEY");
            std::env::remove_var("COMPANIES_HOUSE_API_KEY_TEST");
            std::env::remove_var("CT600_CACHE_DIR");
        }
    }

    /// End-to-end through the per-source types: the example config file
    /// parses into a [`FileConfig`], and a [`Ct600Config`] resolves it
    /// (with an explicit [`RawEnvConfig`], so the test never depends on the
    /// ambient environment) — erroring clearly while the CLI paths are
    /// missing, and succeeding — with everything carried through — once they
    /// are present.
    #[tokio::test]
    async fn ct600_config_resolves_and_requires_paths() {
        let path = Path::new("../../libs/ixbrl/example_data/example2/input-company.json");
        let empty_env = RawEnvConfig::default();

        // The file parses into a FileConfig carrying the company block and
        // the flattened accounts metadata.
        let file = FileConfig::from_file(path).expect("parse example config");
        assert_eq!(file.company.name.as_deref(), Some("Example Biz Ltd."));
        assert_eq!(file.metadata.report_title, "Unaudited Micro-Entity Accounts");

        // Without the CLI paths, resolution errors naming each missing one.
        let ct600 = Ct600Config {
            config_path: path.to_path_buf(),
            book_path: None,
            out_dir: None,
        };
        let err = ct600.resolve(&empty_env).await.expect_err("missing paths must error");
        let msg = format!("{err:#}");
        assert!(msg.contains("--book"), "{msg}");
        assert!(msg.contains("--out"), "{msg}");

        // With only one path present, the error names just the missing one.
        let ct600 = Ct600Config {
            config_path: path.to_path_buf(),
            book_path: Some(PathBuf::from("input.gnucash")),
            out_dir: None,
        };
        let err = ct600.resolve(&empty_env).await.expect_err("missing --out must error");
        let msg = format!("{err:#}");
        assert!(msg.contains("--out"), "{msg}");
        assert!(!msg.contains("--book"), "the present path must not be reported: {msg}");

        // With both, resolution succeeds and the resolved config carries the
        // company, metadata and paths through.
        let ct600 = Ct600Config {
            config_path: path.to_path_buf(),
            book_path: Some(PathBuf::from("input.gnucash")),
            out_dir: Some(PathBuf::from("out")),
        };
        let resolved = ct600.resolve(&empty_env).await.expect("resolves with paths");
        assert_eq!(resolved.company.name, "Example Biz Ltd.");
        assert_eq!(resolved.company.tax_reference, "8596148860");
        assert_eq!(resolved.company.company_number, "12345678");
        assert_eq!(resolved.company.registration_date, date(2020, 1, 1));
        assert_eq!(resolved.book_path, PathBuf::from("input.gnucash"));
        assert_eq!(resolved.out_dir, PathBuf::from("out"));
        assert_eq!(resolved.metadata.report_title, "Unaudited Micro-Entity Accounts");
        // The empty captured env configures no Companies House client.
        assert_eq!(resolved.companies_house_client_type, None);

        // With a sandbox key in the captured env, the resolved config
        // reports it.
        let ct600 = Ct600Config {
            config_path: path.to_path_buf(),
            book_path: Some(PathBuf::from("input.gnucash")),
            out_dir: Some(PathBuf::from("out")),
        };
        let resolved = ct600
            .resolve(&env(None, None, None, Some("sandbox-key")))
            .await
            .expect("resolves with paths");
        assert_eq!(resolved.companies_house_client_type, Some(CompaniesHouseClientType::Sandbox));
    }
}
