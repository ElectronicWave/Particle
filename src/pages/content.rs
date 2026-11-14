use ratail::KeyHint;
use ratatui::{
    Frame,
    crossterm::event::KeyEvent,
    layout::Rect,
    text::Line,
    widgets::{Paragraph, Widget},
};

use crate::app::services::Services;
use crate::pages::route::Route;
use crate::widgets::spinner;

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

pub fn render_message(frame: &mut Frame, area: Rect, message: &str) {
    Paragraph::new(message)
        .centered()
        .render(area, frame.buffer_mut());
}

pub fn render_loading(ctx: &Services, frame: &mut Frame, area: Rect, message: &str) {
    ctx.request_frame(spinner::INTERVAL);
    let line = Line::raw(format!("{} {message}", spinner::frame()));
    frame.render_widget(Paragraph::new(line).centered(), area);
}
