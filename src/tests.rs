#[cfg(test)]
mod tests {
    use crate::agent::AgentEngine;
    use crate::config::{AgentPermission, AppConfig, ProviderType};

    #[test]
    fn test_extract_command() {
        let text = "Here is the command to list files:\n```bash\nls -la\n```\nHope that helps!";
        let cmd = AgentEngine::extract_first_command(text);
        assert_eq!(cmd, Some("ls -la".to_string()));
    }

    #[test]
    fn test_extract_multiline_command() {
        let text = "Execute these steps:\n```sh\ncargo check\ncargo test\n```";
        let cmd = AgentEngine::extract_first_command(text);
        assert_eq!(cmd, Some("cargo check && cargo test".to_string()));
    }

    #[test]
    fn test_permission_cycle() {
        let p = AgentPermission::ConfirmEach;
        assert_eq!(p.next(), AgentPermission::AutoExecute);
        assert_eq!(p.next().next(), AgentPermission::ReadOnly);
        assert_eq!(p.next().next().next(), AgentPermission::ConfirmEach);
    }

    #[test]
    fn test_provider_defaults() {
        let providers = ProviderType::all();
        assert_eq!(providers.len(), 5);
        assert!(ProviderType::Gemini.default_models().contains(&"gemini-2.5-flash"));
        assert!(ProviderType::OpenAI.default_models().contains(&"gpt-4o-mini"));
        assert!(ProviderType::Claude.default_models().contains(&"claude-3-5-haiku-latest"));
        assert!(ProviderType::Cloudflare.default_models().contains(&"@cf/meta/llama-3.3-70b-instruct"));
        assert!(ProviderType::OpenRouter.default_models().contains(&"anthropic/claude-3.7-sonnet"));
    }

    #[test]
    fn test_config_serialization() {
        let config = AppConfig::default();
        let serialized = serde_json::to_string(&config).unwrap();
        let deserialized: AppConfig = serde_json::from_str(&serialized).unwrap();
        assert_eq!(config.active_provider, deserialized.active_provider);
        assert_eq!(deserialized.chat_panel_percent, 50);
    }
}
