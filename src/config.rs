//! Application configuration loaded from environment variables.
//!
//! We fail loud at startup if anything required is missing or nonsensical —
//! misconfiguration should never be a runtime surprise.

use std::env;
use std::net::SocketAddr;
use std::time::Duration;

use anyhow::{Context, Result, anyhow};

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
        let ollama_timeout_secs: u64 = get("OLLAMA_TIMEOUT_SECS")
            .unwrap_or_else(|| "120".into())
            .parse()
            .context("OLLAMA_TIMEOUT_SECS must be a positive integer")?;
        let ollama_timeout = Duration::from_secs(ollama_timeout_secs);

        let rate_limit_per_second: u64 = get("RATE_LIMIT_PER_SECOND")
            .unwrap_or_else(|| "10".into())
            .parse()
            .context("RATE_LIMIT_PER_SECOND must be a positive integer")?;
        let rate_limit_burst: u32 = get("RATE_LIMIT_BURST")
            .unwrap_or_else(|| "20".into())
            .parse()
            .context("RATE_LIMIT_BURST must be a positive integer")?;

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

    #[test]
    fn from_lookup_with_minimum_environment_returns_config() {
        let values = HashMap::from([(
            "GOOGLE_CLIENT_ID",
            "test-client-id.apps.googleusercontent.com".to_string(),
        )]);

        let config = Config::from_lookup(|name| values.get(name).cloned()).expect("valid config");

        assert_eq!(config.env, Environment::Development);
        assert_eq!(config.bind_addr, "127.0.0.1:8080".parse().unwrap());
        assert_eq!(config.public_origin, "http://localhost:8080");
        assert_eq!(
            config.google_client_id,
            "test-client-id.apps.googleusercontent.com"
        );
    }

    #[test]
    fn from_lookup_without_google_client_id_returns_required_error() {
        let error = Config::from_lookup(|_| None).expect_err("missing client id must fail");

        assert_eq!(error.to_string(), "GOOGLE_CLIENT_ID is required");
    }
}
