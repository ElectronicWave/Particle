use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::Span,
    widgets::{Tabs, Widget},
};

use crate::theme::Theme;

const PADDING_LEFT: usize = 1;
const PADDING_RIGHT: usize = 1;
const DIVIDER: &str = "│";
const DIVIDER_WIDTH: usize = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabLevel {
    Primary,
    Secondary,
}

pub struct TabBar<'a> {
    theme: Theme,
    labels: &'a [&'a str],
    selected: usize,
    level: TabLevel,
}

impl<'a> TabBar<'a> {
    pub fn new(theme: Theme, labels: &'a [&'a str], selected: usize) -> Self {
        Self {
            theme,
            labels,
            selected,
            level: TabLevel::Primary,
        }
    }

    pub fn secondary(mut self) -> Self {
        self.level = TabLevel::Secondary;
        self
    }

    fn highlight_style(&self) -> Style {
        match self.level {
            TabLevel::Primary => Style::new()
                .fg(self.theme.accent_fg)
                .bg(self.theme.accent)
                .add_modifier(Modifier::BOLD),
            TabLevel::Secondary => Style::new()
                .fg(self.theme.accent)
                .add_modifier(Modifier::BOLD),
        }
    }
}

impl Widget for TabBar<'_> {
    fn render(self, area: Rect, buffer: &mut Buffer) {
        if self.labels.is_empty() {
            return;
        }

        let selected = self.selected.min(self.labels.len() - 1);
        let (start, end) = visible_range(self.labels, selected, area.width as usize);
        let titles = self.labels[start..end]
            .iter()
            .map(|label| format!(" {label} "))
            .collect::<Vec<String>>();

        Tabs::new(titles)
            .select(selected - start)
            .style(Style::new().fg(self.theme.text_dim))
            .highlight_style(self.highlight_style())
            .divider(Span::styled(DIVIDER, Style::new().fg(self.theme.divider)))
            .padding("", "")
            .render(area, buffer);
    }
}

fn visible_range(labels: &[&str], selected: usize, width: usize) -> (usize, usize) {
    let widths = labels
        .iter()
        .map(|label| label.chars().count() + PADDING_LEFT + PADDING_RIGHT)
        .collect::<Vec<usize>>();
    let mut start = selected;
    let mut end = selected + 1;
    let mut used = widths[selected];
    let mut prefer_left = true;

    loop {
        let mut expanded = false;

        if prefer_left && start > 0 && used + DIVIDER_WIDTH + widths[start - 1] <= width {
            start -= 1;
            used += DIVIDER_WIDTH + widths[start];
            expanded = true;
        } else if !prefer_left && end < widths.len() && used + DIVIDER_WIDTH + widths[end] <= width
        {
            used += DIVIDER_WIDTH + widths[end];
            end += 1;
            expanded = true;
        } else if start > 0 && used + DIVIDER_WIDTH + widths[start - 1] <= width {
            start -= 1;
            used += DIVIDER_WIDTH + widths[start];
            expanded = true;
        } else if end < widths.len() && used + DIVIDER_WIDTH + widths[end] <= width {
            used += DIVIDER_WIDTH + widths[end];
            end += 1;
            expanded = true;
        }

        if !expanded {
            break;
        }

        prefer_left = !prefer_left;
    }

    (start, end)
}
