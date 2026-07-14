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
    pub session_secret: Vec<u8>,
    pub database_url: String,
    pub google_client_id: String,
    pub google_client_secret: String,
    pub google_redirect_url: String,
    pub ollama_url: String,
    pub ollama_model: String,
    pub ollama_timeout: Duration,
    pub rate_limit_per_second: u64,
    pub rate_limit_burst: u32,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        let env = match env::var("APP_ENV")
            .unwrap_or_else(|_| "development".into())
            .to_ascii_lowercase()
            .as_str()
        {
            "production" | "prod" => Environment::Production,
            "development" | "dev" | "" => Environment::Development,
            other => return Err(anyhow!("unknown APP_ENV `{other}`")),
        };

        let bind_addr: SocketAddr = env::var("BIND_ADDR")
            .unwrap_or_else(|_| "127.0.0.1:8080".into())
            .parse()
            .context("BIND_ADDR must be a valid socket address, e.g. 0.0.0.0:8080")?;

        let public_origin =
            env::var("PUBLIC_ORIGIN").unwrap_or_else(|_| format!("http://{bind_addr}"));
        let public_origin = public_origin.trim_end_matches('/').to_string();

        let session_secret_raw =
            env::var("SESSION_SECRET").context("SESSION_SECRET is required")?;
        if session_secret_raw.len() < 64 {
            return Err(anyhow!(
                "SESSION_SECRET must be at least 64 chars (got {})",
                session_secret_raw.len()
            ));
        }
        // Production must not use the shipped example value.
        if env == Environment::Production && session_secret_raw.contains("change-me") {
            return Err(anyhow!(
                "SESSION_SECRET looks like the example value; set a real secret in production"
            ));
        }
        let session_secret = session_secret_raw.into_bytes();

        let database_url = env::var("DATABASE_URL")
            .unwrap_or_else(|_| "sqlite://stuffy-council.sqlite?mode=rwc".into());

        let google_client_id =
            env::var("GOOGLE_CLIENT_ID").context("GOOGLE_CLIENT_ID is required")?;
        if google_client_id.trim().is_empty() {
            return Err(anyhow!("GOOGLE_CLIENT_ID is empty"));
        }
        // .env.example ships GOOGLE_CLIENT_SECRET empty; empty at boot means the
        // operator forgot to paste it in and the app cannot function.
        let google_client_secret =
            env::var("GOOGLE_CLIENT_SECRET").context("GOOGLE_CLIENT_SECRET is required")?;
        if google_client_secret.trim().is_empty() {
            return Err(anyhow!("GOOGLE_CLIENT_SECRET is empty"));
        }
        let google_redirect_url = env::var("GOOGLE_REDIRECT_URL")
            .unwrap_or_else(|_| format!("{public_origin}/auth/google/callback"));

        let ollama_url = env::var("OLLAMA_URL").unwrap_or_else(|_| "http://127.0.0.1:11434".into());
        let ollama_model =
            env::var("OLLAMA_MODEL").unwrap_or_else(|_| "llama3.1:8b-instruct-q4_K_M".into());
        let ollama_timeout_secs: u64 = env::var("OLLAMA_TIMEOUT_SECS")
            .unwrap_or_else(|_| "120".into())
            .parse()
            .context("OLLAMA_TIMEOUT_SECS must be a positive integer")?;
        let ollama_timeout = Duration::from_secs(ollama_timeout_secs);

        let rate_limit_per_second: u64 = env::var("RATE_LIMIT_PER_SECOND")
            .unwrap_or_else(|_| "10".into())
            .parse()
            .context("RATE_LIMIT_PER_SECOND must be a positive integer")?;
        let rate_limit_burst: u32 = env::var("RATE_LIMIT_BURST")
            .unwrap_or_else(|_| "20".into())
            .parse()
            .context("RATE_LIMIT_BURST must be a positive integer")?;

        Ok(Config {
            env,
            bind_addr,
            public_origin,
            session_secret,
            database_url,
            google_client_id,
            google_client_secret,
            google_redirect_url,
            ollama_url,
            ollama_model,
            ollama_timeout,
            rate_limit_per_second,
            rate_limit_burst,
        })
    }
}
