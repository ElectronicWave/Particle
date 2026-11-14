use ratail::KeyHint;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::Rect,
    style::Style,
    widgets::{Block, BorderType, Borders, Clear, List, ListState},
};
use unicode_width::UnicodeWidthStr;

use crate::t;
use crate::theme::Theme;
use crate::widgets::popup::{Popup, PopupEvent};

const MIN_WIDTH: u16 = 16;
const HORIZONTAL_PADDING: u16 = 2;
const HIGHLIGHT_WIDTH: u16 = 2;
const VERTICAL_PADDING: u16 = 2;
const HIGHLIGHT_SYMBOL: &str = "▸ ";

#[derive(Debug, Clone, Copy, Default)]
struct DropdownState {
    selected: usize,
}

/// A dropdown whose options carry values, so callers handle domain results directly.
#[derive(Debug)]
pub struct Dropdown<T> {
    title: String,
    options: Vec<(String, T)>,
    state: DropdownState,
}

impl<T> Dropdown<T> {
    pub fn open(title: impl Into<String>, options: Vec<(String, T)>, selected: usize) -> Self {
        Self {
            title: title.into(),
            options,
            state: DropdownState { selected },
        }
    }

    pub fn selected_value(&self) -> Option<&T> {
        if self.options.is_empty() {
            return None;
        }

        self.options
            .get(self.state.selected.min(self.options.len() - 1))
            .map(|(_, value)| value)
    }
}

impl<T> Popup for Dropdown<T> {
    type Value = T;

    fn handle_key(&mut self, key: KeyEvent) -> PopupEvent<T> {
        if key
            .modifiers
            .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
        {
            return PopupEvent::Ignored;
        }
        if self.options.is_empty() {
            return PopupEvent::Cancelled;
        }

        self.state.selected = self.state.selected.min(self.options.len() - 1);
        match key.code {
            KeyCode::Up => {
                self.state.selected = if self.state.selected == 0 {
                    self.options.len() - 1
                } else {
                    self.state.selected - 1
                };
                PopupEvent::Handled
            }
            KeyCode::Down => {
                self.state.selected = (self.state.selected + 1) % self.options.len();
                PopupEvent::Handled
            }
            KeyCode::Enter => PopupEvent::Completed(self.options.remove(self.state.selected).1),
            KeyCode::Esc => PopupEvent::Cancelled,
            _ => PopupEvent::Handled,
        }
    }

    fn render(&self, frame: &mut Frame, area: Rect, anchor: Rect, theme: Theme) {
        if self.options.is_empty() {
            return;
        }

        let popup = popup_area(area, anchor, &self.title, &self.options);
        frame.render_widget(Clear, popup);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(theme.border_focused))
            .title(self.title.clone());
        let inner = block.inner(popup);
        frame.render_widget(block, popup);
        if inner.is_empty() {
            return;
        }

        let list = List::new(self.options.iter().map(|(label, _)| label.as_str()))
            .style(Style::new().fg(theme.text))
            .highlight_style(theme.selection_style())
            .highlight_symbol(HIGHLIGHT_SYMBOL);

        let visible = inner.height as usize;
        let selected = self.state.selected.min(self.options.len() - 1);
        let offset = selected.saturating_add(1).saturating_sub(visible);
        let mut list_state = ListState::default()
            .with_selected(Some(selected))
            .with_offset(offset);
        frame.render_stateful_widget(list, inner, &mut list_state);
    }

    fn hints(&self) -> Vec<KeyHint> {
        vec![
            KeyHint::new("↑/↓", t!("common.select")),
            KeyHint::new("ENTER", t!("common.select")),
            KeyHint::new("ESC", t!("common.cancel")),
        ]
    }
}

fn popup_area<T>(area: Rect, anchor: Rect, title: &str, options: &[(String, T)]) -> Rect {
    let content_width = options
        .iter()
        .map(|(label, _)| UnicodeWidthStr::width(label.as_str()))
        .max()
        .unwrap_or_default()
        .max(UnicodeWidthStr::width(title)) as u16;
    let width = (content_width + HORIZONTAL_PADDING + HIGHLIGHT_WIDTH + 2)
        .max(MIN_WIDTH)
        .min(area.width);
    let height = (options.len() as u16 + VERTICAL_PADDING).min(area.height);

    let x = anchor.x.min(area.right().saturating_sub(width));
    let below = anchor.y.saturating_add(1);
    let y = if below.saturating_add(height) <= area.bottom() {
        below
    } else {
        anchor.y.saturating_sub(height).max(area.y)
    };

    Rect {
        x,
        y,
        width,
        height,
    }
}
