use crate::config::{AppConfig, ProviderType};
use anyhow::{anyhow, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChatMessage {
    pub role: String, // "system", "user", "assistant"
    pub content: String,
    pub timestamp: String,
    pub command_to_run: Option<String>,
}

pub struct LlmService {
    client: Client,
}

impl LlmService {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(60))
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn send_prompt(
        &self,
        config: &AppConfig,
        system_prompt: &str,
        history: &[ChatMessage],
    ) -> Result<String> {
        let provider_config = config
            .get_active_provider_config()
            .ok_or_else(|| anyhow!("Active provider config not found"))?;

        if provider_config.api_key.trim().is_empty() {
            return Err(anyhow!(
                "API Key is missing for {}. Press [Ctrl+P] to configure your API key.",
                config.active_provider.display_name()
            ));
        }

        match config.active_provider {
            ProviderType::Gemini => {
                self.call_gemini(&config.active_model, &provider_config.api_key, system_prompt, history).await
            }
            ProviderType::OpenAI => {
                let base_url = provider_config
                    .base_url
                    .as_deref()
                    .unwrap_or("https://api.openai.com/v1");
                self.call_openai(base_url, &config.active_model, &provider_config.api_key, system_prompt, history).await
            }
            ProviderType::Claude => {
                self.call_claude(&config.active_model, &provider_config.api_key, system_prompt, history).await
            }
            ProviderType::Cloudflare => {
                let account_id = provider_config
                    .account_id
                    .as_deref()
                    .ok_or_else(|| anyhow!("Cloudflare Account ID is required. Press [Ctrl+P] to enter it."))?;
                self.call_cloudflare(account_id, &config.active_model, &provider_config.api_key, system_prompt, history).await
            }
            ProviderType::OpenRouter => {
                self.call_openrouter(&config.active_model, &provider_config.api_key, system_prompt, history).await
            }
        }
    }

    async fn call_gemini(
        &self,
        model: &str,
        api_key: &str,
        system_prompt: &str,
        history: &[ChatMessage],
    ) -> Result<String> {
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models/{}:generateContent?key={}",
            model, api_key
        );

        let mut contents = Vec::new();
        for msg in history {
            let role = match msg.role.as_str() {
                "assistant" => "model",
                _ => "user",
            };
            contents.push(serde_json::json!({
                "role": role,
                "parts": [{ "text": msg.content }]
            }));
        }

        let body = serde_json::json!({
            "systemInstruction": {
                "parts": [{ "text": system_prompt }]
            },
            "contents": contents,
            "generationConfig": {
                "temperature": 0.4
            }
        });

        let resp = self.client.post(&url).json(&body).send().await?;
        let status = resp.status();
        let resp_text = resp.text().await?;

        if !status.is_success() {
            return Err(anyhow!("Gemini error ({status}): {resp_text}"));
        }

        let val: serde_json::Value = serde_json::from_str(&resp_text)?;
        if let Some(text) = val
            .pointer("/candidates/0/content/parts/0/text")
            .and_then(|v| v.as_str())
        {
            Ok(text.to_string())
        } else {
            Err(anyhow!("Unexpected response format from Gemini: {resp_text}"))
        }
    }

    async fn call_openai(
        &self,
        base_url: &str,
        model: &str,
        api_key: &str,
        system_prompt: &str,
        history: &[ChatMessage],
    ) -> Result<String> {
        let trimmed_base = base_url.trim_end_matches('/');
        let url = format!("{trimmed_base}/chat/completions");

        let mut messages = Vec::new();
        messages.push(serde_json::json!({
            "role": "system",
            "content": system_prompt
        }));

        for msg in history {
            messages.push(serde_json::json!({
                "role": msg.role,
                "content": msg.content
            }));
        }

        let body = serde_json::json!({
            "model": model,
            "messages": messages,
            "temperature": 0.5
        });

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        let resp_text = resp.text().await?;

        if !status.is_success() {
            return Err(anyhow!("OpenAI error ({status}): {resp_text}"));
        }

        let val: serde_json::Value = serde_json::from_str(&resp_text)?;
        if let Some(content) = val
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
        {
            Ok(content.to_string())
        } else {
            Err(anyhow!("Unexpected response format from OpenAI API: {resp_text}"))
        }
    }

    async fn call_openrouter(
        &self,
        model: &str,
        api_key: &str,
        system_prompt: &str,
        history: &[ChatMessage],
    ) -> Result<String> {
        let url = "https://openrouter.ai/api/v1/chat/completions";

        let mut messages = Vec::new();
        messages.push(serde_json::json!({
            "role": "system",
            "content": system_prompt
        }));

        for msg in history {
            messages.push(serde_json::json!({
                "role": msg.role,
                "content": msg.content
            }));
        }

        let body = serde_json::json!({
            "model": model,
            "messages": messages,
            "temperature": 0.5
        });

        let resp = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {api_key}"))
            .header("HTTP-Referer", "https://github.com/rust-agent-tui")
            .header("X-Title", "Rust Agentic Terminal")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        let resp_text = resp.text().await?;

        if !status.is_success() {
            return Err(anyhow!("OpenRouter error ({status}): {resp_text}"));
        }

        let val: serde_json::Value = serde_json::from_str(&resp_text)?;
        if let Some(content) = val
            .pointer("/choices/0/message/content")
            .and_then(|v| v.as_str())
        {
            Ok(content.to_string())
        } else {
            Err(anyhow!("Unexpected response format from OpenRouter: {resp_text}"))
        }
    }

    async fn call_claude(
        &self,
        model: &str,
        api_key: &str,
        system_prompt: &str,
        history: &[ChatMessage],
    ) -> Result<String> {
        let url = "https://api.anthropic.com/v1/messages";

        let mut messages = Vec::new();
        for msg in history {
            if msg.role != "system" {
                messages.push(serde_json::json!({
                    "role": msg.role,
                    "content": msg.content
                }));
            }
        }

        let body = serde_json::json!({
            "model": model,
            "max_tokens": 4096,
            "system": system_prompt,
            "messages": messages,
            "temperature": 0.5
        });

        let resp = self
            .client
            .post(url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        let resp_text = resp.text().await?;

        if !status.is_success() {
            return Err(anyhow!("Claude API error ({status}): {resp_text}"));
        }

        let val: serde_json::Value = serde_json::from_str(&resp_text)?;
        if let Some(content) = val
            .pointer("/content/0/text")
            .and_then(|v| v.as_str())
        {
            Ok(content.to_string())
        } else {
            Err(anyhow!("Unexpected response format from Claude API: {resp_text}"))
        }
    }

    async fn call_cloudflare(
        &self,
        account_id: &str,
        model: &str,
        api_token: &str,
        system_prompt: &str,
        history: &[ChatMessage],
    ) -> Result<String> {
        let url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{account_id}/ai/run/{model}"
        );

        let mut messages = Vec::new();
        messages.push(serde_json::json!({
            "role": "system",
            "content": system_prompt
        }));

        for msg in history {
            messages.push(serde_json::json!({
                "role": msg.role,
                "content": msg.content
            }));
        }

        let body = serde_json::json!({
            "messages": messages
        });

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {api_token}"))
            .json(&body)
            .send()
            .await?;

        let status = resp.status();
        let resp_text = resp.text().await?;

        if !status.is_success() {
            return Err(anyhow!("Cloudflare Workers AI error ({status}): {resp_text}"));
        }

        let val: serde_json::Value = serde_json::from_str(&resp_text)?;
        if let Some(response) = val.pointer("/result/response").and_then(|v| v.as_str()) {
            Ok(response.to_string())
        } else {
            Err(anyhow!("Unexpected response format from Cloudflare: {resp_text}"))
        }
    }
}
