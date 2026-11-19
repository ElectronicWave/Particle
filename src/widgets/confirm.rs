use ratail::KeyHint;
use ratatui::{Frame, crossterm::event::KeyEvent, layout::Rect};

use crate::{
    t,
    theme::Theme,
    widgets::{
        dropdown::Dropdown,
        popup::{Popup, PopupEvent},
    },
};

/// A two-option dropdown that only yields its value after explicit confirmation.
#[derive(Debug)]
pub struct Confirmation<T> {
    prompt: Dropdown<bool>,
    value: Option<T>,
}

impl<T> Confirmation<T> {
    pub fn open(title: impl Into<String>, value: T) -> Self {
        Self {
            prompt: Dropdown::open(
                title,
                vec![
                    (t!("common.cancel").into_owned(), false),
                    (t!("common.remove").into_owned(), true),
                ],
                0,
            ),
            value: Some(value),
        }
    }
}

impl<T> Popup for Confirmation<T> {
    type Value = T;

    fn handle_key(&mut self, key: KeyEvent) -> PopupEvent<T> {
        match self.prompt.handle_key(key) {
            PopupEvent::Completed(true) => {
                PopupEvent::Completed(self.value.take().expect("confirmation completed twice"))
            }
            PopupEvent::Completed(false) => PopupEvent::Cancelled,
            PopupEvent::Cancelled => PopupEvent::Cancelled,
            PopupEvent::Handled => PopupEvent::Handled,
            PopupEvent::Ignored => PopupEvent::Ignored,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, anchor: Rect, theme: Theme) {
        self.prompt.render(frame, area, anchor, theme);
    }

    fn hints(&self) -> Vec<KeyHint> {
        self.prompt.hints()
    }
}
