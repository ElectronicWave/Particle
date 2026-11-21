use elemental::core::auth::accounts::AccountId;
use percent_encoding::percent_decode_str;
use ratail::{KeyHint, NavAction, Page, PageEvent};
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

use super::AccountRoute;
use crate::pages::content::render_message;
use crate::services::Services;

const DND_PREFIX: &str = "authlib-injector:yggdrasil-server:";

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

pub(super) struct YggdrasilPage {
    account_id: Option<AccountId>,
    base_url: TextEdit,
    username: TextEdit,
    password: TextEdit,
    focused: YggdrasilField,
    error: Option<String>,
    waiting: bool,
}

impl YggdrasilPage {
    pub(super) fn add() -> Self {
        Self::new(None, String::new())
    }

    pub(super) fn edit(account_id: AccountId, base_url: String) -> Self {
        Self::new(Some(account_id), base_url)
    }

    fn new(account_id: Option<AccountId>, base_url: String) -> Self {
        let focused = if base_url.is_empty() {
            YggdrasilField::BaseUrl
        } else {
            YggdrasilField::Username
        };
        Self {
            account_id,
            base_url: TextEdit::new(base_url),
            username: TextEdit::new(String::new()),
            password: TextEdit::new(String::new()),
            focused,
            error: None,
            waiting: false,
        }
    }

    fn values(&self) -> (String, String, String) {
        (
            self.base_url.input().trim().to_owned(),
            self.username.input().trim().to_owned(),
            self.password.input().to_owned(),
        )
    }

    fn render_form(&self, frame: &mut Frame, area: Rect, theme: Theme) {
        let dim = Style::new().fg(theme.text_dim);
        let mut lines = vec![
            Line::styled(
                t!("accounts.yggdrasil_title"),
                Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
            ),
            Line::raw(""),
            self.field_line(YggdrasilField::BaseUrl, theme),
            self.field_line(YggdrasilField::Username, theme),
            self.field_line(YggdrasilField::Password, theme),
            Line::raw(""),
        ];

        if let Some(error) = &self.error {
            lines.push(Line::styled(error.clone(), Style::new().fg(theme.danger)));
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

    fn submit(&mut self, ctx: &Services) -> PageEvent<AccountRoute> {
        let (base_url, username, password) = self.values();
        match validate(&base_url, &username, &password) {
            Ok(base_url) => {
                let started = match self.account_id {
                    Some(account_id) => ctx
                        .accounts()
                        .edit_yggdrasil(account_id, base_url, username, password),
                    None => ctx
                        .accounts()
                        .sign_in_yggdrasil(base_url, username, password),
                };
                if !started {
                    self.error = Some(t!("accounts.busy").into_owned());
                } else {
                    self.error = None;
                    self.waiting = true;
                }
                PageEvent::Handled
            }
            Err(error) => {
                self.error = Some(error);
                PageEvent::Handled
            }
        }
    }

    fn handle_form_key(&mut self, ctx: &Services, key: KeyEvent) -> PageEvent<AccountRoute> {
        match key.code {
            KeyCode::Tab | KeyCode::Down => {
                self.focused = self.focused.next();
                self.error = None;
                PageEvent::Handled
            }
            KeyCode::BackTab | KeyCode::Up => {
                self.focused = self.focused.previous();
                self.error = None;
                PageEvent::Handled
            }
            KeyCode::Enter => self.submit(ctx),
            KeyCode::Esc => PageEvent::Navigate(NavAction::Pop),
            _ => match self.active_input_mut().handle_key(key) {
                EditEvent::Changed => {
                    self.error = None;
                    PageEvent::Handled
                }
                EditEvent::Unhandled => PageEvent::Ignored,
                EditEvent::Submit => self.submit(ctx),
                EditEvent::Cancel => PageEvent::Navigate(NavAction::Pop),
            },
        }
    }

    fn field_line(&self, field: YggdrasilField, theme: Theme) -> Line<'static> {
        let focused = self.focused == field;
        let marker = if focused { "▸ " } else { "  " };
        let label_style = if focused {
            Style::new().fg(theme.accent)
        } else {
            Style::new().fg(theme.text_muted)
        };
        let value = match field {
            YggdrasilField::BaseUrl if focused => self.base_url.cursor_display(),
            YggdrasilField::BaseUrl => self.base_url.input().to_owned(),
            YggdrasilField::Username if focused => self.username.cursor_display(),
            YggdrasilField::Username => self.username.input().to_owned(),
            YggdrasilField::Password => {
                let masked = "•".repeat(self.password.input().chars().count());
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
}

impl Page<Services, AccountRoute> for YggdrasilPage {
    fn on_key(
        &mut self,
        ctx: &Services,
        _route: &AccountRoute,
        key: KeyEvent,
    ) -> PageEvent<AccountRoute> {
        if self.waiting {
            if key.code == KeyCode::Esc {
                ctx.accounts().cancel_authorization();
                self.waiting = false;
                return PageEvent::Handled;
            }
            return PageEvent::Ignored;
        }
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return PageEvent::Ignored;
        }
        self.handle_form_key(ctx, key)
    }

    fn keybindings(&self, _route: &AccountRoute) -> Vec<KeyHint> {
        if self.waiting {
            vec![KeyHint::new("ESC", t!("common.cancel"))]
        } else {
            vec![
                KeyHint::new("TAB", t!("accounts.switch_field")),
                KeyHint::new("ENTER", t!("common.sign_in")),
                KeyHint::new("ESC", t!("common.cancel")),
            ]
        }
    }

    fn poll(&mut self, ctx: &Services, _route: &AccountRoute) -> Option<NavAction<AccountRoute>> {
        if !self.waiting || ctx.accounts().is_running() {
            return None;
        }
        self.waiting = false;
        match ctx.accounts().state().snapshot().view().error {
            Some(error) => {
                self.error = Some(error.to_string());
                None
            }
            None => Some(NavAction::Pop),
        }
    }

    fn render(&mut self, ctx: &Services, _route: &AccountRoute, frame: &mut Frame, area: Rect) {
        if self.waiting {
            render_message(frame, area, t!("accounts.yggdrasil_waiting").as_ref());
        } else {
            self.render_form(frame, area, ctx.ui().theme());
        }
    }
}

pub(super) fn validate(base_url: &str, username: &str, password: &str) -> Result<String, String> {
    let base_url = resolve_base_url(base_url)?;

    if base_url.is_empty() || username.is_empty() || password.is_empty() {
        return Err(t!("accounts.yggdrasil_required").into_owned());
    }

    let valid_url = base_url
        .strip_prefix("https://")
        .or_else(|| base_url.strip_prefix("http://"))
        .is_some_and(|rest| !rest.is_empty() && !rest.chars().any(char::is_whitespace));
    if !valid_url {
        return Err(t!("accounts.yggdrasil_invalid_url").into_owned());
    }

    Ok(base_url)
}

fn resolve_base_url(input: &str) -> Result<String, String> {
    let trimmed = input.trim().trim_matches(['"', '\'']);
    let resolved = match trimmed.strip_prefix(DND_PREFIX) {
        Some(encoded) => percent_decode_str(encoded)
            .decode_utf8()
            .map_err(|_| t!("accounts.yggdrasil_invalid_dnd").into_owned())?
            .into_owned(),
        None => trimmed.to_owned(),
    };

    Ok(resolved.trim().trim_end_matches('/').to_owned())
}
