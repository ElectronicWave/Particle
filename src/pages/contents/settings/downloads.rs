use std::borrow::Cow;

use ratail::{InputEvent, KeyHint, MouseKind};
use ratatui::{Frame, crossterm::event::KeyCode, text::Line};

use crate::services::Services;
use crate::t;
use crate::theme::Theme;
use crate::widgets::list::{ListAction, ListSelection};

use super::pane::{self, PaneAreas, PaneEvent, RowHint, SettingsPane};

const MIN_PARALLELISM: usize = 1;
const MAX_PARALLELISM: usize = 64;

#[derive(Debug, Default)]
pub(super) struct DownloadsPane {
    selection: ListSelection,
    error: Option<String>,
}

impl SettingsPane for DownloadsPane {
    fn title(&self) -> Cow<'static, str> {
        t!("settings.tab_downloads")
    }

    fn render(&mut self, ctx: &Services, frame: &mut Frame, areas: PaneAreas, theme: Theme) {
        let config = ctx.config().snapshot();
        let rows = vec![pane::row(
            0,
            0,
            &t!("settings.parallel_downloads"),
            config.download_parallelism.to_string(),
            theme,
        )];
        pane::render_rows(frame, areas.body, rows, &mut self.selection, theme);
    }

    fn on_input(&mut self, ctx: &Services, input: InputEvent) -> PaneEvent {
        if let InputEvent::Mouse(mouse) = input {
            match mouse.kind {
                MouseKind::ScrollUp => {
                    self.adjust(ctx, true);
                    return PaneEvent::Handled;
                }
                MouseKind::ScrollDown => {
                    self.adjust(ctx, false);
                    return PaneEvent::Handled;
                }
                _ => {}
            }
        }

        match self.selection.handle_input(&input) {
            ListAction::Activate(_) => {
                self.adjust(ctx, true);
                return PaneEvent::Handled;
            }
            ListAction::Changed => return PaneEvent::Handled,
            ListAction::Ignored => {}
        }

        let Some(key) = input.key() else {
            return PaneEvent::Ignored;
        };
        match key.code {
            KeyCode::Right => {
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
        let mut config = ctx.config().snapshot();
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
