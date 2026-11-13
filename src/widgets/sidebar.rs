use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::Style,
    symbols::merge::MergeStrategy,
    widgets::{Block, Borders, List, ListState, StatefulWidget, Widget},
};

pub struct Sidebar {
    items: Vec<String>,
    selected: usize,
}

impl Sidebar {
    pub fn new(items: Vec<String>, selected: usize) -> Self {
        Self { items, selected }
    }
}

impl Widget for Sidebar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let list = List::new(self.items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .merge_borders(MergeStrategy::Exact),
            )
            .style(Style::new().white())
            .highlight_style(Style::new().italic().light_green())
            .highlight_symbol(">")
            .repeat_highlight_symbol(true);

        let mut state = ListState::default().with_selected(Some(self.selected));
        StatefulWidget::render(list, area, buf, &mut state);
    }
}
