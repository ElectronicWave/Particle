mod detail;
mod view;

use crate::t;
use ratail::KeyHint;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::Rect,
    widgets::ListState,
};

use crate::pages::content::{Content, ContentEvent};
use crate::services::{InstallSession, InstallSessionId, Services};

fn download_hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("↑/↓", t!("common.browse")),
        KeyHint::new("ENTER", t!("common.dismiss")),
        KeyHint::new("ESC", t!("common.return")),
    ]
}

#[derive(Debug, Default)]
pub struct DownloadsContent {
    selected: Option<InstallSessionId>,
    list: ListState,
}

impl Content for DownloadsContent {
    fn render(&mut self, ctx: &Services, frame: &mut Frame, area: Rect) {
        view::render(self, ctx, frame, area);
    }

    fn on_key(&mut self, ctx: &Services, key: KeyEvent) -> ContentEvent {
        match key.code {
            KeyCode::Down => {
                self.move_selection(ctx, 1);
                ContentEvent::Handled
            }
            KeyCode::Up => {
                self.move_selection(ctx, -1);
                ContentEvent::Handled
            }
            KeyCode::Enter => {
                let sessions = ctx.install().sessions();
                self.ensure_selection(&sessions);
                let can_dismiss = self.selected.is_some_and(|id| {
                    sessions
                        .iter()
                        .any(|session| session.id == id && session.phase.can_dismiss())
                });
                if can_dismiss {
                    if let Some(id) = self.selected {
                        ctx.install().dismiss(id);
                        self.selected = None;
                    }
                }
                ContentEvent::Handled
            }
            _ => ContentEvent::Ignored,
        }
    }

    fn keybindings(&self) -> Vec<KeyHint> {
        download_hints()
    }
}

impl DownloadsContent {
    fn ensure_selection(&mut self, sessions: &[InstallSession]) {
        let selected_exists = self
            .selected
            .is_some_and(|id| sessions.iter().any(|session| session.id == id));
        if !selected_exists {
            self.selected = sessions.first().map(|session| session.id);
        }
    }

    fn move_selection(&mut self, ctx: &Services, direction: isize) {
        let sessions = ctx.install().sessions();
        self.ensure_selection(&sessions);
        let Some(current) = self
            .selected
            .and_then(|id| sessions.iter().position(|session| session.id == id))
        else {
            return;
        };

        let next = if direction < 0 {
            current.saturating_sub(1)
        } else {
            current
                .saturating_add(1)
                .min(sessions.len().saturating_sub(1))
        };
        self.selected = Some(sessions[next].id);
    }
}
