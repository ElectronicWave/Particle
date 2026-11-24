use ratatui::{
    Frame,
    layout::Rect,
    style::Style,
    widgets::{Block, BorderType, Borders},
};

use crate::theme::Theme;

pub fn render(frame: &mut Frame, area: Rect, theme: Theme, focused: bool) -> Rect {
    let border_color = if focused {
        theme.border_focused
    } else {
        theme.border
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::new().fg(border_color));
    let inner = block.inner(area);

    frame.render_widget(block, area);
    Rect {
        x: inner.x + 1,
        y: inner.y,
        width: inner.width.saturating_sub(2),
        height: inner.height,
    }
}
