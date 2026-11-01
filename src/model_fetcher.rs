use crate::config::{AppConfig, ProviderType};
use anyhow::Result;
use reqwest::Client;
use serde_json::Value;

pub struct ModelFetcher {
    client: Client,
}

impl ModelFetcher {
    pub fn new() -> Self {
        Self {
            client: Client::builder()
                .timeout(std::time::Duration::from_secs(10))
                .build()
                .unwrap_or_default(),
        }
    }

    pub async fn fetch_models(&self, config: &AppConfig, provider: ProviderType) -> Result<Vec<String>> {
        let p_cfg = config.get_provider_config(provider);
        let api_key = p_cfg.map(|c| c.api_key.as_str()).unwrap_or("");

        let mut models = match provider {
            ProviderType::OpenRouter => self.fetch_openrouter(api_key).await?,
            ProviderType::OpenAI => {
                let base_url = p_cfg
                    .and_then(|c| c.base_url.as_deref())
                    .unwrap_or("https://api.openai.com/v1");
                self.fetch_openai(base_url, api_key).await?
            }
            ProviderType::Gemini => self.fetch_gemini(api_key).await?,
            ProviderType::Claude => self.fetch_claude(api_key).await?,
            ProviderType::Cloudflare => {
                let account_id = p_cfg.and_then(|c| c.account_id.as_deref()).unwrap_or("");
                self.fetch_cloudflare(account_id, api_key).await?
            }
        };

        if models.is_empty() {
            // Fallback to built-in presets
            models = provider.default_models().iter().map(|&s| s.to_string()).collect();
        } else {
            models.sort();
            models.dedup();
        }

        Ok(models)
    }

    async fn fetch_openrouter(&self, api_key: &str) -> Result<Vec<String>> {
        let mut req = self.client.get("https://openrouter.ai/api/v1/models");
        if !api_key.is_empty() {
            req = req.header("Authorization", format!("Bearer {api_key}"));
        }
        let resp = req.send().await?;
        if !resp.status().is_success() {
            return Ok(Vec::new());
        }
        let val: Value = resp.json().await?;
        let mut list = Vec::new();
        if let Some(arr) = val.get("data").and_then(|d| d.as_array()) {
            for item in arr {
                if let Some(id) = item.get("id").and_then(|s| s.as_str()) {
                    list.push(id.to_string());
                }
            }
        }
        Ok(list)
    }

    async fn fetch_openai(&self, base_url: &str, api_key: &str) -> Result<Vec<String>> {
        if api_key.is_empty() {
            return Ok(Vec::new());
        }
        let trimmed = base_url.trim_end_matches('/');
        let url = format!("{trimmed}/models");
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {api_key}"))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(Vec::new());
        }

        let val: Value = resp.json().await?;
        let mut list = Vec::new();
        if let Some(arr) = val.get("data").and_then(|d| d.as_array()) {
            for item in arr {
                if let Some(id) = item.get("id").and_then(|s| s.as_str()) {
                    list.push(id.to_string());
                }
            }
        }
        Ok(list)
    }

    async fn fetch_gemini(&self, api_key: &str) -> Result<Vec<String>> {
        if api_key.is_empty() {
            return Ok(Vec::new());
        }
        let url = format!(
            "https://generativelanguage.googleapis.com/v1beta/models?key={api_key}"
        );
        let resp = self.client.get(&url).send().await?;
        if !resp.status().is_success() {
            return Ok(Vec::new());
        }
        let val: Value = resp.json().await?;
        let mut list = Vec::new();
        if let Some(arr) = val.get("models").and_then(|d| d.as_array()) {
            for item in arr {
                if let Some(name) = item.get("name").and_then(|s| s.as_str()) {
                    // name is "models/gemini-2.5-flash" -> strip "models/"
                    let clean = name.strip_prefix("models/").unwrap_or(name);
                    if clean.contains("gemini") {
                        list.push(clean.to_string());
                    }
                }
            }
        }
        Ok(list)
    }

    async fn fetch_claude(&self, api_key: &str) -> Result<Vec<String>> {
        if api_key.is_empty() {
            return Ok(Vec::new());
        }
        let resp = self
            .client
            .get("https://api.anthropic.com/v1/models")
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .send()
            .await?;

        if resp.status().is_success() {
            let val: Value = resp.json().await?;
            let mut list = Vec::new();
            if let Some(arr) = val.get("data").and_then(|d| d.as_array()) {
                for item in arr {
                    if let Some(id) = item.get("id").and_then(|s| s.as_str()) {
                        list.push(id.to_string());
                    }
                }
                if !list.is_empty() {
                    return Ok(list);
                }
            }
        }
        Ok(ProviderType::Claude.default_models().iter().map(|&s| s.to_string()).collect())
    }

    async fn fetch_cloudflare(&self, account_id: &str, api_token: &str) -> Result<Vec<String>> {
        if account_id.is_empty() || api_token.is_empty() {
            return Ok(Vec::new());
        }
        let url = format!(
            "https://api.cloudflare.com/client/v4/accounts/{account_id}/ai/models/search"
        );
        let resp = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {api_token}"))
            .send()
            .await?;

        if !resp.status().is_success() {
            return Ok(Vec::new());
        }

        let val: Value = resp.json().await?;
        let mut list = Vec::new();
        if let Some(arr) = val.get("result").and_then(|d| d.as_array()) {
            for item in arr {
                if let Some(name) = item.get("name").and_then(|s| s.as_str()) {
                    list.push(name.to_string());
                }
            }
        }
        Ok(list)
    }
}
