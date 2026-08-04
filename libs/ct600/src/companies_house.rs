//! CT600 adapters over the ixbrl Companies House client.
//!
//! The Companies House client, its layered configuration and the company
//! resolution chain live in `ixbrl::clients::companies_house`.  This module
//! adds the CT600-specific derivations on top: the company header boxes
//! ([`CompanyFormValues`]) from a profile + tax computation, and the
//! enrichment gating ([`CompaniesHouseFormValues`]) for those boxes.

use ixbrl::clients::{ApiResult, CompaniesHouseClient, CompanyProfile, CompanyType};
use ixbrl::reports::uk_frs105_corp_tax::Frs105CorpTax;

use crate::form::CompanyFormValues;

impl CompanyFormValues {
    /// Build the CT600 company header boxes from a Companies House profile
    /// and the tax computation.
    pub(crate) fn from_profile_and_tax(profile: &CompanyProfile, tax: &Frs105CorpTax) -> Self {
        Self {
            company_name: profile.company_name.clone(),
            company_number: profile.company_number.clone(),
            tax_reference: tax.tax_reference().to_string(),
            type_of_company: profile
                .company_type
                .as_deref()
                .and_then(CompanyType::parse_str)
                .map(CompanyType::code)
                .unwrap_or(0),
            start: tax.start(),
            end: tax.end(),
        }
    }
}

/// CT600 company-header enrichment over the ixbrl Companies House client.
///
/// The Companies House lookup is skipped when the caller supplied complete
/// company details (a non-empty name and registration number): the header
/// boxes are then derived from the tax computation alone.  Otherwise — when
/// a company number is configured (the `COMPANY_NUMBER` environment variable)
/// and the company details are absent — the profile is fetched (cache-first,
/// see [`ixbrl::clients::CompaniesHouseClient::get_company_profile_cached`])
/// and boxes 1 (name), 2 (registration number) and 4 (type of company) are
/// enriched from it.  The tax reference (3) and the return period (30/35)
/// always come from the tax computation.
pub trait CompaniesHouseFormValues {
    /// The company header boxes for the tax computation, enriched from
    /// Companies House when the caller's company details are absent.
    async fn company_form_values(&self, tax: &Frs105CorpTax) -> ApiResult<CompanyFormValues>;
}

impl CompaniesHouseFormValues for CompaniesHouseClient {
    async fn company_form_values(&self, tax: &Frs105CorpTax) -> ApiResult<CompanyFormValues> {
        let Some(company_number) = self
            .config()
            .enrichment_number(tax.company_name(), tax.company_number())
        else {
            return Ok(CompanyFormValues::from_tax(tax));
        };
        let profile = self.get_company_profile_cached(&company_number).await?;
        Ok(CompanyFormValues::from_profile_and_tax(&profile, tax))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    use chrono::NaiveDate;
    use ixbrl::clients::test_utils::TestData;
    use ixbrl::ixbrl_fmt::ParsedIxBrlFacts;

    fn seed_cache(cache_dir: &Path, profile: &CompanyProfile) {
        std::fs::create_dir_all(cache_dir).unwrap();
        std::fs::write(
            cache_dir.join(format!("companies-house-{}.json", profile.company_number)),
            serde_json::to_vec(profile).unwrap(),
        )
        .unwrap();
    }

    /// A tax computation with no company name / number, so the enrichment
    /// path of [`CompaniesHouseFormValues::company_form_values`] runs.
    fn tax_without_company_details() -> Frs105CorpTax {
        let company = ixbrl::company::Company::new(
            "",
            "",
            "",
            NaiveDate::from_ymd_opt(2025, 1, 1).unwrap(),
            NaiveDate::from_ymd_opt(2025, 12, 31).unwrap(),
        );
        Frs105CorpTax::from_parsed_facts(&ParsedIxBrlFacts::default(), &company)
    }

    #[test]
    fn test_company_form_values_from_profile_and_tax() {
        let profile = TestData::default_company();
        let tax = TestData::sample_tax();

        let values = CompanyFormValues::from_profile_and_tax(&profile, &tax);

        assert_eq!(values.company_name, profile.company_name);
        assert_eq!(values.company_number, profile.company_number);
        assert_eq!(
            values.type_of_company,
            profile
                .company_type
                .as_deref()
                .and_then(CompanyType::parse_str)
                .map(CompanyType::code)
                .unwrap_or(0),
        );
        assert_eq!(values.tax_reference, tax.tax_reference());
        assert_eq!(values.start, tax.start());
        assert_eq!(values.end, tax.end());
    }

    /// Complete company inputs: the header boxes come from the tax alone and
    /// no Companies House lookup happens (the client is keyless, so any
    /// lookup would attempt the network and fail).
    #[tokio::test]
    async fn company_form_values_complete_inputs_needs_no_lookup() {
        let cache_dir = tempfile::tempdir().unwrap();
        let client = CompaniesHouseClient::new(
            ixbrl::clients::Config::default().with_cache_dir(cache_dir.path()),
        );
        let tax = TestData::sample_tax();

        let values = client
            .company_form_values(&tax)
            .await
            .expect("no lookup needed");

        assert_eq!(values.company_name, "Acme Ltd");
        assert_eq!(values.company_number, "9876543");
        assert_eq!(values.tax_reference, "1234567890");
    }

    /// Absent company inputs + configured number: enriched from the cached
    /// profile (cache-first, no network).
    #[tokio::test]
    async fn company_form_values_enriches_from_cached_profile() {
        let cache_dir = tempfile::tempdir().unwrap();
        let profile = TestData::sample_company();
        seed_cache(cache_dir.path(), &profile);
        let client = CompaniesHouseClient::new(
            ixbrl::clients::Config::default()
                .with_company_number(TestData::sample_company_number())
                .with_cache_dir(cache_dir.path()),
        );
        let tax = tax_without_company_details();

        let values = client
            .company_form_values(&tax)
            .await
            .expect("enrich from cache");

        assert_eq!(values.company_name, profile.company_name);
        assert_eq!(values.company_number, profile.company_number);
        // The tax reference and period always come from the tax computation.
        assert_eq!(values.tax_reference, tax.tax_reference());
        assert_eq!(values.start, tax.start());
        assert_eq!(values.end, tax.end());
    }
}
