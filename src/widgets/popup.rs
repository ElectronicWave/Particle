use ratail::KeyHint;
use ratatui::{Frame, crossterm::event::KeyEvent, layout::Rect};

use crate::theme::Theme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PopupEvent<T> {
    Ignored,
    Handled,
    Cancelled,
    Completed(T),
}

/// A self-contained overlay that owns its state, rendering, hints and result.
pub trait Popup {
    type Value;

    fn handle_key(&mut self, key: KeyEvent) -> PopupEvent<Self::Value>;

    /// `anchor` is the screen rect the popup points at; centered popups ignore it.
    fn render(&self, frame: &mut Frame, area: Rect, anchor: Rect, theme: Theme);

    fn hints(&self) -> Vec<KeyHint>;
}
