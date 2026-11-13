use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Style},
    symbols::merge::MergeStrategy,
    widgets::{Block, Borders, Widget},
};

const TITLE: &str = "⚛  Elemental Minecraft Launcher";

#[derive(Debug, Clone, Copy, Default)]
pub struct TitleBar;

impl Widget for TitleBar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .merge_borders(MergeStrategy::Exact);
        let inner = block.inner(area);

        block.render(area, buf);

        if inner.height == 0 {
            return;
        }

        let y = inner.y + (inner.height - 1) / 2;
        buf.set_string(
            inner.x + 1,
            y,
            TITLE,
            Style::default().fg(Color::White).bold(),
        );
    }
}
