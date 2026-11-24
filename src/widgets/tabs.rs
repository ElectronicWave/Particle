use ratail::{HitMap, KeyChord};
use ratatui::{
    buffer::Buffer,
    crossterm::event::KeyCode,
    layout::Rect,
    style::{Modifier, Style},
    text::Span,
    widgets::{Tabs, Widget},
};
use unicode_width::UnicodeWidthStr;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabStep {
    Next,
    Previous,
}

pub fn tab_step(key: KeyChord) -> Option<TabStep> {
    match key.code {
        KeyCode::Tab => Some(TabStep::Next),
        KeyCode::BackTab => Some(TabStep::Previous),
        _ => None,
    }
}

pub fn cycle_index(len: usize, current: usize, step: TabStep) -> usize {
    if len == 0 {
        return 0;
    }

    match step {
        TabStep::Next => (current + 1) % len,
        TabStep::Previous => (current + len - 1) % len,
    }
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

    pub fn register_hits<T>(&self, area: Rect, hits: &mut HitMap<T>, mut make_id: impl FnMut(usize) -> T) {
        if self.labels.is_empty() || area.is_empty() {
            return;
        }

        let selected = self.selected.min(self.labels.len() - 1);
        let (start, end) = visible_range(self.labels, selected, area.width as usize);
        let mut x = area.x;
        let right = area.right();

        for (offset, label) in self.labels[start..end].iter().enumerate() {
            if offset > 0 {
                x = x.saturating_add(DIVIDER_WIDTH as u16);
            }
            let title_width = UnicodeWidthStr::width(*label) + if offset == 0 { 1 } else { 2 };
            let width = (title_width as u16).min(right.saturating_sub(x));
            if width == 0 {
                break;
            }
            hits.register(Rect::new(x, area.y, width, 1), make_id(start + offset));
            x = x.saturating_add(width);
        }
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
            .enumerate()
            .map(|(offset, label)| {
                if offset == 0 {
                    format!("{label} ")
                } else {
                    format!(" {label} ")
                }
            })
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
        .map(|label| UnicodeWidthStr::width(*label) + PADDING_LEFT + PADDING_RIGHT)
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
