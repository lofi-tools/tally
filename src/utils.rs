use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};
use rand::distributions::Alphanumeric;
use rand::Rng;
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer};
use std::{
    collections::HashMap,
    fmt::{Debug, Display},
    hash::Hash,
    str::FromStr,
};

pub mod errors_debug {
    use backtrace::{Backtrace, BacktraceFrame, BacktraceSymbol};
    use std::process::Command;

    lazy_static::lazy_static! {
      pub static ref WORKDIR: String = workdir();
    }

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
}
impl DateExt for NaiveDate {
    fn ymd(year: i32, month: u32, day: u32) -> Self {
        NaiveDate::from_ymd_opt(year, month, day).unwrap()
    }
}
impl DateExt for DateTime<Utc> {
    fn ymd(year: i32, month: u32, day: u32) -> Self {
        let datetime =
            NaiveDate::ymd(year, month, day).and_time(NaiveTime::from_hms_opt(0, 0, 0).unwrap());
        DateTime::from_naive_utc_and_offset(datetime, Utc)
    }
}

// pub fn f64_to_cents(amount: f64) -> u32 {
//     let cents = (amount * 1000.0).trunc() as u32 / 10; // TODO test
//     cents
// }
