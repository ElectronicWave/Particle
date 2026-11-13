use ratail::KeyHint;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::Rect,
};
use tui_scrollview::ScrollViewState;

use crate::app::services::Services;
use crate::pages::content::{Content, ContentEvent, container};

const HOME_HINTS: &[KeyHint] = &[
    KeyHint::from_static("↑/↓", "Scroll"),
    KeyHint::from_static("ESC", "Return"),
];

#[derive(Debug, Default)]
pub struct HomeContent {
    scroll: ScrollViewState,
}

impl Content for HomeContent {
    fn render(&mut self, _ctx: &Services, frame: &mut Frame, area: Rect) {
        let _inner = container(frame, area);
    }

    fn on_key(&mut self, _ctx: &Services, key: KeyEvent) -> ContentEvent {
        match key.code {
            KeyCode::Down => {
                self.scroll.scroll_down();
                ContentEvent::Handled
            }
            KeyCode::Up => {
                self.scroll.scroll_up();
                ContentEvent::Handled
            }
            _ => ContentEvent::Ignored,
        }
    }

    fn keybindings(&self) -> Vec<KeyHint> {
        HOME_HINTS.to_vec()
    }
}
