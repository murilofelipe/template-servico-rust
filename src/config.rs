use serde::Deserialize;
use std::env;

#[derive(Debug, Deserialize, Clone)]
pub struct AppConfig {
    pub database_url: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_host")]
    pub host: String,
}

fn default_port() -> u16 {
    3000
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

impl AppConfig {
    pub fn load() -> Self {
        dotenvy::dotenv().ok();
        envy::from_env::<AppConfig>().unwrap_or_else(|_| {
            let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://postgres:postgres@localhost:5432/template_db".to_string()
            });

            AppConfig {
                database_url,
                port: default_port(),
                host: default_host(),
            }
        })
    }
}
