use crate::providers::ChatMessage;
use chrono::Local;

pub struct AgentEngine;

impl AgentEngine {
    pub fn build_system_prompt(workspace_summary: &str) -> String {
        format!(
            r#"You are an expert AI Terminal Copilot with workspace control abilities.
You assist the user by answering questions, debugging code, and running commands in the terminal workspace.

{workspace_summary}

RULES FOR COMMAND EXECUTION:
1. When you want to execute a command or inspect the workspace, output the command in a single fenced code block tagged with bash or sh:
```bash
<command>
```
2. Keep commands concise and focused.
3. If no command is needed, answer directly.
4. Explain clearly what each command does before or after the code block.
5. Be helpful, concise, and accurate."#
        )
    }

    pub fn extract_first_command(text: &str) -> Option<String> {
        // Look for ```bash\n<cmd>\n``` or ```sh\n<cmd>\n``` or ```\n<cmd>\n```
        let markers = ["```bash", "```sh", "```zsh", "```shell", "```"];
        for marker in markers {
            if let Some(start_idx) = text.find(marker) {
                let rest = &text[start_idx + marker.len()..];
                // strip leading newline
                let command_part = rest.strip_prefix('\n').unwrap_or(rest);
                if let Some(end_idx) = command_part.find("```") {
                    let cmd = command_part[..end_idx].trim();
                    if !cmd.is_empty() && !cmd.contains('\n') {
                        return Some(cmd.to_string());
                    } else if !cmd.is_empty() {
                        // Take first line or combine lines if joined with && or \
                        let lines: Vec<&str> = cmd.lines().map(|l| l.trim()).filter(|l| !l.is_empty() && !l.starts_with('#')).collect();
                        if !lines.is_empty() {
                            return Some(lines.join(" && "));
                        }
                    }
                }
            }
        }
        None
    }

    pub fn create_user_message(content: String) -> ChatMessage {
        ChatMessage {
            role: "user".to_string(),
            content,
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            command_to_run: None,
        }
    }

    pub fn create_assistant_message(content: String, command: Option<String>) -> ChatMessage {
        ChatMessage {
            role: "assistant".to_string(),
            content,
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            command_to_run: command,
        }
    }

    pub fn create_system_message(content: String) -> ChatMessage {
        ChatMessage {
            role: "system".to_string(),
            content,
            timestamp: Local::now().format("%H:%M:%S").to_string(),
            command_to_run: None,
        }
    }
}
