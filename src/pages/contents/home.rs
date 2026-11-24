use crate::t;
use ratail::{InputEvent, KeyChord, KeyHint, Page, PageEvent};
use ratatui::{
    Frame,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

use crate::pages::route::Route;
use crate::services::Services;

fn home_hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("R", t!("common.refresh")),
        KeyHint::new("ESC", t!("common.return")),
    ]
}

#[derive(Debug, Default)]
pub struct HomeContent;

impl Page<Services> for HomeContent {
    fn init(&mut self, ctx: &Services, _route: &Route) {
        ctx.accounts().load();
        ctx.instances().load();
    }

    fn render(&mut self, ctx: &Services, _route: &Route, frame: &mut Frame, area: Rect) {
        let theme = ctx.ui().theme();
        let config = ctx.config().snapshot();
        let account = account_label(ctx);
        let instances = instance_count(ctx);

        let lines = vec![
            Line::styled(
                t!("app.name"),
                Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
            ),
            Line::styled(t!("app.tagline"), Style::new().fg(theme.text_dim)),
            Line::raw(""),
            Line::from(vec![
                Span::styled(label("home.account"), Style::new().fg(theme.text_dim)),
                Span::raw(account),
            ]),
            Line::from(vec![
                Span::styled(label("home.instances"), Style::new().fg(theme.text_dim)),
                Span::raw(t!("home.installed", count = instances)),
            ]),
            Line::from(vec![
                Span::styled(label("home.storage_root"), Style::new().fg(theme.text_dim)),
                Span::raw(config.storage_root.display().to_string()),
            ]),
            Line::from(vec![
                Span::styled(label("home.java"), Style::new().fg(theme.text_dim)),
                Span::raw(java_label(&config)),
            ]),
            Line::raw(""),
            Line::styled(t!("home.description"), Style::new().fg(theme.text_dim)),
        ];

        frame.render_widget(Paragraph::new(lines).wrap(Wrap { trim: true }), area);
    }

    fn on_input(&mut self, ctx: &Services, _route: &Route, input: InputEvent) -> PageEvent<Route> {
        match input {
            InputEvent::Key(key) if key == KeyChord::char('r') => {
                ctx.accounts().load();
                ctx.instances().load();
                PageEvent::Handled
            }
            _ => PageEvent::Ignored,
        }
    }

    fn keybindings(&self, _route: &Route) -> Vec<KeyHint> {
        home_hints()
    }
}

fn label(key: &str) -> String {
    const WIDTH: usize = 13;
    let text = t!(key);
    let padding = WIDTH.saturating_sub(UnicodeWidthStr::width(text.as_ref()));
    format!("{text}{}", " ".repeat(padding))
}

fn account_label(ctx: &Services) -> String {
    let snapshot = ctx.accounts().state().snapshot();
    let Some(state) = snapshot.view().data else {
        return t!("common.loading").into_owned();
    };
    let Some(selected) = state.selected else {
        return t!("home.no_account").into_owned();
    };
    state
        .accounts
        .iter()
        .find(|account| account.id == selected)
        .map(|account| format!("{} ({})", account.name, account.authorizer_label()))
        .unwrap_or_else(|| t!("home.no_account").into_owned())
}

fn instance_count(ctx: &Services) -> usize {
    let snapshot = ctx.instances().snapshot();
    snapshot
        .view()
        .data
        .map(|report| report.instances.len())
        .unwrap_or(0)
}

fn java_label(config: &crate::app::config::LauncherConfig) -> String {
    if let Some(path) = &config.java_runtime_path {
        return path.display().to_string();
    }

    t!("common.automatic").into_owned()
}
