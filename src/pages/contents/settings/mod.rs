mod downloads;
mod general;
mod pane;
mod runtime;

use ratail::{KeyHint, Page, PageEvent};
use ratatui::{
    Frame,
    crossterm::event::KeyEvent,
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::Line,
    widgets::Paragraph,
};

use crate::pages::route::Route;
use crate::services::Services;
use crate::t;
use crate::theme::Theme;
use crate::widgets::tabs::{TabBar, cycle_index, tab_step};

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

impl Page<Services> for SettingsContent {
    fn init(&mut self, ctx: &Services, _route: &Route) {
        self.panes[self.active].init(ctx);
    }

    fn render(&mut self, ctx: &Services, _route: &Route, frame: &mut Frame, area: Rect) {
        let theme = ctx.ui().theme();
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

    fn on_key(&mut self, ctx: &Services, _route: &Route, key: KeyEvent) -> PageEvent<Route> {
        if self.panes[self.active].on_key(ctx, key) == PaneEvent::Handled {
            return PageEvent::Handled;
        }

        match tab_step(key) {
            Some(step) => {
                self.active = cycle_index(self.panes.len(), self.active, step);
                self.panes[self.active].init(ctx);
                PageEvent::Handled
            }
            None => PageEvent::Ignored,
        }
    }

    fn keybindings(&self, _route: &Route) -> Vec<KeyHint> {
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
    if let Some(error) = ctx.config().error() {
        return Line::styled(
            t!("settings.save_failed", error = error),
            Style::new().fg(theme.danger),
        );
    }

    Line::styled(
        t!(
            "settings.saved",
            path = ctx.config().snapshot().config_path().display().to_string()
        ),
        Style::new().fg(theme.text_dim),
    )
}
