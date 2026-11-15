use crate::t;
use elemental::core::auth::accounts::AccountId;
use ratail::{KeyHint, SnapshotStatus};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState, Paragraph},
};

use crate::app::config::microsoft_client_id;
use crate::app::services::{AccountState, AuthorizationState, Services};
use crate::pages::content::{Content, ContentEvent, render_loading, render_message};
use crate::theme::Theme;
use crate::widgets::{
    dropdown::Dropdown,
    popup::{Popup, PopupEvent},
    text_edit::{EditEvent, TextEdit},
};

fn list_hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("↑/↓", t!("common.browse")),
        KeyHint::new("ENTER", t!("common.select")),
        KeyHint::new("A", t!("common.add_account")),
        KeyHint::new("D", t!("common.remove")),
        KeyHint::new("R", t!("common.refresh")),
        KeyHint::new("ESC", t!("common.return")),
    ]
}

fn add_hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("ENTER", t!("common.sign_in")),
        KeyHint::new("ESC", t!("common.cancel")),
    ]
}

#[derive(Debug)]
enum Mode {
    List,
    AddOffline(TextEdit),
    Authorize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AuthorizerKind {
    Microsoft,
    Offline,
}

#[derive(Debug)]
pub struct AccountsContent {
    mode: Mode,
    authorizer: Option<Dropdown<AuthorizerKind>>,
    list: ListState,
    error: Option<String>,
}

impl Default for AccountsContent {
    fn default() -> Self {
        Self {
            mode: Mode::List,
            authorizer: None,
            list: ListState::default().with_selected(Some(0)),
            error: None,
        }
    }
}

impl Content for AccountsContent {
    fn init(&mut self, ctx: &Services) {
        ctx.load_accounts();
    }

    fn render(&mut self, ctx: &Services, frame: &mut Frame, area: Rect) {
        let theme = ctx.theme();
        let [list_area, footer_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(2)]).areas(area);

        if matches!(self.mode, Mode::Authorize) && ctx.authorization() == AuthorizationState::Idle {
            self.mode = Mode::List;
        }

        if self.render_authorization(ctx, frame, list_area, theme) {
            self.render_footer(frame, footer_area, theme);
            return;
        }

        let snapshot = ctx.account_state().snapshot();
        let view = snapshot.view();
        match (view.data, view.error) {
            (Some(state), Some(error)) => {
                let [error_area, accounts_area] =
                    Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(list_area);
                render_message(
                    frame,
                    error_area,
                    t!("common.error", error = error).as_ref(),
                );
                self.render_accounts(state, frame, accounts_area, theme);
            }
            (Some(state), None) => self.render_accounts(state, frame, list_area, theme),
            (None, Some(error)) => {
                render_message(frame, list_area, t!("common.error", error = error).as_ref())
            }
            (None, None) if view.status == SnapshotStatus::Cancelled => {
                render_message(frame, list_area, t!("common.cancelled").as_ref())
            }
            (None, None) => render_loading(ctx, frame, list_area, t!("accounts.loading").as_ref()),
        }

        self.render_footer(frame, footer_area, theme);

        if let Some(popup) = &self.authorizer {
            let anchor = Rect {
                x: area.x,
                y: footer_area.y,
                width: 1,
                height: 1,
            };
            popup.render(frame, area, anchor, theme);
        }
    }

    fn on_key(&mut self, ctx: &Services, key: KeyEvent) -> ContentEvent {
        if let Some(popup) = &mut self.authorizer {
            match popup.handle_key(key) {
                PopupEvent::Ignored | PopupEvent::Handled => return ContentEvent::Handled,
                PopupEvent::Cancelled => {
                    self.authorizer = None;
                    return ContentEvent::Handled;
                }
                PopupEvent::Completed(kind) => {
                    self.authorizer = None;
                    self.start_authorizer(ctx, kind);
                    return ContentEvent::Handled;
                }
            }
        }

        let mut edit = match std::mem::replace(&mut self.mode, Mode::List) {
            Mode::List => return self.on_list_key(ctx, key),
            Mode::Authorize => {
                if key.code == KeyCode::Esc {
                    ctx.cancel_authorization();
                }
                return ContentEvent::Handled;
            }
            Mode::AddOffline(edit) => edit,
        };

        match edit.handle_key(key) {
            EditEvent::Submit => {
                let name = edit.input().trim().to_owned();
                if !name.is_empty() {
                    ctx.sign_in_offline(name);
                }
            }
            EditEvent::Changed => self.mode = Mode::AddOffline(edit),
            EditEvent::Unhandled => self.mode = Mode::AddOffline(edit),
            EditEvent::Cancel => {}
        }
        ContentEvent::Handled
    }

    fn keybindings(&self) -> Vec<KeyHint> {
        if let Some(popup) = &self.authorizer {
            return popup.hints();
        }
        match self.mode {
            Mode::List => list_hints(),
            Mode::AddOffline(_) => add_hints(),
            Mode::Authorize => vec![KeyHint::new("ESC", t!("common.cancel"))],
        }
    }
}

impl AccountsContent {
    fn render_authorization(
        &self,
        ctx: &Services,
        frame: &mut Frame,
        area: Rect,
        theme: Theme,
    ) -> bool {
        match ctx.authorization() {
            AuthorizationState::Idle => false,
            AuthorizationState::Starting => {
                render_loading(ctx, frame, area, t!("accounts.microsoft_starting").as_ref());
                true
            }
            AuthorizationState::Awaiting {
                verification_uri,
                user_code,
            } => {
                let dim = Style::new().fg(theme.text_dim);
                let strong = Style::new().fg(theme.accent).add_modifier(Modifier::BOLD);
                let lines = vec![
                    Line::styled(
                        t!("accounts.microsoft_title"),
                        Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
                    ),
                    Line::raw(""),
                    Line::from(vec![
                        Span::styled(t!("accounts.microsoft_open"), dim),
                        Span::styled(verification_uri, strong),
                    ]),
                    Line::from(vec![
                        Span::styled(t!("accounts.microsoft_code"), dim),
                        Span::styled(user_code, strong),
                    ]),
                    Line::raw(""),
                    Line::styled(t!("accounts.microsoft_waiting"), dim),
                ];
                frame.render_widget(Paragraph::new(lines), area);
                true
            }
        }
    }

    fn start_authorizer(&mut self, ctx: &Services, kind: AuthorizerKind) {
        self.error = None;
        match kind {
            AuthorizerKind::Offline => {
                self.mode = Mode::AddOffline(TextEdit::new(String::new()));
            }
            AuthorizerKind::Microsoft => {
                let Some(client_id) = microsoft_client_id().map(str::to_owned) else {
                    self.error = Some(t!("accounts.microsoft_not_configured").into_owned());
                    return;
                };
                if ctx.sign_in_microsoft(client_id) {
                    self.mode = Mode::Authorize;
                } else {
                    self.error = Some(t!("accounts.busy").into_owned());
                }
            }
        }
    }

    fn render_accounts(
        &mut self,
        state: &AccountState,
        frame: &mut Frame,
        area: Rect,
        theme: Theme,
    ) {
        if state.accounts.is_empty() {
            render_message(frame, area, t!("accounts.empty").as_ref());
            return;
        }

        let items = state
            .accounts
            .iter()
            .map(|account| {
                let selected = state.selected == Some(account.id);
                let marker = if selected { "*" } else { " " };
                let line = Line::from(vec![
                    Span::styled(
                        format!("{marker} {}", account.name),
                        if selected {
                            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)
                        } else {
                            Style::new()
                        },
                    ),
                    Span::styled(
                        format!("  ({})", account.authorizer),
                        Style::new().fg(theme.text_dim),
                    ),
                ]);
                ListItem::new(line)
            })
            .collect::<Vec<ListItem>>();

        let list = List::new(items).highlight_style(theme.selection_style());
        frame.render_stateful_widget(list, area, &mut self.list);
    }

    fn render_footer(&self, frame: &mut Frame, area: Rect, theme: Theme) {
        let line = match &self.mode {
            Mode::List => match &self.error {
                Some(error) => Line::styled(error.clone(), Style::new().fg(theme.danger)),
                None => Line::styled(t!("accounts.note"), Style::new().fg(theme.text_dim)),
            },
            Mode::AddOffline(edit) => Line::from(vec![
                Span::styled(t!("accounts.username"), Style::new().fg(theme.accent)),
                Span::raw(edit.cursor_display()),
            ]),
            Mode::Authorize => Line::styled(
                t!("accounts.microsoft_waiting"),
                Style::new().fg(theme.text_dim),
            ),
        };

        frame.render_widget(Paragraph::new(line), area);
    }

    fn on_list_key(&mut self, ctx: &Services, key: KeyEvent) -> ContentEvent {
        match key.code {
            KeyCode::Down => {
                self.list.select_next();
                ContentEvent::Handled
            }
            KeyCode::Up => {
                self.list.select_previous();
                ContentEvent::Handled
            }
            KeyCode::Enter => match self.selected_account(ctx) {
                Some(account_id) => {
                    ctx.select_account(account_id);
                    ContentEvent::Handled
                }
                None => ContentEvent::Handled,
            },
            KeyCode::Char('a') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.error = None;
                let options = vec![
                    (
                        t!("accounts.microsoft").into_owned(),
                        AuthorizerKind::Microsoft,
                    ),
                    (t!("accounts.offline").into_owned(), AuthorizerKind::Offline),
                ];
                self.authorizer =
                    Some(Dropdown::open(t!("accounts.authorizer_picker"), options, 0));
                ContentEvent::Handled
            }
            KeyCode::Char('d') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                if let Some(account_id) = self.selected_account(ctx) {
                    ctx.remove_account(account_id);
                }
                ContentEvent::Handled
            }
            KeyCode::Char('r') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                ctx.load_accounts();
                ContentEvent::Handled
            }
            _ => ContentEvent::Ignored,
        }
    }

    fn selected_account(&self, ctx: &Services) -> Option<AccountId> {
        let snapshot = ctx.account_state().snapshot();
        let state = snapshot.view().data?;
        let selected = self.list.selected()?;
        state.accounts.get(selected).map(|account| account.id)
    }
}
