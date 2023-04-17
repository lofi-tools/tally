use std::env;

lazy_static::lazy_static! {
    pub static ref CONFIG:Config = Config::load().set_env();
}
pub struct Config {
    pub nordigen_api_secret_id: String,
    pub nordigen_api_secret_key: String,
    // pub nordigen_starling_requisition_ref: String,
    pub nordigen_starling_requisition_id: String,
}
impl Config {
    pub fn load() -> Self {
        Config {
            nordigen_api_secret_id: expect_env_var("NORDIGEN_API_SECRET_ID"),
            nordigen_api_secret_key: expect_env_var("NORDIGEN_API_SECRET_KEY"),
            nordigen_starling_requisition_id: expect_env_var("NORDIGEN_STARLING_REQUISITION_ID"),
        }
    }
    fn set_env(self) -> Self {
        env::set_var(
            "RUST_LOG",
            env::var("RUST_LOG").unwrap_or_else(|_| "error,actix_web=info".into()),
        );
        env::set_var("RUST_BACKTRACE", "1");

        self
    }
}

fn expect_env_var(var_name: &str) -> String {
    std::env::var(var_name).unwrap_or_else(|_| panic!("missing env var {var_name}"))
}
