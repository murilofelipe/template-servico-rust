use serde::{Deserialize, Serialize};
use std::env;
use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum AppEnvironment {
    #[default]
    #[serde(
        alias = "dev",
        alias = "DEV",
        alias = "Dev",
        alias = "development",
        alias = "DEVELOPMENT",
        alias = "Development"
    )]
    Development,
    #[serde(
        alias = "prod",
        alias = "PROD",
        alias = "Prod",
        alias = "production",
        alias = "PRODUCTION",
        alias = "Production"
    )]
    Production,
    #[serde(alias = "test", alias = "TEST", alias = "Test")]
    Test,
}

impl AppEnvironment {
    #[must_use]
    pub fn is_production(&self) -> bool {
        matches!(self, Self::Production)
    }

    #[must_use]
    pub fn is_development(&self) -> bool {
        matches!(self, Self::Development)
    }

    #[must_use]
    pub fn is_test(&self) -> bool {
        matches!(self, Self::Test)
    }
}

impl fmt::Display for AppEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Development => write!(f, "development"),
            Self::Production => write!(f, "production"),
            Self::Test => write!(f, "test"),
        }
    }
}

impl FromStr for AppEnvironment {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "prod" | "production" => Ok(Self::Production),
            "dev" | "development" => Ok(Self::Development),
            "test" => Ok(Self::Test),
            other => Err(format!("Unknown environment: {other}")),
        }
    }
}

fn default_port() -> u16 {
    3000
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_environment() -> AppEnvironment {
    AppEnvironment::Development
}

fn default_allowed_origins() -> Vec<String> {
    Vec::new()
}

#[must_use]
pub fn parse_allowed_origins(raw: &str) -> Vec<String> {
    raw.split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

fn default_otel_service_name() -> String {
    "template-servico-rust".to_string()
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub database_url: String,
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_host")]
    pub host: String,
    #[serde(default = "default_environment")]
    pub environment: AppEnvironment,
    #[serde(default = "default_allowed_origins")]
    pub allowed_origins: Vec<String>,
    #[serde(default = "default_otel_service_name")]
    pub otel_service_name: String,
    pub otlp_endpoint: Option<String>,
}

impl AppConfig {
    #[must_use]
    pub fn load() -> Self {
        dotenvy::dotenv().ok();

        let env_from_var = env::var("ENVIRONMENT")
            .or_else(|_| env::var("APP_ENV"))
            .or_else(|_| env::var("RUST_ENV"))
            .ok()
            .and_then(|val| AppEnvironment::from_str(&val).ok());

        let mut config = envy::from_env::<AppConfig>().unwrap_or_else(|_| {
            let database_url = env::var("DATABASE_URL").unwrap_or_else(|_| {
                "postgres://postgres:postgres@localhost:5432/template_db".to_string()
            });
            let port = env::var("PORT")
                .ok()
                .and_then(|p| p.parse::<u16>().ok())
                .unwrap_or_else(default_port);
            let host = env::var("HOST").unwrap_or_else(|_| default_host());
            let allowed_origins = env::var("ALLOWED_ORIGINS")
                .map(|raw| parse_allowed_origins(&raw))
                .unwrap_or_default();

            let otel_service_name =
                env::var("OTEL_SERVICE_NAME").unwrap_or_else(|_| default_otel_service_name());
            let otlp_endpoint = env::var("OTLP_ENDPOINT").ok();

            AppConfig {
                database_url,
                port,
                host,
                environment: default_environment(),
                allowed_origins,
                otel_service_name,
                otlp_endpoint,
            }
        });

        if let Some(env) = env_from_var {
            config.environment = env;
        }

        if let Ok(origins_str) = env::var("ALLOWED_ORIGINS") {
            config.allowed_origins = parse_allowed_origins(&origins_str);
        } else {
            config.allowed_origins = config
                .allowed_origins
                .into_iter()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
        }

        config
    }
}

#[cfg(test)]
mod tests {
    use super::{
        default_allowed_origins, default_environment, default_host, default_port,
        parse_allowed_origins, AppConfig, AppEnvironment,
    };
    use std::str::FromStr;

    #[test]
    fn test_environment_from_str() {
        assert_eq!(
            AppEnvironment::from_str("production"),
            Ok(AppEnvironment::Production)
        );
        assert_eq!(
            AppEnvironment::from_str("prod"),
            Ok(AppEnvironment::Production)
        );
        assert_eq!(
            AppEnvironment::from_str("PRODUCTION"),
            Ok(AppEnvironment::Production)
        );
        assert_eq!(
            AppEnvironment::from_str("development"),
            Ok(AppEnvironment::Development)
        );
        assert_eq!(
            AppEnvironment::from_str("dev"),
            Ok(AppEnvironment::Development)
        );
        assert_eq!(AppEnvironment::from_str("test"), Ok(AppEnvironment::Test));
        assert!(AppEnvironment::from_str("invalid").is_err());
    }

    #[test]
    fn test_environment_display() {
        assert_eq!(format!("{}", AppEnvironment::Development), "development");
        assert_eq!(format!("{}", AppEnvironment::Production), "production");
        assert_eq!(format!("{}", AppEnvironment::Test), "test");
    }

    #[test]
    fn test_environment_predicates() {
        assert!(AppEnvironment::Production.is_production());
        assert!(!AppEnvironment::Production.is_development());
        assert!(!AppEnvironment::Production.is_test());

        assert!(AppEnvironment::Development.is_development());
        assert!(!AppEnvironment::Development.is_production());

        assert!(AppEnvironment::Test.is_test());
        assert!(!AppEnvironment::Test.is_production());
    }

    #[test]
    fn test_app_config_defaults() {
        assert_eq!(default_port(), 3000);
        assert_eq!(default_host(), "0.0.0.0");
        assert_eq!(default_environment(), AppEnvironment::Development);
        assert_eq!(default_allowed_origins(), Vec::<String>::new());
    }

    #[test]
    fn test_parse_allowed_origins() {
        assert_eq!(
            parse_allowed_origins("https://example.com"),
            vec!["https://example.com"]
        );
        assert_eq!(
            parse_allowed_origins("https://a.com, https://b.com ,  https://c.com "),
            vec!["https://a.com", "https://b.com", "https://c.com"]
        );
        assert_eq!(
            parse_allowed_origins(",https://a.com,,https://b.com,"),
            vec!["https://a.com", "https://b.com"]
        );
        assert_eq!(parse_allowed_origins("   "), Vec::<String>::new());
        assert_eq!(parse_allowed_origins(""), Vec::<String>::new());
    }

    #[test]
    fn test_app_config_deserialization() {
        let json_data = r#"{"database_url":"postgres://localhost/test","port":8080,"host":"127.0.0.1","environment":"production","allowed_origins":["https://app.example.com"]}"#;
        let config: Result<AppConfig, _> = serde_json::from_str(json_data);
        assert!(config.is_ok());
        let cfg = config.unwrap_or_else(|_| AppConfig {
            database_url: String::new(),
            port: 0,
            host: String::new(),
            environment: AppEnvironment::Development,
            allowed_origins: Vec::new(),
            otel_service_name: "template-servico-rust".to_string(),
            otlp_endpoint: None,
        });
        assert_eq!(cfg.port, 8080);
        assert_eq!(cfg.host, "127.0.0.1");
        assert_eq!(cfg.environment, AppEnvironment::Production);
        assert_eq!(cfg.allowed_origins, vec!["https://app.example.com"]);

        let json_prod_alias = r#"{"database_url":"postgres://localhost/test","port":8080,"host":"127.0.0.1","environment":"prod"}"#;
        let cfg_alias: AppConfig =
            serde_json::from_str(json_prod_alias).unwrap_or_else(|_| AppConfig {
                database_url: String::new(),
                port: 0,
                host: String::new(),
                environment: AppEnvironment::Development,
                allowed_origins: Vec::new(),
                otel_service_name: "template-servico-rust".to_string(),
                otlp_endpoint: None,
            });
        assert_eq!(cfg_alias.environment, AppEnvironment::Production);
        assert_eq!(cfg_alias.allowed_origins, Vec::<String>::new());

        let json_dev_alias = r#"{"database_url":"postgres://localhost/test","port":8080,"host":"127.0.0.1","environment":"dev"}"#;
        let cfg_dev: AppConfig =
            serde_json::from_str(json_dev_alias).unwrap_or_else(|_| AppConfig {
                database_url: String::new(),
                port: 0,
                host: String::new(),
                environment: AppEnvironment::Production,
                allowed_origins: Vec::new(),
                otel_service_name: "template-servico-rust".to_string(),
                otlp_endpoint: None,
            });
        assert_eq!(cfg_dev.environment, AppEnvironment::Development);
        assert_eq!(cfg_dev.allowed_origins, Vec::<String>::new());
    }
}
