use ratail::KeyHint;
use ratatui::{
    Frame,
    crossterm::event::{KeyCode, KeyEvent, KeyModifiers},
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Wrap},
};

use crate::app::services::Services;
use crate::pages::content::{Content, ContentEvent};

const HOME_HINTS: &[KeyHint] = &[
    KeyHint::from_static("R", "Refresh"),
    KeyHint::from_static("ESC", "Return"),
];

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
                "Particle",
                Style::new().fg(theme.accent).add_modifier(Modifier::BOLD),
            ),
            Line::styled(
                "Minecraft launcher powered by Elemental",
                Style::new().fg(theme.text_dim),
            ),
            Line::raw(""),
            Line::from(vec![
                Span::styled("Account      ", Style::new().fg(theme.text_dim)),
                Span::raw(account),
            ]),
            Line::from(vec![
                Span::styled("Instances    ", Style::new().fg(theme.text_dim)),
                Span::raw(format!("{instances} installed")),
            ]),
            Line::from(vec![
                Span::styled("Storage root ", Style::new().fg(theme.text_dim)),
                Span::raw(config.storage_root.display().to_string()),
            ]),
            Line::from(vec![
                Span::styled("Java         ", Style::new().fg(theme.text_dim)),
                Span::raw(
                    config
                        .java_major_version
                        .map(|major| format!("Java {major}"))
                        .unwrap_or_else(|| "automatic".to_owned()),
                ),
            ]),
            Line::raw(""),
            Line::styled(
                "Select an instance to launch, or open Catalog to install a new one.",
                Style::new().fg(theme.text_dim),
            ),
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
        HOME_HINTS.to_vec()
    }
}

fn account_label(ctx: &Services) -> String {
    let snapshot = ctx.account_state().snapshot();
    let Some(state) = snapshot.value().or_else(|| snapshot.previous()) else {
        return "loading...".to_owned();
    };
    let Some(selected) = state.selected else {
        return "no account selected".to_owned();
    };
    state
        .accounts
        .iter()
        .find(|account| account.id == selected)
        .map(|account| format!("{} ({})", account.name, account.authorizer))
        .unwrap_or_else(|| "no account selected".to_owned())
}

fn instance_count(ctx: &Services) -> usize {
    let snapshot = ctx.instances().snapshot();
    snapshot
        .value()
        .or_else(|| snapshot.previous())
        .map(|instances| instances.len())
        .unwrap_or(0)
}
