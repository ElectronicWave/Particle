use ratail::KeyHint;
use ratatui::{
    Frame,
    crossterm::event::KeyEvent,
    layout::Rect,
    style::{Modifier, Style},
    text::Line,
    widgets::{Paragraph, Widget, Wrap},
};

use crate::app::services::Services;
use crate::pages::route::Route;
use crate::theme::Theme;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum ContentEvent {
    Ignored,
    Handled,
    Navigate(Route),
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

pub fn render_placeholder(
    frame: &mut Frame,
    area: Rect,
    theme: Theme,
    title: &str,
    description: &str,
) {
    let lines = vec![
        Line::styled(
            title,
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
        ),
        Line::raw(""),
        Line::styled(description, Style::new().fg(theme.text_muted)),
    ];

    let paragraph = Paragraph::new(lines).wrap(Wrap { trim: true });
    frame.render_widget(paragraph, area);
}

pub fn render_message(frame: &mut Frame, area: Rect, message: &str) {
    Paragraph::new(message)
        .centered()
        .render(area, frame.buffer_mut());
}
