use crate::config::AppConfig;
use crate::providers::ChatMessage;
use crate::ui::theme::Theme;
use crate::workspace::WorkspaceManager;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, List, ListItem, Paragraph},
    Frame,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedPanel {
    Workspace,
    Chat,
}

pub struct RenderState<'a> {
    pub config: &'a AppConfig,
    pub focused_panel: FocusedPanel,
    pub workspace: &'a WorkspaceManager,
    pub workspace_input: &'a str,
    pub chat_messages: &'a [ChatMessage],
    pub chat_input: &'a str,
    pub is_thinking: bool,
    pub pending_command: Option<&'a str>,
    pub workspace_scroll: usize,
    pub chat_scroll: usize,
}

#[derive(Debug, Clone, Copy, Default)]
pub struct UiHitMap {
    pub setup_btn: Rect,
    pub workspace_panel: Rect,
    pub workspace_input: Rect,
    pub chat_panel: Rect,
    pub chat_input: Rect,
    pub divider: Rect,
    pub bottom_model_btn: Rect,
    pub bottom_perm_btn: Rect,
    pub approve_btn: Option<Rect>,
    pub reject_btn: Option<Rect>,
}

pub fn render_main_ui(f: &mut Frame, state: &RenderState, theme: &Theme) -> UiHitMap {
    let mut hit_map = UiHitMap::default();

    let main_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Top Header Bar
            Constraint::Min(10),   // Main Split (Workspace vs Agent)
            Constraint::Length(3), // Bottom Status / Control bar
        ])
        .split(f.area());

    render_header(f, main_layout[0], state, theme, &mut hit_map);
    render_split_panels(f, main_layout[1], state, theme, &mut hit_map);
    render_bottom_toolbar(f, main_layout[2], state, theme, &mut hit_map);

    hit_map
}

fn render_header(
    f: &mut Frame,
    area: Rect,
    state: &RenderState,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    let header_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Length(28), // Title & Logo
            Constraint::Min(30),    // Workspace Path
            Constraint::Length(38), // Shortcuts hints
        ])
        .split(area);

    let title_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent_cyan))
        .style(Style::default().bg(theme.bg_panel));

    let title_p = Paragraph::new(Line::from(vec![
        Span::styled("[APP] ", Style::default().fg(theme.accent_amber).add_modifier(Modifier::BOLD)),
        Span::styled(
            "AGENTIC TERMINAL",
            Style::default()
                .fg(theme.text_primary)
                .add_modifier(Modifier::BOLD),
        ),
        Span::styled(" v0.2", Style::default().fg(theme.text_dim)),
    ]))
    .block(title_block);
    f.render_widget(title_p, header_chunks[0]);

    // Active path
    let path_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border_normal))
        .style(Style::default().bg(theme.bg_panel));

    let path_str = state.workspace.current_dir.display().to_string();
    let max_len = (header_chunks[1].width as usize).saturating_sub(6);
    let display_path = if path_str.len() > max_len {
        format!("...{}", &path_str[path_str.len().saturating_sub(max_len)..])
    } else {
        path_str
    };

    let path_p = Paragraph::new(Line::from(vec![
        Span::styled(" [DIR] ", Style::default().fg(theme.accent_cyan).add_modifier(Modifier::BOLD)),
        Span::styled(display_path, Style::default().fg(theme.text_secondary)),
    ]))
    .block(path_block);
    f.render_widget(path_p, header_chunks[1]);

    // Shortcuts
    let shortcuts_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border_normal))
        .style(Style::default().bg(theme.bg_panel));

    let shortcuts_p = Paragraph::new(Line::from(vec![
        Span::styled("[Tab]", Style::default().fg(theme.accent_cyan).add_modifier(Modifier::BOLD)),
        Span::raw(" Focus | "),
        Span::styled("[^P Setup]", Style::default().fg(theme.accent_purple).add_modifier(Modifier::BOLD)),
        Span::raw(" | "),
        Span::styled("[^Q Exit]", Style::default().fg(theme.accent_rose).add_modifier(Modifier::BOLD)),
    ]))
    .alignment(Alignment::Right)
    .block(shortcuts_block);
    f.render_widget(shortcuts_p, header_chunks[2]);
    hit_map.setup_btn = header_chunks[2];
}

fn render_split_panels(
    f: &mut Frame,
    area: Rect,
    state: &RenderState,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    let chat_percent = state.config.chat_panel_percent.clamp(20, 80);
    let ws_percent = 100 - chat_percent;

    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(ws_percent), // Workspace (Left)
            Constraint::Length(1),              // Resizer Divider Column
            Constraint::Percentage(chat_percent), // Agent Chat (Right)
        ])
        .split(area);

    hit_map.workspace_panel = panels[0];
    hit_map.divider = panels[1];
    hit_map.chat_panel = panels[2];

    render_workspace_panel(f, panels[0], state, theme, hit_map);

    // Resizer divider
    let divider_style = Style::default().fg(theme.border_normal);
    let divider_lines: Vec<Line> = (0..panels[1].height)
        .map(|_| Line::from(Span::styled("│", divider_style)))
        .collect();
    f.render_widget(Paragraph::new(divider_lines), panels[1]);

    render_chat_panel(f, panels[2], state, theme, hit_map);
}

fn render_workspace_panel(
    f: &mut Frame,
    area: Rect,
    state: &RenderState,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    let is_focused = state.focused_panel == FocusedPanel::Workspace;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // File chips header
            Constraint::Min(5),    // Terminal output logs
            Constraint::Length(3), // Shell input bar
        ])
        .split(area);

    hit_map.workspace_input = chunks[2];

    // 1. Files preview header
    let files = state.workspace.list_files();
    let file_chips: Vec<Span> = files
        .iter()
        .take(6)
        .flat_map(|entry| {
            if entry.is_dir {
                vec![
                    Span::styled(
                        format!(" [{}/] ", entry.name),
                        Style::default()
                            .fg(theme.accent_cyan)
                            .add_modifier(Modifier::BOLD),
                    ),
                    Span::raw(" "),
                ]
            } else {
                vec![
                    Span::styled(
                        format!(" {} ", entry.name),
                        Style::default().fg(theme.text_secondary),
                    ),
                    Span::raw(" "),
                ]
            }
        })
        .collect();

    let files_block = Block::default()
        .title(" WORKSPACE FILES ")
        .title_style(Style::default().fg(theme.text_secondary))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border_normal))
        .style(Style::default().bg(theme.bg_panel));

    let files_p = Paragraph::new(Line::from(file_chips)).block(files_block);
    f.render_widget(files_p, chunks[0]);

    // 2. Terminal output logs
    let term_block = Block::default()
        .title(if state.workspace.is_running {
            " TERMINAL OUTPUT (RUNNING...) "
        } else {
            " TERMINAL OUTPUT "
        })
        .title_style(if state.workspace.is_running {
            Style::default().fg(theme.accent_amber).add_modifier(Modifier::BOLD)
        } else {
            theme.panel_border(is_focused)
        })
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.panel_border(is_focused))
        .style(Style::default().bg(theme.bg_dark));

    let log_items: Vec<ListItem> = state
        .workspace
        .history_logs
        .iter()
        .map(|log| {
            let prefix_style = Style::default().fg(theme.text_dim);
            let time_span = Span::styled(format!("[{}] ", log.timestamp), prefix_style);

            let content_span = if log.is_error {
                Span::styled(&log.text, Style::default().fg(theme.accent_rose))
            } else if log.is_command && log.is_agent {
                Span::styled(
                    &log.text,
                    Style::default()
                        .fg(theme.accent_purple)
                        .add_modifier(Modifier::BOLD),
                )
            } else if log.is_command {
                Span::styled(
                    &log.text,
                    Style::default()
                        .fg(theme.accent_cyan)
                        .add_modifier(Modifier::BOLD),
                )
            } else {
                Span::styled(&log.text, Style::default().fg(theme.text_primary))
            };

            ListItem::new(Line::from(vec![time_span, content_span]))
        })
        .collect();

    let inner_height = chunks[1].height.saturating_sub(2) as usize;
    let total_logs = log_items.len();
    let scroll_offset = if total_logs > inner_height {
        (total_logs - inner_height).saturating_sub(state.workspace_scroll)
    } else {
        0
    };

    let log_list = List::new(
        log_items
            .into_iter()
            .skip(scroll_offset)
            .take(inner_height)
            .collect::<Vec<_>>(),
    )
    .block(term_block);

    f.render_widget(log_list, chunks[1]);

    // 3. Shell input bar
    let input_block = Block::default()
        .title(if is_focused { " > Run Command [Enter] " } else { " > Shell (Click or Tab to focus) " })
        .title_style(theme.panel_border(is_focused))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.panel_border(is_focused))
        .style(Style::default().bg(theme.bg_panel));

    let cursor_char = if is_focused { "█" } else { "" };
    let input_p = Paragraph::new(Line::from(vec![
        Span::styled("> ", Style::default().fg(theme.accent_cyan).add_modifier(Modifier::BOLD)),
        Span::styled(state.workspace_input, Style::default().fg(theme.text_primary)),
        Span::styled(cursor_char, Style::default().fg(theme.accent_cyan)),
    ]))
    .block(input_block);

    f.render_widget(input_p, chunks[2]);
}

fn render_chat_panel(
    f: &mut Frame,
    area: Rect,
    state: &RenderState,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    let is_focused = state.focused_panel == FocusedPanel::Chat;

    let has_pending = state.pending_command.is_some();
    let constraints = if has_pending {
        vec![
            Constraint::Length(3), // Inline action confirmation pill
            Constraint::Min(6),    // Chat messages list
            Constraint::Length(3), // Input box
        ]
    } else {
        vec![
            Constraint::Min(6),    // Chat messages list
            Constraint::Length(3), // Input box
        ]
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    let (chat_chunk_idx, input_chunk_idx) = if has_pending {
        // Render action pill
        let pending_cmd = state.pending_command.unwrap();
        let action_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(theme.accent_amber).add_modifier(Modifier::BOLD))
            .style(Style::default().bg(theme.bg_panel));

        let action_p = Paragraph::new(Line::from(vec![
            Span::styled(" [CONFIRM] ", Style::default().fg(theme.accent_amber).add_modifier(Modifier::BOLD)),
            Span::styled(
                format!("$ {pending_cmd} "),
                Style::default().fg(theme.accent_cyan).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                " [APPROVE] ",
                Style::default().fg(Color::Black).bg(theme.accent_emerald).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                " [REJECT] ",
                Style::default().fg(Color::Black).bg(theme.accent_rose).add_modifier(Modifier::BOLD),
            ),
        ]))
        .block(action_block);

        f.render_widget(action_p, chunks[0]);

        // Hit regions for approve and reject buttons
        let pill_y = chunks[0].y + 1;
        let approve_x = chunks[0].x + chunks[0].width.saturating_sub(25);
        hit_map.approve_btn = Some(Rect::new(approve_x, pill_y, 11, 1));
        hit_map.reject_btn = Some(Rect::new(approve_x + 12, pill_y, 10, 1));

        (1, 2)
    } else {
        (0, 1)
    };

    hit_map.chat_input = chunks[input_chunk_idx];

    // Chat messages stream
    let status_str = if state.is_thinking {
        " COPILOT CHAT (THINKING...) "
    } else {
        " COPILOT CHAT "
    };

    let chat_block = Block::default()
        .title(Span::styled(
            status_str,
            if state.is_thinking {
                Style::default().fg(theme.accent_amber).add_modifier(Modifier::BOLD)
            } else {
                theme.panel_border(is_focused)
            },
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.panel_border(is_focused))
        .style(Style::default().bg(theme.bg_dark));

    let mut message_items: Vec<ListItem> = Vec::new();

    if state.chat_messages.is_empty() {
        message_items.push(ListItem::new(Line::from(vec![
            Span::styled(
                "No messages yet. Ask Copilot to inspect code, explain, or run commands.",
                Style::default().fg(theme.text_dim),
            ),
        ])));
    } else {
        for msg in state.chat_messages {
            let is_user = msg.role == "user";
            let is_system = msg.role == "system";

            let (role_label, role_style) = if is_user {
                ("[YOU]", Style::default().fg(theme.accent_cyan).add_modifier(Modifier::BOLD))
            } else if is_system {
                ("[SYSTEM]", Style::default().fg(theme.accent_purple).add_modifier(Modifier::BOLD))
            } else {
                ("[COPILOT]", Style::default().fg(theme.accent_emerald).add_modifier(Modifier::BOLD))
            };

            let header_line = Line::from(vec![
                Span::styled(format!("{role_label} "), role_style),
                Span::styled(format!("({})", msg.timestamp), Style::default().fg(theme.text_dim)),
            ]);
            message_items.push(ListItem::new(header_line));

            // Content lines with markdown highlight for code blocks
            let mut in_code_block = false;
            for line_str in msg.content.lines() {
                if line_str.trim_start().starts_with("```") {
                    in_code_block = !in_code_block;
                    message_items.push(ListItem::new(Line::from(vec![
                        Span::styled(format!("  {line_str}"), Style::default().fg(theme.accent_purple)),
                    ])));
                    continue;
                }

                if in_code_block {
                    message_items.push(ListItem::new(Line::from(vec![
                        Span::styled(
                            format!("    {line_str}"),
                            Style::default().fg(theme.accent_amber).bg(Color::Rgb(25, 28, 38)),
                        ),
                    ])));
                } else {
                    message_items.push(ListItem::new(Line::from(vec![
                        Span::styled(format!("  {line_str}"), Style::default().fg(theme.text_primary)),
                    ])));
                }
            }

            // If this assistant message includes a command
            if let Some(ref cmd) = msg.command_to_run {
                message_items.push(ListItem::new(Line::from(vec![
                    Span::styled(
                        format!("  [PROPOSED COMMAND] {cmd}"),
                        Style::default().fg(theme.accent_cyan).add_modifier(Modifier::ITALIC),
                    ),
                ])));
            }

            message_items.push(ListItem::new(Line::from(Span::raw(""))));
        }
    }

    let chat_height = chunks[chat_chunk_idx].height.saturating_sub(2) as usize;
    let total_chat_items = message_items.len();
    let scroll_offset = if total_chat_items > chat_height {
        (total_chat_items - chat_height).saturating_sub(state.chat_scroll)
    } else {
        0
    };

    let chat_list = List::new(
        message_items
            .into_iter()
            .skip(scroll_offset)
            .take(chat_height)
            .collect::<Vec<_>>(),
    )
    .block(chat_block);

    f.render_widget(chat_list, chunks[chat_chunk_idx]);

    // Chat input box
    let chat_input_block = Block::default()
        .title(if is_focused {
            " Message Copilot [Enter to Send] "
        } else {
            " Message Copilot (Click or Tab to focus) "
        })
        .title_style(theme.panel_border(is_focused))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(theme.panel_border(is_focused))
        .style(Style::default().bg(theme.bg_panel));

    let cursor_char = if is_focused { "█" } else { "" };
    let input_p = Paragraph::new(Line::from(vec![
        Span::styled(state.chat_input, Style::default().fg(theme.text_primary)),
        Span::styled(cursor_char, Style::default().fg(theme.accent_cyan)),
    ]))
    .block(chat_input_block);

    f.render_widget(input_p, chunks[input_chunk_idx]);
}

fn render_bottom_toolbar(
    f: &mut Frame,
    area: Rect,
    state: &RenderState,
    theme: &Theme,
    hit_map: &mut UiHitMap,
) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(35), // Model selector button / status
            Constraint::Percentage(35), // Permission status button
            Constraint::Percentage(30), // Lightweight memory & system status
        ])
        .split(area);

    hit_map.bottom_model_btn = chunks[0];
    hit_map.bottom_perm_btn = chunks[1];

    // 1. Model switch button
    let model_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent_cyan))
        .style(Style::default().bg(theme.bg_panel));

    let provider_name = state.config.active_provider.display_name();
    let model_p = Paragraph::new(Line::from(vec![
        Span::styled(" [F2/Ctrl+M] ", Style::default().fg(theme.accent_cyan).add_modifier(Modifier::BOLD)),
        Span::styled(format!("{}: ", provider_name), Style::default().fg(theme.text_secondary)),
        Span::styled(
            &state.config.active_model,
            Style::default().fg(theme.accent_emerald).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" [CHANGE]", Style::default().fg(theme.accent_cyan)),
    ]))
    .block(model_block);
    f.render_widget(model_p, chunks[0]);

    // 2. Permission mode switch button
    let perm_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border_normal))
        .style(Style::default().bg(theme.bg_panel));

    let perm_color = match state.config.permission {
        crate::config::AgentPermission::ConfirmEach => theme.accent_amber,
        crate::config::AgentPermission::AutoExecute => theme.accent_emerald,
        crate::config::AgentPermission::ReadOnly => theme.accent_rose,
    };

    let perm_p = Paragraph::new(Line::from(vec![
        Span::styled(" [F3/Ctrl+X] ", Style::default().fg(theme.accent_cyan).add_modifier(Modifier::BOLD)),
        Span::styled("Permission: ", Style::default().fg(theme.text_secondary)),
        Span::styled(
            state.config.permission.label(),
            Style::default().fg(perm_color).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" [TOGGLE]", Style::default().fg(theme.text_dim)),
    ]))
    .block(perm_block);
    f.render_widget(perm_p, chunks[1]);

    // 3. System & Memory footprint + panel resize hint
    let sys_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.border_normal))
        .style(Style::default().bg(theme.bg_panel));

    let sys_p = Paragraph::new(Line::from(vec![
        Span::styled("Rust ", Style::default().fg(theme.accent_amber).add_modifier(Modifier::BOLD)),
        Span::styled("| RAM: <16MB | ", Style::default().fg(theme.text_secondary)),
        Span::styled(
            format!("[{}/{}] Resize: [ / ]", 100 - state.config.chat_panel_percent, state.config.chat_panel_percent),
            Style::default().fg(theme.accent_cyan),
        ),
    ]))
    .alignment(Alignment::Right)
    .block(sys_block);
    f.render_widget(sys_p, chunks[2]);
}
