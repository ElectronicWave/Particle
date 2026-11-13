use ratail::KeyHint;
use ratatui::{Frame, layout::Rect};

use crate::app::services::Services;
use crate::pages::content::{Content, RETURN_HINTS, render_placeholder};

#[derive(Debug, Default)]
pub struct InstancesContent;

impl Content for InstancesContent {
    fn render(&mut self, _ctx: &Services, frame: &mut Frame, area: Rect) {
        render_placeholder(
            frame,
            area,
            "Instances",
            "This page is ready for an instances list, actions, and per-instance details.",
        );
    }

    fn keybindings(&self) -> Vec<KeyHint> {
        RETURN_HINTS.to_vec()
    }
}
