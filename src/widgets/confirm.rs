use ratail::{KeyHint, Modal, ModalOutcome};
use ratatui::{Frame, crossterm::event::KeyEvent, layout::Rect};

use crate::{services::Services, t, widgets::dropdown::Dropdown};

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

impl<T> Modal<Services> for Confirmation<T> {
    type Value = T;

    fn on_key(&mut self, ctx: &Services, key: KeyEvent) -> ModalOutcome<T> {
        match self.prompt.on_key(ctx, key) {
            ModalOutcome::Completed(true) => {
                ModalOutcome::Completed(self.value.take().expect("confirmation completed twice"))
            }
            ModalOutcome::Completed(false) => ModalOutcome::Cancelled,
            ModalOutcome::Cancelled => ModalOutcome::Cancelled,
            ModalOutcome::Handled => ModalOutcome::Handled,
            ModalOutcome::Ignored => ModalOutcome::Ignored,
        }
    }

    fn render(&mut self, ctx: &Services, frame: &mut Frame, area: Rect) {
        self.prompt.render(ctx, frame, area);
    }

    fn keybindings(&self) -> Vec<KeyHint> {
        self.prompt.keybindings()
    }
}
