use ratatui::style::{Color, Modifier, Style};

pub struct Theme {
    pub bg_dark: Color,
    pub bg_panel: Color,
    pub border_normal: Color,
    pub border_focus: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_dim: Color,
    pub accent_cyan: Color,
    pub accent_purple: Color,
    pub accent_emerald: Color,
    pub accent_amber: Color,
    pub accent_rose: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            bg_dark: Color::Rgb(13, 15, 20),
            bg_panel: Color::Rgb(20, 23, 31),
            border_normal: Color::Rgb(45, 52, 64),
            border_focus: Color::Rgb(56, 189, 248), // Electric Cyan
            text_primary: Color::Rgb(241, 245, 249),
            text_secondary: Color::Rgb(148, 163, 184),
            text_dim: Color::Rgb(100, 116, 139),
            accent_cyan: Color::Rgb(56, 189, 248),
            accent_purple: Color::Rgb(168, 85, 247),
            accent_emerald: Color::Rgb(52, 211, 153),
            accent_amber: Color::Rgb(251, 191, 36),
            accent_rose: Color::Rgb(244, 63, 94),
        }
    }
}

impl Theme {
    pub fn title_style(&self) -> Style {
        Style::default()
            .fg(self.accent_cyan)
            .add_modifier(Modifier::BOLD)
    }

    pub fn panel_border(&self, focused: bool) -> Style {
        if focused {
            Style::default().fg(self.border_focus).add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(self.border_normal)
        }
    }
}
