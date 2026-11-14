use crate::t;
use ratail::KeyHint;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};
use unicode_width::UnicodeWidthStr;

use crate::app::services::Services;
use crate::pages::content::{Content, ContentEvent};

fn home_hints() -> Vec<KeyHint> {
    vec![
        KeyHint::new("R", t!("common.refresh")),
        KeyHint::new("ESC", t!("common.return")),
    ]
}

#[derive(Debug, Default)]
pub struct HomeContent;

impl Content for HomeContent {
    fn init(&mut self, ctx: &Services) {
        ctx.load_accounts();
        ctx.load_instances();
    }

    fn render(&mut self, ctx: &Services, frame: &mut Frame, area: Rect) {
        let theme = ctx.theme();
        let config = ctx.config();
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

    fn on_key(&mut self, ctx: &Services, key: KeyEvent) -> ContentEvent {
        match key.code {
            KeyCode::Char('r') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                ctx.load_accounts();
                ctx.load_instances();
                ContentEvent::Handled
            }
            _ => ContentEvent::Ignored,
        }
    }

    fn keybindings(&self) -> Vec<KeyHint> {
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
    let snapshot = ctx.account_state().snapshot();
    let Some(state) = snapshot.value().or_else(|| snapshot.previous()) else {
        return t!("common.loading").into_owned();
    };
    let Some(selected) = state.selected else {
        return t!("home.no_account").into_owned();
    };
    state
        .accounts
        .iter()
        .find(|account| account.id == selected)
        .map(|account| format!("{} ({})", account.name, account.authorizer))
        .unwrap_or_else(|| t!("home.no_account").into_owned())
}

fn instance_count(ctx: &Services) -> usize {
    let snapshot = ctx.instances().snapshot();
    snapshot
        .value()
        .or_else(|| snapshot.previous())
        .map(|instances| instances.len())
        .unwrap_or(0)
}

fn java_label(config: &crate::app::config::LauncherConfig) -> String {
    if let Some(path) = &config.java_runtime_path {
        return path.display().to_string();
    }

    t!("common.automatic").into_owned()
}
