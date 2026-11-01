use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProviderType {
    Gemini,
    OpenAI,
    Claude,
    Cloudflare,
    OpenRouter,
}

impl ProviderType {
    pub fn all() -> &'static [ProviderType] {
        &[
            ProviderType::Gemini,
            ProviderType::OpenAI,
            ProviderType::Claude,
            ProviderType::Cloudflare,
            ProviderType::OpenRouter,
        ]
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            ProviderType::Gemini => "Google Gemini",
            ProviderType::OpenAI => "ChatGPT / OpenAI Compatible",
            ProviderType::Claude => "Anthropic Claude",
            ProviderType::Cloudflare => "Cloudflare Workers AI",
            ProviderType::OpenRouter => "OpenRouter",
        }
    }

    pub fn default_models(&self) -> &'static [&'static str] {
        match self {
            ProviderType::Gemini => &[
                "gemini-2.5-flash",
                "gemini-2.5-pro",
                "gemini-2.0-flash",
                "gemini-1.5-flash",
            ],
            ProviderType::OpenAI => &[
                "gpt-4o",
                "gpt-4o-mini",
                "o3-mini",
                "deepseek-chat",
            ],
            ProviderType::Claude => &[
                "claude-3-7-sonnet-latest",
                "claude-3-5-sonnet-latest",
                "claude-3-5-haiku-latest",
            ],
            ProviderType::Cloudflare => &[
                "@cf/meta/llama-3.3-70b-instruct",
                "@cf/meta/llama-3.1-8b-instruct",
                "@cf/mistral/mistral-7b-instruct-v0.2",
                "@cf/qwen/qwen2.5-72b-instruct",
            ],
            ProviderType::OpenRouter => &[
                "anthropic/claude-3.7-sonnet",
                "meta-llama/llama-3.3-70b-instruct",
                "deepseek/deepseek-chat",
                "google/gemini-2.0-flash-001",
                "openai/gpt-4o-mini",
            ],
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentPermission {
    ConfirmEach, // Ask user to approve command
    AutoExecute, // Run commands automatically
    ReadOnly,    // Agent cannot execute workspace commands
}

impl AgentPermission {
    pub fn label(&self) -> &'static str {
        match self {
            AgentPermission::ConfirmEach => "[CONFIRM]",
            AgentPermission::AutoExecute => "[AUTO-EXEC]",
            AgentPermission::ReadOnly => "[READ-ONLY]",
        }
    }

    pub fn next(&self) -> Self {
        match self {
            AgentPermission::ConfirmEach => AgentPermission::AutoExecute,
            AgentPermission::AutoExecute => AgentPermission::ReadOnly,
            AgentPermission::ReadOnly => AgentPermission::ConfirmEach,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderConfig {
    pub api_key: String,
    pub base_url: Option<String>,
    pub account_id: Option<String>, // For Cloudflare
    pub default_model: String,
}

impl Default for ProviderConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: None,
            account_id: None,
            default_model: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub active_provider: ProviderType,
    pub active_model: String,
    pub permission: AgentPermission,
    pub providers: HashMap<String, ProviderConfig>,
    pub workspace_path: String,
    #[serde(default = "default_chat_panel_percent")]
    pub chat_panel_percent: u16,
}

fn default_chat_panel_percent() -> u16 {
    50
}

impl Default for AppConfig {
    fn default() -> Self {
        let mut providers = HashMap::new();
        providers.insert(
            "Gemini".to_string(),
            ProviderConfig {
                api_key: std::env::var("GEMINI_API_KEY").unwrap_or_default(),
                base_url: None,
                account_id: None,
                default_model: "gemini-2.5-flash".to_string(),
            },
        );
        providers.insert(
            "OpenAI".to_string(),
            ProviderConfig {
                api_key: std::env::var("OPENAI_API_KEY").unwrap_or_default(),
                base_url: Some("https://api.openai.com/v1".to_string()),
                account_id: None,
                default_model: "gpt-4o-mini".to_string(),
            },
        );
        providers.insert(
            "Claude".to_string(),
            ProviderConfig {
                api_key: std::env::var("ANTHROPIC_API_KEY").unwrap_or_default(),
                base_url: None,
                account_id: None,
                default_model: "claude-3-5-haiku-latest".to_string(),
            },
        );
        providers.insert(
            "Cloudflare".to_string(),
            ProviderConfig {
                api_key: std::env::var("CLOUDFLARE_API_TOKEN").unwrap_or_default(),
                base_url: None,
                account_id: std::env::var("CLOUDFLARE_ACCOUNT_ID").ok(),
                default_model: "@cf/meta/llama-3.3-70b-instruct".to_string(),
            },
        );
        providers.insert(
            "OpenRouter".to_string(),
            ProviderConfig {
                api_key: std::env::var("OPENROUTER_API_KEY").unwrap_or_default(),
                base_url: Some("https://openrouter.ai/api/v1".to_string()),
                account_id: None,
                default_model: "anthropic/claude-3.7-sonnet".to_string(),
            },
        );

        let current_dir = std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .to_string_lossy()
            .to_string();

        Self {
            active_provider: ProviderType::Gemini,
            active_model: "gemini-2.5-flash".to_string(),
            permission: AgentPermission::ConfirmEach,
            providers,
            workspace_path: current_dir,
            chat_panel_percent: 50,
        }
    }
}

impl AppConfig {
    pub fn config_path() -> PathBuf {
        if let Some(config_dir) = dirs::config_dir() {
            let app_dir = config_dir.join("rust-agent-tui");
            let _ = fs::create_dir_all(&app_dir);
            app_dir.join("config.json")
        } else {
            PathBuf::from("agent_config.json")
        }
    }

    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            if let Ok(content) = fs::read_to_string(&path) {
                if let Ok(mut config) = serde_json::from_str::<AppConfig>(&content) {
                    // Update current dir
                    if let Ok(cd) = std::env::current_dir() {
                        config.workspace_path = cd.to_string_lossy().to_string();
                    }
                    if config.chat_panel_percent == 0 {
                        config.chat_panel_percent = 50;
                    }
                    return config;
                }
            }
        }
        let config = AppConfig::default();
        let _ = config.save();
        config
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        let json = serde_json::to_string_pretty(self)?;
        fs::write(path, json)?;
        Ok(())
    }

    pub fn provider_key(&self, provider: ProviderType) -> &str {
        match provider {
            ProviderType::Gemini => "Gemini",
            ProviderType::OpenAI => "OpenAI",
            ProviderType::Claude => "Claude",
            ProviderType::Cloudflare => "Cloudflare",
            ProviderType::OpenRouter => "OpenRouter",
        }
    }

    pub fn get_provider_config(&self, provider: ProviderType) -> Option<&ProviderConfig> {
        self.providers.get(self.provider_key(provider))
    }

    pub fn get_active_provider_config(&self) -> Option<&ProviderConfig> {
        self.get_provider_config(self.active_provider)
    }

    pub fn set_provider_config(
        &mut self,
        provider: ProviderType,
        api_key: String,
        base_url: Option<String>,
        account_id: Option<String>,
        default_model: String,
    ) {
        let key = self.provider_key(provider).to_string();
        self.providers.insert(
            key,
            ProviderConfig {
                api_key,
                base_url,
                account_id,
                default_model: default_model.clone(),
            },
        );
        if self.active_provider == provider {
            self.active_model = default_model;
        }
        let _ = self.save();
    }
}
