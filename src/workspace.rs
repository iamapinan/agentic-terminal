use anyhow::Result;
use chrono::Local;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

#[derive(Debug, Clone)]
pub struct WorkspaceEntry {
    pub name: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone)]
pub struct TerminalLogLine {
    pub timestamp: String,
    pub is_command: bool,
    pub is_error: bool,
    pub is_agent: bool,
    pub text: String,
}

pub struct WorkspaceManager {
    pub current_dir: PathBuf,
    pub history_logs: Vec<TerminalLogLine>,
    pub max_logs: usize,
    pub is_running: bool,
}

impl WorkspaceManager {
    pub fn new() -> Self {
        let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let mut mgr = Self {
            current_dir,
            history_logs: Vec::new(),
            max_logs: 1000,
            is_running: false,
        };
        mgr.add_log(false, false, false, "Terminal Workspace initialized.".to_string());
        mgr.add_log(
            false,
            false,
            false,
            format!("Directory: {}", mgr.current_dir.display()),
        );
        mgr
    }

    pub fn add_log(&mut self, is_command: bool, is_error: bool, is_agent: bool, text: String) {
        let timestamp = Local::now().format("%H:%M:%S").to_string();
        self.history_logs.push(TerminalLogLine {
            timestamp,
            is_command,
            is_error,
            is_agent,
            text,
        });

        if self.history_logs.len() > self.max_logs {
            let remove_count = self.history_logs.len() - self.max_logs;
            self.history_logs.drain(0..remove_count);
        }
    }

    pub fn list_files(&self) -> Vec<WorkspaceEntry> {
        let mut entries = Vec::new();
        if let Ok(read_dir) = std::fs::read_dir(&self.current_dir) {
            for entry in read_dir.flatten() {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with('.') && name != ".env" {
                    continue;
                }
                let file_type = entry.file_type();
                let is_dir = file_type.map(|t| t.is_dir()).unwrap_or(false);
                entries.push(WorkspaceEntry { name, is_dir });
            }
        }
        entries.sort_by(|a, b| {
            if a.is_dir == b.is_dir {
                a.name.cmp(&b.name)
            } else if a.is_dir {
                std::cmp::Ordering::Less
            } else {
                std::cmp::Ordering::Greater
            }
        });
        entries
    }

    pub fn get_workspace_summary(&self) -> String {
        let files = self.list_files();
        let file_names: Vec<String> = files
            .into_iter()
            .take(25)
            .map(|f| if f.is_dir { format!("{}/", f.name) } else { f.name })
            .collect();

        format!(
            "Workspace: {}\nFiles in directory:\n{}",
            self.current_dir.display(),
            file_names.join(", ")
        )
    }

    pub async fn run_shell_command(&mut self, cmd_str: &str, is_agent: bool) -> Result<(i32, String)> {
        let trimmed = cmd_str.trim();
        if trimmed.is_empty() {
            return Ok((0, String::new()));
        }

        let prefix = if is_agent { "[AGENT]" } else { ">" };
        self.add_log(true, false, is_agent, format!("{prefix} {trimmed}"));

        // Handle `cd` command internally
        if trimmed.starts_with("cd ") || trimmed == "cd" {
            let target = trimmed.strip_prefix("cd").unwrap().trim();
            let new_path = if target.is_empty() || target == "~" {
                dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
            } else if target.starts_with('/') {
                PathBuf::from(target)
            } else {
                self.current_dir.join(target)
            };

            if let Ok(canonical) = new_path.canonicalize() {
                self.current_dir = canonical;
                self.add_log(false, false, is_agent, format!("Changed dir to: {}", self.current_dir.display()));
                return Ok((0, format!("Directory changed to {}", self.current_dir.display())));
            } else {
                let err_msg = format!("cd: no such file or directory: {target}");
                self.add_log(false, true, is_agent, err_msg.clone());
                return Ok((1, err_msg));
            }
        }

        self.is_running = true;

        let shell = if cfg!(target_os = "macos") {
            "/bin/zsh"
        } else {
            "/bin/sh"
        };

        let mut child = Command::new(shell)
            .arg("-c")
            .arg(trimmed)
            .current_dir(&self.current_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        let mut output_collector = Vec::new();

        if let Some(stdout) = child.stdout.take() {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                self.add_log(false, false, is_agent, line.clone());
                output_collector.push(line);
            }
        }

        if let Some(stderr) = child.stderr.take() {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                self.add_log(false, true, is_agent, line.clone());
                output_collector.push(line);
            }
        }

        let status = child.wait().await?;
        let code = status.code().unwrap_or(-1);

        self.is_running = false;
        if code == 0 {
            self.add_log(false, false, is_agent, "[OK] Process exited with code 0".to_string());
        } else {
            self.add_log(false, true, is_agent, format!("[ERR] Process exited with code {code}"));
        }

        Ok((code, output_collector.join("\n")))
    }
}
