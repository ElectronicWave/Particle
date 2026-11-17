mod detail;
mod view;

use crate::t;
use ratail::KeyHint;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::Rect,
};

use crate::pages::content::{Content, ContentEvent};
use crate::services::Services;
use crate::widgets::list::ListSelection;

fn download_hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("↑/↓", t!("common.browse")),
        KeyHint::new("ENTER", t!("common.dismiss")),
        KeyHint::new("ESC", t!("common.return")),
    ]
}

#[derive(Debug, Default)]
pub struct DownloadsContent {
    list: ListSelection,
}

impl Content for DownloadsContent {
    fn render(&mut self, ctx: &Services, frame: &mut Frame, area: Rect) {
        view::render(self, ctx, frame, area);
    }

    fn on_key(&mut self, ctx: &Services, key: KeyEvent) -> ContentEvent {
        match key.code {
            KeyCode::Down | KeyCode::Up | KeyCode::Home | KeyCode::End => {
                self.list.handle_key(key.code);
                ContentEvent::Handled
            }
            KeyCode::Enter => {
                let sessions = ctx.install().sessions();
                if let Some(session) = self.list.selected().and_then(|index| sessions.get(index))
                    && session.phase.can_dismiss()
                {
                    ctx.install().dismiss(session.id);
                    self.list.select(Some(0));
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
