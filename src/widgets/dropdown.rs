use ratail::{HitMap, InputEvent, KeyHint, Modal, ModalOutcome, MouseKind};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyModifiers},
    layout::Rect,
    style::Style,
    widgets::{Block, BorderType, Borders, Clear, List, ListState},
};
use unicode_width::UnicodeWidthStr;

use crate::services::Services;
use crate::t;
use crate::widgets::list::ListSelection;

const MIN_WIDTH: u16 = 16;
const HORIZONTAL_PADDING: u16 = 2;
const HIGHLIGHT_WIDTH: u16 = 2;
const VERTICAL_PADDING: u16 = 2;
const HIGHLIGHT_SYMBOL: &str = "▸ ";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DropdownRegion {
    Popup,
    List,
}

/// A dropdown whose options carry values, so callers handle domain results directly.
#[derive(Debug)]
pub struct Dropdown<T> {
    title: String,
    options: Vec<(String, T)>,
    selection: ListSelection,
    anchor: Option<Rect>,
    hits: HitMap<DropdownRegion>,
    offset: usize,
}

impl<T> Dropdown<T> {
    pub fn open(title: impl Into<String>, options: Vec<(String, T)>, selected: usize) -> Self {
        Self {
            title: title.into(),
            options,
            selection: ListSelection::wrap(selected),
            anchor: None,
            hits: HitMap::new(),
            offset: 0,
        }
    }

    /// Sets the screen rect the dropdown points at. Callers that position the
    /// dropdown themselves set this before rendering it through [`Modal`].
    pub fn set_anchor(&mut self, anchor: Rect) {
        self.anchor = Some(anchor);
    }

    pub fn selected_value(&self) -> Option<&T> {
        if self.options.is_empty() {
            return None;
        }

        self.options
            .get(self.selection.index().min(self.options.len() - 1))
            .map(|(_, value)| value)
    }
}

impl<T> Modal<Services> for Dropdown<T> {
    type Value = T;

    fn on_input(&mut self, _ctx: &Services, input: InputEvent) -> ModalOutcome<T> {
        if self.options.is_empty() {
            return ModalOutcome::Cancelled;
        }

        self.selection.set_len(self.options.len());
        match input {
            InputEvent::Key(key) => {
                if key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                {
                    return ModalOutcome::Ignored;
                }
                match key.code {
                    KeyCode::Up | KeyCode::Down | KeyCode::Home | KeyCode::End => {
                        self.selection.handle_key(key.code);
                        ModalOutcome::Handled
                    }
                    KeyCode::Enter => {
                        ModalOutcome::Completed(self.options.remove(self.selection.index()).1)
                    }
                    KeyCode::Esc => ModalOutcome::Cancelled,
                    _ => ModalOutcome::Handled,
                }
            }
            InputEvent::Mouse(mouse) => {
                let hit = self.hits.hit_mouse_copied(mouse);
                match (hit, mouse.kind) {
                    (None, MouseKind::Click | MouseKind::RightClick) => ModalOutcome::Cancelled,
                    (Some(_), MouseKind::ScrollDown) => {
                        self.selection.next();
                        ModalOutcome::Handled
                    }
                    (Some(_), MouseKind::ScrollUp) => {
                        self.selection.previous();
                        ModalOutcome::Handled
                    }
                    (Some(hit), MouseKind::Click | MouseKind::DoubleClick)
                        if hit.id == DropdownRegion::List =>
                    {
                        let index = self.offset + hit.row as usize;
                        if index < self.options.len() {
                            self.selection.select(Some(index));
                            ModalOutcome::Completed(self.options.remove(index).1)
                        } else {
                            ModalOutcome::Ignored
                        }
                    }
                    _ => ModalOutcome::Ignored,
                }
            }
            InputEvent::Paste(_) => ModalOutcome::Ignored,
        }
    }

    fn render(&mut self, ctx: &Services, frame: &mut Frame, area: Rect) {
        self.hits.clear();
        if self.options.is_empty() {
            return;
        }

        let theme = ctx.ui().theme();
        let anchor = self.anchor.unwrap_or_else(|| default_anchor(area));
        let popup = popup_area(area, anchor, &self.title, &self.options);
        frame.render_widget(Clear, popup);

        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::new().fg(theme.border_focused))
            .title(self.title.clone());
        let inner = block.inner(popup);
        frame.render_widget(block, popup);
        self.hits.register(popup, DropdownRegion::Popup);
        if inner.is_empty() {
            return;
        }
        self.hits.register(inner, DropdownRegion::List);

        let list = List::new(self.options.iter().map(|(label, _)| label.as_str()))
            .style(Style::new().fg(theme.text))
            .highlight_style(theme.selection_style())
            .highlight_symbol(HIGHLIGHT_SYMBOL);

        let visible = inner.height as usize;
        let selected = self.selection.index().min(self.options.len() - 1);
        let offset = selected.saturating_add(1).saturating_sub(visible);
        self.offset = offset;
        let mut list_state = ListState::default()
            .with_selected(Some(selected))
            .with_offset(offset);
        frame.render_stateful_widget(list, inner, &mut list_state);
    }

    fn keybindings(&self) -> Vec<KeyHint> {
        vec![
            KeyHint::new("↑/↓", t!("common.select")),
            KeyHint::new("ENTER", t!("common.select")),
            KeyHint::new("ESC", t!("common.cancel")),
        ]
    }
}

fn default_anchor(area: Rect) -> Rect {
    Rect {
        x: area.x,
        y: area.bottom().saturating_sub(2),
        width: 1,
        height: 1,
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
