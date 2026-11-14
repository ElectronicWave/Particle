use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    widgets::{
        Block, BorderType, Borders, HighlightSpacing, List, ListState, Padding, StatefulWidget,
        Widget,
    },
};

use crate::theme::Theme;

pub struct Sidebar {
    items: Vec<String>,
    selected: usize,
    focused: bool,
    theme: Theme,
}

impl Sidebar {
    pub fn new(items: Vec<String>, selected: usize, focused: bool, theme: Theme) -> Self {
        Self {
            items,
            selected,
            focused,
            theme,
        }
    }
}

impl Widget for Sidebar {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let border_style = if self.focused {
            Style::new().fg(self.theme.border_focused)
        } else {
            Style::new().fg(self.theme.border)
        };
        let highlight_style = if self.focused {
            Style::new()
                .fg(self.theme.selection_fg)
                .bg(self.theme.selection_bg)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::new().fg(self.theme.accent)
        };

        let list = List::new(self.items)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_type(BorderType::Rounded)
                    .border_style(border_style)
                    .padding(Padding::horizontal(1)),
            )
            .style(Style::new().fg(self.theme.text_muted))
            .highlight_style(highlight_style)
            .highlight_symbol("▌ ")
            .highlight_spacing(HighlightSpacing::Always);

        let mut state = ListState::default().with_selected(Some(self.selected));
        StatefulWidget::render(list, area, buf, &mut state);
    }
}
