//! Application configuration loaded from environment variables.
//!
//! We fail loud at startup if anything required is missing or nonsensical —
//! misconfiguration should never be a runtime surprise.

use std::env;
use std::net::SocketAddr;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};

fn parse_positive_u64(name: &str, raw: String) -> Result<u64> {
    let message = || format!("{name} must be a positive integer");
    let value = raw.parse::<u64>().with_context(message)?;
    if value == 0 {
        return Err(anyhow!(message()));
    }
    Ok(value)
}

fn parse_positive_u32(name: &str, raw: String) -> Result<u32> {
    let message = || format!("{name} must be a positive integer");
    let value = raw.parse::<u32>().with_context(message)?;
    if value == 0 {
        return Err(anyhow!(message()));
    }
    Ok(value)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Environment {
    Development,
    Production,
}

impl Environment {
    /// Cookies get `Secure` only in production; local dev typically runs on plain HTTP.
    pub fn cookies_require_secure(self) -> bool {
        matches!(self, Environment::Production)
    }
}

#[derive(Debug, Clone)]
pub struct Config {
    pub env: Environment,
    pub bind_addr: SocketAddr,
    pub public_origin: String,
    pub database_url: String,
    /// Public OAuth 2.0 client id from Google Cloud Console. Safe to embed
    /// in HTML; not a secret. Sign-in uses Google Identity Services — there
    /// is no client secret in this codebase.
    pub google_client_id: String,
    pub ollama_url: String,
    pub ollama_model: String,
    pub ollama_timeout: Duration,
    pub rate_limit_per_second: u64,
    pub rate_limit_burst: u32,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Self::from_lookup(|name| env::var(name).ok())
    }

    fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Result<Self> {
        let env = match get("APP_ENV")
            .unwrap_or_else(|| "development".into())
            .to_ascii_lowercase()
            .as_str()
        {
            "production" | "prod" => Environment::Production,
            "development" | "dev" | "" => Environment::Development,
            other => return Err(anyhow!("unknown APP_ENV `{other}`")),
        };

        let bind_addr: SocketAddr = get("BIND_ADDR")
            .unwrap_or_else(|| "127.0.0.1:8080".into())
            .parse()
            .context("BIND_ADDR must be a valid socket address, e.g. 0.0.0.0:8080")?;

        // Fallback uses `localhost` (not `bind_addr`) because Google GIS
        // rejects `127.0.0.1` for plain-HTTP local dev; the operator is
        // expected to override PUBLIC_ORIGIN in production anyway.
        let public_origin = get("PUBLIC_ORIGIN")
            .unwrap_or_else(|| format!("http://localhost:{}", bind_addr.port()));
        let public_origin = public_origin.trim_end_matches('/').to_string();

        let database_url =
            get("DATABASE_URL").unwrap_or_else(|| "sqlite://stuffy-council.sqlite?mode=rwc".into());

        let google_client_id = get("GOOGLE_CLIENT_ID").context("GOOGLE_CLIENT_ID is required")?;
        if google_client_id.trim().is_empty() {
            return Err(anyhow!("GOOGLE_CLIENT_ID is empty"));
        }

        let ollama_url = get("OLLAMA_URL").unwrap_or_else(|| "http://127.0.0.1:11434".into());
        let ollama_model =
            get("OLLAMA_MODEL").unwrap_or_else(|| "llama3.1:8b-instruct-q4_K_M".into());
        let ollama_timeout_secs = parse_positive_u64(
            "OLLAMA_TIMEOUT_SECS",
            get("OLLAMA_TIMEOUT_SECS").unwrap_or_else(|| "120".into()),
        )?;
        let ollama_timeout = Duration::from_secs(ollama_timeout_secs);

        let rate_limit_per_second = parse_positive_u64(
            "RATE_LIMIT_PER_SECOND",
            get("RATE_LIMIT_PER_SECOND").unwrap_or_else(|| "10".into()),
        )?;
        let rate_limit_burst = parse_positive_u32(
            "RATE_LIMIT_BURST",
            get("RATE_LIMIT_BURST").unwrap_or_else(|| "20".into()),
        )?;

        Ok(Config {
            env,
            bind_addr,
            public_origin,
            database_url,
            google_client_id,
            ollama_url,
            ollama_model,
            ollama_timeout,
            rate_limit_per_second,
            rate_limit_burst,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    // State-transition coverage is N/A: configuration parsing is pure and
    // does not retain or mutate state between calls.
    fn config_from(values: &[(&str, &str)]) -> Result<Config> {
        let values: HashMap<_, _> = values
            .iter()
            .map(|(name, value)| ((*name).to_string(), (*value).to_string()))
            .collect();
        Config::from_lookup(|name| values.get(name).cloned())
    }

    #[test]
    fn from_lookup_with_minimum_environment_returns_config() {
        let config = config_from(&[(
            "GOOGLE_CLIENT_ID",
            "test-client-id.apps.googleusercontent.com",
        )])
        .expect("valid config");

        assert_eq!(config.env, Environment::Development);
        assert_eq!(config.bind_addr, "127.0.0.1:8080".parse().unwrap());
        assert_eq!(config.public_origin, "http://localhost:8080");
        assert_eq!(
            config.database_url,
            "sqlite://stuffy-council.sqlite?mode=rwc"
        );
        assert_eq!(
            config.google_client_id,
            "test-client-id.apps.googleusercontent.com"
        );
        assert_eq!(config.ollama_url, "http://127.0.0.1:11434");
        assert_eq!(config.ollama_model, "llama3.1:8b-instruct-q4_K_M");
        assert_eq!(config.ollama_timeout, Duration::from_secs(120));
        assert_eq!(config.rate_limit_per_second, 10);
        assert_eq!(config.rate_limit_burst, 20);
    }

    #[test]
    fn from_lookup_with_full_environment_returns_configured_values() {
        let config = config_from(&[
            ("APP_ENV", "production"),
            ("BIND_ADDR", "0.0.0.0:9090"),
            ("PUBLIC_ORIGIN", "https://stories.example.test"),
            ("DATABASE_URL", "sqlite:///data/stories.sqlite?mode=rwc"),
            ("GOOGLE_CLIENT_ID", "configured-client-id"),
            ("OLLAMA_URL", "http://ollama.internal:11434"),
            ("OLLAMA_MODEL", "configured-model"),
            ("OLLAMA_TIMEOUT_SECS", "45"),
            ("RATE_LIMIT_PER_SECOND", "7"),
            ("RATE_LIMIT_BURST", "11"),
        ])
        .expect("valid config");

        assert_eq!(config.env, Environment::Production);
        assert!(config.env.cookies_require_secure());
        assert_eq!(config.bind_addr, "0.0.0.0:9090".parse().unwrap());
        assert_eq!(config.public_origin, "https://stories.example.test");
        assert_eq!(
            config.database_url,
            "sqlite:///data/stories.sqlite?mode=rwc"
        );
        assert_eq!(config.google_client_id, "configured-client-id");
        assert_eq!(config.ollama_url, "http://ollama.internal:11434");
        assert_eq!(config.ollama_model, "configured-model");
        assert_eq!(config.ollama_timeout, Duration::from_secs(45));
        assert_eq!(config.rate_limit_per_second, 7);
        assert_eq!(config.rate_limit_burst, 11);
    }

    #[test]
    fn from_lookup_without_google_client_id_returns_required_error() {
        let error = config_from(&[]).expect_err("missing client id must fail");

        assert_eq!(error.to_string(), "GOOGLE_CLIENT_ID is required");
    }

    #[test]
    fn from_lookup_with_blank_google_client_id_returns_empty_error() {
        let error =
            config_from(&[("GOOGLE_CLIENT_ID", "  \t")]).expect_err("blank client id must fail");

        assert_eq!(error.to_string(), "GOOGLE_CLIENT_ID is empty");
    }

    #[test]
    fn from_lookup_with_unknown_app_env_returns_named_error() {
        let error = config_from(&[
            ("APP_ENV", "staging"),
            ("GOOGLE_CLIENT_ID", "configured-client-id"),
        ])
        .expect_err("unknown environment must fail");

        assert_eq!(error.to_string(), "unknown APP_ENV `staging`");
    }

    #[test]
    fn from_lookup_with_empty_app_env_returns_development() {
        let config = config_from(&[
            ("APP_ENV", ""),
            ("GOOGLE_CLIENT_ID", "configured-client-id"),
        ])
        .expect("empty environment means development");

        assert_eq!(config.env, Environment::Development);
        assert!(!config.env.cookies_require_secure());
    }

    #[test]
    fn from_lookup_without_public_origin_uses_localhost_and_bind_port() {
        let config = config_from(&[
            ("BIND_ADDR", "0.0.0.0:9090"),
            ("GOOGLE_CLIENT_ID", "configured-client-id"),
        ])
        .expect("valid config");

        assert_eq!(config.public_origin, "http://localhost:9090");
    }

    #[test]
    fn from_lookup_with_public_origin_trailing_slashes_trims_all_slashes() {
        let config = config_from(&[
            ("PUBLIC_ORIGIN", "https://stories.example.test///"),
            ("GOOGLE_CLIENT_ID", "configured-client-id"),
        ])
        .expect("valid config");

        assert_eq!(config.public_origin, "https://stories.example.test");
    }

    #[test]
    fn from_lookup_with_invalid_bind_addr_returns_contextual_error() {
        let error = config_from(&[
            ("BIND_ADDR", "localhost:8080"),
            ("GOOGLE_CLIENT_ID", "configured-client-id"),
        ])
        .expect_err("invalid bind address must fail");

        assert_eq!(
            error.to_string(),
            "BIND_ADDR must be a valid socket address, e.g. 0.0.0.0:8080"
        );
    }

    #[test]
    fn from_lookup_with_non_integer_ollama_timeout_returns_positive_integer_error() {
        let error = config_from(&[
            ("GOOGLE_CLIENT_ID", "configured-client-id"),
            ("OLLAMA_TIMEOUT_SECS", "soon"),
        ])
        .expect_err("invalid timeout must fail");

        assert_eq!(
            error.to_string(),
            "OLLAMA_TIMEOUT_SECS must be a positive integer"
        );
    }

    #[test]
    fn from_lookup_with_zero_ollama_timeout_returns_positive_integer_error() {
        let error = config_from(&[
            ("GOOGLE_CLIENT_ID", "configured-client-id"),
            ("OLLAMA_TIMEOUT_SECS", "0"),
        ])
        .expect_err("zero timeout must fail");

        assert_eq!(
            error.to_string(),
            "OLLAMA_TIMEOUT_SECS must be a positive integer"
        );
    }

    #[test]
    fn from_lookup_with_non_integer_rate_per_second_returns_positive_integer_error() {
        let error = config_from(&[
            ("GOOGLE_CLIENT_ID", "configured-client-id"),
            ("RATE_LIMIT_PER_SECOND", "many"),
        ])
        .expect_err("invalid rate must fail");

        assert_eq!(
            error.to_string(),
            "RATE_LIMIT_PER_SECOND must be a positive integer"
        );
    }

    #[test]
    fn from_lookup_with_zero_rate_per_second_returns_positive_integer_error() {
        let error = config_from(&[
            ("GOOGLE_CLIENT_ID", "configured-client-id"),
            ("RATE_LIMIT_PER_SECOND", "0"),
        ])
        .expect_err("zero rate must fail");

        assert_eq!(
            error.to_string(),
            "RATE_LIMIT_PER_SECOND must be a positive integer"
        );
    }

    #[test]
    fn from_lookup_with_non_integer_rate_burst_returns_positive_integer_error() {
        let error = config_from(&[
            ("GOOGLE_CLIENT_ID", "configured-client-id"),
            ("RATE_LIMIT_BURST", "many"),
        ])
        .expect_err("invalid burst must fail");

        assert_eq!(
            error.to_string(),
            "RATE_LIMIT_BURST must be a positive integer"
        );
    }

    #[test]
    fn from_lookup_with_zero_rate_burst_returns_positive_integer_error() {
        let error = config_from(&[
            ("GOOGLE_CLIENT_ID", "configured-client-id"),
            ("RATE_LIMIT_BURST", "0"),
        ])
        .expect_err("zero burst must fail");

        assert_eq!(
            error.to_string(),
            "RATE_LIMIT_BURST must be a positive integer"
        );
    }
}
