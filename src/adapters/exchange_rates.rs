use self::models::RateHistory;
use crate::{utils::api_client_utils::FetchErr, CONFIG};
use chrono::{DateTime, Utc};
use models::PricePoint;
use serde::Deserialize;
use std::collections::HashMap;

pub trait RatesApi {
    fn fetch_rate_hist(
        &self,
        from_day: DateTime<Utc>,
        to_day: DateTime<Utc>,
        currencies: &CurrencyPair,
    ) -> impl futures::Future<Output = Result<RateHistory, ExchangeRateErr>>;
}
impl RatesApi for twelvedata::TwelvedataClient {
    async fn fetch_rate_hist(
        &self,
        from_day: DateTime<Utc>,
        to_day: DateTime<Utc>,
        currencies: &CurrencyPair,
    ) -> Result<RateHistory, ExchangeRateErr> {
        let price_points: Vec<PricePoint> = self
            .fetch_rates(from_day, to_day, currencies)
            .await?
            .into_iter()
            .map(PricePoint::from)
            .collect();
        Ok(RateHistory {
            from_currency: currencies.from_currency,
            to_currency: currencies.to_currency,
            rates: price_points,
        })
    }
}

pub mod twelvedata {
    use super::models::PricePoint;
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
            from_day: DateTime<Utc>,
            to_day: DateTime<Utc>,
            currencies: &CurrencyPair,
        ) -> Result<Vec<TdPricePoint>, ExchangeRateErr> {
            const FORMAT_DATE: &'static str = "%Y-%m-%d";
            let CurrencyPair {
                from_currency,
                to_currency,
            } = currencies;

            let req = self.get("/time_series").query(&HashMap::from([
                ("symbol", format!("{from_currency}/{to_currency}")),
                ("start_date", from_day.format(FORMAT_DATE).to_string()),
                ("end_date", to_day.format(FORMAT_DATE).to_string()),
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
    }
    impl From<TdPricePoint> for PricePoint {
        fn from(pp: TdPricePoint) -> Self {
            PricePoint {
                datetime: pp.datetime,
                rate: pp.max(),
            }
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
            let from_day = first_of_last_month();
            let to_day = last_of_last_month();
            let currencies = CurrencyPair {
                from_currency: Currency::USD,
                to_currency: Currency::EUR,
            };

            let rates = TwelvedataClient::new()
                .fetch_rate_hist(from_day, to_day, &currencies)
                .await?;
            dbg!(&rates.max_rate());
            Ok(())
        }
    }
}

pub mod models {
    use super::*;

    #[derive(Debug)]
    pub struct RateHistory {
        pub from_currency: Currency,
        pub to_currency: Currency,
        pub rates: Vec<PricePoint>,
    }
    impl RateHistory {
        pub fn max_rate(&self) -> anyhow::Result<&PricePoint> {
            self.rates
                .iter()
                .max_by(|a, b| a.rate.total_cmp(&b.rate))
                .ok_or(anyhow::Error::msg("No maximum found"))
        }
    }
    #[derive(Debug, Clone)]
    pub struct PricePoint {
        pub datetime: DateTime<Utc>,
        pub rate: f64,
    }
}

#[derive(thiserror::Error, Debug)]
pub enum ExchangeRateErr {
    #[error("Failed fetching rates: {0}")]
    FetchErr(#[from] FetchErr),
    #[error("No rate found for currencies and date")]
    NoRateForCurrencyAndDate, // TODO include currencies and date
    #[error("Failed parsing response: {0}")]
    ParseResp(serde_json::Error),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
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
    type Output = CurrencyAmount;
    fn mul(self, currency: Currency) -> Self::Output {
        CurrencyAmount {
            currency,
            amount: self,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Hash)]
pub struct CurrencyPair {
    pub from_currency: Currency,
    pub to_currency: Currency,
}

#[derive(Debug, Clone)]
pub struct CurrencyAmount {
    pub currency: Currency,
    pub amount: f64,
}
