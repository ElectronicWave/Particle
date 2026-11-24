use ratail::{InputEvent, MouseKind};
use ratatui::{
    crossterm::event::{KeyCode, KeyModifiers},
    layout::{Position, Rect},
    widgets::ListState,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListAction {
    Ignored,
    Changed,
    Activate(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Boundary {
    Clamp,
    Wrap { len: usize },
}

/// Selection state shared by list-backed screens.
///
/// Clamped lists rely on ratatui's deferred bounds handling. Wrapped lists
/// refresh their row count with [`ListSelection::set_len`] when data changes.
#[derive(Debug, Clone, Copy)]
pub struct ListSelection {
    state: ListState,
    boundary: Boundary,
    area: Rect,
    rendered_len: usize,
}

impl Default for ListSelection {
    fn default() -> Self {
        Self::clamp()
    }
}

impl ListSelection {
    pub fn clamp() -> Self {
        Self {
            state: ListState::default().with_selected(Some(0)),
            boundary: Boundary::Clamp,
            area: Rect::ZERO,
            rendered_len: 0,
        }
    }

    pub fn wrap(selected: usize) -> Self {
        Self {
            state: ListState::default().with_selected(Some(selected)),
            boundary: Boundary::Wrap { len: 0 },
            area: Rect::ZERO,
            rendered_len: 0,
        }
    }

    pub fn set_area(&mut self, area: Rect, len: usize) {
        self.area = area;
        self.rendered_len = len;
    }

    pub fn clear_area(&mut self) {
        self.area = Rect::ZERO;
        self.rendered_len = 0;
    }

    pub fn selected(&self) -> Option<usize> {
        self.state.selected()
    }

    pub fn index(&self) -> usize {
        self.selected().unwrap_or_default()
    }

    pub fn select(&mut self, selected: Option<usize>) {
        self.state.select(selected);
    }

    /// Updates the wrapped row count and keeps the selection inside it.
    pub fn set_len(&mut self, len: usize) {
        self.rendered_len = len;
        if let Boundary::Wrap { len: wrap_len } = &mut self.boundary {
            *wrap_len = len;
        }
        if len == 0 {
            self.state.select(None);
            return;
        }
        let current = self.index().min(len - 1);
        self.state.select(Some(current));
    }

    pub fn next(&mut self) {
        match self.boundary {
            Boundary::Wrap { .. } => self.step(true),
            Boundary::Clamp => self.state.select_next(),
        }
    }

    pub fn previous(&mut self) {
        match self.boundary {
            Boundary::Wrap { .. } => self.step(false),
            Boundary::Clamp => self.state.select_previous(),
        }
    }

    pub fn first(&mut self) {
        match self.boundary {
            Boundary::Wrap { len: 0 } => self.state.select(None),
            _ => self.state.select_first(),
        }
    }

    pub fn last(&mut self) {
        match self.boundary {
            Boundary::Wrap { len } if len > 0 => self.state.select(Some(len - 1)),
            Boundary::Wrap { .. } => self.state.select(None),
            Boundary::Clamp => self.state.select_last(),
        }
    }

    pub fn handle_key(&mut self, code: KeyCode) -> bool {
        match code {
            KeyCode::Down => self.next(),
            KeyCode::Up => self.previous(),
            KeyCode::Home => self.first(),
            KeyCode::End => self.last(),
            _ => return false,
        }
        true
    }

    /// Handles keyboard navigation/Enter and mouse scroll/click/double-click
    /// inside the last rendered list area.
    pub fn handle_input(&mut self, input: &InputEvent) -> ListAction {
        match input {
            InputEvent::Key(key) => {
                if key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
                {
                    return ListAction::Ignored;
                }
                if self.handle_key(key.code) {
                    return ListAction::Changed;
                }
                if key.code == KeyCode::Enter
                    && let Some(index) = self.selected()
                {
                    return ListAction::Activate(index);
                }
                ListAction::Ignored
            }
            InputEvent::Mouse(mouse) => {
                if !self.area.contains(Position::new(mouse.column, mouse.row)) {
                    return ListAction::Ignored;
                }
                match mouse.kind {
                    MouseKind::ScrollDown => {
                        self.next();
                        ListAction::Changed
                    }
                    MouseKind::ScrollUp => {
                        self.previous();
                        ListAction::Changed
                    }
                    MouseKind::Click => {
                        let index = self.state.offset() + (mouse.row - self.area.y) as usize;
                        if index < self.rendered_len {
                            self.select(Some(index));
                            ListAction::Changed
                        } else {
                            ListAction::Ignored
                        }
                    }
                    MouseKind::DoubleClick => {
                        let index = self.state.offset() + (mouse.row - self.area.y) as usize;
                        if index < self.rendered_len {
                            self.select(Some(index));
                            ListAction::Activate(index)
                        } else {
                            ListAction::Ignored
                        }
                    }
                    _ => ListAction::Ignored,
                }
            }
            InputEvent::Paste(_) => ListAction::Ignored,
        }
    }

    pub fn state_mut(&mut self) -> &mut ListState {
        &mut self.state
    }

    fn step(&mut self, forward: bool) {
        let Boundary::Wrap { len } = self.boundary else {
            return;
        };
        if len == 0 {
            self.state.select(None);
            return;
        }
        let current = self.index().min(len - 1);
        let next = if forward {
            (current + 1) % len
        } else {
            (current + len - 1) % len
        };
        self.state.select(Some(next));
    }
}
