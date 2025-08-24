use self::models::RateHistory;
use crate::{CONFIG, utils::api_client_utils::FetchErr};
use chrono::{DateTime, Duration, NaiveDate, Utc};
use file_cache::{CacheInRepo, CacheLocation, FileBytes};
use models::DayPricePoint;
use num_traits::FromPrimitive;
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

pub static RATES_API: LazyLock<CachedRatesApi> = LazyLock::new(|| CachedRatesApi::new().unwrap());
pub const GBP_EUR_PAIR: LazyLock<AssetPair> = LazyLock::new(|| AssetPair {
    from_currency: Currency::GBP,
    to_currency: Currency::EUR,
});

pub trait RatesApi {
    fn rate_hist(
        &self,
        time_range: &TimeRange,
        currencies: &AssetPair,
    ) -> impl Future<Output = Result<RateHistory, ExchangeRateErr>>;

    fn rate_at(
        &self,
        date: &NaiveDate,
        currencies: &AssetPair,
    ) -> impl Future<Output = Result<DayPricePoint, ExchangeRateErr>> {
        async {
            let want_time_range = TimeRange::new(
                DateTime::<Utc>::from_utc(date.and_hms_opt(0, 0, 0).unwrap(), Utc),
                DateTime::<Utc>::from_utc(date.and_hms_opt(23, 59, 59).unwrap(), Utc),
            );
            let rate_hist = self.rate_hist(&want_time_range, currencies).await?;
            rate_hist.rate_at(date.clone()).cloned().map_err(|_| {
                ExchangeRateErr::NoRateForCurrencyAndDate {
                    currencies: currencies.clone(),
                    date: date.clone(),
                }
            })
        }
    }
}

pub struct CachedRatesApi {
    pub api_client: twelvedata::TwelvedataClient,
    pub rates_gbp_eur: RwLock<Option<RateHistory>>,
}
impl CachedRatesApi {
    pub fn new() -> anyhow::Result<Self> {
        Ok(CachedRatesApi {
            api_client: twelvedata::TwelvedataClient::new(),
            rates_gbp_eur: {
                let cache_file = CacheInRepo::file_path("rates_gbp_eur")?;
                match RateHistory::from_file(&cache_file) {
                    Ok(hist) => RwLock::new(Some(hist)),
                    Err(_) => RwLock::new(None),
                }
            },
        })
    }

    pub async fn find_complement_timerange(
        &self,
        want_time_range: &TimeRange,
    ) -> Option<TimeRange> {
        let cached_rates = self.rates_gbp_eur.read().unwrap();

        let cached_rates = match cached_rates.as_ref() {
            Some(cached_rates) => cached_rates,
            None => return Some(want_time_range.clone()),
        };
        // complete overlap: 2 cases
        if cached_rates.time_range().contains_range(want_time_range) {
            return None;
        }
        if want_time_range.contains_range(&cached_rates.time_range()) {
            return Some(want_time_range.clone());
        }

        // partial/no overlap: only 2 cases left
        let new_timerange = match want_time_range.end > cached_rates.datetime_end() {
            true => TimeRange::new(cached_rates.datetime_end(), want_time_range.end),
            false => TimeRange::new(want_time_range.start, cached_rates.datetime_start()),
        };

        match new_timerange.dates().num_days() <= 1 {
            true => None,
            false => Some(new_timerange),
        }
    }
}
impl RatesApi for CachedRatesApi {
    async fn rate_hist(
        &self,
        want_time_range: &TimeRange,
        currencies: &AssetPair,
    ) -> Result<RateHistory, ExchangeRateErr> {
        if currencies.from_currency != Currency::GBP || currencies.to_currency != Currency::EUR {
            return Err(ExchangeRateErr::Other(anyhow::anyhow!(
                "Only implemented for GBP/EUR"
            )));
        }

        // if in cache, return from cache
        if let Some(cached_rates) = self.rates_gbp_eur.read().unwrap().as_ref() {
            if cached_rates.time_range().contains_range(want_time_range) {
                return Ok(cached_rates.subset(want_time_range));
            }
        }

        // if not in cache, fetch and cache
        let complement_time_range = match self.find_complement_timerange(want_time_range).await {
            Some(complement_time_range) => complement_time_range,
            None => {
                return self
                    .rates_gbp_eur
                    .read()
                    .unwrap()
                    .clone()
                    .ok_or(ExchangeRateErr::Other(anyhow::anyhow!("No cached rates")));
            }
        };
        dbg!(&complement_time_range);

        let new_rates = self
            .api_client
            .rate_hist(&complement_time_range, currencies)
            .await?;

        let mut cached_rates = self.rates_gbp_eur.write().unwrap();
        // this updates in-mem cache
        let updated_rates = match cached_rates.as_mut() {
            Some(cached_rates) => cached_rates.mut_merge(&new_rates),
            None => cached_rates.insert(new_rates),
        };

        // update file cache
        let cache_file = CacheInRepo::file_path("rates_gbp_eur").map_err(ExchangeRateErr::Other)?;
        updated_rates
            .to_file(&cache_file)
            .map_err(ExchangeRateErr::Other)?;

        Ok(updated_rates.subset(want_time_range))
    }
}

impl RatesApi for twelvedata::TwelvedataClient {
    async fn rate_hist(
        &self,
        time_range: &TimeRange,
        currencies: &AssetPair,
    ) -> Result<RateHistory, ExchangeRateErr> {
        let price_points: Vec<DayPricePoint> = self
            .fetch_rates(time_range, currencies)
            .await?
            .into_iter()
            .map(|p| {
                Ok(DayPricePoint {
                    datetime: p.datetime,
                    rate_high: Decimal::from_f64(p.high).ok_or(ExchangeRateErr::OtherParseErr {
                        field_name: "rate_high".to_string(),
                        source: None,
                    })?,
                    rate_low: Decimal::from_f64(p.low).ok_or(ExchangeRateErr::OtherParseErr {
                        field_name: "rate_low".to_string(),
                        source: None,
                    })?,
                })
            })
            .collect::<Result<_, ExchangeRateErr>>()?;
        Ok(RateHistory {
            from_currency: currencies.from_currency,
            to_currency: currencies.to_currency,
            rates: price_points,
        })
    }
}

pub mod twelvedata {
    use super::*;
    use crate::utils::api_client_utils::{ApiClient, RequestBuilderExt};
    use reqwest::RequestBuilder;

    #[derive(Default)]
    pub struct TwelvedataClient {
        http_client: reqwest::Client,
    }
    impl ApiClient for TwelvedataClient {
        fn base_url(&self) -> &str {
            "https://api.twelvedata.com"
        }
        fn http_client(&self) -> &reqwest::Client {
            &self.http_client
        }
        fn default_params(&self, request_builder: RequestBuilder) -> RequestBuilder {
            request_builder.header(
                "Authorization",
                format!("apikey {}", CONFIG.twelvedata_api_key),
            )
        }
    }
    impl TwelvedataClient {
        pub fn new() -> Self {
            Self::default()
        }
        pub async fn fetch_rates(
            &self,
            // from_day: DateTime<Utc>,
            // to_day: DateTime<Utc>,
            times: &TimeRange,
            currencies: &AssetPair,
        ) -> Result<Vec<TdPricePoint>, ExchangeRateErr> {
            const FORMAT_DATE: &'static str = "%Y-%m-%d";
            let AssetPair {
                from_currency,
                to_currency,
            } = currencies;

            let req = self.get("/time_series").query(&HashMap::from([
                ("symbol", format!("{from_currency}/{to_currency}")),
                ("start_date", times.start.format(FORMAT_DATE).to_string()),
                ("end_date", times.end.format(FORMAT_DATE).to_string()),
                ("interval", "1h".to_string()),
            ]));

            let resp = req
                .fetch_json::<FetchRatesResp>()
                .await
                .map_err(ExchangeRateErr::FetchErr)?;
            Ok(resp.values)
        }
    }

    #[derive(Deserialize, Debug)]
    pub struct FetchRatesResp {
        pub meta: serde_json::Value,
        pub values: Vec<TdPricePoint>,
    }
    #[derive(Debug)]
    pub struct TdPricePoint {
        pub datetime: DateTime<Utc>,
        pub open: f64,
        pub high: f64,
        pub low: f64,
        pub close: f64,
    }
    impl<'de> Deserialize<'de> for TdPricePoint {
        fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
        where
            D: serde::Deserializer<'de>,
        {
            #[derive(Deserialize, Debug)]
            pub struct TdPricePointResp {
                pub datetime: String,
                pub open: String,
                pub high: String,
                pub low: String,
                pub close: String,
            }
            fn err_parse_f64<E: std::fmt::Display, R>(e: E) -> R
            where
                R: serde::de::Error,
            {
                serde::de::Error::custom(format!("Failed parsing f64: {e}"))
            }

            let price_point_resp = TdPricePointResp::deserialize(deserializer)?;

            const FORMAT_DATE: &str = "%Y-%m-%d %H:%M:%S %z";
            Ok(TdPricePoint {
                datetime: DateTime::parse_from_str(
                    &format!("{} +0000", price_point_resp.datetime),
                    FORMAT_DATE,
                )
                .map_err(|e| serde::de::Error::custom(format!("Failed parsing DateTime: {e}")))?
                .into(),
                open: price_point_resp
                    .open
                    .parse::<f64>()
                    .map_err(err_parse_f64)?,
                high: price_point_resp
                    .high
                    .parse::<f64>()
                    .map_err(err_parse_f64)?,
                low: price_point_resp.low.parse::<f64>().map_err(err_parse_f64)?,
                close: price_point_resp
                    .close
                    .parse::<f64>()
                    .map_err(err_parse_f64)?,
            })
        }
    }
    impl TdPricePoint {
        pub fn max(&self) -> f64 {
            self.open.max(self.high).max(self.low).max(self.close)
        }
        pub fn min(&self) -> f64 {
            self.open.min(self.high).min(self.low).min(self.close)
        }
    }

    #[cfg(test)]
    pub mod tests {
        use super::*;
        use chrono::{Datelike, Duration, Months};

        pub fn first_of_month(date: DateTime<Utc>) -> DateTime<Utc> {
            date.with_day(1).unwrap()
        }
        pub fn last_of_month(date: DateTime<Utc>) -> DateTime<Utc> {
            date.with_day(1).unwrap() + Months::new(1) - Duration::days(1)
        }
        pub fn first_of_last_month() -> DateTime<Utc> {
            last_of_last_month().with_day(1).unwrap()
        }
        pub fn last_of_last_month() -> DateTime<Utc> {
            first_of_month(Utc::now()) - Duration::days(1)
        }

        #[tokio::test]
        #[ignore = "hits API with limit"]
        async fn test_fetch_rates() -> anyhow::Result<()> {
            let time_range = TimeRange::new(first_of_last_month(), last_of_last_month());
            let currencies = AssetPair {
                from_currency: Currency::USD,
                to_currency: Currency::EUR,
            };

            let _rates = TwelvedataClient::new()
                .rate_hist(&time_range, &currencies)
                .await?;

            Ok(())
        }
    }
}

pub mod models {
    use super::*;
    use chrono::NaiveDate;
    use file_cache::FileBytes;
    use rust_decimal::Decimal;
    use serde::Serialize;

    #[derive(Serialize, Deserialize, Debug, Clone)]
    // TODO use impl Cacheable (from rust-utils)
    pub struct RateHistory {
        pub from_currency: Currency,
        pub to_currency: Currency,
        pub rates: Vec<DayPricePoint>,
    }
    impl RateHistory {
        pub fn max_rate(&self) -> anyhow::Result<PricePoint> {
            self.rates
                .iter()
                .max_by_key(|dpp| dpp.rate_high)
                .map(|dpp| PricePoint {
                    datetime: dpp.datetime,
                    rate: dpp.rate_high,
                })
                .ok_or(anyhow::Error::msg("No maximum found"))
        }
        pub fn rate_at(&self, date: NaiveDate) -> anyhow::Result<&DayPricePoint> {
            self.rates
                .iter()
                .find(|dpp| dpp.datetime.date_naive() == date)
                .ok_or(anyhow::anyhow!("No rate found for date {date}"))
        }
        pub fn datetime_start(&self) -> DateTime<Utc> {
            self.rates
                .iter()
                .min_by_key(|dpp| dpp.datetime)
                .unwrap()
                .datetime
        }
        pub fn datetime_end(&self) -> DateTime<Utc> {
            self.rates
                .iter()
                .max_by_key(|dpp| dpp.datetime)
                .unwrap()
                .datetime
        }
        pub fn time_range(&self) -> TimeRange {
            TimeRange::new(self.datetime_start(), self.datetime_end())
        }
        pub fn contains_datetime(&self, datetime: DateTime<Utc>) -> bool {
            self.time_range().contains_datetime(datetime)
        }

        pub fn subset(&self, time_range: &TimeRange) -> Self {
            Self {
                from_currency: self.from_currency,
                to_currency: self.to_currency,
                rates: self
                    .rates
                    .iter()
                    .filter(|dpp| time_range.contains_datetime(dpp.datetime))
                    .cloned()
                    .collect(),
            }
        }

        pub fn mut_merge(&mut self, other: &RateHistory) -> &mut Self {
            self.rates.extend(other.rates.clone());
            self.rates.sort_by_key(|dpp| dpp.datetime);
            self
        }
        pub fn with_merge(&self, other: &RateHistory) -> Self {
            let mut rates = self.rates.clone();
            rates.extend(other.rates.clone());
            rates.sort_by_key(|dpp| dpp.datetime);
            Self {
                from_currency: self.from_currency,
                to_currency: self.to_currency,
                rates,
            }
        }
    }
    impl FileBytes for RateHistory {
        fn as_file_bytes(&self) -> anyhow::Result<Vec<u8>> {
            Ok(serde_json::to_vec_pretty(self)?)
        }
        fn from_file_bytes(bytes: &[u8]) -> anyhow::Result<Self> {
            Ok(serde_json::from_slice(bytes)?)
        }
    }

    #[derive(Debug, Clone)]
    pub struct PricePoint {
        pub datetime: DateTime<Utc>,
        pub rate: Decimal,
    }

    // #[derive(thiserror::Error, Debug)]
    // pub enum RateHistoryErr {
    //     #[error("No rates found for date")]
    //     NoRateForDate { date: NaiveDate },
    //     #[error("Failed fetching rates: {0}")]
    //     FetchErr(#[from] FetchErr),
    // }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct DayPricePoint {
        pub datetime: DateTime<Utc>,
        pub rate_high: Decimal,
        pub rate_low: Decimal,
    }
    impl DayPricePoint {
        pub fn max(self, other: Self) -> Self {
            if other.rate_high > self.rate_high {
                other
            } else {
                self
            }
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ExchangeRateErr {
    #[error("Failed fetching rates: {0}")]
    FetchErr(#[from] FetchErr),
    #[error("No rate found for {currencies} on {date:?}")]
    NoRateForCurrencyAndDate {
        currencies: AssetPair,
        date: NaiveDate,
    },
    #[error("Failed parsing response: {0}")]
    ParseJson(serde_json::Error),
    #[error("Failed parsing field {field_name}: {source:?}")]
    OtherParseErr {
        field_name: String,
        source: Option<anyhow::Error>,
    },
    #[error("other:{0}")]
    Other(anyhow::Error),
}

#[derive(Serialize, Deserialize, Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub enum Currency {
    USD,
    EUR,
    GBP,
}
impl Currency {
    pub fn symbol(&self) -> &'static str {
        match self {
            Currency::USD => "$",
            Currency::EUR => "€",
            Currency::GBP => "£",
        }
    }
}
impl std::fmt::Display for Currency {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        match self {
            Currency::USD => write!(f, "USD"),
            Currency::EUR => write!(f, "EUR"),
            Currency::GBP => write!(f, "GBP"),
        }
    }
}
impl std::ops::Mul<Currency> for f64 {
    type Output = AssetAmount;
    fn mul(self, currency: Currency) -> Self::Output {
        AssetAmount {
            currency,
            amount: self,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AssetPair {
    pub from_currency: Currency,
    pub to_currency: Currency,
}
impl std::fmt::Display for AssetPair {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> Result<(), std::fmt::Error> {
        write!(f, "{}/{}", self.from_currency, self.to_currency)
    }
}

#[derive(Debug, Clone)]
pub struct AssetAmount {
    pub currency: Currency,
    pub amount: f64,
}

// TODO move to shared models
#[derive(Debug, Clone)]
pub struct TimeRange {
    pub start: DateTime<Utc>,
    pub end: DateTime<Utc>,
}
// TODO impl days iter
impl TimeRange {
    pub fn new(start: DateTime<Utc>, end: DateTime<Utc>) -> Self {
        if start >= end {
            panic!("start must be before end");
        }
        Self {
            start: start,
            end: end,
        }
    }
    pub fn duration(&self) -> Duration {
        self.end - self.start
    }

    pub fn contains_range(&self, other: &TimeRange) -> bool {
        self.start <= other.start && self.end >= other.end
    }
    pub fn contains_datetime(&self, dt: DateTime<Utc>) -> bool {
        self.start <= dt && self.end >= dt
    }
    pub fn contains_date(&self, date: NaiveDate) -> bool {
        self.start.date_naive() <= date && self.end.date_naive() >= date
    }
    pub fn dates(&self) -> DateRange {
        DateRange {
            start: self.start.date_naive(),
            end: self.end.date_naive(),
        }
    }
}

pub struct DateRange {
    pub start: NaiveDate,
    pub end: NaiveDate,
}
impl DateRange {
    pub fn num_days(&self) -> i64 {
        self.end.signed_duration_since(self.start).num_days()
    }
}
