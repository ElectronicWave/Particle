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

use crate::app::services::{AccountState, Services};
use crate::pages::content::{Content, ContentEvent, render_loading, render_message};
use crate::theme::Theme;
use crate::widgets::text_edit::{EditEvent, TextEdit};

fn list_hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("↑/↓", t!("common.browse")),
        KeyHint::new("ENTER", t!("common.select")),
        KeyHint::new("A", t!("common.add_offline")),
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
    Add(TextEdit),
}

#[derive(Debug)]
pub struct AccountsContent {
    mode: Mode,
    list: ListState,
}

impl Default for AccountsContent {
    fn default() -> Self {
        Self {
            mode: Mode::List,
            list: ListState::default().with_selected(Some(0)),
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
    }

    fn on_key(&mut self, ctx: &Services, key: KeyEvent) -> ContentEvent {
        let mut edit = match std::mem::replace(&mut self.mode, Mode::List) {
            Mode::List => return self.on_list_key(ctx, key),
            Mode::Add(edit) => edit,
        };

        match edit.handle_key(key) {
            EditEvent::Submit => {
                let name = edit.input().trim().to_owned();
                if !name.is_empty() {
                    ctx.sign_in_offline(name);
                }
            }
            EditEvent::Changed => self.mode = Mode::Add(edit),
            EditEvent::Unhandled => self.mode = Mode::Add(edit),
            EditEvent::Cancel => {}
        }
        ContentEvent::Handled
    }

    fn keybindings(&self) -> Vec<KeyHint> {
        match self.mode {
            Mode::List => list_hints(),
            Mode::Add(_) => add_hints(),
        }
    }
}

impl AccountsContent {
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
            Mode::List => Line::from(Span::styled(
                t!("accounts.note"),
                Style::new().fg(theme.text_dim),
            )),
            Mode::Add(edit) => Line::from(vec![
                Span::styled(t!("accounts.username"), Style::new().fg(theme.accent)),
                Span::raw(edit.cursor_display()),
            ]),
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
                self.mode = Mode::Add(TextEdit::new(String::new()));
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
