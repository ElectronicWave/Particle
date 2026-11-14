use std::borrow::Cow;

use ratail::KeyHint;
use ratatui::{
    Frame,
    crossterm::event::KeyEvent,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{List, ListItem, ListState},
};
use unicode_width::UnicodeWidthStr;

use crate::app::services::Services;
use crate::t;
use crate::theme::Theme;

pub(super) const LABEL_WIDTH: usize = 20;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum RowHint {
    Edit,
    Select,
    Toggle,
    Browse,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum PaneEvent {
    Ignored,
    Handled,
}

#[derive(Debug, Clone, Copy)]
pub(super) struct PaneAreas {
    pub(super) body: Rect,
    pub(super) full: Rect,
}

/// One settings tab. Each pane owns its rows, selection, editing and popups.
pub(super) trait SettingsPane {
    fn title(&self) -> Cow<'static, str>;

    fn init(&mut self, _ctx: &Services) {}

    fn render(&mut self, ctx: &Services, frame: &mut Frame, areas: PaneAreas, theme: Theme);

    fn on_key(&mut self, ctx: &Services, key: KeyEvent) -> PaneEvent;

    fn hints(&self) -> Vec<KeyHint>;

    fn is_modal(&self) -> bool;

    fn status(&self, ctx: &Services, theme: Theme) -> Option<Line<'static>>;
}

pub(super) fn row_hints(row: RowHint) -> Vec<KeyHint> {
    let enter = match row {
        RowHint::Edit => KeyHint::new("ENTER", t!("common.edit")),
        RowHint::Select => KeyHint::new("ENTER", t!("common.select")),
        RowHint::Toggle => KeyHint::new("ENTER", t!("common.toggle")),
        RowHint::Browse => KeyHint::new("ENTER", t!("settings.pick_path")),
    };
    let mut hints = vec![KeyHint::new("↑/↓", t!("common.field")), enter];
    if row == RowHint::Browse {
        hints.push(KeyHint::new("E", t!("settings.type_path")));
    }
    hints.push(KeyHint::new("←/→", t!("common.change")));
    hints.push(KeyHint::new("ESC", t!("common.return")));
    hints
}

pub(super) fn edit_hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("ENTER", t!("common.save")),
        KeyHint::new("ESC", t!("common.cancel")),
    ]
}

pub(super) fn edit_status(theme: Theme) -> Line<'static> {
    Line::styled(t!("settings.edit_help"), Style::new().fg(theme.text_dim))
}

pub(super) fn error_status(error: Option<&String>, theme: Theme) -> Option<Line<'static>> {
    error.map(|error| Line::styled(error.clone(), Style::new().fg(theme.danger)))
}

pub(super) fn row(
    index: usize,
    selected: usize,
    label: &str,
    value: String,
    theme: Theme,
) -> Line<'static> {
    let style = if index == selected {
        Style::new().fg(theme.accent).add_modifier(Modifier::BOLD)
    } else {
        Style::new()
    };
    Line::from(vec![
        Span::styled(pad_label(label), style),
        Span::raw(value),
    ])
}

pub(super) fn render_rows(
    frame: &mut Frame,
    area: Rect,
    rows: Vec<Line<'static>>,
    selected: usize,
    theme: Theme,
) {
    let items = rows
        .into_iter()
        .map(ListItem::new)
        .collect::<Vec<ListItem>>();
    let list = List::new(items).highlight_style(theme.selection_style());
    let mut state = ListState::default().with_selected(Some(selected));
    frame.render_stateful_widget(list, area, &mut state);
}

pub(super) fn row_anchor(area: Rect) -> Rect {
    Rect {
        x: area.x + LABEL_WIDTH as u16,
        y: area.y,
        width: 1,
        height: 1,
    }
}

fn pad_label(label: &str) -> String {
    let padding = LABEL_WIDTH.saturating_sub(UnicodeWidthStr::width(label));
    format!("{label}{}", " ".repeat(padding))
}
