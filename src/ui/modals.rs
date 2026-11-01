use crate::config::{AppConfig, ProviderType};
use crate::ui::theme::Theme;
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Borders, Clear, List, ListItem, Paragraph},
    Frame,
};

#[derive(Debug, Clone)]
pub struct ProviderSettingsState {
    pub selected_provider_idx: usize,
    pub api_key: String,
    pub base_url: String,
    pub account_id: String,
    pub selected_model_idx: usize,
    pub custom_model: String,
    pub active_field: usize, // 0: Provider, 1: ApiKey, 2: ExtraField, 3: Model, 4: CustomModel
    pub mask_key: bool,
}

impl ProviderSettingsState {
    pub fn new(config: &AppConfig) -> Self {
        let providers = ProviderType::all();
        let current_p_idx = providers
            .iter()
            .position(|&p| p == config.active_provider)
            .unwrap_or(0);
        let current_cfg = config.get_active_provider_config().cloned().unwrap_or_default();

        let models = config.active_provider.default_models();
        let model_idx = models
            .iter()
            .position(|&m| m == config.active_model)
            .unwrap_or(0);

        Self {
            selected_provider_idx: current_p_idx,
            api_key: current_cfg.api_key,
            base_url: current_cfg
                .base_url
                .unwrap_or_else(|| "https://api.openai.com/v1".to_string()),
            account_id: current_cfg.account_id.unwrap_or_default(),
            selected_model_idx: model_idx,
            custom_model: if !models.contains(&config.active_model.as_str()) {
                config.active_model.clone()
            } else {
                String::new()
            },
            active_field: 0,
            mask_key: true,
        }
    }

    pub fn current_provider(&self) -> ProviderType {
        ProviderType::all()[self.selected_provider_idx]
    }

    pub fn on_provider_change(&mut self, config: &AppConfig) {
        let p = self.current_provider();
        if let Some(cfg) = config.get_provider_config(p) {
            self.api_key = cfg.api_key.clone();
            self.base_url = cfg
                .base_url
                .clone()
                .unwrap_or_else(|| match p {
                    ProviderType::OpenRouter => "https://openrouter.ai/api/v1".to_string(),
                    _ => "https://api.openai.com/v1".to_string(),
                });
            self.account_id = cfg.account_id.clone().unwrap_or_default();
            self.selected_model_idx = 0;
            self.custom_model = String::new();
        } else {
            self.api_key.clear();
            self.selected_model_idx = 0;
            self.custom_model.clear();
        }
    }
}

pub struct ModalHitMap {
    pub save_btn: Option<Rect>,
    pub cancel_btn: Option<Rect>,
    pub step1_area: Option<Rect>,
    pub step2_area: Option<Rect>,
    pub step2b_area: Option<Rect>,
    pub step3_area: Option<Rect>,
}

pub fn render_provider_settings_modal(
    f: &mut Frame,
    state: &ProviderSettingsState,
    theme: &Theme,
) -> ModalHitMap {
    let area = centered_rect(76, 78, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" AI PROVIDER CONFIGURATION ")
        .title_style(theme.title_style())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent_cyan))
        .style(Style::default().bg(theme.bg_panel));

    f.render_widget(block, area);

    let inner_area = Rect::new(
        area.x + 2,
        area.y + 1,
        area.width.saturating_sub(4),
        area.height.saturating_sub(2),
    );

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7), // 1. Select Provider (5 providers now)
            Constraint::Length(4), // 2. API Key
            Constraint::Length(4), // 2b. Base URL or Account ID
            Constraint::Min(6),    // 3. Select Default Model
            Constraint::Length(3), // Footer hints & buttons
        ])
        .split(inner_area);

    // Section 1: Provider Selection
    let providers = ProviderType::all();
    let provider_lines: Vec<Line> = providers
        .iter()
        .enumerate()
        .map(|(idx, &p)| {
            let selected = idx == state.selected_provider_idx;
            let is_focused = state.active_field == 0 && selected;
            let prefix = if selected { " > " } else { "   " };
            let style = if is_focused {
                Style::default().fg(theme.accent_cyan).add_modifier(Modifier::BOLD)
            } else if selected {
                Style::default().fg(theme.text_primary).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_dim)
            };
            Line::from(vec![
                Span::styled(prefix, style),
                Span::styled(format!("{}. {}", idx + 1, p.display_name()), style),
            ])
        })
        .collect();

    let p1_block = Block::default()
        .title(Span::styled(
            " Step 1: Select Provider (Click, Up/Down, or 1-5) ",
            if state.active_field == 0 {
                Style::default().fg(theme.accent_cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_secondary)
            },
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if state.active_field == 0 {
            Style::default().fg(theme.accent_cyan)
        } else {
            Style::default().fg(theme.border_normal)
        });

    f.render_widget(Paragraph::new(provider_lines).block(p1_block), chunks[0]);

    // Section 2: API Key
    let key_display = if state.api_key.is_empty() {
        "Paste or type API key here...".to_string()
    } else if state.mask_key {
        let len = state.api_key.len();
        if len > 8 {
            format!("{}...{}", &state.api_key[..4], "*".repeat(len - 8))
        } else {
            "*".repeat(len)
        }
    } else {
        state.api_key.clone()
    };

    let p2_block = Block::default()
        .title(Span::styled(
            format!(
                " Step 2: API Key {} ",
                if state.mask_key { "[F4: show]" } else { "[F4: hide]" }
            ),
            if state.active_field == 1 {
                Style::default().fg(theme.accent_cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_secondary)
            },
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if state.active_field == 1 {
            Style::default().fg(theme.accent_cyan)
        } else {
            Style::default().fg(theme.border_normal)
        });

    let key_style = if state.api_key.is_empty() {
        Style::default().fg(theme.text_dim)
    } else {
        Style::default().fg(theme.accent_emerald)
    };

    f.render_widget(
        Paragraph::new(Span::styled(format!(" [KEY] {key_display}"), key_style)).block(p2_block),
        chunks[1],
    );

    // Section 2b: Extra Field (Base URL for OpenAI/OpenRouter, Account ID for Cloudflare)
    let cur_provider = state.current_provider();
    let (extra_label, extra_val, extra_hint) = match cur_provider {
        ProviderType::OpenAI => (
            " Base URL (OpenAI / Local / Groq / Ollama / DeepSeek) ",
            &state.base_url,
            "Default: https://api.openai.com/v1",
        ),
        ProviderType::OpenRouter => (
            " OpenRouter Endpoint Base URL ",
            &state.base_url,
            "Default: https://openrouter.ai/api/v1",
        ),
        ProviderType::Cloudflare => (
            " Cloudflare Account ID (Required) ",
            &state.account_id,
            "Find in Cloudflare Dashboard URL",
        ),
        _ => (
            " Optional Endpoint Setting (Not required for this provider) ",
            &String::new(),
            "Standard API endpoint will be used",
        ),
    };

    let p2b_block = Block::default()
        .title(Span::styled(
            extra_label,
            if state.active_field == 2 {
                Style::default().fg(theme.accent_cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_secondary)
            },
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if state.active_field == 2 {
            Style::default().fg(theme.accent_cyan)
        } else {
            Style::default().fg(theme.border_normal)
        });

    let extra_display = if extra_val.is_empty() {
        extra_hint.to_string()
    } else {
        extra_val.clone()
    };

    f.render_widget(
        Paragraph::new(Span::styled(
            format!(" [URL] {extra_display}"),
            if extra_val.is_empty() {
                Style::default().fg(theme.text_dim)
            } else {
                Style::default().fg(theme.text_primary)
            },
        ))
        .block(p2b_block),
        chunks[2],
    );

    // Section 3: Select Default Model
    let models = cur_provider.default_models();
    let mut model_items: Vec<ListItem> = models
        .iter()
        .enumerate()
        .map(|(idx, &m)| {
            let is_sel = idx == state.selected_model_idx && state.custom_model.is_empty();
            let prefix = if is_sel { " [x] " } else { " [ ] " };
            let style = if is_sel {
                Style::default().fg(theme.accent_emerald).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_secondary)
            };
            ListItem::new(Span::styled(format!("{prefix}{m}"), style))
        })
        .collect();

    let custom_item_style = if !state.custom_model.is_empty() {
        Style::default().fg(theme.accent_cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text_dim)
    };
    model_items.push(ListItem::new(Span::styled(
        format!(
            " [{}] Custom: {}",
            if !state.custom_model.is_empty() { "x" } else { " " },
            if state.custom_model.is_empty() {
                "(Click or tab to type custom model name)"
            } else {
                &state.custom_model
            }
        ),
        custom_item_style,
    )));

    let p3_block = Block::default()
        .title(Span::styled(
            " Step 3: Default Model to Use (Click or Up/Down) ",
            if state.active_field == 3 || state.active_field == 4 {
                Style::default().fg(theme.accent_cyan).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(theme.text_secondary)
            },
        ))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(if state.active_field == 3 || state.active_field == 4 {
            Style::default().fg(theme.accent_cyan)
        } else {
            Style::default().fg(theme.border_normal)
        });

    f.render_widget(List::new(model_items).block(p3_block), chunks[3]);

    // Footer actions
    let footer_text = vec![
        Span::styled(" [SAVE & ACTIVATE (ENTER)] ", Style::default().fg(Color::Black).bg(theme.accent_emerald).add_modifier(Modifier::BOLD)),
        Span::raw("   "),
        Span::styled(" [CANCEL (ESC)] ", Style::default().fg(Color::Black).bg(theme.accent_rose).add_modifier(Modifier::BOLD)),
    ];

    let footer = Paragraph::new(Line::from(footer_text))
        .alignment(Alignment::Center)
        .style(Style::default().fg(theme.text_secondary));

    f.render_widget(footer, chunks[4]);

    let footer_y = chunks[4].y + 1;
    let center_x = chunks[4].x + chunks[4].width / 2;
    ModalHitMap {
        save_btn: Some(Rect::new(center_x.saturating_sub(25), footer_y, 22, 1)),
        cancel_btn: Some(Rect::new(center_x + 2, footer_y, 16, 1)),
        step1_area: Some(chunks[0]),
        step2_area: Some(chunks[1]),
        step2b_area: Some(chunks[2]),
        step3_area: Some(chunks[3]),
    }
}

pub fn render_quick_model_modal(
    f: &mut Frame,
    provider: ProviderType,
    current_model: &str,
    selected_idx: usize,
    custom_input: &str,
    theme: &Theme,
) {
    let area = centered_rect(54, 50, f.area());
    f.render_widget(Clear, area);

    let models = provider.default_models();
    let mut items = Vec::new();

    for (i, &m) in models.iter().enumerate() {
        let is_selected = i == selected_idx;
        let is_active = m == current_model;
        let prefix = if is_selected { " > " } else { "   " };
        let active_mark = if is_active { " [ACTIVE]" } else { "" };

        let style = if is_selected {
            Style::default().fg(theme.accent_cyan).add_modifier(Modifier::BOLD)
        } else if is_active {
            Style::default().fg(theme.accent_emerald)
        } else {
            Style::default().fg(theme.text_secondary)
        };

        items.push(ListItem::new(Span::styled(
            format!("{prefix}{m}{active_mark}"),
            style,
        )));
    }

    let custom_idx = models.len();
    let custom_selected = selected_idx == custom_idx;
    let custom_style = if custom_selected {
        Style::default().fg(theme.accent_cyan).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(theme.text_dim)
    };

    items.push(ListItem::new(Span::styled(
        format!(
            " {} Custom: {}",
            if custom_selected { ">" } else { " " },
            if custom_input.is_empty() {
                "Type custom model..."
            } else {
                custom_input
            }
        ),
        custom_style,
    )));

    let block = Block::default()
        .title(format!(" SWITCH MODEL ({}) ", provider.display_name()))
        .title_style(theme.title_style())
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent_cyan))
        .style(Style::default().bg(theme.bg_panel));

    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(4), Constraint::Length(2)])
        .split(Rect::new(area.x + 1, area.y + 1, area.width - 2, area.height - 2));

    f.render_widget(block, area);
    f.render_widget(List::new(items), layout[0]);

    let footer = Paragraph::new(Line::from(vec![
        Span::styled("[Click/Up/Down]", Style::default().fg(theme.accent_cyan)),
        Span::raw(" Select | "),
        Span::styled("[Enter]", Style::default().fg(theme.accent_emerald)),
        Span::raw(" Switch | "),
        Span::styled("[Esc]", Style::default().fg(theme.accent_rose)),
        Span::raw(" Close"),
    ]))
    .alignment(Alignment::Center);

    f.render_widget(footer, layout[1]);
}

pub fn render_command_confirm_modal(f: &mut Frame, cmd: &str, theme: &Theme) {
    let area = centered_rect(65, 30, f.area());
    f.render_widget(Clear, area);

    let block = Block::default()
        .title(" AGENT WORKSPACE PERMISSION REQUIRED ")
        .title_style(Style::default().fg(theme.accent_amber).add_modifier(Modifier::BOLD))
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent_amber))
        .style(Style::default().bg(theme.bg_panel));

    f.render_widget(block, area);

    let inner = Rect::new(area.x + 2, area.y + 2, area.width - 4, area.height - 4);
    let layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(2),
            Constraint::Length(3),
            Constraint::Length(2),
            Constraint::Length(2),
        ])
        .split(inner);

    let prompt = Paragraph::new(Line::from(vec![
        Span::styled("AI Agent requested workspace execution:", Style::default().fg(theme.text_primary)),
    ]));

    let cmd_box = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(theme.accent_cyan));

    let cmd_text = Paragraph::new(Span::styled(
        format!(" $ {cmd}"),
        Style::default().fg(theme.accent_cyan).add_modifier(Modifier::BOLD),
    ))
    .block(cmd_box);

    let buttons = Line::from(vec![
        Span::styled("  [ APPROVE & EXECUTE (ENTER) ]  ", Style::default().fg(Color::Black).bg(theme.accent_emerald).add_modifier(Modifier::BOLD)),
        Span::raw("    "),
        Span::styled("  [ REJECT / CANCEL (ESC) ]  ", Style::default().fg(Color::Black).bg(theme.accent_rose).add_modifier(Modifier::BOLD)),
    ]);

    let hint = Paragraph::new(Span::styled(
        "(Press Ctrl+X or click Permission button to switch to Auto-Execute mode)",
        Style::default().fg(theme.text_dim),
    ))
    .alignment(Alignment::Center);

    f.render_widget(prompt, layout[0]);
    f.render_widget(cmd_text, layout[1]);
    f.render_widget(Paragraph::new(buttons).alignment(Alignment::Center), layout[2]);
    f.render_widget(hint, layout[3]);
}

pub fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}
