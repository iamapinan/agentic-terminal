use eframe::egui::{self, Color32, Margin, RichText, Rounding, Stroke, Vec2};
use rust_agent_tui::agent::AgentEngine;
use rust_agent_tui::config::{AgentPermission, AppConfig, ProviderType};
use rust_agent_tui::model_fetcher::ModelFetcher;
use rust_agent_tui::providers::{ChatMessage, LlmService};
use rust_agent_tui::workspace::WorkspaceManager;
use std::path::PathBuf;
use std::process::Stdio;
use std::sync::mpsc;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::Command;

// Vector / Unicode UI Icons following better-icons design system
struct Icons;
impl Icons {
    const APP: &'static str = "◆";
    const FOLDER: &'static str = "📁";
    const TERMINAL: &'static str = ">_";
    const BOT: &'static str = "🤖";
    const GEAR: &'static str = "⚙";
    const SEARCH: &'static str = "🔍";
    const SHIELD: &'static str = "🛡";
    const PLAY: &'static str = "▶";
    const CHECK: &'static str = "✓";
    const CROSS: &'static str = "✕";
    const REFRESH: &'static str = "↻";
    const TRASH: &'static str = "🗑";
    const COPY: &'static str = "📋";
    const SEND: &'static str = "➤";
}

enum GuiEvent {
    AgentSuccess { reply: String, command: Option<String> },
    AgentFailed { error: String },
    ModelsLoaded { provider: ProviderType, models: Vec<String> },
    TerminalLine { is_error: bool, is_agent: bool, text: String },
    CommandFinished { code: i32, is_agent: bool, full_output: String, new_dir: Option<PathBuf> },
}

struct AgenticGuiApp {
    config: AppConfig,
    workspace: WorkspaceManager,
    llm_service: Arc<LlmService>,
    model_fetcher: Arc<ModelFetcher>,

    // Event Bus
    event_tx: mpsc::Sender<GuiEvent>,
    event_rx: mpsc::Receiver<GuiEvent>,

    // UI state
    chat_input: String,
    workspace_input: String,
    chat_messages: Vec<ChatMessage>,
    is_thinking: bool,
    is_command_running: bool,
    pending_command: Option<String>,

    // Modals
    show_model_dialog: bool,
    show_settings_dialog: bool,
    model_search_query: String,
    cached_models: Vec<String>,
    is_loading_models: bool,
    models_status: String,

    // Settings fields
    settings_api_key: String,
    settings_base_url: String,
    settings_account_id: String,
}

impl AgenticGuiApp {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // macOS Native Dark Styling
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(13, 16, 23);
        visuals.window_fill = Color32::from_rgb(21, 25, 35);
        visuals.extreme_bg_color = Color32::from_rgb(9, 11, 16);
        visuals.widgets.noninteractive.bg_fill = Color32::from_rgb(18, 22, 31);
        visuals.widgets.inactive.bg_fill = Color32::from_rgb(26, 31, 44);
        visuals.widgets.hovered.bg_fill = Color32::from_rgb(38, 45, 62);
        visuals.widgets.active.bg_fill = Color32::from_rgb(56, 189, 248);
        visuals.window_rounding = Rounding::same(10.0);
        visuals.window_stroke = Stroke::new(1.0_f32, Color32::from_rgb(45, 55, 72));
        cc.egui_ctx.set_visuals(visuals);

        let config = AppConfig::load();
        let (event_tx, event_rx) = mpsc::channel();
        let llm_service = Arc::new(LlmService::new());
        let model_fetcher = Arc::new(ModelFetcher::new());

        let mut app = Self {
            workspace: WorkspaceManager::new(),
            llm_service,
            model_fetcher,
            event_tx,
            event_rx,
            chat_input: String::new(),
            workspace_input: String::new(),
            chat_messages: Vec::new(),
            is_thinking: false,
            is_command_running: false,
            pending_command: None,
            show_model_dialog: false,
            show_settings_dialog: false,
            model_search_query: String::new(),
            cached_models: config.active_provider.default_models().iter().map(|&s| s.to_string()).collect(),
            is_loading_models: false,
            models_status: "Initial models loaded".to_string(),
            settings_api_key: String::new(),
            settings_base_url: String::new(),
            settings_account_id: String::new(),
            config,
        };

        app.chat_messages.push(AgentEngine::create_system_message(format!(
            "Welcome to Agentic Terminal Desktop. Active Provider: {} | Model: {}. The Agent can run commands directly in the workspace.",
            app.config.active_provider.display_name(),
            app.config.active_model
        )));

        // Automatically fetch live models on startup
        app.load_models_for_provider(app.config.active_provider);

        app
    }

    fn load_models_for_provider(&mut self, provider: ProviderType) {
        self.is_loading_models = true;
        self.models_status = format!("Discovering live models from {}...", provider.display_name());
        let fetcher = self.model_fetcher.clone();
        let cfg = self.config.clone();
        let tx = self.event_tx.clone();

        tokio::spawn(async move {
            match fetcher.fetch_models(&cfg, provider).await {
                Ok(models) => {
                    let _ = tx.send(GuiEvent::ModelsLoaded { provider, models });
                }
                Err(_e) => {
                    let fallback = provider.default_models().iter().map(|&s| s.to_string()).collect();
                    let _ = tx.send(GuiEvent::ModelsLoaded { provider, models: fallback });
                }
            }
        });
    }

    fn send_chat_prompt(&mut self) {
        let prompt = self.chat_input.trim().to_string();
        if prompt.is_empty() || self.is_thinking {
            return;
        }

        self.chat_input.clear();
        self.chat_messages.push(AgentEngine::create_user_message(prompt));
        self.is_thinking = true;

        let cfg = self.config.clone();
        let ws_summary = self.workspace.get_workspace_summary();
        let history = self.chat_messages.clone();
        let service = self.llm_service.clone();
        let tx = self.event_tx.clone();

        tokio::spawn(async move {
            let sys_prompt = AgentEngine::build_system_prompt(&ws_summary);
            match service.send_prompt(&cfg, &sys_prompt, &history).await {
                Ok(reply) => {
                    let cmd = AgentEngine::extract_first_command(&reply);
                    let _ = tx.send(GuiEvent::AgentSuccess { reply, command: cmd });
                }
                Err(e) => {
                    let _ = tx.send(GuiEvent::AgentFailed { error: e.to_string() });
                }
            }
        });
    }

    // Direct streaming execution of commands directly to the terminal!
    fn run_command_directly(&mut self, cmd: String, is_agent: bool) {
        let trimmed = cmd.trim().to_string();
        if trimmed.is_empty() {
            return;
        }

        self.is_command_running = true;
        let prefix = if is_agent { "[AGENT]" } else { ">" };
        self.workspace.add_log(true, false, is_agent, format!("{prefix} {trimmed}"));

        let tx = self.event_tx.clone();
        let current_dir = self.workspace.current_dir.clone();

        tokio::spawn(async move {
            // Handle `cd` internally
            if trimmed.starts_with("cd ") || trimmed == "cd" {
                let target = trimmed.strip_prefix("cd").unwrap().trim();
                let new_path = if target.is_empty() || target == "~" {
                    dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"))
                } else if target.starts_with('/') {
                    PathBuf::from(target)
                } else {
                    current_dir.join(target)
                };

                if let Ok(canonical) = new_path.canonicalize() {
                    let msg = format!("Directory changed to: {}", canonical.display());
                    let _ = tx.send(GuiEvent::TerminalLine { is_error: false, is_agent, text: msg.clone() });
                    let _ = tx.send(GuiEvent::CommandFinished { code: 0, is_agent, full_output: msg, new_dir: Some(canonical) });
                } else {
                    let err = format!("cd: no such file or directory: {target}");
                    let _ = tx.send(GuiEvent::TerminalLine { is_error: true, is_agent, text: err.clone() });
                    let _ = tx.send(GuiEvent::CommandFinished { code: 1, is_agent, full_output: err, new_dir: None });
                }
                return;
            }

            let shell = if cfg!(target_os = "macos") { "/bin/zsh" } else { "/bin/sh" };

            let spawn_res = Command::new(shell)
                .arg("-c")
                .arg(&trimmed)
                .current_dir(&current_dir)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn();

            match spawn_res {
                Ok(mut child) => {
                    let mut collector = Vec::new();

                    if let Some(stdout) = child.stdout.take() {
                        let mut reader = BufReader::new(stdout).lines();
                        while let Ok(Some(line)) = reader.next_line().await {
                            let _ = tx.send(GuiEvent::TerminalLine { is_error: false, is_agent, text: line.clone() });
                            collector.push(line);
                        }
                    }

                    if let Some(stderr) = child.stderr.take() {
                        let mut reader = BufReader::new(stderr).lines();
                        while let Ok(Some(line)) = reader.next_line().await {
                            let _ = tx.send(GuiEvent::TerminalLine { is_error: true, is_agent, text: line.clone() });
                            collector.push(line);
                        }
                    }

                    let code = match child.wait().await {
                        Ok(st) => st.code().unwrap_or(-1),
                        Err(_) => -1,
                    };

                    let full_output = collector.join("\n");
                    let _ = tx.send(GuiEvent::CommandFinished { code, is_agent, full_output, new_dir: None });
                }
                Err(e) => {
                    let err = format!("Failed to spawn process: {e}");
                    let _ = tx.send(GuiEvent::TerminalLine { is_error: true, is_agent, text: err.clone() });
                    let _ = tx.send(GuiEvent::CommandFinished { code: -1, is_agent, full_output: err, new_dir: None });
                }
            }
        });
    }
}

impl eframe::App for AgenticGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Handle background async events
        while let Ok(ev) = self.event_rx.try_recv() {
            match ev {
                GuiEvent::AgentSuccess { reply, command } => {
                    self.is_thinking = false;
                    self.chat_messages.push(AgentEngine::create_assistant_message(reply, command.clone()));

                    if let Some(cmd) = command {
                        match self.config.permission {
                            AgentPermission::AutoExecute => {
                                self.chat_messages.push(AgentEngine::create_system_message(
                                    format!("[AUTO-EXEC] Running agent command: `{cmd}`")
                                ));
                                self.run_command_directly(cmd, true);
                            }
                            AgentPermission::ConfirmEach => {
                                self.pending_command = Some(cmd);
                            }
                            AgentPermission::ReadOnly => {
                                self.chat_messages.push(AgentEngine::create_system_message(
                                    format!("[READ-ONLY] Agent suggested command `{cmd}`, but permission is Read-Only.")
                                ));
                            }
                        }
                    }
                }
                GuiEvent::AgentFailed { error } => {
                    self.is_thinking = false;
                    self.chat_messages.push(AgentEngine::create_system_message(format!("[ERROR] {error}")));
                }
                GuiEvent::ModelsLoaded { provider, models } => {
                    self.is_loading_models = false;
                    self.models_status = format!("{} models available on {}", models.len(), provider.display_name());
                    self.cached_models = models;
                }
                GuiEvent::TerminalLine { is_error, is_agent, text } => {
                    self.workspace.add_log(false, is_error, is_agent, text);
                }
                GuiEvent::CommandFinished { code, is_agent, full_output, new_dir } => {
                    self.is_command_running = false;
                    if let Some(nd) = new_dir {
                        self.workspace.current_dir = nd;
                    }

                    if code == 0 {
                        self.workspace.add_log(false, false, is_agent, "[OK] Process exited with code 0".to_string());
                    } else {
                        self.workspace.add_log(false, true, is_agent, format!("[ERR] Process exited with code {code}"));
                    }

                    // Feed result back to the Copilot so it knows the terminal output!
                    if is_agent {
                        let snippet = if full_output.len() > 600 {
                            format!("{}...\n[Output truncated]", &full_output[..600])
                        } else if full_output.is_empty() {
                            "(No output)".to_string()
                        } else {
                            full_output
                        };
                        self.chat_messages.push(AgentEngine::create_system_message(
                            format!("[Command Result Exit {code}]:\n```\n{snippet}\n```")
                        ));
                    }
                }
            }
        }

        // Top Native Header Toolbar
        egui::TopBottomPanel::top("native_toolbar").show(ctx, |ui| {
            ui.add_space(5.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} AGENTIC TERMINAL", Icons::APP)).color(Color32::from_rgb(56, 189, 248)).strong().size(15.0));
                ui.separator();

                // Active Workspace Path Pill
                let path_str = self.workspace.current_dir.display().to_string();
                let short_path = if path_str.len() > 40 {
                    format!("...{}", &path_str[path_str.len() - 37..])
                } else {
                    path_str
                };
                ui.label(RichText::new(format!("{} {}", Icons::FOLDER, short_path)).color(Color32::from_rgb(148, 163, 184)));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("Native UI • ~20MB RAM").color(Color32::DARK_GRAY).small());
                    ui.separator();

                    // Settings Button
                    if ui.button(RichText::new(format!("{} Settings", Icons::GEAR)).color(Color32::from_rgb(168, 85, 247))).clicked() {
                        let p = self.config.active_provider;
                        if let Some(cfg) = self.config.get_provider_config(p) {
                            self.settings_api_key = cfg.api_key.clone();
                            self.settings_base_url = cfg.base_url.clone().unwrap_or_default();
                            self.settings_account_id = cfg.account_id.clone().unwrap_or_default();
                        }
                        self.show_settings_dialog = true;
                    }

                    // Models Explorer Button
                    if ui.button(RichText::new(format!("{} Models ({})", Icons::SEARCH, self.cached_models.len())).color(Color32::from_rgb(56, 189, 248))).clicked() {
                        self.show_model_dialog = true;
                    }
                });
            });
            ui.add_space(5.0);
        });

        // Bottom Native Status & Permission Bar
        egui::TopBottomPanel::bottom("native_bottom_bar").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                // Active Model Button
                let model_label = format!("{} {}: {}", Icons::BOT, self.config.active_provider.display_name(), self.config.active_model);
                if ui.button(RichText::new(model_label).color(Color32::from_rgb(56, 189, 248)).strong()).clicked() {
                    self.show_model_dialog = true;
                }

                ui.separator();

                // Permission Toggle Button
                let (perm_text, perm_color) = match self.config.permission {
                    AgentPermission::ConfirmEach => (format!("{} Permission: [CONFIRM]", Icons::SHIELD), Color32::from_rgb(251, 191, 36)),
                    AgentPermission::AutoExecute => (format!("{} Permission: [AUTO-EXEC]", Icons::PLAY), Color32::from_rgb(52, 211, 153)),
                    AgentPermission::ReadOnly => (format!("{} Permission: [READ-ONLY]", Icons::SHIELD), Color32::from_rgb(244, 63, 94)),
                };
                if ui.button(RichText::new(perm_text).color(perm_color).strong()).clicked() {
                    self.config.permission = self.config.permission.next();
                    let _ = self.config.save();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(egui::Slider::new(&mut self.config.chat_panel_percent, 25..=75).text("Split Width %"));
                    if self.is_command_running {
                        ui.label(RichText::new("Running Command...").color(Color32::from_rgb(251, 191, 36)).small());
                        ui.spinner();
                    }
                });
            });
            ui.add_space(4.0);
        });

        // Main Desktop Split View
        let total_width = ctx.available_rect().width();
        let chat_width = total_width * (self.config.chat_panel_percent as f32 / 100.0);

        // Right Panel: AI Copilot Chat
        egui::SidePanel::right("native_chat_panel")
            .exact_width(chat_width)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new(format!("{} COPILOT CHAT", Icons::BOT)).color(Color32::from_rgb(56, 189, 248)).strong());
                    if self.is_thinking {
                        ui.spinner();
                        ui.label(RichText::new("Generating response...").color(Color32::from_rgb(251, 191, 36)).small());
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button(RichText::new(format!("{} New", Icons::REFRESH)).small()).clicked() {
                            self.chat_messages.clear();
                            self.chat_messages.push(AgentEngine::create_system_message("Started fresh conversation.".to_string()));
                        }
                    });
                });
                ui.separator();

                // Interactive Agent Action Proposal Banner
                if let Some(cmd) = self.pending_command.clone() {
                    egui::Frame::none()
                        .fill(Color32::from_rgb(27, 33, 48))
                        .stroke(Stroke::new(1.5_f32, Color32::from_rgb(251, 191, 36)))
                        .rounding(Rounding::same(8.0))
                        .inner_margin(Margin::same(10.0))
                        .show(ui, |ui| {
                            ui.horizontal(|ui| {
                                ui.label(RichText::new(format!("{} ACTION REQUESTED", Icons::SHIELD)).color(Color32::from_rgb(251, 191, 36)).strong());
                                ui.label(RichText::new("Agent wants to run:").color(Color32::LIGHT_GRAY));
                            });
                            ui.add_space(4.0);
                            ui.label(RichText::new(format!("$ {cmd}")).color(Color32::from_rgb(56, 189, 248)).code().strong());
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                if ui.button(RichText::new(format!("{} Approve & Run (Enter)", Icons::CHECK)).color(Color32::from_rgb(52, 211, 153)).strong()).clicked() {
                                    self.pending_command = None;
                                    self.chat_messages.push(AgentEngine::create_system_message(format!("[APPROVED] `{cmd}`")));
                                    self.run_command_directly(cmd, true);
                                }
                                if ui.button(RichText::new(format!("{} Reject (Esc)", Icons::CROSS)).color(Color32::from_rgb(244, 63, 94))).clicked() {
                                    self.pending_command = None;
                                    self.chat_messages.push(AgentEngine::create_system_message("[REJECTED] Command cancelled by user.".to_string()));
                                }
                            });
                        });
                    ui.add_space(6.0);
                }

                // Scrollable Chat Message Feed
                let mut cmd_to_run_now: Option<String> = None;
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .max_height(ui.available_height() - 50.0)
                    .show(ui, |ui| {
                        for msg in &self.chat_messages {
                            let (bg_color, role_label, role_color) = match msg.role.as_str() {
                                "user" => (Color32::from_rgb(20, 26, 38), "[YOU]", Color32::from_rgb(56, 189, 248)),
                                "system" => (Color32::from_rgb(24, 20, 36), "[SYSTEM]", Color32::from_rgb(168, 85, 247)),
                                _ => (Color32::from_rgb(18, 24, 30), "[COPILOT]", Color32::from_rgb(52, 211, 153)),
                            };

                            egui::Frame::none()
                                .fill(bg_color)
                                .rounding(Rounding::same(8.0))
                                .inner_margin(Margin::same(8.0))
                                .show(ui, |ui| {
                                    ui.horizontal(|ui| {
                                        ui.label(RichText::new(role_label).color(role_color).strong());
                                        ui.label(RichText::new(&msg.timestamp).color(Color32::DARK_GRAY).small());
                                    });
                                    ui.add_space(3.0);

                                    for line in msg.content.lines() {
                                        if line.starts_with("```") {
                                            ui.horizontal(|ui| {
                                                ui.label(RichText::new(line).color(Color32::from_rgb(168, 85, 247)).small());
                                                if ui.button(RichText::new(format!("{} Copy", Icons::COPY)).small()).clicked() {
                                                    ui.output_mut(|o| o.copied_text = msg.content.clone());
                                                }
                                            });
                                        } else {
                                            ui.label(RichText::new(line).color(Color32::LIGHT_GRAY));
                                        }
                                    }

                                    if let Some(ref c) = msg.command_to_run {
                                        ui.add_space(4.0);
                                        ui.horizontal(|ui| {
                                            ui.label(RichText::new(format!("{} Command: {}", Icons::PLAY, c)).color(Color32::from_rgb(56, 189, 248)).code());
                                            if ui.button(RichText::new("Run Now").small()).clicked() {
                                                cmd_to_run_now = Some(c.clone());
                                            }
                                        });
                                    }
                                });
                            ui.add_space(4.0);
                        }
                    });

                if let Some(cmd) = cmd_to_run_now {
                    self.run_command_directly(cmd, true);
                }

                // Chat Input Field
                ui.horizontal(|ui| {
                    let edit = egui::TextEdit::singleline(&mut self.chat_input)
                        .hint_text("Ask Copilot... (Press Enter)")
                        .desired_width(ui.available_width() - 65.0);
                    let resp = ui.add(edit);
                    if (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || ui.button(RichText::new(format!("{} Send", Icons::SEND)).strong()).clicked() {
                        self.send_chat_prompt();
                    }
                });
            });

        // Left Panel: Native Terminal Workspace
        egui::CentralPanel::default().show(ctx, |ui| {
            // Workspace Header Bar with Quick Action Buttons
            ui.horizontal(|ui| {
                ui.label(RichText::new(format!("{} TERMINAL WORKSPACE", Icons::TERMINAL)).color(Color32::from_rgb(56, 189, 248)).strong());
                ui.separator();

                // Quick command shortcut chips
                if ui.button(RichText::new("git status").small()).clicked() {
                    self.run_command_directly("git status".to_string(), false);
                }
                if ui.button(RichText::new("ls -la").small()).clicked() {
                    self.run_command_directly("ls -la".to_string(), false);
                }
                if ui.button(RichText::new("cargo check").small()).clicked() {
                    self.run_command_directly("cargo check".to_string(), false);
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.button(RichText::new(format!("{} Clear", Icons::TRASH)).small()).clicked() {
                        self.workspace.history_logs.clear();
                    }
                });
            });
            ui.separator();

            // Directory Files Bar
            ui.horizontal_wrapped(|ui| {
                ui.label(RichText::new(format!("{} Files:", Icons::FOLDER)).color(Color32::GRAY).small());
                let files = self.workspace.list_files();
                for f in files.iter().take(8) {
                    if f.is_dir {
                        if ui.button(RichText::new(format!("[{}/]", f.name)).color(Color32::from_rgb(56, 189, 248)).small()).clicked() {
                            self.run_command_directly(format!("cd {}", f.name), false);
                        }
                    } else {
                        ui.label(RichText::new(&f.name).color(Color32::LIGHT_GRAY).small());
                    }
                }
            });
            ui.add_space(4.0);

            // Native Live Terminal Output Screen
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .max_height(ui.available_height() - 48.0)
                .show(ui, |ui| {
                    egui::Frame::none()
                        .fill(Color32::from_rgb(9, 11, 16))
                        .rounding(Rounding::same(6.0))
                        .inner_margin(Margin::same(8.0))
                        .show(ui, |ui| {
                            for log in &self.workspace.history_logs {
                                let col = if log.is_error {
                                    Color32::from_rgb(244, 63, 94)
                                } else if log.is_command && log.is_agent {
                                    Color32::from_rgb(168, 85, 247)
                                } else if log.is_command {
                                    Color32::from_rgb(56, 189, 248)
                                } else {
                                    Color32::from_rgb(226, 232, 240)
                                };

                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(format!("[{}]", log.timestamp)).color(Color32::DARK_GRAY).monospace().small());
                                    ui.label(RichText::new(&log.text).color(col).monospace());
                                });
                            }
                        });
                });

            ui.add_space(4.0);

            // Interactive Shell Command Input
            ui.horizontal(|ui| {
                ui.label(RichText::new(Icons::TERMINAL).color(Color32::from_rgb(56, 189, 248)).strong());
                let cmd_edit = egui::TextEdit::singleline(&mut self.workspace_input)
                    .hint_text("Type shell command to execute directly...")
                    .desired_width(ui.available_width() - 80.0);
                let resp = ui.add(cmd_edit);
                if (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || ui.button(RichText::new(format!("{} Run", Icons::PLAY)).strong()).clicked() {
                    let cmd = self.workspace_input.trim().to_string();
                    if !cmd.is_empty() {
                        self.workspace_input.clear();
                        self.run_command_directly(cmd, false);
                    }
                }
            });
        });

        // Dialog: AI Models Explorer with Automatic Discovery & Real-Time Filter
        if self.show_model_dialog {
            egui::Window::new(format!("{} AI Models Explorer", Icons::SEARCH))
                .collapsible(false)
                .resizable(true)
                .default_size(Vec2::new(620.0, 520.0))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(RichText::new("Provider:").strong());
                        for &p in ProviderType::all() {
                            let is_active = self.config.active_provider == p;
                            if ui.selectable_label(is_active, p.display_name()).clicked() {
                                self.config.active_provider = p;
                                self.load_models_for_provider(p);
                            }
                        }
                    });

                    ui.separator();

                    // Search input & auto-load status
                    ui.horizontal(|ui| {
                        ui.label(RichText::new(format!("{} Search:", Icons::SEARCH)).strong().color(Color32::from_rgb(56, 189, 248)));
                        ui.add(
                            egui::TextEdit::singleline(&mut self.model_search_query)
                                .hint_text("Filter models (e.g. claude, gpt, flash, deepseek, llama)...")
                                .desired_width(ui.available_width() - 95.0)
                        );
                        if ui.button(RichText::new(format!("{} Reload", Icons::REFRESH)).small()).clicked() {
                            self.load_models_for_provider(self.config.active_provider);
                        }
                    });

                    ui.horizontal(|ui| {
                        if self.is_loading_models {
                            ui.spinner();
                        }
                        ui.label(RichText::new(&self.models_status).color(Color32::GRAY).small());
                    });

                    ui.separator();

                    // Filtered Models List
                    let query = self.model_search_query.to_lowercase();
                    let filtered: Vec<&String> = self
                        .cached_models
                        .iter()
                        .filter(|m| query.is_empty() || m.to_lowercase().contains(&query))
                        .collect();

                    ui.label(RichText::new(format!("Showing {} of {} models:", filtered.len(), self.cached_models.len())).small());

                    egui::ScrollArea::vertical().max_height(320.0).show(ui, |ui| {
                        for model_name in filtered {
                            let is_selected = self.config.active_model == *model_name;
                            let prefix = if is_selected { format!("{} [ACTIVE] ", Icons::CHECK) } else { "  ".to_string() };
                            let label_text = format!("{prefix}{model_name}");

                            let resp = ui.selectable_label(
                                is_selected,
                                RichText::new(label_text)
                                    .color(if is_selected { Color32::from_rgb(52, 211, 153) } else { Color32::WHITE })
                                    .strong()
                            );

                            if resp.clicked() {
                                self.config.active_model = model_name.clone();
                                let _ = self.config.save();
                                self.chat_messages.push(AgentEngine::create_system_message(format!(
                                    "[OK] Switched active model to `{model_name}`"
                                )));
                                self.show_model_dialog = false;
                            }
                        }
                    });

                    ui.separator();
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new(format!("{} Close", Icons::CROSS))).clicked() {
                            self.show_model_dialog = false;
                        }
                    });
                });
        }

        // Dialog: Provider API Keys Settings
        if self.show_settings_dialog {
            egui::Window::new(format!("{} Provider API Settings", Icons::GEAR))
                .collapsible(false)
                .resizable(false)
                .default_size(Vec2::new(520.0, 340.0))
                .show(ctx, |ui| {
                    let cur_p = self.config.active_provider;
                    ui.label(RichText::new(format!("Configure {}", cur_p.display_name())).strong().color(Color32::from_rgb(56, 189, 248)));
                    ui.separator();

                    ui.label("API Key / Bearer Token:");
                    ui.add(egui::TextEdit::singleline(&mut self.settings_api_key).password(true).desired_width(ui.available_width()));

                    if cur_p == ProviderType::OpenAI || cur_p == ProviderType::OpenRouter {
                        ui.add_space(4.0);
                        ui.label("Endpoint Base URL:");
                        ui.add(egui::TextEdit::singleline(&mut self.settings_base_url).desired_width(ui.available_width()));
                    }

                    if cur_p == ProviderType::Cloudflare {
                        ui.add_space(4.0);
                        ui.label("Cloudflare Account ID:");
                        ui.add(egui::TextEdit::singleline(&mut self.settings_account_id).desired_width(ui.available_width()));
                    }

                    ui.add_space(14.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new(format!("{} Save & Discover Models", Icons::CHECK)).color(Color32::from_rgb(52, 211, 153)).strong()).clicked() {
                            let base_url = if !self.settings_base_url.trim().is_empty() {
                                Some(self.settings_base_url.trim().to_string())
                            } else {
                                None
                            };
                            let account_id = if !self.settings_account_id.trim().is_empty() {
                                Some(self.settings_account_id.trim().to_string())
                            } else {
                                None
                            };
                            self.config.set_provider_config(
                                cur_p,
                                self.settings_api_key.trim().to_string(),
                                base_url,
                                account_id,
                                self.config.active_model.clone(),
                            );
                            self.show_settings_dialog = false;
                            self.load_models_for_provider(cur_p);
                        }
                        if ui.button(RichText::new(format!("{} Cancel", Icons::CROSS))).clicked() {
                            self.show_settings_dialog = false;
                        }
                    });
                });
        }
    }
}

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([880.0, 540.0])
            .with_title("Agentic Terminal Desktop"),
        ..Default::default()
    };

    eframe::run_native(
        "Agentic Terminal Desktop",
        native_options,
        Box::new(|cc| Ok(Box::new(AgenticGuiApp::new(cc)))),
    )
}
