use ratail::KeyHint;
use ratatui::{Frame, layout::Rect};

use crate::app::services::Services;
use crate::pages::content::{Content, RETURN_HINTS, render_placeholder};

#[derive(Debug, Default)]
pub struct SettingsContent;

impl Content for SettingsContent {
    fn render(&mut self, _ctx: &Services, frame: &mut Frame, area: Rect) {
        render_placeholder(
            frame,
            area,
            "Settings",
            "This page can render forms, tabs, and validation state without changing sidebar storage.",
        );
    }

    fn keybindings(&self) -> Vec<KeyHint> {
        RETURN_HINTS.to_vec()
    }
}
