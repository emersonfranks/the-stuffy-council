//! Ollama HTTP client (https://github.com/ollama/ollama/blob/main/docs/api.md).
//!
//! We use the `/api/generate` endpoint with streaming disabled — for a
//! bedtime-story-length response this is simpler than streaming and the
//! extra latency is not user-visible (generation is cached per day).

use std::time::Duration;

use anyhow::{Context, Result, anyhow};
use async_trait::async_trait;
use reqwest::Client;
use serde::{Deserialize, Serialize};

use super::StoryGenerator;

#[derive(Clone)]
pub struct OllamaGenerator {
    client: Client,
    base_url: String,
    model: String,
}

impl OllamaGenerator {
    pub fn new(base_url: impl Into<String>, model: impl Into<String>, timeout: Duration) -> Result<Self> {
        let client = Client::builder()
            .timeout(timeout)
            .user_agent(concat!("stuffy-council/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("building HTTP client for Ollama")?;
        Ok(Self {
            client,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            model: model.into(),
        })
    }
}

#[derive(Serialize)]
struct GenerateRequest<'a> {
    model: &'a str,
    prompt: &'a str,
    stream: bool,
    options: GenerateOptions,
}

#[derive(Serialize)]
struct GenerateOptions {
    /// Slightly warm — creative but not incoherent for small local models.
    temperature: f32,
    /// Cap output length to something sensible for bedtime.
    num_predict: i32,
    top_p: f32,
}

impl Default for GenerateOptions {
    fn default() -> Self {
        Self {
            temperature: 0.8,
            num_predict: 900,
            top_p: 0.95,
        }
    }
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
    #[serde(default)]
    done: bool,
}

#[async_trait]
impl StoryGenerator for OllamaGenerator {
    fn model_id(&self) -> &str {
        &self.model
    }

    async fn generate(&self, prompt: &str) -> Result<String> {
        let url = format!("{}/api/generate", self.base_url);
        let req = GenerateRequest {
            model: &self.model,
            prompt,
            stream: false,
            options: GenerateOptions::default(),
        };

        let resp = self
            .client
            .post(&url)
            .json(&req)
            .send()
            .await
            .with_context(|| format!("POST {url} failed (is Ollama running?)"))?;

        let status = resp.status();
        if !status.is_success() {
            let body = resp.text().await.unwrap_or_default();
            return Err(anyhow!(
                "Ollama returned HTTP {status}: {}",
                body.chars().take(500).collect::<String>()
            ));
        }

        let body: GenerateResponse = resp
            .json()
            .await
            .context("decoding Ollama /api/generate response")?;

        if !body.done {
            // With stream=false, Ollama always returns done=true on the final chunk.
            // If we ever see otherwise, treat as protocol drift.
            tracing::warn!("Ollama returned done=false with stream=false");
        }

        if body.response.trim().is_empty() {
            return Err(anyhow!("Ollama returned an empty response"));
        }

        Ok(body.response)
    }
}
