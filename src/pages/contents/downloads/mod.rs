mod detail;
mod view;

use crate::t;
use ratail::{KeyHint, Page, PageEvent};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::Rect,
};

use crate::pages::route::Route;
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

impl Page<Services> for DownloadsContent {
    fn render(&mut self, ctx: &Services, _route: &Route, frame: &mut Frame, area: Rect) {
        view::render(self, ctx, frame, area);
    }

    fn on_key(&mut self, ctx: &Services, _route: &Route, key: KeyEvent) -> PageEvent<Route> {
        match key.code {
            KeyCode::Down | KeyCode::Up | KeyCode::Home | KeyCode::End => {
                self.list.handle_key(key.code);
                PageEvent::Handled
            }
            KeyCode::Enter => {
                let sessions = ctx.install().sessions();
                if let Some(session) = self.list.selected().and_then(|index| sessions.get(index))
                    && session.phase.can_dismiss()
                {
                    ctx.install().dismiss(session.id);
                    self.list.select(Some(0));
                }
                PageEvent::Handled
            }
            _ => PageEvent::Ignored,
        }
    }

    fn keybindings(&self, _route: &Route) -> Vec<KeyHint> {
        download_hints()
    }
}
