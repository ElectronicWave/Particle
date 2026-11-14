mod downloads;
mod general;
mod pane;
mod runtime;

use ratail::KeyHint;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent},
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Line,
    widgets::Paragraph,
};

use crate::app::services::Services;
use crate::pages::content::{Content, ContentEvent};
use crate::t;
use crate::theme::Theme;
use crate::widgets::tabs::TabBar;

use pane::{PaneAreas, PaneEvent, SettingsPane};

/// Hosts tab navigation only; each pane owns its own rows and interactions.
pub struct SettingsContent {
    panes: Vec<Box<dyn SettingsPane>>,
    active: usize,
}

impl Default for SettingsContent {
    fn default() -> Self {
        Self {
            panes: vec![
                Box::new(general::GeneralPane::default()),
                Box::new(runtime::RuntimePane::default()),
                Box::new(downloads::DownloadsPane::default()),
            ],
            active: 0,
        }
    }
}

impl Content for SettingsContent {
    fn init(&mut self, ctx: &Services) {
        self.panes[self.active].init(ctx);
    }

    fn render(&mut self, ctx: &Services, frame: &mut Frame, area: Rect) {
        let theme = ctx.theme();
        let [tabs_area, body_area, status_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Fill(1),
            Constraint::Length(2),
        ])
        .areas(area);

        let labels = self
            .panes
            .iter()
            .map(|pane| pane.title().into_owned())
            .collect::<Vec<String>>();
        let label_refs = labels.iter().map(String::as_str).collect::<Vec<&str>>();
        frame.render_widget(TabBar::new(theme, &label_refs, self.active), tabs_area);

        let pane = &mut self.panes[self.active];
        let status = pane
            .status(ctx, theme)
            .unwrap_or_else(|| default_status(ctx, theme));
        frame.render_widget(Paragraph::new(status), status_area);

        pane.render(
            ctx,
            frame,
            PaneAreas {
                body: body_area,
                full: area,
            },
            theme,
        );
    }

    fn on_key(&mut self, ctx: &Services, key: KeyEvent) -> ContentEvent {
        if self.panes[self.active].on_key(ctx, key) == PaneEvent::Handled {
            return ContentEvent::Handled;
        }

        match key.code {
            KeyCode::Tab => {
                self.active = (self.active + 1) % self.panes.len();
                self.panes[self.active].init(ctx);
                ContentEvent::Handled
            }
            KeyCode::BackTab => {
                self.active = (self.active + self.panes.len() - 1) % self.panes.len();
                self.panes[self.active].init(ctx);
                ContentEvent::Handled
            }
            _ => ContentEvent::Ignored,
        }
    }

    fn keybindings(&self) -> Vec<KeyHint> {
        let pane = &self.panes[self.active];
        if pane.is_modal() {
            return pane.hints();
        }

        let mut hints = vec![KeyHint::new("TAB", t!("common.switch_tab"))];
        hints.extend(pane.hints());
        hints
    }
}

fn default_status(ctx: &Services, theme: Theme) -> Line<'static> {
    if let Some(error) = ctx.config_error() {
        return Line::styled(
            t!("settings.save_failed", error = error),
            Style::new().fg(theme.danger),
        );
    }

    Line::styled(
        t!(
            "settings.saved",
            path = ctx.config().config_path().display().to_string()
        ),
        Style::new().fg(theme.text_dim),
    )
}
