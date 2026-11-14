use std::borrow::Cow;

use ratail::KeyHint;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    text::Line,
};

use crate::app::services::Services;
use crate::t;
use crate::theme::Theme;

use super::pane::{self, PaneAreas, PaneEvent, RowHint, SettingsPane};

const MIN_PARALLELISM: usize = 1;
const MAX_PARALLELISM: usize = 64;

#[derive(Debug, Default)]
pub(super) struct DownloadsPane {
    error: Option<String>,
}

impl SettingsPane for DownloadsPane {
    fn title(&self) -> Cow<'static, str> {
        t!("settings.tab_downloads")
    }

    fn render(&mut self, ctx: &Services, frame: &mut Frame, areas: PaneAreas, theme: Theme) {
        let config = ctx.config();
        let rows = vec![pane::row(
            0,
            0,
            &t!("settings.parallel_downloads"),
            config.download_parallelism.to_string(),
            theme,
        )];
        pane::render_rows(frame, areas.body, rows, 0, theme);
    }

    fn on_key(&mut self, ctx: &Services, key: KeyEvent) -> PaneEvent {
        match key.code {
            KeyCode::Enter | KeyCode::Right => {
                self.adjust(ctx, true);
                PaneEvent::Handled
            }
            KeyCode::Left => {
                self.adjust(ctx, false);
                PaneEvent::Handled
            }
            _ => PaneEvent::Ignored,
        }
    }

    fn hints(&self) -> Vec<KeyHint> {
        pane::row_hints(RowHint::Edit)
    }

    fn is_modal(&self) -> bool {
        false
    }

    fn status(&self, _ctx: &Services, theme: Theme) -> Option<Line<'static>> {
        pane::error_status(self.error.as_ref(), theme)
    }
}

impl DownloadsPane {
    fn adjust(&mut self, ctx: &Services, forward: bool) {
        let mut config = ctx.config();
        config.download_parallelism = if forward {
            config
                .download_parallelism
                .saturating_add(1)
                .min(MAX_PARALLELISM)
        } else {
            config
                .download_parallelism
                .saturating_sub(1)
                .max(MIN_PARALLELISM)
        };
        match ctx.update_config(config) {
            Ok(()) => self.error = None,
            Err(error) => self.error = Some(error),
        }
    }
}
