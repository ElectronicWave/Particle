use ratail::KeyHint;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use super::services::Services;

pub fn render(_ctx: &Services, hints: &[KeyHint], area: Rect, buf: &mut Buffer) {
    let mut spans = Vec::new();

    for (index, hint) in hints.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw("  "));
        }

        spans.push(Span::styled(
            format!("[{}]", hint.key),
            Style::default()
                .fg(Color::LightGreen)
                .add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(format!("{} ", hint.description)));
    }

    Paragraph::new(Line::from(spans)).render(area, buf);
}
