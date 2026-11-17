use anyhow::Error;
use elemental::core::auth::accounts::AccountId;
use ratail::{KeyHint, SnapshotStatus, Task};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Line,
    widgets::{Block, BorderType, Borders, Clear, Paragraph},
};

use crate::services::{AccountPreview, AccountSummary, PreviewSource, Services};
use crate::t;
use crate::theme::Theme;
use crate::widgets::popup::{Popup, PopupEvent};

use super::view::skin_model_label;

const POPUP_WIDTH: u16 = 20;
const POPUP_HEIGHT: u16 = 12;

pub(super) struct AccountPreviewPopup {
    account_id: AccountId,
    name: String,
    authorizer: String,
    task: Task<AccountPreview, Error>,
}

impl std::fmt::Debug for AccountPreviewPopup {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AccountPreviewPopup")
            .field("name", &self.name)
            .field("authorizer", &self.authorizer)
            .finish_non_exhaustive()
    }
}

impl AccountPreviewPopup {
    pub(super) fn open(ctx: &Services, account: &AccountSummary) -> Self {
        Self {
            account_id: account.id,
            name: account.name.clone(),
            authorizer: account.authorizer_label(),
            task: ctx.skins().preview(account.id),
        }
    }

    pub(super) fn account_id(&self) -> AccountId {
        self.account_id
    }
}

impl Popup for AccountPreviewPopup {
    type Value = ();

    fn handle_key(&mut self, key: KeyEvent) -> PopupEvent<()> {
        match key.code {
            KeyCode::Esc => PopupEvent::Cancelled,
            KeyCode::Enter => PopupEvent::Completed(()),
            _ => PopupEvent::Handled,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, anchor: Rect, theme: Theme) {
        let popup = anchored(area, anchor);
        frame.render_widget(Clear, popup);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(theme.border_focused))
            .title(format!(" {} ", self.name));
        let inner = block.inner(popup);
        frame.render_widget(block, popup);
        if inner.is_empty() {
            return;
        }

        let [image_area, footer_area] =
            Layout::vertical([Constraint::Fill(1), Constraint::Length(2)]).areas(inner);
        let snapshot = self.task.snapshot();
        let view = snapshot.view();

        match view.data {
            Some(preview) => match &preview.image {
                Some(image) => frame.render_widget(image, image_area),
                None => render_notice(
                    frame,
                    image_area,
                    t!("accounts.preview_no_skin").as_ref(),
                    theme,
                ),
            },
            None if view.status == SnapshotStatus::Idle
                || view.status == SnapshotStatus::Loading =>
            {
                render_notice(frame, image_area, t!("common.loading").as_ref(), theme)
            }
            None => {
                let message = match view.error {
                    Some(error) => t!("common.error", error = error.as_ref()).into_owned(),
                    None => t!("common.cancelled").into_owned(),
                };
                render_notice(frame, image_area, &message, theme);
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
                Line::styled(self.authorizer.clone(), Style::new().fg(theme.text_dim)),
                Line::styled(detail, Style::new().fg(theme.text_dim)),
            ]),
            footer_area,
        );
    }

    fn hints(&self) -> Vec<KeyHint> {
        vec![
            KeyHint::new("ENTER", t!("accounts.preview_activate")),
            KeyHint::new("ESC", t!("common.return")),
        ]
    }
}

fn render_notice(frame: &mut Frame, area: Rect, message: &str, theme: Theme) {
    frame.render_widget(
        Paragraph::new(message)
            .centered()
            .style(Style::new().fg(theme.text_dim)),
        area,
    );
}

fn source_label(source: PreviewSource) -> std::borrow::Cow<'static, str> {
    match source {
        PreviewSource::Profile => t!("accounts.preview_source_profile"),
        PreviewSource::Local => t!("accounts.preview_source_local"),
    }
}

fn anchored(area: Rect, anchor: Rect) -> Rect {
    let width = POPUP_WIDTH.min(area.width);
    let height = POPUP_HEIGHT.min(area.height);
    let x = area.right().saturating_sub(width).max(area.x);
    let y = anchor
        .y
        .min(area.bottom().saturating_sub(height))
        .max(area.y);
    Rect {
        x,
        y,
        width,
        height,
    }
}
