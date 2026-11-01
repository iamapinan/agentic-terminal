mod agent;
mod config;
mod providers;
mod ui;
mod workspace;
#[cfg(test)]
mod tests;

use agent::AgentEngine;
use config::{AgentPermission, AppConfig, ProviderType};
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind,
        KeyModifiers, MouseButton, MouseEventKind,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use providers::{ChatMessage, LlmService};
use ratatui::{backend::CrosstermBackend, layout::Rect, Terminal};
use std::io;
use std::sync::Arc;
use tokio::sync::mpsc;
use ui::{
    layout::{render_main_ui, FocusedPanel, RenderState, UiHitMap},
    modals::{
        render_command_confirm_modal, render_provider_settings_modal, render_quick_model_modal,
        ModalHitMap, ProviderSettingsState,
    },
    theme::Theme,
};
use workspace::WorkspaceManager;

enum AppEvent {
    AgentSuccess { reply: String, command: Option<String> },
    AgentFailed { error: String },
    CommandExecuted { code: i32, output: String, is_agent: bool },
}

enum ModalState {
    None,
    ProviderSettings(ProviderSettingsState),
    ModelSelector { selected_idx: usize, custom_input: String },
    CommandConfirm(String),
}

fn rect_contains(r: Rect, x: u16, y: u16) -> bool {
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Panic hook to restore terminal and disable mouse capture
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let _ = disable_raw_mode();
        let _ = execute!(io::stdout(), LeaveAlternateScreen, DisableMouseCapture);
        default_panic(info);
    }));

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut config = AppConfig::load();
    let theme = Theme::default();
    let mut workspace = WorkspaceManager::new();
    let llm_service = Arc::new(LlmService::new());

    let mut focused_panel = FocusedPanel::Chat;
    let mut modal_state = ModalState::None;

    let mut workspace_input = String::new();
    let mut chat_input = String::new();
    let mut chat_messages: Vec<ChatMessage> = Vec::new();
    let mut is_thinking = false;
    let mut pending_command: Option<String> = None;
    let mut workspace_scroll = 0;
    let mut chat_scroll = 0;

    // Hit-testing records
    let mut last_hit_map = UiHitMap::default();
    let mut last_modal_hit_map = ModalHitMap {
        save_btn: None,
        cancel_btn: None,
        step1_area: None,
        step2_area: None,
        step2b_area: None,
        step3_area: None,
    };

    // Welcome message
    chat_messages.push(AgentEngine::create_system_message(format!(
        "Welcome to Rust Agent Copilot. Active provider: {} | Model: {}. Press [Ctrl+P] to configure API keys.",
        config.active_provider.display_name(),
        config.active_model
    )));

    let (event_tx, mut event_rx) = mpsc::channel::<AppEvent>(32);

    let mut should_quit = false;

    while !should_quit {
        // Draw frame and record clickable boundaries
        terminal.draw(|f| {
            let render_state = RenderState {
                config: &config,
                focused_panel,
                workspace: &workspace,
                workspace_input: &workspace_input,
                chat_messages: &chat_messages,
                chat_input: &chat_input,
                is_thinking,
                pending_command: pending_command.as_deref(),
                workspace_scroll,
                chat_scroll,
            };

            last_hit_map = render_main_ui(f, &render_state, &theme);

            // Render overlays
            match &modal_state {
                ModalState::ProviderSettings(state) => {
                    last_modal_hit_map = render_provider_settings_modal(f, state, &theme);
                }
                ModalState::ModelSelector { selected_idx, custom_input } => {
                    render_quick_model_modal(
                        f,
                        config.active_provider,
                        &config.active_model,
                        *selected_idx,
                        custom_input,
                        &theme,
                    );
                }
                ModalState::CommandConfirm(cmd) => {
                    render_command_confirm_modal(f, cmd, &theme);
                }
                ModalState::None => {}
            }
        })?;

        // Poll for async events or keyboard/mouse
        tokio::select! {
            Some(app_ev) = event_rx.recv() => {
                match app_ev {
                    AppEvent::AgentSuccess { reply, command } => {
                        is_thinking = false;
                        chat_messages.push(AgentEngine::create_assistant_message(reply, command.clone()));

                        if let Some(cmd) = command {
                            match config.permission {
                                AgentPermission::AutoExecute => {
                                    chat_messages.push(AgentEngine::create_system_message(
                                        format!("[AUTO-EXEC] Running agent command: `{cmd}`")
                                    ));
                                    let tx = event_tx.clone();
                                    let ws_dir = workspace.current_dir.clone();
                                    tokio::spawn(async move {
                                        let mut temp_ws = WorkspaceManager {
                                            current_dir: ws_dir,
                                            history_logs: Vec::new(),
                                            max_logs: 100,
                                            is_running: false,
                                        };
                                        if let Ok((code, out)) = temp_ws.run_shell_command(&cmd, true).await {
                                            let _ = tx.send(AppEvent::CommandExecuted { code, output: out, is_agent: true }).await;
                                        }
                                    });
                                }
                                AgentPermission::ConfirmEach => {
                                    pending_command = Some(cmd);
                                }
                                AgentPermission::ReadOnly => {
                                    chat_messages.push(AgentEngine::create_system_message(
                                        format!("[READ-ONLY] Agent suggested command `{cmd}`, but permission is Read-Only.")
                                    ));
                                }
                            }
                        }
                    }
                    AppEvent::AgentFailed { error } => {
                        is_thinking = false;
                        chat_messages.push(AgentEngine::create_system_message(format!("[ERROR] {error}")));
                    }
                    AppEvent::CommandExecuted { code, output, is_agent } => {
                        if is_agent {
                            let snippet = if output.len() > 500 {
                                format!("{}...\n[Output truncated]", &output[..500])
                            } else if output.is_empty() {
                                "(No output)".to_string()
                            } else {
                                output
                            };

                            chat_messages.push(AgentEngine::create_system_message(
                                format!("[Command Exit {code}]:\n```\n{snippet}\n```")
                            ));
                        }
                    }
                }
            }
            _ = tokio::time::sleep(std::time::Duration::from_millis(25)) => {
                if event::poll(std::time::Duration::from_millis(0))? {
                    match event::read()? {
                        Event::Key(key) => {
                            if key.kind == KeyEventKind::Press {
                                handle_key_event(
                                    key,
                                    &mut should_quit,
                                    &mut modal_state,
                                    &mut focused_panel,
                                    &mut config,
                                    &mut workspace,
                                    &mut workspace_input,
                                    &mut chat_input,
                                    &mut chat_messages,
                                    &mut pending_command,
                                    &mut is_thinking,
                                    &mut workspace_scroll,
                                    &mut chat_scroll,
                                    llm_service.clone(),
                                    event_tx.clone(),
                                ).await;
                            }
                        }
                        Event::Mouse(mouse) => {
                            handle_mouse_event(
                                mouse,
                                &mut modal_state,
                                &mut focused_panel,
                                &mut config,
                                &mut workspace,
                                &mut pending_command,
                                &mut workspace_scroll,
                                &mut chat_scroll,
                                &last_hit_map,
                                &last_modal_hit_map,
                                terminal.size()?.width,
                            ).await;
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, DisableMouseCapture)?;
    terminal.show_cursor()?;
    Ok(())
}

async fn handle_mouse_event(
    mouse: crossterm::event::MouseEvent,
    modal_state: &mut ModalState,
    focused_panel: &mut FocusedPanel,
    config: &mut AppConfig,
    workspace: &mut WorkspaceManager,
    pending_command: &mut Option<String>,
    workspace_scroll: &mut usize,
    chat_scroll: &mut usize,
    hit_map: &UiHitMap,
    modal_hit_map: &ModalHitMap,
    total_term_width: u16,
) {
    let col = mouse.column;
    let row = mouse.row;

    match mouse.kind {
        MouseEventKind::Down(MouseButton::Left) => {
            // Check modal click first if open
            match modal_state {
                ModalState::ProviderSettings(state) => {
                    if let Some(r) = modal_hit_map.save_btn {
                        if rect_contains(r, col, row) {
                            let provider = state.current_provider();
                            let model = if !state.custom_model.trim().is_empty() {
                                state.custom_model.trim().to_string()
                            } else {
                                let models = provider.default_models();
                                models[state.selected_model_idx.min(models.len() - 1)].to_string()
                            };

                            let base_url = if (provider == ProviderType::OpenAI || provider == ProviderType::OpenRouter)
                                && !state.base_url.trim().is_empty()
                            {
                                Some(state.base_url.trim().to_string())
                            } else {
                                None
                            };

                            let account_id = if provider == ProviderType::Cloudflare
                                && !state.account_id.trim().is_empty()
                            {
                                Some(state.account_id.trim().to_string())
                            } else {
                                None
                            };

                            config.set_provider_config(
                                provider,
                                state.api_key.trim().to_string(),
                                base_url,
                                account_id,
                                model.clone(),
                            );
                            config.active_provider = provider;
                            config.active_model = model;
                            let _ = config.save();
                            *modal_state = ModalState::None;
                            return;
                        }
                    }
                    if let Some(r) = modal_hit_map.cancel_btn {
                        if rect_contains(r, col, row) {
                            *modal_state = ModalState::None;
                            return;
                        }
                    }
                    if let Some(r) = modal_hit_map.step1_area {
                        if rect_contains(r, col, row) {
                            state.active_field = 0;
                            let rel_y = row.saturating_sub(r.y + 1) as usize;
                            if rel_y < ProviderType::all().len() {
                                state.selected_provider_idx = rel_y;
                                state.on_provider_change(config);
                            }
                            return;
                        }
                    }
                    if let Some(r) = modal_hit_map.step2_area {
                        if rect_contains(r, col, row) {
                            state.active_field = 1;
                            return;
                        }
                    }
                    if let Some(r) = modal_hit_map.step2b_area {
                        if rect_contains(r, col, row) {
                            state.active_field = 2;
                            return;
                        }
                    }
                    if let Some(r) = modal_hit_map.step3_area {
                        if rect_contains(r, col, row) {
                            let rel_y = row.saturating_sub(r.y + 1) as usize;
                            let models = state.current_provider().default_models();
                            if rel_y < models.len() {
                                state.active_field = 3;
                                state.selected_model_idx = rel_y;
                                state.custom_model.clear();
                            } else if rel_y == models.len() {
                                state.active_field = 4;
                            }
                            return;
                        }
                    }
                    return;
                }
                ModalState::ModelSelector { selected_idx, custom_input: _ } => {
                    // Modal handles click to select
                    let models = config.active_provider.default_models();
                    *selected_idx = (*selected_idx + 1) % (models.len() + 1);
                    return;
                }
                ModalState::CommandConfirm(cmd) => {
                    // Enter or Esc
                    let cmd_to_run = cmd.clone();
                    *modal_state = ModalState::None;
                    *pending_command = None;
                    let _ = workspace.run_shell_command(&cmd_to_run, true).await;
                    return;
                }
                ModalState::None => {}
            }

            // Normal UI Clicks
            if rect_contains(hit_map.setup_btn, col, row) {
                *modal_state = ModalState::ProviderSettings(ProviderSettingsState::new(config));
                return;
            }

            if rect_contains(hit_map.bottom_model_btn, col, row) {
                *modal_state = ModalState::ModelSelector {
                    selected_idx: 0,
                    custom_input: String::new(),
                };
                return;
            }

            if rect_contains(hit_map.bottom_perm_btn, col, row) {
                config.permission = config.permission.next();
                let _ = config.save();
                return;
            }

            if let Some(approve_r) = hit_map.approve_btn {
                if rect_contains(approve_r, col, row) {
                    if let Some(cmd) = pending_command.take() {
                        let _ = workspace.run_shell_command(&cmd, true).await;
                    }
                    return;
                }
            }

            if let Some(reject_r) = hit_map.reject_btn {
                if rect_contains(reject_r, col, row) {
                    *pending_command = None;
                    return;
                }
            }

            if rect_contains(hit_map.workspace_panel, col, row) {
                *focused_panel = FocusedPanel::Workspace;
                return;
            }

            if rect_contains(hit_map.chat_panel, col, row) {
                *focused_panel = FocusedPanel::Chat;
                return;
            }
        }
        MouseEventKind::Drag(MouseButton::Left) => {
            // Dragging divider to resize panels!
            if total_term_width > 0 {
                let left_pct = ((col as f32 / total_term_width as f32) * 100.0) as u16;
                let chat_pct = 100_u16.saturating_sub(left_pct);
                config.chat_panel_percent = chat_pct.clamp(20, 80);
                let _ = config.save();
            }
        }
        MouseEventKind::ScrollUp => {
            if rect_contains(hit_map.workspace_panel, col, row) {
                *workspace_scroll = workspace_scroll.saturating_add(3);
            } else if rect_contains(hit_map.chat_panel, col, row) {
                *chat_scroll = chat_scroll.saturating_add(3);
            }
        }
        MouseEventKind::ScrollDown => {
            if rect_contains(hit_map.workspace_panel, col, row) {
                *workspace_scroll = workspace_scroll.saturating_sub(3);
            } else if rect_contains(hit_map.chat_panel, col, row) {
                *chat_scroll = chat_scroll.saturating_sub(3);
            }
        }
        _ => {}
    }
}

async fn handle_key_event(
    key: event::KeyEvent,
    should_quit: &mut bool,
    modal_state: &mut ModalState,
    focused_panel: &mut FocusedPanel,
    config: &mut AppConfig,
    workspace: &mut WorkspaceManager,
    workspace_input: &mut String,
    chat_input: &mut String,
    chat_messages: &mut Vec<ChatMessage>,
    pending_command: &mut Option<String>,
    is_thinking: &mut bool,
    workspace_scroll: &mut usize,
    chat_scroll: &mut usize,
    llm_service: Arc<LlmService>,
    event_tx: mpsc::Sender<AppEvent>,
) {
    // 1. Check active modal first
    match modal_state {
        ModalState::ProviderSettings(state) => {
            match key.code {
                KeyCode::Esc => {
                    *modal_state = ModalState::None;
                }
                KeyCode::Tab => {
                    state.active_field = (state.active_field + 1) % 5;
                }
                KeyCode::BackTab => {
                    if state.active_field == 0 {
                        state.active_field = 4;
                    } else {
                        state.active_field -= 1;
                    }
                }
                KeyCode::F(4) => {
                    state.mask_key = !state.mask_key;
                }
                KeyCode::Up => match state.active_field {
                    0 => {
                        let total = ProviderType::all().len();
                        state.selected_provider_idx = (state.selected_provider_idx + total - 1) % total;
                        state.on_provider_change(config);
                    }
                    3 => {
                        let models = state.current_provider().default_models();
                        if state.selected_model_idx > 0 {
                            state.selected_model_idx -= 1;
                        } else {
                            state.selected_model_idx = models.len().saturating_sub(1);
                        }
                    }
                    _ => {}
                },
                KeyCode::Down => match state.active_field {
                    0 => {
                        let total = ProviderType::all().len();
                        state.selected_provider_idx = (state.selected_provider_idx + 1) % total;
                        state.on_provider_change(config);
                    }
                    3 => {
                        let models = state.current_provider().default_models();
                        state.selected_model_idx = (state.selected_model_idx + 1) % models.len();
                    }
                    _ => {}
                },
                KeyCode::Char(c) => match state.active_field {
                    0 if ('1'..='5').contains(&c) => {
                        let idx = (c as usize) - ('1' as usize);
                        if idx < ProviderType::all().len() {
                            state.selected_provider_idx = idx;
                            state.on_provider_change(config);
                        }
                    }
                    1 => state.api_key.push(c),
                    2 => match state.current_provider() {
                        ProviderType::OpenAI | ProviderType::OpenRouter => state.base_url.push(c),
                        ProviderType::Cloudflare => state.account_id.push(c),
                        _ => {}
                    },
                    4 => state.custom_model.push(c),
                    _ => {}
                },
                KeyCode::Backspace => match state.active_field {
                    1 => {
                        state.api_key.pop();
                    }
                    2 => match state.current_provider() {
                        ProviderType::OpenAI | ProviderType::OpenRouter => {
                            state.base_url.pop();
                        }
                        ProviderType::Cloudflare => {
                            state.account_id.pop();
                        }
                        _ => {}
                    },
                    4 => {
                        state.custom_model.pop();
                    }
                    _ => {}
                },
                KeyCode::Enter => {
                    let provider = state.current_provider();
                    let model = if !state.custom_model.trim().is_empty() {
                        state.custom_model.trim().to_string()
                    } else {
                        let models = provider.default_models();
                        models[state.selected_model_idx.min(models.len() - 1)].to_string()
                    };

                    let base_url = if (provider == ProviderType::OpenAI || provider == ProviderType::OpenRouter)
                        && !state.base_url.trim().is_empty()
                    {
                        Some(state.base_url.trim().to_string())
                    } else {
                        None
                    };

                    let account_id = if provider == ProviderType::Cloudflare && !state.account_id.trim().is_empty() {
                        Some(state.account_id.trim().to_string())
                    } else {
                        None
                    };

                    config.set_provider_config(
                        provider,
                        state.api_key.trim().to_string(),
                        base_url,
                        account_id,
                        model.clone(),
                    );
                    config.active_provider = provider;
                    config.active_model = model.clone();
                    let _ = config.save();

                    chat_messages.push(AgentEngine::create_system_message(format!(
                        "[OK] Provider updated to {} (Model: {})",
                        provider.display_name(),
                        model
                    )));

                    *modal_state = ModalState::None;
                }
                _ => {}
            }
            return;
        }
        ModalState::ModelSelector { selected_idx, custom_input } => {
            let models = config.active_provider.default_models();
            let total_options = models.len() + 1; // plus custom

            match key.code {
                KeyCode::Esc => {
                    *modal_state = ModalState::None;
                }
                KeyCode::Up => {
                    *selected_idx = (*selected_idx + total_options - 1) % total_options;
                }
                KeyCode::Down => {
                    *selected_idx = (*selected_idx + 1) % total_options;
                }
                KeyCode::Char(c) if *selected_idx == models.len() => {
                    custom_input.push(c);
                }
                KeyCode::Backspace if *selected_idx == models.len() => {
                    custom_input.pop();
                }
                KeyCode::Enter => {
                    let new_model = if *selected_idx < models.len() {
                        models[*selected_idx].to_string()
                    } else if !custom_input.trim().is_empty() {
                        custom_input.trim().to_string()
                    } else {
                        config.active_model.clone()
                    };

                    config.active_model = new_model.clone();
                    let _ = config.save();

                    chat_messages.push(AgentEngine::create_system_message(format!(
                        "[OK] Switched model to {}",
                        new_model
                    )));

                    *modal_state = ModalState::None;
                }
                _ => {}
            }
            return;
        }
        ModalState::CommandConfirm(cmd) => {
            match key.code {
                KeyCode::Enter => {
                    let cmd_to_run = cmd.clone();
                    *modal_state = ModalState::None;
                    *pending_command = None;

                    chat_messages.push(AgentEngine::create_system_message(format!(
                        "[APPROVED] `{cmd_to_run}`"
                    )));

                    let _ = workspace.run_shell_command(&cmd_to_run, true).await;
                }
                KeyCode::Esc => {
                    *modal_state = ModalState::None;
                    *pending_command = None;
                    chat_messages.push(AgentEngine::create_system_message(
                        "[REJECTED] Command cancelled by user.".to_string(),
                    ));
                }
                _ => {}
            }
            return;
        }
        ModalState::None => {}
    }

    // 2. Global Shortcuts
    if key.modifiers.contains(KeyModifiers::CONTROL) {
        match key.code {
            KeyCode::Char('c') | KeyCode::Char('q') => {
                *should_quit = true;
                return;
            }
            KeyCode::Char('p') => {
                *modal_state = ModalState::ProviderSettings(ProviderSettingsState::new(config));
                return;
            }
            KeyCode::Char('m') => {
                *modal_state = ModalState::ModelSelector {
                    selected_idx: 0,
                    custom_input: String::new(),
                };
                return;
            }
            KeyCode::Char('o') => {
                if let Some(cmd) = pending_command.clone() {
                    *modal_state = ModalState::CommandConfirm(cmd);
                    return;
                }
            }
            KeyCode::Char('x') => {
                config.permission = config.permission.next();
                let _ = config.save();
                chat_messages.push(AgentEngine::create_system_message(format!(
                    "Agent Permission set to {}",
                    config.permission.label()
                )));
                return;
            }
            KeyCode::Char('l') => {
                workspace.history_logs.clear();
                return;
            }
            KeyCode::Left => {
                config.chat_panel_percent = (config.chat_panel_percent + 5).min(80);
                let _ = config.save();
                return;
            }
            KeyCode::Right => {
                config.chat_panel_percent = config.chat_panel_percent.saturating_sub(5).max(20);
                let _ = config.save();
                return;
            }
            _ => {}
        }
    }

    // Function keys and bracket shortcuts for resizing
    match key.code {
        KeyCode::Char('[') => {
            // Expand workspace / shrink chat
            config.chat_panel_percent = config.chat_panel_percent.saturating_sub(5).max(20);
            let _ = config.save();
            return;
        }
        KeyCode::Char(']') => {
            // Shrink workspace / expand chat
            config.chat_panel_percent = (config.chat_panel_percent + 5).min(80);
            let _ = config.save();
            return;
        }
        KeyCode::F(2) => {
            *modal_state = ModalState::ModelSelector {
                selected_idx: 0,
                custom_input: String::new(),
            };
            return;
        }
        KeyCode::F(3) => {
            config.permission = config.permission.next();
            let _ = config.save();
            chat_messages.push(AgentEngine::create_system_message(format!(
                "Agent Permission set to {}",
                config.permission.label()
            )));
            return;
        }
        KeyCode::Tab => {
            *focused_panel = match *focused_panel {
                FocusedPanel::Workspace => FocusedPanel::Chat,
                FocusedPanel::Chat => FocusedPanel::Workspace,
            };
            return;
        }
        _ => {}
    }

    // Pending command quick confirm in chat panel
    if let Some(cmd) = pending_command.clone() {
        if *focused_panel == FocusedPanel::Chat {
            if key.code == KeyCode::Enter {
                *pending_command = None;
                chat_messages.push(AgentEngine::create_system_message(format!(
                    "[APPROVED] `{cmd}`"
                )));
                let _ = workspace.run_shell_command(&cmd, true).await;
                return;
            } else if key.code == KeyCode::Esc {
                *pending_command = None;
                chat_messages.push(AgentEngine::create_system_message(
                    "[REJECTED] Command cancelled by user.".to_string(),
                ));
                return;
            }
        }
    }

    // 3. Panel-specific handling
    match *focused_panel {
        FocusedPanel::Workspace => match key.code {
            KeyCode::Enter => {
                let cmd = workspace_input.trim().to_string();
                if !cmd.is_empty() {
                    workspace_input.clear();
                    let _ = workspace.run_shell_command(&cmd, false).await;
                }
            }
            KeyCode::Char(c) => {
                workspace_input.push(c);
            }
            KeyCode::Backspace => {
                workspace_input.pop();
            }
            KeyCode::PageUp => {
                *workspace_scroll = workspace_scroll.saturating_add(5);
            }
            KeyCode::PageDown => {
                *workspace_scroll = workspace_scroll.saturating_sub(5);
            }
            _ => {}
        },
        FocusedPanel::Chat => match key.code {
            KeyCode::Enter => {
                let prompt = chat_input.trim().to_string();
                if !prompt.is_empty() && !*is_thinking {
                    chat_input.clear();
                    chat_messages.push(AgentEngine::create_user_message(prompt));
                    *is_thinking = true;

                    let config_snapshot = config.clone();
                    let ws_summary = workspace.get_workspace_summary();
                    let history_snapshot = chat_messages.clone();
                    let service = llm_service.clone();
                    let tx = event_tx.clone();

                    tokio::spawn(async move {
                        let sys_prompt = AgentEngine::build_system_prompt(&ws_summary);
                        match service.send_prompt(&config_snapshot, &sys_prompt, &history_snapshot).await {
                            Ok(reply) => {
                                let cmd = AgentEngine::extract_first_command(&reply);
                                let _ = tx.send(AppEvent::AgentSuccess { reply, command: cmd }).await;
                            }
                            Err(e) => {
                                let _ = tx.send(AppEvent::AgentFailed { error: e.to_string() }).await;
                            }
                        }
                    });
                }
            }
            KeyCode::Char(c) => {
                chat_input.push(c);
            }
            KeyCode::Backspace => {
                chat_input.pop();
            }
            KeyCode::PageUp => {
                *chat_scroll = chat_scroll.saturating_add(5);
            }
            KeyCode::PageDown => {
                *chat_scroll = chat_scroll.saturating_sub(5);
            }
            _ => {}
        },
    }
}
