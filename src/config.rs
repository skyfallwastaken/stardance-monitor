use std::path::PathBuf;

use color_eyre::eyre::Context;
use once_cell::sync::Lazy;
use reqwest::Url;
use serde::Deserialize;

#[derive(Deserialize)]
pub struct Config {
    pub cookie: String,
    pub webhook_url: Url,
    #[serde(default = "default_user_agent")]
    pub user_agent: String,
    #[serde(default = "default_base_url")]
    pub base_url: Url,
    #[serde(default = "default_storage_path")]
    pub storage_path: PathBuf,
    pub sentry_dsn: Option<String>,
    pub openai_api_key: Option<String>,
    pub openai_base_url: Option<Url>,
    pub openai_model: Option<String>,
    #[serde(default = "default_cdn_key")]
    pub cdn_key: String,
    #[serde(default = "default_cdn_base_url")]
    pub cdn_base_url: Url,
}

fn default_user_agent() -> String {
    "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/143.0.0.0 Safari/537.36".into()
}

fn default_base_url() -> Url {
    Url::parse("https://stardance.hackclub.com/").unwrap()
}

fn default_storage_path() -> PathBuf {
    std::env::current_dir().unwrap().join("stardance-storage")
}

fn default_cdn_key() -> String {
    "beans".into()
}

fn default_cdn_base_url() -> Url {
    Url::parse("https://cdn.hackclub.com/api/file").unwrap()
}

pub static CONFIG: Lazy<Config> = Lazy::new(|| {
    envy::from_env::<Config>()
        .wrap_err("failed to load config")
        .unwrap()
});
