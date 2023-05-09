use rand::distributions::Alphanumeric;
use rand::Rng;
use rust_decimal::Decimal;
use serde::{Deserialize, Deserializer};
use std::str::FromStr;

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
