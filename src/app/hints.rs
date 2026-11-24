use ratail::KeyHint;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};

use crate::services::Services;

pub fn render(ctx: &Services, hints: &[KeyHint], area: Rect, buf: &mut Buffer) {
    let theme = ctx.ui().theme();
    let mut spans = Vec::new();

    for (index, hint) in hints.iter().enumerate() {
        if index > 0 {
            spans.push(Span::raw("  "));
        }

        spans.push(Span::styled(
            format!("[{}]", hint.key),
            Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
        ));
        spans.push(Span::raw(format!("{} ", hint.description)));
    }

    Paragraph::new(Line::from(spans)).render(area, buf);
}
