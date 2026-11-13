use ratail::KeyHint;
use ratatui::{
    Frame,
    crossterm::event::KeyEvent,
    layout::Rect,
    style::Style,
    symbols::merge::MergeStrategy,
    text::Line,
    widgets::{Block, Borders, Paragraph, Widget, Wrap},
};

use crate::app::services::Services;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ContentEvent {
    Ignored,
    Handled,
}

pub trait Content {
    fn init(&mut self, _ctx: &Services) {}

    fn render(&mut self, ctx: &Services, frame: &mut Frame, area: Rect);

    fn on_key(&mut self, _ctx: &Services, _key: KeyEvent) -> ContentEvent {
        ContentEvent::Ignored
    }

    fn keybindings(&self) -> Vec<KeyHint> {
        Vec::new()
    }
}

pub const RETURN_HINTS: &[KeyHint] = &[KeyHint::from_static("ESC", "Return")];

pub fn render_placeholder(frame: &mut Frame, area: Rect, title: &str, description: &str) {
    let lines = vec![
        Line::styled(title, Style::new().bold().white()),
        Line::raw(""),
        Line::raw(description),
    ];

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    let content_area = container(frame, area);
    frame.render_widget(paragraph, content_area);
}

pub fn container(frame: &mut Frame, area: Rect) -> Rect {
    let block = Block::default()
        .borders(Borders::ALL)
        .merge_borders(MergeStrategy::Exact);
    let inner = block.inner(area);

    block.render(area, frame.buffer_mut());
    Rect {
        x: inner.x + 1,
        y: inner.y,
        width: inner.width.saturating_sub(2),
        height: inner.height,
    }
}

pub fn render_message(frame: &mut Frame, area: Rect, message: &str) {
    Paragraph::new(message)
        .centered()
        .render(area, frame.buffer_mut());
}
