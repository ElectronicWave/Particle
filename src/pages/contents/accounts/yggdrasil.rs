use ratail::KeyHint;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};
use unicode_width::UnicodeWidthStr;

use crate::t;
use crate::theme::Theme;
use crate::widgets::text_edit::{EditEvent, TextEdit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum YggdrasilField {
    BaseUrl,
    Username,
    Password,
}

impl YggdrasilField {
    fn next(self) -> Self {
        match self {
            Self::BaseUrl => Self::Username,
            Self::Username => Self::Password,
            Self::Password => Self::BaseUrl,
        }
    }

    fn previous(self) -> Self {
        match self {
            Self::BaseUrl => Self::Password,
            Self::Username => Self::BaseUrl,
            Self::Password => Self::Username,
        }
    }

    fn label(self) -> String {
        match self {
            Self::BaseUrl => t!("accounts.yggdrasil_base_url").into_owned(),
            Self::Username => t!("accounts.yggdrasil_username").into_owned(),
            Self::Password => t!("accounts.yggdrasil_password").into_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum YggdrasilFormEvent {
    Ignored,
    Changed,
    Submit,
    Cancel,
}

#[derive(Debug)]
pub(super) struct YggdrasilForm {
    base_url: TextEdit,
    username: TextEdit,
    password: TextEdit,
    focused: YggdrasilField,
}

impl YggdrasilForm {
    pub(super) fn new() -> Self {
        Self {
            base_url: TextEdit::new(String::new()),
            username: TextEdit::new(String::new()),
            password: TextEdit::new(String::new()),
            focused: YggdrasilField::BaseUrl,
        }
    }

    pub(super) fn handle_key(&mut self, key: KeyEvent) -> YggdrasilFormEvent {
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return YggdrasilFormEvent::Ignored;
        }

        match key.code {
            KeyCode::Tab | KeyCode::Down => {
                self.focused = self.focused.next();
                YggdrasilFormEvent::Changed
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.focused = self.focused.previous();
                YggdrasilFormEvent::Changed
            }
            KeyCode::Enter => YggdrasilFormEvent::Submit,
            KeyCode::Esc => YggdrasilFormEvent::Cancel,
            _ => match self.active_input_mut().handle_key(key) {
                EditEvent::Changed => YggdrasilFormEvent::Changed,
                EditEvent::Unhandled => YggdrasilFormEvent::Ignored,
                EditEvent::Submit => YggdrasilFormEvent::Submit,
                EditEvent::Cancel => YggdrasilFormEvent::Cancel,
            },
        }
    }

    pub(super) fn values(&self) -> (String, String, String) {
        (
            self.base_url
                .input()
                .trim()
                .trim_end_matches('/')
                .to_owned(),
            self.username.input().trim().to_owned(),
            self.password.input().to_owned(),
        )
    }

    pub(super) fn render(&self, frame: &mut Frame, area: Rect, theme: Theme, error: Option<&str>) {
        let dim = Style::new().fg(theme.text_dim);
        let mut lines = vec![
            Line::styled(
                t!("accounts.yggdrasil_title"),
                Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
            ),
            Line::raw(""),
            field_line(self, YggdrasilField::BaseUrl, theme),
            field_line(self, YggdrasilField::Username, theme),
            field_line(self, YggdrasilField::Password, theme),
            Line::raw(""),
        ];

        if let Some(error) = error {
            lines.push(Line::styled(
                error.to_owned(),
                Style::new().fg(theme.danger),
            ));
        }
        lines.push(Line::styled(t!("accounts.yggdrasil_help"), dim));

        frame.render_widget(Paragraph::new(lines), area);
    }

    fn active_input_mut(&mut self) -> &mut TextEdit {
        match self.focused {
            YggdrasilField::BaseUrl => &mut self.base_url,
            YggdrasilField::Username => &mut self.username,
            YggdrasilField::Password => &mut self.password,
        }
    }
}

pub(super) fn hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("TAB", t!("accounts.switch_field")),
        KeyHint::new("ENTER", t!("common.sign_in")),
        KeyHint::new("ESC", t!("common.cancel")),
    ]
}

pub(super) fn validate(base_url: &str, username: &str, password: &str) -> Option<String> {
    if base_url.is_empty() || username.is_empty() || password.is_empty() {
        return Some(t!("accounts.yggdrasil_required").into_owned());
    }

    let valid_url = base_url
        .strip_prefix("https://")
        .or_else(|| base_url.strip_prefix("http://"))
        .is_some_and(|rest| !rest.is_empty() && !rest.chars().any(char::is_whitespace));
    if !valid_url {
        return Some(t!("accounts.yggdrasil_invalid_url").into_owned());
    }

    None
}

fn field_line(form: &YggdrasilForm, field: YggdrasilField, theme: Theme) -> Line<'static> {
    let focused = form.focused == field;
    let marker = if focused { "▸ " } else { "  " };
    let label_style = if focused {
        Style::new().fg(theme.accent)
    } else {
        Style::new().fg(theme.text_muted)
    };
    let value = match field {
        YggdrasilField::BaseUrl if focused => form.base_url.cursor_display(),
        YggdrasilField::BaseUrl => form.base_url.input().to_owned(),
        YggdrasilField::Username if focused => form.username.cursor_display(),
        YggdrasilField::Username => form.username.input().to_owned(),
        YggdrasilField::Password => {
            let masked = "•".repeat(form.password.input().chars().count());
            if focused {
                format!("{masked}█")
            } else {
                masked
            }
        }
    };

    let label = field.label();
    let padding = 20usize.saturating_sub(UnicodeWidthStr::width(label.as_str()));

    Line::from(vec![
        Span::styled(marker, Style::new().fg(theme.accent)),
        Span::styled(format!("{label}{}", " ".repeat(padding)), label_style),
        Span::raw(value),
    ])
}
