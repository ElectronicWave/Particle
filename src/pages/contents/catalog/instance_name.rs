use crate::t;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use crate::theme::Theme;

pub(super) enum NameAction {
    Handled,
    Cancel,
    Submit(String),
}

#[derive(Debug)]
pub(super) struct InstanceNameScreen {
    input: String,
}

impl InstanceNameScreen {
    pub(super) fn new(default: String) -> Self {
        Self { input: default }
    }

    pub(super) fn render(&self, frame: &mut Frame, area: Rect, theme: Theme) {
        let [label_area, input_area, help_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Fill(1),
        ])
        .areas(area);

        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::styled(t!("catalog.instance_name"), Style::new().fg(theme.accent)),
                Span::styled(
                    format!("  {}", t!("catalog.instance_name_hint")),
                    Style::new().fg(theme.text_dim),
                ),
            ])),
            label_area,
        );
        frame.render_widget(
            Paragraph::new(Line::from(vec![
                Span::raw(self.input.clone()),
                Span::styled("_", Style::new().add_modifier(Modifier::BOLD)),
            ])),
            input_area,
        );
        frame.render_widget(
            Paragraph::new(t!("catalog.instance_name_help"))
                .style(Style::new().fg(theme.text_dim))
                .wrap(Wrap { trim: true }),
            help_area,
        );
    }

    pub(super) fn on_key(&mut self, key: KeyEvent) -> NameAction {
        match key.code {
            KeyCode::Esc => NameAction::Cancel,
            KeyCode::Backspace => {
                self.input.pop();
                NameAction::Handled
            }
            KeyCode::Enter => {
                let name = self.input.trim().to_owned();
                if name.is_empty() {
                    NameAction::Handled
                } else {
                    NameAction::Submit(name)
                }
            }
            KeyCode::Char(character)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                if !matches!(
                    character,
                    '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|'
                ) {
                    self.input.push(character);
                }
                NameAction::Handled
            }
            _ => NameAction::Handled,
        }
    }
}
