//! Corporation-tax calculation regimes, selected by financial year.
//!
//! The rules changed on **1 April 2023** (the start of FY2023/24): before
//! that every company paid a flat rate (19% from FY2017/18 onwards, which
//! this tool targets); since then a company with profits at
//! or below the lower limit pays the small-profits rate (19%), a company
//! with profits above the upper limit pays the main rate (25%), and a
//! company in between pays tax at the main rate less a marginal relief
//! that tapers the effective rate between the two.
//!
//! A return's accounting period usually spans two financial years.  The
//! profits *and* the limits are time-apportioned between the years (by
//! days in each), so each year's calculation runs independently:
//! [`CorpTaxCalc::tax`] takes the apportioned profit and a `limit_scale`
//! (the fraction of the accounting period falling in that financial year)
//! that scales the limits.  The profit of one financial year does **not**
//! reduce the other's thresholds.
//!
//! Sources: the rates, limits and the 3/200 fraction are set out in
//! [HMRC CTM03910](https://www.gov.uk/hmrc-internal-manuals/company-taxation-manual/ctm03910);
//! the time-apportionment of profits between financial years follows CTA
//! 2010 s.8; the proportional reduction of the limits for a period
//! straddling two financial years is described in
//! [HMRC CTM03955](https://www.gov.uk/hmrc-internal-manuals/company-taxation-manual/ctm03955).

/// The outcome of a single tax calculation: the taxable profit, the tax at
/// the main rate, the marginal relief (zero for the flat-rate regime), the
/// resulting corporation tax and the effective rate.  The per-threshold
/// values are kept so the report's worksheets can show the calculation.
#[derive(Debug, Clone, PartialEq)]
pub struct CorporationTaxCalculation {
    pub taxable_profit: f64,
    pub tax_at_main_rate: f64,
    pub marginal_relief: f64,
    pub corporation_tax: f64,
    pub effective_rate: f64,
}

/// A corporation-tax calculation regime.
///
/// Object-safe, so a return can hold the old and the new calculation
/// interchangeably.  The regime is a pure function of the financial year
/// ([`for_fy`]), so it never needs to be stored or serialised — the report
/// resolves the regime for each year and keeps only the computed
/// [`CorporationTaxCalculation`] results.
pub trait CorpTaxCalc {
    /// A short display name, e.g. `"flat 19%"` or `"marginal relief"`.
    fn name(&self) -> String;
    /// The rate of tax for a profit: the flat rate for [`FlatRate`], the
    /// effective computed rate for [`MarginalRelief`].  `limit_scale`
    /// scales the limits; the flat-rate regime ignores it.
    fn rate(&self, profit: f64, limit_scale: f64) -> f64;
    /// The full calculation for an apportioned profit, with the threshold
    /// breakdown.  `limit_scale` scales the limits (the fraction of the
    /// accounting period in this financial year); the flat-rate regime
    /// ignores it.
    fn tax(&self, profit: f64, limit_scale: f64) -> CorporationTaxCalculation;
}

/// The old calculation: a flat rate of tax on all profits (19% for
/// FY2022/23 and earlier).
#[derive(Debug, Clone, PartialEq)]
pub struct FlatRate {
    /// The rate, in percent (e.g. `19.0`).
    pub rate: f64,
}

impl FlatRate {
    pub fn new(rate: f64) -> Self {
        Self { rate }
    }
}

impl CorpTaxCalc for FlatRate {
    fn name(&self) -> String {
        format!("flat {}%", self.rate)
    }

    fn rate(&self, _profit: f64, _limit_scale: f64) -> f64 {
        self.rate
    }

    fn tax(&self, profit: f64, _limit_scale: f64) -> CorporationTaxCalculation {
        let corporation_tax = round2(profit * self.rate / 100.0);
        CorporationTaxCalculation {
            taxable_profit: profit,
            // With no marginal relief the tax at the "main rate" is the
            // whole tax.
            tax_at_main_rate: corporation_tax,
            marginal_relief: 0.0,
            corporation_tax,
            effective_rate: self.rate,
        }
    }
}

/// The current calculation (FY2023/24 onwards): a small-profits rate up to
/// the lower limit, a marginal-relief band between the limits, and the main
/// rate above the upper limit (CTA 2010 s.18B; [HMRC CTM03910](https://www.gov.uk/hmrc-internal-manuals/company-taxation-manual/ctm03910)).
///
/// Marginal relief: `tax = profit × main rate − (upper limit − profit) ×
/// relief fraction`.  The limits are shared between the financial years of
/// a return and time-apportioned per year through the `limit_scale`
/// argument of [`CorpTaxCalc::tax`] ([HMRC CTM03955](https://www.gov.uk/hmrc-internal-manuals/company-taxation-manual/ctm03955)).
#[derive(Debug, Clone, PartialEq)]
pub struct MarginalRelief {
    /// Lower limit, below which the small-profits rate applies (£50,000).
    pub small_profits_limit: f64,
    /// Upper limit, above which the main rate applies (£250,000).
    pub upper_limit: f64,
    /// The main rate, in percent (25).
    pub main_rate: f64,
    /// The small-profits rate, in percent (19).
    pub small_profits_rate: f64,
    /// The marginal-relief fraction (3/200).
    pub marginal_relief_fraction: f64,
}

impl Default for MarginalRelief {
    fn default() -> Self {
        Self {
            small_profits_limit: 50_000.0,
            upper_limit: 250_000.0,
            main_rate: 25.0,
            small_profits_rate: 19.0,
            marginal_relief_fraction: 3.0 / 200.0,
        }
    }
}

impl CorpTaxCalc for MarginalRelief {
    fn name(&self) -> String {
        "marginal relief".to_string()
    }

    fn rate(&self, profit: f64, limit_scale: f64) -> f64 {
        self.tax(profit, limit_scale).effective_rate
    }

    fn tax(&self, profit: f64, limit_scale: f64) -> CorporationTaxCalculation {
        let lower = self.small_profits_limit * limit_scale;
        let upper = self.upper_limit * limit_scale;
        let tax_at_main_rate = round2(profit * self.main_rate / 100.0);

        let marginal_relief = if profit <= lower {
            // At or below the small-profits limit: no marginal relief.
            0.0
        } else if profit <= upper {
            // In the marginal-relief band.
            round2((upper - profit) * self.marginal_relief_fraction)
        } else {
            // Above the upper limit: no marginal relief.
            0.0
        };

        let corporation_tax = if profit <= lower {
            // The small-profits rate applies directly.
            round2(profit * self.small_profits_rate / 100.0)
        } else {
            // The main rate less the marginal relief.
            round2(tax_at_main_rate - marginal_relief)
        };

        let effective_rate = if profit > 0.0 {
            round2((corporation_tax / profit) * 100.0)
        } else {
            0.0
        };

        CorporationTaxCalculation {
            taxable_profit: profit,
            tax_at_main_rate,
            marginal_relief,
            corporation_tax,
            effective_rate,
        }
    }
}

/// The calculation regime for a financial year: the flat rate for FY2022/23
/// and earlier, marginal relief from FY2023/24 onwards (the main rate rose
/// to 25% and marginal relief was reintroduced on 1 April 2023).
pub fn for_fy(fy: i32) -> Box<dyn CorpTaxCalc> {
    if fy <= 2022 {
        Box::new(FlatRate::new(19.0))
    } else {
        Box::new(MarginalRelief::default())
    }
}

/// Round to 2 decimal places (pounds and pence).
fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn example_from_spec() {
        let calc = for_fy(2024).tax(150_000.0, 1.0);
        assert_eq!(calc.taxable_profit, 150_000.0);
        assert_eq!(calc.tax_at_main_rate, 37_500.0);
        assert_eq!(calc.marginal_relief, 1_500.0);
        assert_eq!(calc.corporation_tax, 36_000.0);
        assert_eq!(calc.effective_rate, 24.0);
    }

    #[test]
    fn below_small_profits_limit() {
        let calc = for_fy(2024).tax(50_000.0, 1.0);
        assert_eq!(calc.taxable_profit, 50_000.0);
        assert_eq!(calc.tax_at_main_rate, 12_500.0);
        assert_eq!(calc.marginal_relief, 0.0);
        assert_eq!(calc.corporation_tax, 9_500.0);
        assert_eq!(calc.effective_rate, 19.0);
    }

    #[test]
    fn just_above_small_profits_limit() {
        let calc = for_fy(2024).tax(50_001.0, 1.0);
        assert_eq!(calc.taxable_profit, 50_001.0);
        assert_eq!(calc.tax_at_main_rate, 12_500.25);
        assert_eq!(calc.marginal_relief, 2_999.98);
        assert_eq!(calc.corporation_tax, 9_500.27);
    }

    #[test]
    fn at_upper_limit() {
        let calc = for_fy(2024).tax(250_000.0, 1.0);
        assert_eq!(calc.taxable_profit, 250_000.0);
        assert_eq!(calc.tax_at_main_rate, 62_500.0);
        assert_eq!(calc.marginal_relief, 0.0);
        assert_eq!(calc.corporation_tax, 62_500.0);
        assert_eq!(calc.effective_rate, 25.0);
    }

    #[test]
    fn just_above_upper_limit() {
        let calc = for_fy(2024).tax(250_001.0, 1.0);
        assert_eq!(calc.taxable_profit, 250_001.0);
        assert_eq!(calc.tax_at_main_rate, 62_500.25);
        assert_eq!(calc.marginal_relief, 0.0);
        assert_eq!(calc.corporation_tax, 62_500.25);
        assert_eq!(calc.effective_rate, 25.0);
    }

    #[test]
    fn zero_profit() {
        let calc = for_fy(2024).tax(0.0, 1.0);
        assert_eq!(calc.taxable_profit, 0.0);
        assert_eq!(calc.tax_at_main_rate, 0.0);
        assert_eq!(calc.marginal_relief, 0.0);
        assert_eq!(calc.corporation_tax, 0.0);
        assert_eq!(calc.effective_rate, 0.0);
    }

    #[test]
    fn small_profit() {
        let calc = for_fy(2024).tax(10_000.0, 1.0);
        assert_eq!(calc.taxable_profit, 10_000.0);
        assert_eq!(calc.tax_at_main_rate, 2_500.0);
        assert_eq!(calc.marginal_relief, 0.0);
        assert_eq!(calc.corporation_tax, 1_900.0);
        assert_eq!(calc.effective_rate, 19.0);
    }

    #[test]
    fn mid_marginal_band() {
        let calc = for_fy(2024).tax(100_000.0, 1.0);
        assert_eq!(calc.taxable_profit, 100_000.0);
        assert_eq!(calc.tax_at_main_rate, 25_000.0);
        assert_eq!(calc.marginal_relief, 2_250.0);
        assert_eq!(calc.corporation_tax, 22_750.0);
        assert_eq!(calc.effective_rate, 22.75);
    }

    #[test]
    fn large_profit() {
        let calc = for_fy(2024).tax(1_000_000.0, 1.0);
        assert_eq!(calc.taxable_profit, 1_000_000.0);
        assert_eq!(calc.tax_at_main_rate, 250_000.0);
        assert_eq!(calc.marginal_relief, 0.0);
        assert_eq!(calc.corporation_tax, 250_000.0);
        assert_eq!(calc.effective_rate, 25.0);
    }

    #[test]
    fn near_small_profits_limit() {
        let calc = for_fy(2024).tax(49_999.0, 1.0);
        assert_eq!(calc.taxable_profit, 49_999.0);
        assert_eq!(calc.tax_at_main_rate, 12_499.75);
        assert_eq!(calc.marginal_relief, 0.0);
        assert_eq!(calc.corporation_tax, 9_499.81);
        assert_eq!(calc.effective_rate, 19.0);
    }

    #[test]
    fn near_upper_limit() {
        let calc = for_fy(2024).tax(249_999.0, 1.0);
        assert_eq!(calc.taxable_profit, 249_999.0);
        assert_eq!(calc.tax_at_main_rate, 62_499.75);
        assert_eq!(calc.marginal_relief, 0.02);
        assert_eq!(calc.corporation_tax, 62_499.73);
        assert_eq!(calc.effective_rate, 25.0);
    }

    #[test]
    fn flat_rate_taxes_everything_at_the_flat_rate() {
        let calc = for_fy(2020).tax(150_000.0, 1.0);
        assert_eq!(calc.taxable_profit, 150_000.0);
        assert_eq!(calc.tax_at_main_rate, 28_500.0);
        assert_eq!(calc.marginal_relief, 0.0);
        assert_eq!(calc.corporation_tax, 28_500.0);
        assert_eq!(calc.effective_rate, 19.0);
        // The rate method returns the flat rate for any profit.
        assert_eq!(for_fy(2020).rate(1_000_000.0, 1.0), 19.0);
    }

    #[test]
    fn for_fy_switches_regime_at_fy2023() {
        assert_eq!(for_fy(2022).name(), "flat 19%");
        assert_eq!(for_fy(2023).name(), "marginal relief");
        assert_eq!(for_fy(2019).name(), "flat 19%");
        assert_eq!(for_fy(2026).name(), "marginal relief");
    }

    #[test]
    fn marginal_relief_limits_are_time_apportioned() {
        // A 6-month slice of a financial year halves the limits: at £30,000
        // profit the apportioned lower limit is £25,000, so the marginal
        // band (up to £125,000) applies.
        let calc = for_fy(2024).tax(30_000.0, 0.5);
        assert_eq!(calc.tax_at_main_rate, 7_500.0);
        assert_eq!(calc.marginal_relief, 1_425.0);
        assert_eq!(calc.corporation_tax, 6_075.0);
        assert_eq!(calc.effective_rate, 20.25);

        // Below the apportioned lower limit the small-profits rate applies.
        let calc = for_fy(2024).tax(10_000.0, 0.5);
        assert_eq!(calc.marginal_relief, 0.0);
        assert_eq!(calc.corporation_tax, 1_900.0);
        assert_eq!(calc.effective_rate, 19.0);
    }

    #[test]
    fn flat_rate_ignores_limit_scale() {
        let calc = for_fy(2020).tax(30_000.0, 0.5);
        assert_eq!(calc.corporation_tax, 5_700.0);
        assert_eq!(calc.marginal_relief, 0.0);
    }
}
