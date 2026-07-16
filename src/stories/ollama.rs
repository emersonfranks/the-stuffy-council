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
    #[serde(default)]
    done_reason: Option<String>,
    #[serde(default)]
    eval_count: Option<u64>,
}

impl GenerateResponse {
    fn into_complete_text(self) -> Result<String> {
        if !self.done {
            return Err(anyhow!(
                "Ollama returned done=false with stream=false; refusing partial output"
            ));
        }
        if self.done_reason.as_deref() == Some("length") {
            let count = self
                .eval_count
                .map(|value| format!(" after {value} output tokens"))
                .unwrap_or_default();
            return Err(anyhow!(
                "Ollama stopped at the output token limit{count}; refusing truncated output"
            ));
        }
        if let Some(reason) = self.done_reason.as_deref()
            && reason != "stop"
        {
            return Err(anyhow!(
                "Ollama returned unrecognized done_reason `{reason}`; refusing ambiguous output"
            ));
        }
        if self.response.trim().is_empty() {
            return Err(anyhow!("Ollama returned an empty response"));
        }
        Ok(self.response)
    }
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
        body.into_complete_text()
    }
}

#[cfg(test)]
mod tests {
    // Protocol completion validation covers functional, negative, and error
    // paths. Boundary and state-transition dimensions are N/A: the response
    // is a small immutable value with no bounded caller input or state.
    use super::*;

    #[test]
    fn into_complete_text_stopped_response_returns_text() {
        let response = GenerateResponse {
            response: "TITLE: Finished\n\nA complete story.".into(),
            done: true,
            done_reason: Some("stop".into()),
            eval_count: Some(42),
        };

        let text = response.into_complete_text().expect("complete response");

        assert_eq!(text, "TITLE: Finished\n\nA complete story.");
    }

    #[test]
    fn into_complete_text_legacy_json_without_completion_metadata_returns_text() {
        let response: GenerateResponse =
            serde_json::from_str(r#"{"response":"complete","done":true}"#)
                .expect("deserialize legacy response");

        let text = response.into_complete_text().expect("legacy response is complete");

        assert_eq!(text, "complete");
    }

    #[test]
    fn into_complete_text_length_response_rejects_truncated_output() {
        let response = GenerateResponse {
            response: "Once upon a".into(),
            done: true,
            done_reason: Some("length".into()),
            eval_count: Some(900),
        };

        let error = response.into_complete_text().expect_err("length is incomplete");

        assert_eq!(
            error.to_string(),
            "Ollama stopped at the output token limit after 900 output tokens; refusing truncated output"
        );
    }

    #[test]
    fn into_complete_text_done_false_rejects_partial_output() {
        let response = GenerateResponse {
            response: "partial".into(),
            done: false,
            done_reason: None,
            eval_count: None,
        };

        let error = response.into_complete_text().expect_err("done false is partial");

        assert_eq!(
            error.to_string(),
            "Ollama returned done=false with stream=false; refusing partial output"
        );
    }

    #[test]
    fn into_complete_text_unknown_done_reason_rejects_ambiguous_output() {
        let response = GenerateResponse {
            response: "possibly complete".into(),
            done: true,
            done_reason: Some("future_reason".into()),
            eval_count: Some(80),
        };

        let error = response
            .into_complete_text()
            .expect_err("unknown completion reason is ambiguous");

        assert_eq!(
            error.to_string(),
            "Ollama returned unrecognized done_reason `future_reason`; refusing ambiguous output"
        );
    }

    #[test]
    fn into_complete_text_blank_response_rejects_empty_output() {
        let response = GenerateResponse {
            response: " \n".into(),
            done: true,
            done_reason: Some("stop".into()),
            eval_count: Some(0),
        };

        let error = response.into_complete_text().expect_err("blank is invalid");

        assert_eq!(error.to_string(), "Ollama returned an empty response");
    }
}
