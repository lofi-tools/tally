use std::{env, sync::LazyLock};

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| Config::from_env().set_env());
pub struct Config {
    pub nordigen_api_secret_id: String,
    pub nordigen_api_secret_key: String,
    pub twelvedata_api_key: String,
    // pub nordigen_starling_requisition_ref: String,
    // pub nordigen_starling_requisition_id: String,
}
impl Config {
    pub fn from_env() -> Self {
        Config {
            nordigen_api_secret_id: expect_env_var("NORDIGEN_API_SECRET_ID"),
            nordigen_api_secret_key: expect_env_var("NORDIGEN_API_SECRET_KEY"),
            twelvedata_api_key: expect_env_var("TWELVEDATA_API_KEY"),
            // nordigen_starling_requisition_id: expect_env_var("NORDIGEN_STARLING_REQUISITION_ID"),
        }
    }
    fn set_env(self) -> Self {
        unsafe {
            env::set_var(
                "RUST_LOG",
                env::var("RUST_LOG").unwrap_or_else(|_| "error,actix_web=info".into()),
            );
            env::set_var("RUST_BACKTRACE", "1");
        }
        self
    }
}

fn expect_env_var(var_name: &str) -> String {
    std::env::var(var_name).unwrap_or_else(|_| panic!("missing env var {var_name}"))
}
