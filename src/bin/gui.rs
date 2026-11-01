use eframe::egui::{self, Color32, RichText, Vec2};
use rust_agent_tui::agent::AgentEngine;
use rust_agent_tui::config::{AgentPermission, AppConfig, ProviderType};
use rust_agent_tui::model_fetcher::ModelFetcher;
use rust_agent_tui::providers::{ChatMessage, LlmService};
use rust_agent_tui::workspace::WorkspaceManager;
use std::sync::mpsc;
use std::sync::Arc;

enum GuiEvent {
    AgentSuccess { reply: String, command: Option<String> },
    AgentFailed { error: String },
    ModelsLoaded { provider: ProviderType, models: Vec<String> },
    CommandExecuted { code: i32, output: String, is_agent: bool },
}

struct AgenticGuiApp {
    config: AppConfig,
    workspace: WorkspaceManager,
    llm_service: Arc<LlmService>,
    model_fetcher: Arc<ModelFetcher>,

    // Channels
    event_tx: mpsc::Sender<GuiEvent>,
    event_rx: mpsc::Receiver<GuiEvent>,

    // UI state
    chat_input: String,
    workspace_input: String,
    chat_messages: Vec<ChatMessage>,
    is_thinking: bool,
    pending_command: Option<String>,

    // Model management
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
        // Dark theme setup
        let mut visuals = egui::Visuals::dark();
        visuals.panel_fill = Color32::from_rgb(15, 17, 23);
        visuals.window_fill = Color32::from_rgb(22, 25, 34);
        visuals.extreme_bg_color = Color32::from_rgb(10, 11, 15);
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
            pending_command: None,
            show_model_dialog: false,
            show_settings_dialog: false,
            model_search_query: String::new(),
            cached_models: config.active_provider.default_models().iter().map(|&s| s.to_string()).collect(),
            is_loading_models: false,
            models_status: "Default models loaded".to_string(),
            settings_api_key: String::new(),
            settings_base_url: String::new(),
            settings_account_id: String::new(),
            config,
        };

        app.chat_messages.push(AgentEngine::create_system_message(format!(
            "Welcome to Agentic Terminal GUI. Provider: {} | Model: {}. Click [Setup API Keys] to configure.",
            app.config.active_provider.display_name(),
            app.config.active_model
        )));

        // Automatically load models for initial active provider
        app.load_models_for_provider(app.config.active_provider);

        app
    }

    fn load_models_for_provider(&mut self, provider: ProviderType) {
        self.is_loading_models = true;
        self.models_status = format!("Loading live models from {}...", provider.display_name());
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

    fn execute_workspace_command(&mut self, cmd: String, is_agent: bool) {
        let tx = self.event_tx.clone();
        let dir = self.workspace.current_dir.clone();

        // Add log immediately
        self.workspace.add_log(true, false, is_agent, format!("{} {}", if is_agent { "[AGENT]" } else { ">" }, cmd));

        tokio::spawn(async move {
            let mut temp_ws = WorkspaceManager {
                current_dir: dir,
                history_logs: Vec::new(),
                max_logs: 100,
                is_running: false,
            };
            if let Ok((code, out)) = temp_ws.run_shell_command(&cmd, is_agent).await {
                let _ = tx.send(GuiEvent::CommandExecuted { code, output: out, is_agent });
            }
        });
    }
}

impl eframe::App for AgenticGuiApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Poll async events
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
                                self.execute_workspace_command(cmd, true);
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
                    self.models_status = format!("Loaded {} models from {}", models.len(), provider.display_name());
                    self.cached_models = models;
                }
                GuiEvent::CommandExecuted { code, output, is_agent } => {
                    let lines: Vec<&str> = output.lines().collect();
                    for line in lines.iter().take(30) {
                        self.workspace.add_log(false, code != 0, is_agent, line.to_string());
                    }
                    if is_agent {
                        let snippet = if output.len() > 500 {
                            format!("{}...\n[Output truncated]", &output[..500])
                        } else if output.is_empty() {
                            "(No output)".to_string()
                        } else {
                            output
                        };
                        self.chat_messages.push(AgentEngine::create_system_message(
                            format!("[Command Exit {code}]:\n```\n{snippet}\n```")
                        ));
                    }
                }
            }
        }

        // Top Header Bar
        egui::TopBottomPanel::top("header_panel").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                ui.label(RichText::new("[APP] AGENTIC TERMINAL").color(Color32::from_rgb(56, 189, 248)).strong());
                ui.separator();
                ui.label(RichText::new(format!("[DIR] {}", self.workspace.current_dir.display())).color(Color32::GRAY));

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(RichText::new("RAM: ~20MB").color(Color32::DARK_GRAY));
                    ui.separator();
                    if ui.button(RichText::new("[Setup API Keys]").color(Color32::from_rgb(168, 85, 247))).clicked() {
                        let p = self.config.active_provider;
                        if let Some(cfg) = self.config.get_provider_config(p) {
                            self.settings_api_key = cfg.api_key.clone();
                            self.settings_base_url = cfg.base_url.clone().unwrap_or_default();
                            self.settings_account_id = cfg.account_id.clone().unwrap_or_default();
                        }
                        self.show_settings_dialog = true;
                    }
                });
            });
            ui.add_space(4.0);
        });

        // Bottom Control Bar
        egui::TopBottomPanel::bottom("bottom_bar").show(ctx, |ui| {
            ui.add_space(4.0);
            ui.horizontal(|ui| {
                // Model Switcher Button
                let model_btn_text = format!(
                    "[MODEL] {}: {}  [SEARCH / CHANGE]",
                    self.config.active_provider.display_name(),
                    self.config.active_model
                );
                if ui.button(RichText::new(model_btn_text).color(Color32::from_rgb(56, 189, 248)).strong()).clicked() {
                    self.show_model_dialog = true;
                }

                ui.separator();

                // Permission Toggle Button
                let (perm_text, perm_color) = match self.config.permission {
                    AgentPermission::ConfirmEach => ("[PERMISSION: CONFIRM]", Color32::from_rgb(251, 191, 36)),
                    AgentPermission::AutoExecute => ("[PERMISSION: AUTO-EXEC]", Color32::from_rgb(52, 211, 153)),
                    AgentPermission::ReadOnly => ("[PERMISSION: READ-ONLY]", Color32::from_rgb(244, 63, 94)),
                };
                if ui.button(RichText::new(perm_text).color(perm_color).strong()).clicked() {
                    self.config.permission = self.config.permission.next();
                    let _ = self.config.save();
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(egui::Slider::new(&mut self.config.chat_panel_percent, 20..=80).text("Chat Width %"));
                });
            });
            ui.add_space(4.0);
        });

        // Main Body: Split View (Left Workspace, Right Chat)
        let total_width = ctx.available_rect().width();
        let chat_width = total_width * (self.config.chat_panel_percent as f32 / 100.0);

        // Right Panel: AI Agent Copilot Chat
        egui::SidePanel::right("chat_side_panel")
            .exact_width(chat_width)
            .resizable(false)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(RichText::new("COPILOT CHAT").strong().color(Color32::from_rgb(56, 189, 248)));
                    if self.is_thinking {
                        ui.label(RichText::new("(Thinking...)").color(Color32::from_rgb(251, 191, 36)));
                    }
                });
                ui.separator();

                // Pending Command Approval Banner
                if let Some(cmd) = self.pending_command.clone() {
                    ui.group(|ui| {
                        ui.horizontal(|ui| {
                            ui.label(RichText::new("[ACTION REQUESTED]").color(Color32::from_rgb(251, 191, 36)).strong());
                            ui.label(RichText::new(format!("$ {cmd}")).color(Color32::WHITE).code());
                        });
                        ui.horizontal(|ui| {
                            if ui.button(RichText::new("[APPROVE & RUN]").color(Color32::from_rgb(52, 211, 153)).strong()).clicked() {
                                self.pending_command = None;
                                self.chat_messages.push(AgentEngine::create_system_message(format!("[APPROVED] `{cmd}`")));
                                self.execute_workspace_command(cmd, true);
                            }
                            if ui.button(RichText::new("[REJECT]").color(Color32::from_rgb(244, 63, 94))).clicked() {
                                self.pending_command = None;
                                self.chat_messages.push(AgentEngine::create_system_message("[REJECTED] Command cancelled.".to_string()));
                            }
                        });
                    });
                    ui.add_space(4.0);
                }

                // Chat Messages List
                egui::ScrollArea::vertical()
                    .stick_to_bottom(true)
                    .max_height(ui.available_height() - 45.0)
                    .show(ui, |ui| {
                        for msg in &self.chat_messages {
                            ui.group(|ui| {
                                let (label, col) = match msg.role.as_str() {
                                    "user" => ("[YOU]", Color32::from_rgb(56, 189, 248)),
                                    "system" => ("[SYSTEM]", Color32::from_rgb(168, 85, 247)),
                                    _ => ("[COPILOT]", Color32::from_rgb(52, 211, 153)),
                                };
                                ui.horizontal(|ui| {
                                    ui.label(RichText::new(label).color(col).strong());
                                    ui.label(RichText::new(&msg.timestamp).color(Color32::DARK_GRAY).small());
                                });
                                ui.add_space(2.0);

                                for line in msg.content.lines() {
                                    if line.starts_with("```") {
                                        ui.label(RichText::new(line).color(Color32::from_rgb(168, 85, 247)));
                                    } else {
                                        ui.label(RichText::new(line).color(Color32::LIGHT_GRAY));
                                    }
                                }

                                if let Some(ref c) = msg.command_to_run {
                                    ui.label(RichText::new(format!("  [PROPOSED COMMAND] {c}")).color(Color32::from_rgb(56, 189, 248)).code());
                                }
                            });
                            ui.add_space(4.0);
                        }
                    });

                // Input field
                ui.horizontal(|ui| {
                    let text_edit = egui::TextEdit::singleline(&mut self.chat_input)
                        .hint_text("Ask Copilot... (Press Enter to send)")
                        .desired_width(ui.available_width() - 70.0);
                    let resp = ui.add(text_edit);
                    if (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || ui.button("[Send]").clicked() {
                        self.send_chat_prompt();
                    }
                });
            });

        // Left Panel: Terminal Workspace
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(RichText::new("TERMINAL WORKSPACE").strong().color(Color32::from_rgb(56, 189, 248)));
                ui.separator();
                let files = self.workspace.list_files();
                for f in files.iter().take(5) {
                    if f.is_dir {
                        ui.label(RichText::new(format!("[{}/]", f.name)).color(Color32::from_rgb(56, 189, 248)).small());
                    } else {
                        ui.label(RichText::new(&f.name).color(Color32::GRAY).small());
                    }
                }
            });
            ui.separator();

            // Terminal Log Viewer
            egui::ScrollArea::vertical()
                .stick_to_bottom(true)
                .max_height(ui.available_height() - 45.0)
                .show(ui, |ui| {
                    for log in &self.workspace.history_logs {
                        let col = if log.is_error {
                            Color32::from_rgb(244, 63, 94)
                        } else if log.is_command && log.is_agent {
                            Color32::from_rgb(168, 85, 247)
                        } else if log.is_command {
                            Color32::from_rgb(56, 189, 248)
                        } else {
                            Color32::LIGHT_GRAY
                        };
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(format!("[{}]", log.timestamp)).color(Color32::DARK_GRAY).small());
                            ui.label(RichText::new(&log.text).color(col).monospace());
                        });
                    }
                });

            // Shell command input
            ui.horizontal(|ui| {
                ui.label(RichText::new(">").color(Color32::from_rgb(56, 189, 248)).strong());
                let cmd_edit = egui::TextEdit::singleline(&mut self.workspace_input)
                    .hint_text("Type shell command...")
                    .desired_width(ui.available_width() - 60.0);
                let resp = ui.add(cmd_edit);
                if (resp.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter))) || ui.button("[Run]").clicked() {
                    let cmd = self.workspace_input.trim().to_string();
                    if !cmd.is_empty() {
                        self.workspace_input.clear();
                        self.execute_workspace_command(cmd, false);
                    }
                }
            });
        });

        // Dialog: AI Models List with Automatic Loading & Live Search
        if self.show_model_dialog {
            egui::Window::new("AI Models Explorer")
                .collapsible(false)
                .resizable(true)
                .min_size(Vec2::new(560.0, 480.0))
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
                        ui.label(RichText::new("Search Model:").strong().color(Color32::from_rgb(56, 189, 248)));
                        ui.add(
                            egui::TextEdit::singleline(&mut self.model_search_query)
                                .hint_text("Type to filter (e.g. claude, gpt, flash, llama)...")
                                .desired_width(ui.available_width() - 90.0)
                        );
                        if ui.button("[Refresh]").clicked() {
                            self.load_models_for_provider(self.config.active_provider);
                        }
                    });

                    ui.horizontal(|ui| {
                        if self.is_loading_models {
                            ui.spinner();
                        }
                        ui.label(RichText::new(&self.models_status).color(Color32::DARK_GRAY).small());
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

                    egui::ScrollArea::vertical().max_height(280.0).show(ui, |ui| {
                        for model_name in filtered {
                            let is_selected = self.config.active_model == *model_name;
                            let text = format!("{} {}", if is_selected { "[ACTIVE] " } else { " " }, model_name);
                            if ui.selectable_label(is_selected, RichText::new(text).strong()).clicked() {
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
                        if ui.button("[Close]").clicked() {
                            self.show_model_dialog = false;
                        }
                    });
                });
        }

        // Dialog: Provider API Keys Settings
        if self.show_settings_dialog {
            egui::Window::new("Provider API Settings")
                .collapsible(false)
                .resizable(false)
                .min_size(Vec2::new(500.0, 320.0))
                .show(ctx, |ui| {
                    let cur_p = self.config.active_provider;
                    ui.label(RichText::new(format!("Configure {}", cur_p.display_name())).strong().color(Color32::from_rgb(56, 189, 248)));
                    ui.separator();

                    ui.label("API Key / Token:");
                    ui.add(egui::TextEdit::singleline(&mut self.settings_api_key).password(true).desired_width(ui.available_width()));

                    if cur_p == ProviderType::OpenAI || cur_p == ProviderType::OpenRouter {
                        ui.add_space(4.0);
                        ui.label("Base URL:");
                        ui.add(egui::TextEdit::singleline(&mut self.settings_base_url).desired_width(ui.available_width()));
                    }

                    if cur_p == ProviderType::Cloudflare {
                        ui.add_space(4.0);
                        ui.label("Cloudflare Account ID:");
                        ui.add(egui::TextEdit::singleline(&mut self.settings_account_id).desired_width(ui.available_width()));
                    }

                    ui.add_space(12.0);
                    ui.horizontal(|ui| {
                        if ui.button(RichText::new("[Save & Apply]").color(Color32::from_rgb(52, 211, 153)).strong()).clicked() {
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
                        if ui.button("[Cancel]").clicked() {
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
            .with_inner_size([1120.0, 720.0])
            .with_min_inner_size([800.0, 500.0])
            .with_title("Agentic Terminal GUI"),
        ..Default::default()
    };

    eframe::run_native(
        "Agentic Terminal GUI",
        native_options,
        Box::new(|cc| Ok(Box::new(AgenticGuiApp::new(cc)))),
    )
}
