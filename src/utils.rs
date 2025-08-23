use chrono::{DateTime, Duration, NaiveDate, NaiveTime, Utc};
use num_traits::FromPrimitive;
use rand::Rng;
use rand::distributions::Alphanumeric;
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer};
use std::{collections::HashMap, fmt::Debug, hash::Hash, str::FromStr};

pub mod api_client_utils {
    use reqwest::{RequestBuilder, StatusCode};
    use serde::de::DeserializeOwned;
    use std::time::Duration;

    pub trait ApiClient {
        fn base_url(&self) -> &str;
        fn http_client(&self) -> &reqwest::Client;

        fn path(&self, url_path: &str) -> String {
            if url_path.starts_with("http") {
                return url_path.to_string();
            }

            let origin = self.base_url().trim().trim_end_matches('/');
            let path = url_path.trim().trim_start_matches('/');
            format!("{origin}/{path}")
        }
        fn default_params(&self, request_builder: RequestBuilder) -> RequestBuilder {
            request_builder.timeout(Duration::new(5, 0))
        }

        // fn with_base_url(&mut self, url: &str) -> &mut Self {
        //     // Self {
        //     //     base_url: url.to_string(),
        //     //     ..self.clone()
        //     // }
        //     // self.base_url = url.to_string();
        //     // self
        //     todo!()
        // }
        fn get(&self, url_path: &str) -> RequestBuilder {
            self.default_params(self.http_client().get(self.path(url_path)))
        }
        fn post(&self, url_path: &str) -> RequestBuilder {
            self.default_params(self.http_client().post(self.path(url_path)))
        }
    }

    #[async_trait::async_trait]
    pub trait RequestBuilderExt {
        async fn fetch_json<D: DeserializeOwned>(self) -> Result<D, FetchErr>;
        // fn replace_header(self, key: &str, value: impl Into<String>) -> RequestBuilder;
    }
    #[async_trait::async_trait]
    impl RequestBuilderExt for RequestBuilder {
        async fn fetch_json<D: DeserializeOwned>(self) -> Result<D, FetchErr> {
            let builder = self
                .header("Content-Type", "application/json")
                .header("Accept", "application/json");
            let (client, req) = builder.build_split();
            let req = req?;
            // let headers = req.headers().clone();

            let method = req.method().clone();
            let resp = client.execute(req).await?;
            let status = resp.status();
            let url = resp.url().clone();
            let resp_text = resp.text().await?;

            if !status.is_success() {
                return Err(FetchErr::ErrResp {
                    method,
                    url,
                    status,
                    body_str: resp_text,
                });
                //         return Err(anyhow::anyhow!(
                //     "{method} {url} \n Expected response with status ok, got: status: {status}, resp: {resp_text:?}",
                //   ));
            }
            let deserialized: D = serde_json::from_str(&resp_text)
                // .wrap_err(&format!("Failed deserializing. resp_text: {resp_text}"))?;
                .map_err(|e| FetchErr::DeserialErr {
                    body_str: resp_text,
                    source: e,
                })?;
            Ok(deserialized)
        }

        // fn replace_header(self, key: &str, value: impl Into<String>) -> RequestBuilder {
        //     // self.header(key, value.into())
        //     self.re().insert(key, value.into());
        // }
    }

    #[derive(thiserror::Error, Debug)]
    pub enum FetchErr {
        #[error("Failed sending request: {0}")]
        ReqwestErr(#[from] reqwest::Error),
        #[error("{method} {url} \nReceived {status} error response: {body_str}")]
        ErrResp {
            method: reqwest::Method,
            url: reqwest::Url,
            status: StatusCode,
            body_str: String,
        },
        #[error("Failed deserializing: body: {body_str}")]
        DeserialErr {
            body_str: String,
            source: serde_json::Error,
        },
    }
}

pub mod errors_debug {
    use backtrace::{Backtrace, BacktraceFrame, BacktraceSymbol};
    use std::{process::Command, sync::LazyLock};

    pub static WORKDIR: LazyLock<String> = LazyLock::new(|| workdir());

    pub trait WrapErr {
        type Ok;
        fn wrap_err(self, msg: &str) -> Result<Self::Ok, anyhow::Error>;
    }
    impl<T, E: std::fmt::Display> WrapErr for Result<T, E> {
        type Ok = T;
        fn wrap_err(self, msg: &str) -> Result<<Self as WrapErr>::Ok, anyhow::Error> {
            let caller = previous_symbol(3).unwrap();
            let filename = caller.filename().unwrap().to_str().unwrap();
            let filename = filename.strip_prefix(WORKDIR.trim()).unwrap_or(filename);
            let filename = filename.strip_prefix("/").unwrap_or(filename);
            let line = caller.lineno().unwrap();
            let col = caller.colno().unwrap();

            return self.map_err(|e| anyhow::anyhow!("[{filename}:{line}:{col}] {msg}: {e}"));
        }
    }

    pub fn previous_symbol(level: u32) -> Option<BacktraceSymbol> {
        let (trace, curr_file, curr_line) = (Backtrace::new(), file!(), line!());
        let frames = trace.frames();
        frames
            .iter()
            .flat_map(BacktraceFrame::symbols)
            .skip_while(|s| {
                s.filename()
                    .map(|p| !p.ends_with(curr_file))
                    .unwrap_or(true)
                    || s.lineno() != Some(curr_line)
            })
            .nth(1 + level as usize)
            .cloned()
    }

    pub fn workdir() -> String {
        let wd_bytes_utf8 = Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .unwrap()
            .stdout;
        String::from_utf8_lossy(&wd_bytes_utf8).to_string()
    }
}

pub fn rand_string(size: usize) -> String {
    rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(size)
        .map(char::from)
        .collect()
}

pub fn deserialize_decimal<'de, D>(deserializer: D) -> Result<Decimal, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    Decimal::from_str(&s).map_err(serde::de::Error::custom)
}

pub trait MapExt<K, V> {
    fn try_get(&self, key: &K) -> anyhow::Result<&V>;
}
impl<K: Hash + Eq + Debug, V> MapExt<K, V> for HashMap<K, V> {
    fn try_get(&self, key: &K) -> anyhow::Result<&V> {
        self.get(key)
            .ok_or(anyhow::Error::msg(format!("key not found: {key:?}")))
    }
}

pub trait DateExt {
    fn ymd(year: i32, month: u32, day: u32) -> Self;
    fn from_utc(utc: DateTime<Utc>) -> Self;
    fn naive_date(&self) -> NaiveDate;
}
impl DateExt for NaiveDate {
    fn ymd(year: i32, month: u32, day: u32) -> Self {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }
    fn from_utc(utc: DateTime<Utc>) -> Self {
        utc.date_naive()
    }
    fn naive_date(&self) -> NaiveDate {
        *self
    }
}
impl DateExt for DateTime<Utc> {
    fn ymd(year: i32, month: u32, day: u32) -> Self {
        DateTime::from_naive_date(NaiveDate::ymd(year, month, day))
    }
    fn from_utc(utc: DateTime<Utc>) -> Self {
        utc
    }
    fn naive_date(&self) -> NaiveDate {
        self.date_naive()
    }
}

pub trait DatetimeUtcExt {
    fn from_naive_date(datetime: NaiveDate) -> Self;
    fn with_start_of_day(&self) -> Self;
    fn with_end_of_day(&self) -> Self;
}
impl DatetimeUtcExt for DateTime<Utc> {
    fn from_naive_date(date: NaiveDate) -> Self {
        DateTime::from_naive_utc_and_offset(
            date.and_time(
                NaiveTime::from_hms_opt(0, 0, 0)
                    .ok_or(anyhow::Error::msg("Invalid hour, minute and/or second."))
                    .unwrap(),
            ),
            Utc,
        )
    }
    fn with_start_of_day(&self) -> Self {
        self.with_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap())
            .unwrap()
    }
    fn with_end_of_day(&self) -> Self {
        self.with_time(NaiveTime::from_hms_opt(23, 59, 59).unwrap())
            .unwrap()
    }
}

pub struct DateRange(pub NaiveDate, pub NaiveDate);
impl DateRange {
    pub fn new(start: impl DateExt, end: impl DateExt) -> Self {
        DateRange(start.naive_date(), end.naive_date())
    }
}
impl<'a> IntoIterator for &'a DateRange {
    type Item = NaiveDate;
    type IntoIter = DateRangeIter<'a>;
    fn into_iter(self) -> Self::IntoIter {
        DateRangeIter {
            range: &self,
            curr: self.0,
        }
    }
}
pub struct DateRangeIter<'a> {
    pub range: &'a DateRange,
    pub curr: NaiveDate,
}
impl<'a> Iterator for DateRangeIter<'a> {
    type Item = NaiveDate;
    fn next(&mut self) -> Option<Self::Item> {
        if self.curr <= self.range.1 {
            let next = self.curr + Duration::days(1);
            self.curr = next;
            Some(next)
        } else {
            None
        }
    }
}

// pub fn f64_to_cents(amount: f64) -> u32 {
//     let cents = (amount * 1000.0).trunc() as u32 / 10; // TODO test
//     cents
// }

pub trait NumExt {
    fn is_close_to(&self, other: impl Into<Decimal>) -> bool;
}
impl NumExt for Decimal {
    fn is_close_to(&self, other: impl Into<Decimal>) -> bool {
        Decimal::abs(&(self - other.into())) <= Decimal::from_f64(0.005).unwrap()
    }
}
