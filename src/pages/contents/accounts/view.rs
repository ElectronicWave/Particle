use ratail::SnapshotStatus;
use ratatui::{
    Frame,
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, Paragraph},
};

use super::{AccountsContent, Mode};
use crate::pages::content::{render_loading, render_message};
use crate::services::{AccountState, AuthorizationState, Services};
use crate::t;
use crate::theme::Theme;
use crate::widgets::popup::Popup;

pub(super) fn render(content: &mut AccountsContent, ctx: &Services, frame: &mut Frame, area: Rect) {
    let theme = ctx.ui().theme();
    let [list_area, footer_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(2)]).areas(area);

    if matches!(content.mode, Mode::Authorize)
        && ctx.accounts().authorization() == AuthorizationState::Idle
    {
        content.mode = Mode::List;
    }

    if render_authorization(ctx, frame, list_area, theme) {
        render_footer(content, frame, footer_area, theme);
        return;
    }

    if let Mode::Yggdrasil(form) = &content.mode {
        form.render(frame, list_area, theme, content.error.as_deref());
        render_footer(content, frame, footer_area, theme);
        return;
    }

    let snapshot = ctx.accounts().state().snapshot();
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
            render_accounts(content, state, frame, accounts_area, theme);
        }
        (Some(state), None) => render_accounts(content, state, frame, list_area, theme),
        (None, Some(error)) => {
            render_message(frame, list_area, t!("common.error", error = error).as_ref())
        }
        (None, None) if view.status == SnapshotStatus::Cancelled => {
            render_message(frame, list_area, t!("common.cancelled").as_ref())
        }
        (None, None) => render_loading(ctx, frame, list_area, t!("accounts.loading").as_ref()),
    }

    render_footer(content, frame, footer_area, theme);

    if let Some(popup) = &content.authorizer {
        let anchor = Rect {
            x: area.x,
            y: footer_area.y,
            width: 1,
            height: 1,
        };
        popup.render(frame, area, anchor, theme);
    }
}

fn render_authorization(ctx: &Services, frame: &mut Frame, area: Rect, theme: Theme) -> bool {
    match ctx.accounts().authorization() {
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

fn render_accounts(
    content: &mut AccountsContent,
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
    frame.render_stateful_widget(list, area, &mut content.list);
}

fn render_footer(content: &AccountsContent, frame: &mut Frame, area: Rect, theme: Theme) {
    let line = match &content.mode {
        Mode::List => match &content.error {
            Some(error) => Line::styled(error.clone(), Style::new().fg(theme.danger)),
            None => Line::styled(t!("accounts.note"), Style::new().fg(theme.text_dim)),
        },
        Mode::AddOffline(edit) => Line::from(vec![
            Span::styled(t!("accounts.username"), Style::new().fg(theme.accent)),
            Span::raw(edit.cursor_display()),
        ]),
        Mode::Yggdrasil(_) => Line::styled(
            t!("accounts.yggdrasil_footer"),
            Style::new().fg(theme.text_dim),
        ),
        Mode::Authorize => Line::styled(
            t!("accounts.microsoft_waiting"),
            Style::new().fg(theme.text_dim),
        ),
    };

    frame.render_widget(Paragraph::new(line), area);
}
