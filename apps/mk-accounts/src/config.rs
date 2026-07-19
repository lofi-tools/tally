use std::{env, sync::LazyLock};

use crate::adapters::{
    nordigen_banks::nordigen_client::NordigenConfig, truelayer_banks::TruelayerConfig,
    yapily_banks::YapilyConfig,
};

pub static CONFIG: LazyLock<Config> = LazyLock::new(|| Config::from_env().set_env());
pub struct Config {
    // pub nordigen_api_secret_id: String,
    // pub nordigen_api_secret_key: String,
    pub nordigen_cfg: Option<NordigenConfig>,
    pub twelvedata_api_key: String,
    // pub truelayer_client_id: String,
    // pub truelayer_client_secret: String,
    // pub truelayer_redirect_uri: String,
    // pub nordigen_starling_requisition_ref: String,
    // pub nordigen_starling_requisition_id: String,
    pub truelayer_cfg: TruelayerConfig,
    pub yapily_config: YapilyConfig,
}
impl Config {
    pub fn from_env() -> Self {
        Config {
            // nordigen_api_secret_id: expect_env_var("NORDIGEN_API_SECRET_ID"),
            // nordigen_api_secret_key: expect_env_var("NORDIGEN_API_SECRET_KEY"),
            twelvedata_api_key: expect_env_var("TWELVEDATA_API_KEY"),
            // truelayer_client_id: expect_env_var("TRUELAYER_CLIENT_ID"),
            // truelayer_client_secret: expect_env_var("TRUELAYER_CLIENT_SECRET"),
            // truelayer_redirect_uri: expect_env_var("TRUELAYER_REDIRECT_URI"),
            truelayer_cfg: TruelayerConfig::from_env(),
            nordigen_cfg: None,
            yapily_config: YapilyConfig::from_env(),
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

pub fn expect_env_var(var_name: &str) -> String {
    std::env::var(var_name).unwrap_or_else(|_| panic!("missing env var {var_name}"))
}
