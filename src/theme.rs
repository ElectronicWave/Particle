use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub accent: Color,
    pub accent_fg: Color,
    pub border: Color,
    pub border_focused: Color,
    pub divider: Color,
    pub text: Color,
    pub text_muted: Color,
    pub text_dim: Color,
    pub success: Color,
    pub warning: Color,
    pub danger: Color,
    pub info: Color,
    pub selection_bg: Color,
    pub selection_fg: Color,
}

impl Theme {
    pub const fn dark() -> Self {
        Self {
            accent: Color::LightGreen,
            accent_fg: Color::Black,
            border: Color::DarkGray,
            border_focused: Color::LightGreen,
            divider: Color::DarkGray,
            text: Color::Reset,
            text_muted: Color::Gray,
            text_dim: Color::DarkGray,
            success: Color::LightGreen,
            warning: Color::Yellow,
            danger: Color::Red,
            info: Color::Cyan,
            selection_bg: Color::LightGreen,
            selection_fg: Color::Black,
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}
