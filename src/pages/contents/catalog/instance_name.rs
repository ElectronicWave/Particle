use ratail::{KeyHint, NavAction, Page, PageEvent};
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use super::{CatalogRoute, LoaderSelection};
use crate::services::Services;
use crate::t;
use crate::theme::Theme;

pub(super) struct InstanceNamePage {
    driver: usize,
    selection: LoaderSelection,
    input: String,
}

impl InstanceNamePage {
    pub(super) fn new(driver: usize, selection: LoaderSelection) -> Self {
        let input = default_instance_name(&selection);
        Self {
            driver,
            selection,
            input,
        }
    }

    fn start_install(&self, ctx: &Services, name: String) {
        ctx.install().start(
            name,
            ctx.catalog().drivers()[self.driver],
            self.selection.game.clone(),
            self.selection.loader.clone(),
        );
    }
}

impl Page<Services, CatalogRoute> for InstanceNamePage {
    fn on_key(
        &mut self,
        ctx: &Services,
        _route: &CatalogRoute,
        key: KeyEvent,
    ) -> PageEvent<CatalogRoute> {
        match key.code {
            KeyCode::Esc => PageEvent::Navigate(NavAction::Pop),
            KeyCode::Backspace => {
                self.input.pop();
                PageEvent::Handled
            }
            KeyCode::Enter => {
                let name = self.input.trim().to_owned();
                if name.is_empty() {
                    PageEvent::Handled
                } else {
                    self.start_install(ctx, name);
                    PageEvent::Navigate(NavAction::Pop)
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
                PageEvent::Handled
            }
            _ => PageEvent::Ignored,
        }
    }

    fn render(&mut self, ctx: &Services, _route: &CatalogRoute, frame: &mut Frame, area: Rect) {
        let theme = ctx.ui().theme();
        let [header_area, body_area] =
            Layout::vertical([Constraint::Length(1), Constraint::Fill(1)]).areas(area);
        super::render_flow_header(
            frame,
            header_area,
            self.selection.game.as_ref(),
            &t!("catalog.instance_name_suffix"),
            theme,
        );
        render_body(&self.input, frame, body_area, theme);
    }

    fn keybindings(&self, _route: &CatalogRoute) -> Vec<KeyHint> {
        super::name_hints()
    }
}

fn render_body(input: &str, frame: &mut Frame, area: Rect, theme: Theme) {
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
            Span::raw(input.to_owned()),
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

fn default_instance_name(selection: &LoaderSelection) -> String {
    match &selection.loader {
        Some(loader) => format!("{}-{}", selection.game, loader),
        None => selection.game.to_string(),
    }
}
