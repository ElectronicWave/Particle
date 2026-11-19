use std::borrow::Cow;

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
use crate::widgets::cape::CapePreview;
use crate::widgets::popup::Popup;
use crate::widgets::skin::SkinPreview;
use crate::widgets::text_edit::TextEdit;

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

    if content.skins.is_some() {
        render_skins(content, ctx, frame, list_area, theme);
        render_footer(content, frame, footer_area, theme);
        if let Some(skin_mode) = &content.skins
            && let Some(picker) = &skin_mode.picker
        {
            let anchor = Rect {
                x: area.x,
                y: footer_area.y,
                width: 1,
                height: 1,
            };
            picker.render(frame, area, anchor, theme);
        }
        return;
    }

    if content.capes.is_some() {
        render_capes(content, ctx, frame, list_area, theme);
        render_footer(content, frame, footer_area, theme);
        if let Some(cape_mode) = &content.capes
            && let Some(picker) = &cape_mode.picker
        {
            let anchor = Rect {
                x: area.x,
                y: footer_area.y,
                width: 1,
                height: 1,
            };
            picker.render(frame, area, anchor, theme);
        }
        return;
    }

    let snapshot = ctx.accounts().state().snapshot();
    let view = snapshot.view();
    match (view.data, view.error) {
        (Some(state), Some(error)) => {
            let [error_area, body_area] =
                Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(list_area);
            render_message(
                frame,
                error_area,
                t!("common.error", error = error).as_ref(),
            );
            let (accounts_area, preview_area) = account_body_layout(content, ctx, state, body_area);
            render_accounts(content, state, frame, accounts_area, theme);
            render_account_preview(content, ctx, state, frame, preview_area, theme);
        }
        (Some(state), None) => {
            let (accounts_area, preview_area) = account_body_layout(content, ctx, state, list_area);
            render_accounts(content, state, frame, accounts_area, theme);
            render_account_preview(content, ctx, state, frame, preview_area, theme);
        }
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

fn account_body_layout(
    content: &mut AccountsContent,
    ctx: &Services,
    state: &AccountState,
    area: Rect,
) -> (Rect, Rect) {
    let Some(account) = content
        .list
        .selected()
        .and_then(|index| state.accounts.get(index))
    else {
        return (area, Rect::new(area.x, area.y, 0, 0));
    };
    let preview = ctx.skins().preview_snapshot(account.id);
    if preview.is_none() {
        ctx.skins().preview(account.id);
    }
    let has_skin = preview.is_some_and(|snapshot| {
        snapshot
            .view()
            .data
            .is_some_and(|preview| preview.image.is_some())
    });
    if !has_skin || area.width < 46 {
        return (area, Rect::new(area.x, area.y, 0, 0));
    }
    let width = (area.width / 3).clamp(18, 30);
    let [list, preview] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(width)]).areas(area);
    (list, preview)
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
    content.list.set_len(state.accounts.len());
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
                    format!("  ({})", account.authorizer_label()),
                    Style::new().fg(theme.text_dim),
                ),
            ]);
            ListItem::new(line)
        })
        .collect::<Vec<ListItem>>();

    let list = List::new(items).highlight_style(theme.selection_style());
    frame.render_stateful_widget(list, area, content.list.state_mut());
}

fn render_account_preview(
    content: &mut AccountsContent,
    ctx: &Services,
    state: &AccountState,
    frame: &mut Frame,
    area: Rect,
    theme: Theme,
) {
    if area.width == 0 || area.height == 0 {
        return;
    }
    let Some(account) = content
        .list
        .selected()
        .and_then(|index| state.accounts.get(index))
    else {
        return;
    };
    let account_id = account.id;
    let Some(snapshot) = ctx.skins().preview_snapshot(account_id) else {
        return;
    };
    let view = snapshot.view();
    let [image_area, footer_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(2)]).areas(area);

    match view.data {
        Some(preview) => match &preview.image {
            Some(image) => frame.render_widget(image, image_area),
            None => return,
        },
        None if view.status == SnapshotStatus::Idle || view.status == SnapshotStatus::Loading => {
            render_preview_notice(frame, image_area, t!("common.loading").as_ref(), theme)
        }
        None => {
            let message = match view.error {
                Some(error) => t!("common.error", error = error.as_ref()).into_owned(),
                None => t!("common.cancelled").into_owned(),
            };
            render_preview_notice(frame, image_area, &message, theme);
        }
    }

    let detail = view
        .data
        .filter(|preview| preview.image.is_some())
        .map(|preview| {
            let mut parts = Vec::new();
            if let Some(model) = preview.model {
                parts.push(skin_model_label(model).into_owned());
            }
            parts.push(source_label(preview.source).into_owned());
            parts.join(" · ")
        })
        .unwrap_or_default();
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(account.authorizer_label(), Style::new().fg(theme.text_dim)),
            Line::styled(detail, Style::new().fg(theme.text_dim)),
        ]),
        footer_area,
    );
}

fn render_preview_notice(frame: &mut Frame, area: Rect, message: &str, theme: Theme) {
    frame.render_widget(
        Paragraph::new(message)
            .centered()
            .style(Style::new().fg(theme.text_dim)),
        area,
    );
}

fn source_label(source: crate::services::PreviewSource) -> Cow<'static, str> {
    match source {
        crate::services::PreviewSource::Profile => t!("accounts.preview_source_profile"),
        crate::services::PreviewSource::Local => t!("accounts.preview_source_local"),
    }
}

fn render_footer(content: &AccountsContent, frame: &mut Frame, area: Rect, theme: Theme) {
    if let Some(skin_mode) = &content.skins {
        if let Some(import) = &skin_mode.import {
            render_name_prompt(
                frame,
                area,
                theme,
                t!("accounts.skin_name").as_ref(),
                &import.edit,
                import.error.as_deref(),
                t!("accounts.skin_name_help").as_ref(),
            );
            return;
        }
        if let Some(rename) = &skin_mode.rename {
            render_name_prompt(
                frame,
                area,
                theme,
                t!("accounts.skin_name").as_ref(),
                &rename.edit,
                rename.error.as_deref(),
                t!("accounts.rename_help").as_ref(),
            );
            return;
        }
    }

    if let Some(cape_mode) = &content.capes {
        if let Some(import) = &cape_mode.import {
            render_name_prompt(
                frame,
                area,
                theme,
                t!("accounts.cape_name").as_ref(),
                &import.edit,
                import.error.as_deref(),
                t!("accounts.cape_name_help").as_ref(),
            );
            return;
        }
        if let Some(rename) = &cape_mode.rename {
            render_name_prompt(
                frame,
                area,
                theme,
                t!("accounts.cape_name").as_ref(),
                &rename.edit,
                rename.error.as_deref(),
                t!("accounts.rename_help").as_ref(),
            );
            return;
        }
    }

    let line = match &content.mode {
        Mode::List => match &content.error {
            Some(error) => Line::styled(error.clone(), Style::new().fg(theme.danger)),
            None => Line::default(),
        },
        Mode::AddOffline(edit) => Line::from(vec![
            Span::styled(t!("accounts.username"), Style::new().fg(theme.accent)),
            Span::raw(edit.cursor_display()),
        ]),
        Mode::AddLocalYggdrasil(edit) => Line::from(vec![
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

    let paragraph = match &content.mode {
        Mode::AddOffline(_) => Paragraph::new(vec![
            line,
            Line::styled(t!("accounts.offline_help"), Style::new().fg(theme.text_dim)),
        ]),
        Mode::AddLocalYggdrasil(_) => Paragraph::new(vec![
            line,
            Line::styled(
                t!("accounts.local_yggdrasil_help"),
                Style::new().fg(theme.text_dim),
            ),
        ]),
        _ => Paragraph::new(line),
    };
    frame.render_widget(paragraph, area);
}

fn render_name_prompt(
    frame: &mut Frame,
    area: Rect,
    theme: Theme,
    label: &str,
    edit: &TextEdit,
    error: Option<&str>,
    help: &str,
) {
    let help_style = if error.is_some() {
        Style::new().fg(theme.danger)
    } else {
        Style::new().fg(theme.text_dim)
    };
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(format!("{label}: "), Style::new().fg(theme.accent)),
                Span::raw(edit.cursor_display()),
            ]),
            Line::styled(error.unwrap_or(help).to_owned(), help_style),
        ]),
        area,
    );
}

fn render_skins(
    content: &mut AccountsContent,
    ctx: &Services,
    frame: &mut Frame,
    area: Rect,
    theme: Theme,
) {
    let Some(skin_mode) = &mut content.skins else {
        return;
    };
    let [body_area, status_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(2)]).areas(area);

    let snapshot = ctx.skins().snapshot();
    let view = snapshot.view();
    let skins_empty = view
        .data
        .filter(|state| state.account_id == skin_mode.account_id)
        .is_some_and(|state| state.entries.is_empty());
    let (list_area, preview_area) = if skins_empty {
        (body_area, Rect::new(body_area.x, body_area.y, 0, 0))
    } else {
        texture_body_layout(body_area)
    };
    match (view.data, view.error) {
        (Some(state), Some(error)) if state.account_id == skin_mode.account_id => {
            let [error_area, list_area] =
                Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(list_area);
            render_message(
                frame,
                error_area,
                t!("common.error", error = error).as_ref(),
            );
            render_skin_list(skin_mode, state, frame, list_area, theme);
        }
        (Some(state), None) if state.account_id == skin_mode.account_id => {
            render_skin_list(skin_mode, state, frame, list_area, theme);
        }
        (None, Some(error)) => {
            render_message(frame, list_area, t!("common.error", error = error).as_ref())
        }
        _ => render_loading(ctx, frame, list_area, t!("accounts.skins_loading").as_ref()),
    }

    if preview_area.width > 0
        && let Some(state) = view
            .data
            .filter(|state| state.account_id == skin_mode.account_id)
    {
        render_skin_preview(skin_mode, state, frame, preview_area, theme);
    }

    let model = skin_model_label(skin_mode.model);
    let selected = snapshot
        .view()
        .data
        .filter(|state| state.account_id == skin_mode.account_id)
        .and_then(|state| state.selected.as_ref())
        .map(|selected| {
            (
                selected.asset.name.clone(),
                skin_model_label(selected.model),
            )
        });
    let skin = selected
        .as_ref()
        .map(|(name, model)| {
            t!(
                "accounts.skin_selected_model",
                skin = name,
                model = model.as_ref()
            )
        })
        .unwrap_or_else(|| {
            t!(
                "accounts.skin_selected",
                skin = t!("accounts.skin_none").as_ref()
            )
        });
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(
                t!("accounts.skin_current_model", model = model.as_ref()),
                Style::new().fg(theme.text_dim),
            ),
            Line::styled(skin, Style::new().fg(theme.text_dim)),
        ]),
        status_area,
    );
}

fn texture_body_layout(area: Rect) -> (Rect, Rect) {
    if area.width < 46 {
        return (area, Rect::new(area.x, area.y, 0, 0));
    }
    let width = (area.width / 3).clamp(18, 36);
    let [list, preview] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Length(width)]).areas(area);
    (list, preview)
}

fn render_skin_preview(
    skin_mode: &mut super::SkinMode,
    state: &crate::services::SkinState,
    frame: &mut Frame,
    area: Rect,
    theme: Theme,
) {
    let Some(entry) = skin_mode
        .list
        .selected()
        .and_then(|index| state.entries.get(index))
    else {
        return;
    };
    let model = state
        .selected
        .as_ref()
        .filter(|selected| selected.asset.id == entry.asset.id)
        .map(|selected| selected.model)
        .unwrap_or(entry.asset.default_model);
    let cached = skin_mode.preview.as_ref().is_some_and(|slot| {
        slot.path == entry.path && slot.model == model && slot.view == skin_mode.view
    });
    if !cached {
        let image = SkinPreview::from_path(&entry.path, model, skin_mode.view)
            .map_err(|error| format!("{error:#}"));
        skin_mode.preview = Some(super::SkinPreviewSlot {
            path: entry.path.clone(),
            model,
            view: skin_mode.view,
            image,
        });
    }
    let Some(slot) = &skin_mode.preview else {
        return;
    };
    match &slot.image {
        Ok(preview) => frame.render_widget(preview, area),
        Err(error) => frame.render_widget(
            Paragraph::new(t!("accounts.skin_preview_failed", error = error).into_owned())
                .style(Style::new().fg(theme.danger)),
            area,
        ),
    }
}

fn render_skin_list(
    skin_mode: &mut super::SkinMode,
    state: &crate::services::SkinState,
    frame: &mut Frame,
    area: Rect,
    theme: Theme,
) {
    skin_mode.list.set_len(state.entries.len());
    if state.entries.is_empty() {
        let [_, message_area, _] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .areas(area);
        render_message(frame, message_area, t!("accounts.skins_empty").as_ref());
        return;
    }

    let items = state
        .entries
        .iter()
        .map(|entry| {
            let selection = state
                .selected
                .as_ref()
                .filter(|selected| selected.asset.id == entry.asset.id);
            let marker = if selection.is_some() { "*" } else { " " };
            let model = selection
                .map(|selected| selected.model)
                .unwrap_or(entry.asset.default_model);
            Line::from(vec![
                Span::styled(format!("{marker} {}", entry.asset.name), Style::new()),
                Span::styled(
                    format!("  ({})", skin_model_label(model).as_ref()),
                    Style::new().fg(theme.text_dim),
                ),
            ])
        })
        .collect::<Vec<Line>>();
    let list = List::new(items).highlight_style(theme.selection_style());
    frame.render_stateful_widget(list, area, skin_mode.list.state_mut());
}

fn render_capes(
    content: &mut AccountsContent,
    ctx: &Services,
    frame: &mut Frame,
    area: Rect,
    theme: Theme,
) {
    let Some(cape_mode) = &mut content.capes else {
        return;
    };
    let [body_area, status_area] =
        Layout::vertical([Constraint::Fill(1), Constraint::Length(2)]).areas(area);

    let snapshot = ctx.capes().snapshot();
    let view = snapshot.view();
    let capes_empty = view
        .data
        .filter(|state| state.account_id == cape_mode.account_id)
        .is_some_and(|state| state.entries.is_empty());
    let (list_area, preview_area) = if capes_empty {
        (body_area, Rect::new(body_area.x, body_area.y, 0, 0))
    } else {
        texture_body_layout(body_area)
    };
    match (view.data, view.error) {
        (Some(state), Some(error)) if state.account_id == cape_mode.account_id => {
            let [error_area, list_area] =
                Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(list_area);
            render_message(
                frame,
                error_area,
                t!("common.error", error = error).as_ref(),
            );
            render_cape_list(cape_mode, state, frame, list_area, theme);
        }
        (Some(state), None) if state.account_id == cape_mode.account_id => {
            render_cape_list(cape_mode, state, frame, list_area, theme);
        }
        (None, Some(error)) => {
            render_message(frame, list_area, t!("common.error", error = error).as_ref())
        }
        _ => render_loading(ctx, frame, list_area, t!("accounts.capes_loading").as_ref()),
    }

    if preview_area.width > 0
        && let Some(state) = view
            .data
            .filter(|state| state.account_id == cape_mode.account_id)
    {
        render_cape_preview(cape_mode, state, frame, preview_area, theme);
    }

    let selected = snapshot
        .view()
        .data
        .filter(|state| state.account_id == cape_mode.account_id)
        .and_then(|state| state.selected.as_ref())
        .map(|selected| selected.asset.name.clone());
    let cape = selected
        .map(|name| t!("accounts.cape_selected", cape = name))
        .unwrap_or_else(|| {
            t!(
                "accounts.cape_selected",
                cape = t!("accounts.cape_none").as_ref()
            )
        });
    frame.render_widget(
        Paragraph::new(vec![
            Line::styled(cape, Style::new().fg(theme.text_dim)),
            Line::styled(
                t!("accounts.cape_scope_help"),
                Style::new().fg(theme.text_dim),
            ),
        ]),
        status_area,
    );
}

fn render_cape_preview(
    cape_mode: &mut super::CapeMode,
    state: &crate::services::CapeState,
    frame: &mut Frame,
    area: Rect,
    theme: Theme,
) {
    let Some(entry) = cape_mode
        .list
        .selected()
        .and_then(|index| state.entries.get(index))
    else {
        return;
    };
    let cached = cape_mode
        .preview
        .as_ref()
        .is_some_and(|slot| slot.path == entry.path);
    if !cached {
        let image = CapePreview::from_path(&entry.path).map_err(|error| format!("{error:#}"));
        cape_mode.preview = Some(super::CapePreviewSlot {
            path: entry.path.clone(),
            image,
        });
    }
    let Some(slot) = &cape_mode.preview else {
        return;
    };
    match &slot.image {
        Ok(preview) => frame.render_widget(preview, area),
        Err(error) => frame.render_widget(
            Paragraph::new(t!("accounts.cape_preview_failed", error = error).into_owned())
                .style(Style::new().fg(theme.danger)),
            area,
        ),
    }
}

fn render_cape_list(
    cape_mode: &mut super::CapeMode,
    state: &crate::services::CapeState,
    frame: &mut Frame,
    area: Rect,
    theme: Theme,
) {
    cape_mode.list.set_len(state.entries.len());
    if state.entries.is_empty() {
        let [_, message_area, _] = Layout::vertical([
            Constraint::Fill(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .areas(area);
        render_message(frame, message_area, t!("accounts.capes_empty").as_ref());
        return;
    }

    let items = state
        .entries
        .iter()
        .map(|entry| {
            let selection = state
                .selected
                .as_ref()
                .filter(|selected| selected.asset.id == entry.asset.id);
            let marker = if selection.is_some() { "*" } else { " " };
            Line::styled(format!("{marker} {}", entry.asset.name), Style::new())
        })
        .collect::<Vec<Line>>();
    let list = List::new(items).highlight_style(theme.selection_style());
    frame.render_stateful_widget(list, area, cape_mode.list.state_mut());
}

pub(super) fn skin_model_label(
    model: elemental::core::auth::texture::SkinModel,
) -> Cow<'static, str> {
    match model {
        elemental::core::auth::texture::SkinModel::Classic => {
            t!("accounts.skin_classic")
        }
        elemental::core::auth::texture::SkinModel::Slim => t!("accounts.skin_slim"),
    }
}
